use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, Record, SequenceRange,
    SignedNanos, Source, Version,
};

use super::{Cancel, EventBus, Filter, Limits, MAX_QUEUE, MAX_WORKERS, Runtime};

fn limits(workers: usize, event_queue: usize, global_queue: usize, wait_ms: u64) -> Limits {
    Limits {
        workers,
        task_queue: 4,
        event_queue,
        global_queue,
        batch: event_queue.min(8),
        subscriptions: 4,
        admission_wait: Duration::from_millis(wait_ms),
    }
}

fn source(provider: &str, epoch: &str) -> Source {
    Source {
        device: Id::new("device-1").unwrap(),
        provider: Id::new(provider).unwrap(),
        epoch: Id::new(epoch).unwrap(),
    }
}

fn observation(epoch: &str, sequence: u64, event_type: &str, unix_ns: i64) -> Event {
    Event {
        version: Version::CURRENT,
        source: source("provider.events", epoch),
        sequence: Counter::new(sequence).unwrap(),
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new(event_type).unwrap(),
            subject: None,
            user: None,
            process: None,
            attribution: Attribution::Unknown,
            completeness: Completeness::Complete,
        },
    }
}

fn gap(epoch: &str, sequence: u64, reason: GapReason, range: Option<(u64, u64)>) -> Event {
    Event {
        version: Version::CURRENT,
        source: source("provider.events", epoch),
        sequence: Counter::new(sequence).unwrap(),
        time: EventTime {
            unix_ns: SignedNanos::new(sequence as i64),
            precision_ns: None,
        },
        record: Record::Gap {
            reason,
            range: range.map(|(first, last)| SequenceRange {
                first: Counter::new(first).unwrap(),
                last: Counter::new(last).unwrap(),
            }),
        },
    }
}

fn recv_one(subscription: &super::Subscription) -> Event {
    let batch = subscription
        .recv_batch(1, Duration::from_secs(2))
        .expect("batch");
    assert_eq!(batch.len(), 1, "expected one delivered record");
    Arc::unwrap_or_clone(batch.into_iter().next().expect("event"))
}

fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !ready() {
        assert!(Instant::now() < deadline, "condition was not reached");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn standard_limits_fit_and_excess_limits_are_rejected() {
    Limits::standard().check().unwrap();
    let mut too_many_workers = Limits::standard();
    too_many_workers.workers = MAX_WORKERS + 1;
    assert_eq!(
        too_many_workers.check().unwrap_err().code(),
        "invalid_limit"
    );
    let mut unbounded = Limits::standard();
    unbounded.event_queue = MAX_QUEUE + 1;
    assert_eq!(unbounded.check().unwrap_err().code(), "invalid_limit");
    match Runtime::start(Limits {
        workers: 0,
        ..Limits::standard()
    }) {
        Err(error) => assert_eq!(error.code(), "invalid_limit"),
        Ok(_) => panic!("zero workers were accepted"),
    }
}

#[test]
fn scheduler_delivers_real_events_and_reports_throughput() {
    let total = 20_000u64;
    let runtime = Runtime::start(Limits {
        workers: 2,
        task_queue: 8,
        event_queue: 2_048,
        global_queue: 2_048,
        batch: 256,
        subscriptions: 2,
        admission_wait: Duration::from_secs(2),
    })
    .unwrap();
    wait_until(|| runtime.scheduler().stats().live_threads >= 3);
    let subscription = runtime.bus().subscribe(1, Filter::default()).unwrap();
    let bus = runtime.bus().clone();
    let started = Instant::now();
    let publisher = runtime
        .scheduler()
        .submit(1, Duration::from_secs(30), move |_cancel: &Cancel| {
            for sequence in 1..=total {
                bus.publish(observation(
                    "throughput",
                    sequence,
                    "process.observed",
                    sequence as i64,
                ))
                .unwrap_or_else(|error| panic!("{}", error.error));
            }
        })
        .unwrap();
    let mut next = 1u64;
    while next <= total {
        let batch = subscription
            .recv_batch(256, Duration::from_secs(2))
            .unwrap();
        assert!(!batch.is_empty(), "delivery stalled at sequence {next}");
        for event in batch {
            assert_eq!(event.sequence.get(), next);
            assert!(matches!(event.record, Record::Observation { .. }));
            next += 1;
        }
    }
    let elapsed = started.elapsed();
    wait_until(|| runtime.scheduler().stats().finished >= 1);
    let stats = runtime.bus().stats();
    assert_eq!(stats.accepted, total);
    assert_eq!(stats.lost, 0);
    assert_eq!(stats.queued, 0);
    assert_eq!(stats.pending, 0);
    assert!(
        elapsed < Duration::from_secs(10),
        "moved {total} events in {elapsed:?}"
    );
    println!(
        "throughput {total} events in {:?} ({:.0}/s)",
        elapsed,
        total as f64 / elapsed.as_secs_f64()
    );
    drop(publisher);
    drop(subscription);
    runtime.shutdown();
    assert_eq!(runtime.scheduler().stats().live_threads, 0);
    #[cfg(target_os = "linux")]
    assert_eq!(runtime.scheduler().named_threads_alive(), 0);
}

#[test]
fn full_queue_waits_then_records_a_protocol_loss_gap() {
    let runtime = Runtime::start(limits(1, 2, 2, 80)).unwrap();
    let subscription = runtime.bus().subscribe(1, Filter::default()).unwrap();
    for sequence in 1..=2 {
        let outcome = runtime
            .bus()
            .publish(observation(
                "loss",
                sequence,
                "process.observed",
                sequence as i64,
            ))
            .unwrap();
        assert_eq!(outcome.enqueued, 1);
        assert_eq!(outcome.lost, 0);
    }
    assert_eq!(subscription.backlog(), (2, 0));
    for sequence in 3..=5 {
        let outcome = runtime
            .bus()
            .publish(observation(
                "loss",
                sequence,
                "process.observed",
                sequence as i64,
            ))
            .unwrap();
        assert_eq!(
            outcome.enqueued, 0,
            "sequence {sequence} entered a full queue"
        );
        assert_eq!(outcome.lost, 1);
    }
    assert_eq!(subscription.backlog(), (2, 3));
    assert!(runtime.bus().stats().global_queued <= 2);
    let first = recv_one(&subscription);
    let second = recv_one(&subscription);
    let marker = recv_one(&subscription);
    assert_eq!(first.sequence.get(), 1);
    assert_eq!(second.sequence.get(), 2);
    assert_eq!(marker.sequence.get(), 5);
    match &marker.record {
        Record::Gap {
            reason: GapReason::Lost,
            range: Some(range),
        } => {
            assert_eq!((range.first.get(), range.last.get()), (3, 4));
        }
        other => panic!("unexpected loss marker: {other:?}"),
    }
    let encoded = marker.to_json().unwrap();
    assert_eq!(Event::from_json(&encoded).unwrap(), marker);
    assert_eq!(subscription.backlog(), (0, 0));
    assert_eq!(runtime.bus().stats().lost, 3);
    assert_eq!(runtime.bus().stats().accepted, 5);

    let waiting = Runtime::start(Limits {
        admission_wait: Duration::from_secs(2),
        ..limits(1, 1, 1, 80)
    })
    .unwrap();
    let delayed = waiting.bus().subscribe(1, Filter::default()).unwrap();
    waiting
        .bus()
        .publish(observation("delay", 1, "process.observed", 1))
        .unwrap();
    assert_eq!(delayed.backlog(), (1, 0));
    let bus = waiting.bus().clone();
    let started = Arc::new(AtomicBool::new(false));
    let running = Arc::clone(&started);
    let publisher = thread::spawn(move || {
        running.store(true, Ordering::Release);
        bus.publish(observation("delay", 2, "process.observed", 2))
            .unwrap()
    });
    wait_until(|| started.load(Ordering::Acquire));
    thread::sleep(Duration::from_millis(50));
    assert!(
        !publisher.is_finished(),
        "publish returned before the subscriber drained the full queue"
    );
    let drained = delayed.recv_batch(1, Duration::from_secs(1)).unwrap();
    let outcome = publisher.join().unwrap();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].sequence.get(), 1);
    assert_eq!(
        outcome.lost, 0,
        "backpressure should wait for a draining subscriber"
    );
    assert_eq!(outcome.enqueued, 1);
    assert_eq!(recv_one(&delayed).sequence.get(), 2);
}

#[test]
fn missing_duplicate_and_filtered_records_follow_each_source() {
    let runtime = Runtime::start(limits(1, 8, 8, 20)).unwrap();
    let all = runtime.bus().subscribe(1, Filter::default()).unwrap();
    let started = runtime
        .bus()
        .subscribe(
            1,
            Filter {
                event_type: Some(Id::new("process.started").unwrap()),
                ..Filter::default()
            },
        )
        .unwrap();
    let other = runtime
        .bus()
        .subscribe(
            1,
            Filter {
                provider: Some(Id::new("other.events").unwrap()),
                ..Filter::default()
            },
        )
        .unwrap();

    runtime
        .bus()
        .publish(observation("cover", 1, "process.started", 100))
        .unwrap();
    runtime
        .bus()
        .publish(observation("cover", 2, "process.stopped", 90))
        .unwrap();
    let skipped = runtime
        .bus()
        .publish(observation("cover", 4, "process.started", -5))
        .unwrap_err();
    assert_eq!(skipped.error.code(), "missing_record");
    assert_eq!(skipped.event.sequence.get(), 4);
    assert_eq!(runtime.bus().stats().accepted, 2);

    let mut covered = gap("cover", 4, GapReason::Lost, None);
    runtime.bus().publish(covered.clone()).unwrap();
    covered.record = Record::Gap {
        reason: GapReason::Lost,
        range: Some(SequenceRange {
            first: Counter::new(3).unwrap(),
            last: Counter::new(3).unwrap(),
        }),
    };
    assert_eq!(recv_one(&all).sequence.get(), 1);
    assert_eq!(recv_one(&all).sequence.get(), 2);
    assert_eq!(recv_one(&all), covered);
    runtime
        .bus()
        .publish(observation("cover", 5, "process.started", -5))
        .unwrap();
    assert_eq!(recv_one(&all).sequence.get(), 5);

    let replay = runtime
        .bus()
        .publish(observation("cover", 5, "process.started", -5))
        .unwrap_err();
    assert_eq!(replay.error.code(), "duplicate_sequence");
    let mismatch = runtime
        .bus()
        .publish(gap("cover", 8, GapReason::Lost, Some((6, 6))))
        .unwrap_err();
    assert_eq!(mismatch.error.code(), "missing_record");
    runtime
        .bus()
        .publish(gap("cover", 7, GapReason::Paused, Some((6, 6))))
        .unwrap();
    match recv_one(&all).record {
        Record::Gap {
            reason: GapReason::Paused,
            range: Some(range),
        } => assert_eq!((range.first.get(), range.last.get()), (6, 6)),
        other => panic!("paused marker was rewritten: {other:?}"),
    }

    let invalid = Event {
        sequence: Counter::new(2).unwrap(),
        record: Record::Gap {
            reason: GapReason::Lost,
            range: Some(SequenceRange {
                first: Counter::new(2).unwrap(),
                last: Counter::new(2).unwrap(),
            }),
        },
        ..observation("invalid", 1, "process.started", 1)
    };
    assert_eq!(
        runtime.bus().publish(invalid).unwrap_err().error.code(),
        "invalid_record"
    );

    assert_eq!(recv_one(&started).sequence.get(), 1);
    match recv_one(&started).record {
        Record::Gap {
            reason: GapReason::Filtered,
            range: None,
        } => {}
        other => panic!("filtered type was not marked: {other:?}"),
    }
    match recv_one(&started).record {
        Record::Gap {
            reason: GapReason::Lost,
            range: Some(range),
        } => assert_eq!((range.first.get(), range.last.get()), (3, 3)),
        other => panic!("source gap was hidden by the type filter: {other:?}"),
    }
    assert_eq!(recv_one(&started).sequence.get(), 5);
    runtime
        .bus()
        .publish(observation("offset", 10, "process.started", 10))
        .unwrap();
    assert_eq!(
        runtime
            .bus()
            .publish(observation("offset", 12, "process.started", 12))
            .unwrap_err()
            .error
            .code(),
        "missing_record"
    );
    assert_eq!(other.backlog(), (0, 0));
    assert_eq!(
        other.recv_batch(4, Duration::from_millis(0)).unwrap().len(),
        0
    );
}

#[test]
fn priority_cancellation_timeout_and_queue_limits_release_workers() {
    let runtime = Runtime::start(Limits {
        workers: 1,
        task_queue: 2,
        event_queue: 4,
        global_queue: 1,
        batch: 4,
        subscriptions: 4,
        admission_wait: Duration::from_millis(60),
    })
    .unwrap();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let order = Arc::new(Mutex::new(Vec::new()));
    let entered = Arc::new(AtomicUsize::new(0));
    let release = Arc::clone(&gate);
    let blocker_order = Arc::clone(&order);
    let blocker_entered = Arc::clone(&entered);
    runtime
        .scheduler()
        .submit(0, Duration::from_secs(2), move |_cancel| {
            blocker_order.lock().unwrap().push("block");
            blocker_entered.fetch_add(1, Ordering::Release);
            let (lock, wake) = &*release;
            let mut open = lock.lock().unwrap();
            while !*open {
                open = wake.wait(open).unwrap();
            }
        })
        .unwrap();
    wait_until(|| entered.load(Ordering::Acquire) == 1);

    let low_order = Arc::clone(&order);
    let low = runtime
        .scheduler()
        .submit(1, Duration::from_secs(2), move |_| {
            low_order.lock().unwrap().push("low");
        })
        .unwrap();
    let high_order = Arc::clone(&order);
    runtime
        .scheduler()
        .submit(9, Duration::from_secs(2), move |_| {
            high_order.lock().unwrap().push("high");
        })
        .unwrap();
    let cancelled_ran = Arc::new(AtomicBool::new(false));
    let cancelled_flag = Arc::clone(&cancelled_ran);
    let rejected = runtime
        .scheduler()
        .submit(1, Duration::from_secs(2), move |_| {
            cancelled_flag.store(true, Ordering::Release);
        });
    match rejected {
        Err(error) => assert_eq!(error.code(), "queue_full"),
        Ok(_) => panic!("a full task queue accepted another task"),
    }
    low.cancel.cancel();
    {
        let (lock, wake) = &*gate;
        *lock.lock().unwrap() = true;
        wake.notify_one();
    }
    wait_until(|| order.lock().unwrap().len() == 2);
    assert_eq!(order.lock().unwrap().as_slice(), ["block", "high"]);
    assert!(!cancelled_ran.load(Ordering::Acquire));
    assert!(runtime.scheduler().stats().cancelled >= 1);
    assert_eq!(runtime.scheduler().stats().rejected, 1);

    let observed = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&observed);
    runtime
        .scheduler()
        .submit(1, Duration::from_millis(40), move |cancel| {
            while !cancel.is_cancelled() {
                thread::sleep(Duration::from_millis(5));
            }
            signal.store(true, Ordering::Release);
        })
        .unwrap();
    wait_until(|| observed.load(Ordering::Acquire));
    wait_until(|| runtime.scheduler().stats().inflight == 0);
    assert!(runtime.scheduler().stats().timed_out >= 1);

    let ran = Arc::new(AtomicBool::new(false));
    let after_panic = Arc::clone(&ran);
    runtime
        .scheduler()
        .submit(1, Duration::from_secs(1), |_| panic!("task failed"))
        .unwrap();
    runtime
        .scheduler()
        .submit(1, Duration::from_secs(1), move |_| {
            after_panic.store(true, Ordering::Release);
        })
        .unwrap();
    wait_until(|| ran.load(Ordering::Acquire));
    assert!(runtime.scheduler().stats().panicked >= 1);
    assert_eq!(runtime.scheduler().stats().inflight, 0);

    let high = runtime.bus().subscribe(9, Filter::default()).unwrap();
    let low = runtime.bus().subscribe(1, Filter::default()).unwrap();
    let shared = runtime
        .bus()
        .publish(observation("budget", 1, "process.observed", 1))
        .unwrap();
    assert_eq!((shared.enqueued, shared.lost), (1, 1));
    assert!(matches!(recv_one(&high).record, Record::Observation { .. }));
    match recv_one(&low).record {
        Record::Gap {
            reason: GapReason::Lost,
            range: None,
        } => {}
        other => panic!("lower priority subscriber lost the event silently: {other:?}"),
    }

    runtime.shutdown();
    assert_eq!(runtime.scheduler().stats().live_threads, 0);
    let closed = runtime
        .bus()
        .publish(observation("budget", 2, "process.observed", 2))
        .unwrap_err();
    assert_eq!(closed.error.code(), "shutting_down");
    assert_eq!(closed.event.sequence.get(), 2);
    #[cfg(target_os = "linux")]
    assert_eq!(runtime.scheduler().named_threads_alive(), 0);
}

#[test]
fn unsubscribe_and_shutdown_release_queues_without_keeping_threads() {
    let runtime = Runtime::start(Limits {
        subscriptions: 1,
        ..limits(1, 4, 4, 20)
    })
    .unwrap();
    let first = runtime.bus().subscribe(1, Filter::default()).unwrap();
    match runtime.bus().subscribe(1, Filter::default()) {
        Err(error) => assert_eq!(error.code(), "resource_limit"),
        Ok(_) => panic!("subscription limit was not enforced"),
    }
    runtime
        .bus()
        .publish(observation("life", 1, "process.observed", 1))
        .unwrap();
    drop(first);
    assert_eq!(runtime.bus().stats().subscribers, 0);
    assert!(runtime.bus().stats().released >= 1);
    let second = runtime.bus().subscribe(1, Filter::default()).unwrap();
    runtime
        .bus()
        .publish(observation("life", 2, "process.observed", 2))
        .unwrap();
    let batch = second.recv_batch(8, Duration::from_secs(1)).unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].sequence.get(), 2);

    let bounded = EventBus::new(Limits {
        batch: 3,
        ..limits(1, 8, 8, 0)
    })
    .unwrap();
    let subscription = bounded.subscribe(1, Filter::default()).unwrap();
    for sequence in 1..=5 {
        bounded
            .publish(observation(
                "batch",
                sequence,
                "process.observed",
                sequence as i64,
            ))
            .unwrap();
    }
    assert_eq!(
        subscription
            .recv_batch(100, Duration::from_millis(0))
            .unwrap()
            .len(),
        3
    );
    drop(subscription);
    runtime.shutdown();
    assert_eq!(runtime.scheduler().stats().live_threads, 0);
    #[cfg(target_os = "linux")]
    assert_eq!(runtime.scheduler().named_threads_alive(), 0);
}

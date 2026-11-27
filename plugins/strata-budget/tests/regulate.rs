use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::thread;

use winstrata_core::{Core, EventBus, Limits, PluginHost, Runtime};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, Record, SignedNanos,
    Source, Version,
};

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "winstrata-budget-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn collector() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-budget"))
}

fn limits() -> Limits {
    Limits {
        workers: 1,
        task_queue: 2,
        event_queue: 16,
        global_queue: 16,
        batch: 16,
        subscriptions: 4,
        admission_wait: Duration::from_millis(0),
    }
}

fn observation(sequence: u64) -> Event {
    Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new("device-budget").unwrap(),
            provider: Id::new("provider.budget").unwrap(),
            epoch: Id::new("epoch-budget").unwrap(),
        },
        sequence: Counter::new(sequence).unwrap(),
        time: EventTime {
            unix_ns: SignedNanos::new(sequence as i64),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new("budget.probe").unwrap(),
            subject: None,
            user: None,
            process: None,
            attribution: Attribution::Unknown,
            completeness: Completeness::Complete,
        },
    }
}

fn fields(line: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for token in line.split(' ') {
        let (key, value) = token.split_once('=').expect("field");
        assert!(map.insert(key.to_owned(), value.to_owned()).is_none());
    }
    map
}

fn regulate(host: &mut PluginHost, payload: &str) -> BTreeMap<String, String> {
    fields(
        &host
            .call("strata.budget", "budget.regulate", payload)
            .unwrap_or_else(|error| panic!("{}: {}", error.code(), error.detail())),
    )
}

fn drain(subscription: &winstrata_core::Subscription) -> Vec<Arc<Event>> {
    let mut events = Vec::new();
    loop {
        let batch = subscription
            .recv_batch(16, Duration::from_millis(50))
            .unwrap();
        if batch.is_empty() {
            break;
        }
        events.extend(batch);
    }
    events
}

fn budget_gaps(events: &[Arc<Event>]) -> Vec<Arc<Event>> {
    events
        .iter()
        .filter(|event| event.source.provider.as_str() == "strata.budget")
        .cloned()
        .collect()
}

fn assert_lost(event: &Event) {
    match &event.record {
        Record::Gap {
            reason: GapReason::Lost,
            range: None,
        } => {}
        other => panic!("loss marker was rewritten: {other:?}"),
    }
}

fn spin(duration: Duration) {
    let start = Instant::now();
    let mut acc = 0u64;
    while start.elapsed() < duration {
        acc = acc.wrapping_add(1);
    }
    std::hint::black_box(acc);
}

const OPEN: &str = "cpu_ppm=1000000 mem_kib=4194304 queue=65536";

#[test]
fn budget_stays_absent_until_the_binary_is_beside_the_runtime() {
    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    let runtime = root.join("runtime");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    fs::write(&runtime, b"runtime").unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(&runtime).unwrap();
    let report = host.report("strata.budget").unwrap();
    assert_eq!(report.phase, "implementation_absent");
    assert_eq!(report.availability, "implementation_absent");
    assert!(!report.enabled);
    assert_eq!(
        host.enable("strata.budget").unwrap_err().code(),
        "implementation_absent"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn pressure_adjusts_limits_pauses_resumes_and_marks_loss() {
    let pressure = vec![0xA5u8; 8 * 1024 * 1024];
    std::hint::black_box(pressure.len());
    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(collector()).unwrap();
    let runtime = Runtime::start(limits()).unwrap();
    host.attach_bus(runtime.bus().clone());
    host.attach_scheduler(runtime.scheduler().handle());
    let subscription = runtime.bus().subscribe(1, Default::default()).unwrap();

    let mut next = 1u64;
    for _ in 0..16 {
        let outcome = runtime.bus().publish(observation(next)).unwrap();
        assert_eq!(outcome.lost, 0, "sequence {next} was lost early");
        next += 1;
    }
    let overflow = runtime.bus().publish(observation(next)).unwrap();
    next += 1;
    assert!(overflow.lost >= 1, "a full queue did not mark a loss");
    assert!(runtime.bus().stats().lost >= 1);
    let drained = drain(&subscription);
    let overflow_gap = drained
        .iter()
        .find(|event| {
            matches!(
                event.record,
                Record::Gap {
                    reason: GapReason::Lost,
                    ..
                }
            )
        })
        .cloned()
        .expect("queue pressure produced no loss marker");
    assert_lost(&overflow_gap);
    assert_ne!(overflow_gap.source.provider.as_str(), "strata.budget");
    assert_eq!(runtime.bus().stats().queued, 0);

    let before = host.report("strata.budget").unwrap();
    assert_eq!(before.kind, "builtin");
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    assert_eq!(before.availability, "ready");
    let state = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.budget");
    let running = host.enable("strata.budget").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");
    assert!(running.pid.is_some());
    #[cfg(target_os = "linux")]
    #[cfg(target_os = "linux")]
    let plugin_pid = running.pid.expect("budget pid");
    #[cfg(not(target_os = "linux"))]
    let _ = running.pid;

    let held = regulate(&mut host, "");
    assert_eq!(held["action"], "hold");
    assert_eq!(held["loss"], "0");
    assert_eq!(held["reason"], "none");
    assert_eq!(held["rss"], "measured");
    assert!(held["rss_kib"].parse::<u64>().unwrap() > 1);
    assert_eq!(held["cpu"], "baseline");
    assert_eq!(held["bus"], "present");
    assert_eq!(held["reports"], "1");
    assert!(budget_gaps(&drain(&subscription)).is_empty());

    let paused = regulate(&mut host, "mem_kib=1");
    assert_eq!(paused["action"], "pause");
    assert_eq!(paused["loss"], "1");
    assert_eq!(paused["reason"], "memory");
    assert_eq!(paused["mem_kib"], "1");
    let memory_gaps = budget_gaps(&drain(&subscription));
    assert_eq!(memory_gaps.len(), 1);
    assert_lost(&memory_gaps[0]);
    assert_eq!(memory_gaps[0].sequence.get(), 1);

    let still = regulate(&mut host, "mem_kib=1");
    assert_eq!(still["action"], "pause");
    assert_eq!(still["loss"], "0");
    assert_eq!(still["reason"], "memory");
    assert!(budget_gaps(&drain(&subscription)).is_empty());
    assert_lost(&memory_gaps[0]);

    let resumed = regulate(&mut host, OPEN);
    assert_eq!(resumed["action"], "resume");
    assert_eq!(resumed["loss"], "0");
    assert_eq!(resumed["reason"], "none");
    assert_eq!(resumed["mem_kib"], "4194304");
    assert_lost(&memory_gaps[0]);

    for _ in 0..4 {
        let outcome = runtime.bus().publish(observation(next)).unwrap();
        assert_eq!(outcome.lost, 0);
        next += 1;
    }
    assert!(runtime.bus().stats().queued >= 1);
    let queued = regulate(&mut host, "queue=0");
    assert_eq!(queued["action"], "pause");
    assert_eq!(queued["loss"], "1");
    assert_eq!(queued["reason"], "queue");
    assert_eq!(queued["queue"], "0");
    let queue_gaps = budget_gaps(&drain(&subscription));
    assert_eq!(queue_gaps.len(), 1);
    assert_lost(&queue_gaps[0]);
    assert_eq!(queue_gaps[0].sequence.get(), 2);
    assert_lost(&memory_gaps[0]);
    assert_lost(&overflow_gap);

    let resumed = regulate(&mut host, OPEN);
    assert_eq!(resumed["action"], "resume");
    assert_eq!(resumed["loss"], "0");
    // 队列里还留着刚才的四条事件。先取走，避免下一次限额比较被队列挡住。
    let _ = drain(&subscription);
    assert_eq!(runtime.bus().stats().queued, 0);

    let started = Instant::now();
    let mut cpu_pause = None;
    while started.elapsed() < Duration::from_secs(2) {
        spin(Duration::from_millis(200));
        let sample = regulate(&mut host, "cpu_ppm=1 mem_kib=4194304 queue=65536");
        match sample["action"].as_str() {
            "pause" => {
                assert_eq!(sample["reason"], "cpu");
                assert_eq!(sample["loss"], "1");
                assert!(sample["cpu_used_ppm"].parse::<u64>().unwrap() > 1);
                cpu_pause = Some(sample);
                break;
            }
            "hold" | "resume" => {
                assert_eq!(sample["loss"], "0");
                assert_eq!(sample["reason"], "none");
            }
            other => panic!("unexpected budget action {other}"),
        }
    }
    let cpu_pause = cpu_pause.expect("process CPU did not exceed 1 ppm");
    assert_eq!(cpu_pause["cpu"], "measured");
    let cpu_gaps = budget_gaps(&drain(&subscription));
    assert_eq!(cpu_gaps.len(), 1);
    assert_lost(&cpu_gaps[0]);
    assert_eq!(cpu_gaps[0].sequence.get(), 3);

    let resumed = regulate(&mut host, OPEN);
    assert_eq!(resumed["action"], "resume");
    assert_eq!(resumed["loss"], "0");
    let lines_before_frame_pause = fs::read_to_string(state.join("regulation.jsonl")).unwrap();
    host.pause("strata.budget").unwrap();
    assert_eq!(host.report("strata.budget").unwrap().phase, "paused");
    let kept = regulate(&mut host, "mem_kib=1");
    assert_eq!(kept["action"], "keep");
    assert_eq!(kept["loss"], "0");
    assert_eq!(kept["reason"], "none");
    assert_eq!(
        fs::read_to_string(state.join("regulation.jsonl")).unwrap(),
        lines_before_frame_pause
    );
    assert!(budget_gaps(&drain(&subscription)).is_empty());
    host.resume("strata.budget").unwrap();
    assert_eq!(host.report("strata.budget").unwrap().phase, "running");

    let paused = regulate(&mut host, "mem_kib=1");
    assert_eq!(paused["action"], "pause");
    assert_eq!(paused["loss"], "1");
    assert_eq!(paused["reason"], "memory");
    let again = budget_gaps(&drain(&subscription));
    assert_eq!(again.len(), 1);
    assert_lost(&again[0]);
    assert_eq!(
        host.emit("strata.budget", "budget.loss")
            .unwrap_err()
            .code(),
        "budget_paused"
    );
    let resumed = regulate(&mut host, OPEN);
    assert_eq!(resumed["action"], "resume");
    assert_eq!(
        host.emit("strata.budget", "budget.loss")
            .unwrap_err()
            .code(),
        "permission_denied"
    );
    assert_lost(&memory_gaps[0]);
    assert_lost(&queue_gaps[0]);
    assert_lost(&cpu_gaps[0]);
    assert_lost(&overflow_gap);
    assert_lost(&again[0]);

    let disabled = host.disable("strata.budget").unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.phase, "disabled");
    assert!(disabled.pid.is_none());
    assert_eq!(fs::read(state.join("released")).unwrap(), b"joined");
    assert_eq!(
        host.uninstall("strata.budget").unwrap_err().code(),
        "builtin_retained"
    );
    #[cfg(target_os = "linux")]
    {
        let gone = Path::new("/proc").join(plugin_pid.to_string());
        let started = Instant::now();
        while gone.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "budget process {plugin_pid} is still alive"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    std::hint::black_box(pressure[0]);
    drop(runtime);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn direct_bus_publish_still_accepts_events_while_the_budget_is_paused() {
    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(collector()).unwrap();
    let bus = EventBus::new(limits()).unwrap();
    let subscription = bus.subscribe(1, Default::default()).unwrap();
    host.attach_bus(bus.clone());
    host.enable("strata.budget").unwrap();
    let paused = regulate(&mut host, "mem_kib=1");
    assert_eq!(paused["action"], "pause");
    assert_eq!(paused["loss"], "1");
    let outcome = bus.publish(observation(1)).unwrap();
    assert_eq!(outcome.enqueued, 1);
    assert_eq!(outcome.lost, 0);
    let events = drain(&subscription);
    assert!(
        events
            .iter()
            .any(|event| event.source.provider.as_str() == "strata.budget")
    );
    assert!(events.iter().any(|event| matches!(
        event.record,
        Record::Gap {
            reason: GapReason::Lost,
            range: None
        }
    )));
    host.disable("strata.budget").unwrap();
    let _ = fs::remove_dir_all(&root);
}

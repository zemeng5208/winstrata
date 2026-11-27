use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use winstrata_core::{Core, Limits, Runtime};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, Id, Record, SignedNanos, Source, Version,
};

struct Release(Option<Sender<()>>);

impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "winstrata-health-{}-{}",
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
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-health"))
}

fn limits() -> Limits {
    Limits {
        workers: 1,
        task_queue: 1,
        event_queue: 2,
        global_queue: 2,
        batch: 2,
        subscriptions: 4,
        admission_wait: Duration::from_millis(0),
    }
}

fn observation(sequence: u64) -> Event {
    Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new("device-health").unwrap(),
            provider: Id::new("provider.health").unwrap(),
            epoch: Id::new("epoch-health").unwrap(),
        },
        sequence: Counter::new(sequence).unwrap(),
        time: EventTime {
            unix_ns: SignedNanos::new(sequence as i64),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new("health.probe").unwrap(),
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

fn number(map: &BTreeMap<String, String>, key: &str) -> u64 {
    map[key].parse().expect(key)
}

fn signed(map: &BTreeMap<String, String>, key: &str) -> i64 {
    map[key].parse().expect(key)
}

fn wait_until(deadline: Duration, mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < deadline,
            "timed out waiting for diagnostics"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn health_stays_absent_until_the_binary_is_beside_the_runtime() {
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
    let report = host.report("strata.health").unwrap();
    assert_eq!(report.phase, "implementation_absent");
    assert_eq!(report.availability, "implementation_absent");
    assert!(!report.enabled);
    assert_eq!(
        host.enable("strata.health").unwrap_err().code(),
        "implementation_absent"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn live_faults_queues_latency_and_loss_match_the_runtime() {
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
    for sequence in 1..=2 {
        let outcome = runtime.bus().publish(observation(sequence)).unwrap();
        assert_eq!(outcome.enqueued, 1);
        assert_eq!(outcome.lost, 0);
    }
    for sequence in 3..=5 {
        let outcome = runtime.bus().publish(observation(sequence)).unwrap();
        assert_eq!(outcome.lost, 1, "sequence {sequence} was not lost");
    }
    assert!(runtime.bus().stats().lost >= 1);
    assert!(runtime.bus().stats().queued >= 1);
    assert_eq!(subscription.backlog().0, runtime.bus().stats().queued);

    let (sender, receiver) = mpsc::channel();
    let release = Release(Some(sender));
    runtime
        .scheduler()
        .submit(1, Duration::from_millis(40), move |_| {
            let _ = receiver.recv();
        })
        .unwrap();
    wait_until(Duration::from_secs(2), || {
        runtime.scheduler().stats().inflight >= 1
    });
    runtime
        .scheduler()
        .submit(1, Duration::from_secs(30), |_| {})
        .unwrap();
    match runtime
        .scheduler()
        .submit(1, Duration::from_secs(30), |_| {})
    {
        Err(error) => assert_eq!(error.code(), "queue_full"),
        Ok(_) => panic!("a full task queue accepted another task"),
    }
    wait_until(Duration::from_secs(2), || {
        let stats = runtime.scheduler().stats();
        stats.timed_out >= 1 && stats.queued >= 1 && stats.rejected >= 1
    });

    let before = host.report("strata.health").unwrap();
    assert_eq!(before.kind, "builtin");
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    assert_eq!(before.availability, "ready");
    if host.report("strata.process").unwrap().phase != "implementation_absent" {
        assert_eq!(host.report("strata.process").unwrap().phase, "disabled");
        assert!(!host.report("strata.process").unwrap().enabled);
    }
    let state = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.health");
    let process_events = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.process")
        .join("events.jsonl");
    assert!(!process_events.exists());

    let running = host.enable("strata.health").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");
    assert!(running.pid.is_some());
    #[cfg(target_os = "linux")]
    let plugin_pid = running.pid.expect("health pid");
    assert_eq!(host.subscriber_count(), 1);
    if host
        .list()
        .iter()
        .any(|item| item.id == "strata.process" && item.phase != "implementation_absent")
    {
        assert_eq!(host.report("strata.process").unwrap().phase, "disabled");
    }

    let detail = host
        .call("strata.health", "health.observe", "status")
        .unwrap();
    let bus = runtime.bus().stats();
    let sched = runtime.scheduler().stats();
    let lines = fs::read_to_string(state.join("diagnostics.jsonl")).unwrap();
    let recorded: Vec<_> = lines.lines().filter(|line| !line.is_empty()).collect();
    assert_eq!(recorded.len(), 1);
    assert_reading(&fields(recorded[0]), &bus, &sched, &detail, 1);
    assert_eq!(runtime.bus().stats().lost, bus.lost);
    assert_eq!(runtime.bus().stats().queued, bus.queued);
    assert_eq!(runtime.bus().stats().subscribers, 1);
    assert_eq!(subscription.backlog().0, bus.queued);

    host.pause("strata.health").unwrap();
    let paused = host
        .call("strata.health", "health.observe", "status")
        .unwrap();
    assert!(paused.contains("reports=1"));
    assert_eq!(
        fs::read_to_string(state.join("diagnostics.jsonl")).unwrap(),
        lines
    );

    host.resume("strata.health").unwrap();
    let resumed = host
        .call("strata.health", "health.observe", "status")
        .unwrap();
    let after = fs::read_to_string(state.join("diagnostics.jsonl")).unwrap();
    let recorded: Vec<_> = after.lines().filter(|line| !line.is_empty()).collect();
    assert_eq!(recorded.len(), 2);
    assert_reading(
        &fields(recorded[1]),
        &runtime.bus().stats(),
        &runtime.scheduler().stats(),
        &resumed,
        2,
    );
    assert!(!process_events.exists());

    let disabled = host.disable("strata.health").unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.phase, "disabled");
    assert!(disabled.pid.is_none());
    assert_eq!(fs::read(state.join("released")).unwrap(), b"joined");
    assert_eq!(
        host.uninstall("strata.health").unwrap_err().code(),
        "builtin_retained"
    );
    #[cfg(target_os = "linux")]
    {
        let gone = Path::new("/proc").join(plugin_pid.to_string());
        let started = Instant::now();
        while gone.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "health process {plugin_pid} is still alive"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    drop(release);
    let _ = runtime;
    let _ = fs::remove_dir_all(&root);
}

fn assert_reading(
    map: &BTreeMap<String, String>,
    bus: &winstrata_core::BusStats,
    sched: &winstrata_core::SchedulerStats,
    detail: &str,
    reports: u64,
) {
    assert_eq!(number(map, "bus_queued"), bus.queued as u64);
    assert_eq!(number(map, "bus_pending"), bus.pending as u64);
    assert_eq!(number(map, "bus_global"), bus.global_queued as u64);
    assert_eq!(number(map, "lost"), bus.lost);
    assert_eq!(number(map, "filtered"), bus.filtered);
    assert_eq!(number(map, "queue_rejects"), bus.queue_rejects);
    assert_eq!(number(map, "sched_queued"), sched.queued as u64);
    assert_eq!(number(map, "sched_inflight"), sched.inflight as u64);
    assert_eq!(number(map, "sched_rejected"), sched.rejected);
    assert_eq!(number(map, "sched_timed_out"), sched.timed_out);
    assert_eq!(number(map, "sched_panicked"), sched.panicked);
    assert_eq!(map["bus"], "present");
    assert_eq!(map["sched"], "present");
    assert_eq!(number(map, "plugin_faults"), 0);
    let faults = number(map, "plugin_faults") + sched.timed_out + sched.panicked;
    let queues =
        (bus.queued + bus.pending + bus.global_queued + sched.queued + sched.inflight) as u64;
    let loss = bus.lost + bus.filtered + bus.queue_rejects + sched.rejected;
    assert!(bus.lost >= 1);
    assert!(bus.queued >= 1);
    assert!(sched.queued >= 1);
    assert!(sched.rejected >= 1);
    assert!(sched.timed_out >= 1);
    assert_eq!(number(map, "faults"), faults);
    assert_eq!(number(map, "queues"), queues);
    assert_eq!(number(map, "loss"), loss);
    let sent = signed(map, "sent_ns");
    let observed = signed(map, "observed_ns");
    let latency = number(map, "latency_ns");
    assert_eq!(latency, observed.saturating_sub(sent) as u64);
    assert!(observed.abs_diff(sent) < 2_000_000_000);
    assert_eq!(
        detail,
        format!(
            "faults={faults} queues={queues} latency_ns={latency} loss={loss} reports={reports}"
        )
    );
}

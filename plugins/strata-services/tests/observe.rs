use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use winstrata_core::{Core, EventBus, Limits};
use winstrata_protocols::{Event, Record};
use winstrata_strata_services::capture;

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "winstrata-services-{}-{}",
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
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-services"))
}

fn read_events(state: &Path) -> Vec<Event> {
    let mut events = Vec::new();
    for name in ["events.prev.jsonl", "events.jsonl"] {
        let Ok(text) = fs::read_to_string(state.join(name)) else {
            continue;
        };
        let mut lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
        if !text.ends_with('\n') {
            lines.pop();
        }
        for line in lines {
            events.push(Event::from_json(line).unwrap_or_else(|error| {
                panic!("service event is not valid protocol JSON: {error}")
            }));
        }
    }
    events
}

fn samples(state: &Path) -> u64 {
    fs::read_to_string(state.join("samples"))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

fn event_type(event: &Event) -> Option<&str> {
    match &event.record {
        Record::Observation { event_type, .. } => Some(event_type.as_str()),
        Record::Gap { .. } => None,
    }
}

fn wait_until(limit: Duration, mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(start.elapsed() < limit, "timed out waiting for service observation");
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn real_service_and_driver_observation_releases_on_disable() {
    let live = capture().expect("live service capture");
    let again = capture().expect("second live capture");
    assert_eq!(live.device, again.device);
    assert_eq!(live.boot, again.boot);
    assert!(matches!(live.evidence, "procfs" | "scm"));
    assert_ne!(live.evidence, "toolhelp");

    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(collector()).unwrap();
    host.attach_bus(EventBus::new(Limits::standard()).unwrap());
    let state = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.services");

    let before = host.report("strata.services").unwrap();
    assert_eq!(before.kind, "builtin");
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    assert_eq!(before.availability, "ready");
    let running = host.enable("strata.services").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");
    #[cfg(target_os = "linux")]
    let plugin_pid = running.pid.expect("collector process");
    #[cfg(not(target_os = "linux"))]
    let _plugin_pid = running.pid.expect("collector process");
    let status = host
        .call("strata.services", "services.observe", "status")
        .unwrap();
    assert!(status.starts_with("samples="), "{status}");

    wait_until(Duration::from_secs(8), || samples(&state) >= 1);
    let summary: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(state.join("state.json")).unwrap()).unwrap();
    let services = summary["services"].as_u64().unwrap();
    let drivers = summary["drivers"].as_u64().unwrap();
    let events = read_events(&state);
    for event in &events {
        assert_eq!(event.source.device.as_str(), "strata.services");
        assert!(matches!(
            event.source.provider.as_str(),
            "procfs" | "windows.scm"
        ));
        assert_ne!(event.source.provider.as_str(), "winstrata.process");
        assert!(event.time.precision_ns.is_none());
    }
    if services == 0 && drivers == 0 {
        assert!(events.iter().all(|event| {
            !matches!(
                event_type(event),
                Some("service.state" | "driver.state" | "service.stopped" | "service.created")
            )
        }));
    } else {
        assert!(
            events.iter().any(|event| {
                matches!(
                    event_type(event),
                    Some("service.state" | "driver.state" | "driver.loaded" | "service.created")
                )
            }),
            "a non-empty sample produced no state event"
        );
    }
    let first_states = events
        .iter()
        .filter(|event| matches!(event_type(event), Some("service.state" | "driver.state")))
        .count();
    let settled = samples(&state);
    wait_until(Duration::from_secs(3), || samples(&state) > settled);
    let later = read_events(&state);
    let later_states = later
        .iter()
        .filter(|event| matches!(event_type(event), Some("service.state" | "driver.state")))
        .count();
    assert_eq!(first_states, later_states);

    host.pause("strata.services").unwrap();
    assert_eq!(host.report("strata.services").unwrap().phase, "paused");
    thread::sleep(Duration::from_millis(450));
    let paused = samples(&state);
    thread::sleep(Duration::from_millis(450));
    assert_eq!(samples(&state), paused);
    host.resume("strata.services").unwrap();
    wait_until(Duration::from_secs(3), || samples(&state) > paused);

    let disabled = host.disable("strata.services").unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.phase, "disabled");
    assert!(disabled.pid.is_none());
    assert_eq!(fs::read(state.join("released")).unwrap(), b"joined");
    #[cfg(target_os = "linux")]
    {
        let gone = Path::new("/proc").join(plugin_pid.to_string());
        let started = Instant::now();
        while gone.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "collector process {plugin_pid} is still alive"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    let frozen = samples(&state);
    thread::sleep(Duration::from_millis(500));
    assert_eq!(samples(&state), frozen);
    let _ = fs::remove_dir_all(&root);
}

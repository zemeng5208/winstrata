use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use winstrata_core::{Core, EventBus, Limits};
#[cfg(target_os = "linux")]
use winstrata_protocols::Attribution;
use winstrata_protocols::{Event, Record};
use winstrata_strata_startup::capture;

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "winstrata-startup-{}-{}",
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
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-startup"))
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
                panic!("startup event is not valid protocol JSON: {error}")
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

#[cfg(target_os = "linux")]
fn subject_id(event: &Event) -> Option<&str> {
    match &event.record {
        Record::Observation {
            subject: Some(subject),
            ..
        } => Some(subject.id.as_str()),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn observed(event: &Event) -> Option<&str> {
    match &event.record {
        Record::Observation {
            attribution: Attribution::Observed { evidence },
            ..
        } => Some(evidence.as_str()),
        _ => None,
    }
}

fn wait_until(limit: Duration, mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < limit,
            "timed out waiting for startup observation"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn real_startup_and_task_observation_releases_on_disable() {
    let live = capture().expect("live startup capture");
    let again = capture().expect("second live capture");
    assert_eq!(live.device, again.device);
    assert_eq!(live.boot, again.boot);
    assert!(matches!(live.evidence, "host-files" | "windows"));

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
        .join("strata.startup");

    let before = host.report("strata.startup").unwrap();
    assert_eq!(before.kind, "builtin");
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    assert_eq!(before.availability, "ready");
    let running = host.enable("strata.startup").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");
    #[cfg(target_os = "linux")]
    let plugin_pid = running.pid.expect("collector process");
    #[cfg(not(target_os = "linux"))]
    let _plugin_pid = running.pid.expect("collector process");
    let status = host
        .call("strata.startup", "startup.observe", "status")
        .unwrap();
    assert!(status.starts_with("samples="), "{status}");

    wait_until(Duration::from_secs(8), || samples(&state) >= 1);
    let summary: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(state.join("state.json")).unwrap()).unwrap();
    let startup = summary["startup"].as_u64().unwrap();
    let tasks = summary["tasks"].as_u64().unwrap();
    let events = read_events(&state);
    for event in &events {
        assert_eq!(event.source.device.as_str(), "strata.startup");
        assert!(matches!(
            event.source.provider.as_str(),
            "posix.startup" | "windows.startup"
        ));
        assert!(event.time.precision_ns.is_none());
        if let Record::Observation { user, process, .. } = &event.record {
            assert!(user.is_none());
            assert!(process.is_none());
        }
    }
    if startup == 0 && tasks == 0 {
        assert!(
            events.iter().all(|event| {
                !matches!(event_type(event), Some("startup.state" | "task.state"))
            })
        );
    } else {
        assert!(
            events
                .iter()
                .any(|event| { matches!(event_type(event), Some("startup.state" | "task.state")) }),
            "a non-empty sample produced no state event"
        );
    }
    let first_states = events
        .iter()
        .filter(|event| matches!(event_type(event), Some("startup.state" | "task.state")))
        .count();
    let settled = samples(&state);
    wait_until(Duration::from_secs(3), || samples(&state) > settled);
    let later = read_events(&state);
    let later_states = later
        .iter()
        .filter(|event| matches!(event_type(event), Some("startup.state" | "task.state")))
        .count();
    assert_eq!(first_states, later_states);
    assert!(state.join("inventory.json").is_file());

    host.pause("strata.startup").unwrap();
    assert_eq!(host.report("strata.startup").unwrap().phase, "paused");
    thread::sleep(Duration::from_millis(450));
    let paused = samples(&state);
    thread::sleep(Duration::from_millis(450));
    assert_eq!(samples(&state), paused);
    host.resume("strata.startup").unwrap();
    wait_until(Duration::from_secs(3), || samples(&state) > paused);

    let disabled = host.disable("strata.startup").unwrap();
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

#[cfg(target_os = "linux")]
#[test]
fn linux_autostart_and_timer_changes_keep_attribution_across_restart() {
    let config = winstrata_strata_startup::user_config_dir().expect("user config");
    let autostart = config.join("autostart");
    let timers = config.join("systemd/user");
    fs::create_dir_all(&autostart).unwrap();
    fs::create_dir_all(&timers).unwrap();
    let name = format!("winstrata-023-{}.desktop", std::process::id());
    let timer_name = format!("winstrata-023-{}.timer", std::process::id());
    let desktop = autostart.join(&name);
    let timer = timers.join(&timer_name);
    let _guard = RemoveOnDrop {
        paths: vec![desktop.clone(), timer.clone()],
    };
    let _ = fs::remove_file(&desktop);
    let _ = fs::remove_file(&timer);

    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(collector()).unwrap();
    let state = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.startup");
    host.enable("strata.startup").unwrap();
    wait_until(Duration::from_secs(8), || samples(&state) >= 1);

    fs::write(
        &desktop,
        "[Desktop Entry]\nType=Application\nName=WinStrata023Probe\nExec=/bin/true\n",
    )
    .unwrap();
    let id = format!("autostart:{name}");
    wait_until(Duration::from_secs(8), || {
        read_events(&state).iter().any(|event| {
            event_type(event) == Some("startup.added") && subject_id(event) == Some(id.as_str())
        })
    });
    let added = read_events(&state)
        .into_iter()
        .find(|event| {
            event_type(event) == Some("startup.added") && subject_id(event) == Some(id.as_str())
        })
        .unwrap();
    let evidence = observed(&added).unwrap();
    assert!(evidence.contains("origin=autostart"), "{evidence}");
    assert!(evidence.contains("command=/bin/true"), "{evidence}");
    assert!(evidence.contains("principal=user"), "{evidence}");

    fs::write(
        &timer,
        "[Timer]\nOnCalendar=daily\n[Install]\nWantedBy=timers.target\n",
    )
    .unwrap();
    let timer_id = format!("timer:user:{timer_name}");
    wait_until(Duration::from_secs(8), || {
        read_events(&state).iter().any(|event| {
            event_type(event) == Some("task.added") && subject_id(event) == Some(timer_id.as_str())
        })
    });
    let task = read_events(&state)
        .into_iter()
        .find(|event| subject_id(event) == Some(timer_id.as_str()))
        .unwrap();
    let task_evidence = observed(&task).unwrap();
    assert!(task_evidence.contains("origin=timer"), "{task_evidence}");
    assert!(task_evidence.contains("enabled=0"), "{task_evidence}");
    assert!(
        task_evidence.contains("schedule=OnCalendar=daily"),
        "{task_evidence}"
    );

    fs::write(
        &desktop,
        "[Desktop Entry]\nType=Application\nName=WinStrata023Probe\nExec=/bin/false\n",
    )
    .unwrap();
    wait_until(Duration::from_secs(8), || {
        read_events(&state).iter().any(|event| {
            event_type(event) == Some("startup.changed")
                && subject_id(event) == Some(id.as_str())
                && observed(event).is_some_and(|text| text.contains("before=/bin/true"))
        })
    });

    host.disable("strata.startup").unwrap();
    assert_eq!(fs::read(state.join("released")).unwrap(), b"joined");
    fs::write(
        &desktop,
        "[Desktop Entry]\nType=Application\nName=WinStrata023Probe\nExec=/bin/other\n",
    )
    .unwrap();
    let mark = read_events(&state).len();
    host.enable("strata.startup").unwrap();
    wait_until(Duration::from_secs(8), || {
        read_events(&state).iter().skip(mark).any(|event| {
            event_type(event) == Some("startup.changed")
                && subject_id(event) == Some(id.as_str())
                && observed(event).is_some_and(|text| {
                    text.contains("before=/bin/false") && text.contains("command=/bin/other")
                })
        })
    });
    assert!(
        read_events(&state).iter().skip(mark).all(|event| {
            !(event_type(event) == Some("startup.state") && subject_id(event) == Some(id.as_str()))
        }),
        "reloading history must not baseline the probe again"
    );

    fs::remove_file(&desktop).unwrap();
    fs::remove_file(&timer).unwrap();
    wait_until(Duration::from_secs(8), || {
        let events = read_events(&state);
        events.iter().skip(mark).any(|event| {
            event_type(event) == Some("startup.removed") && subject_id(event) == Some(id.as_str())
        }) && events.iter().skip(mark).any(|event| {
            event_type(event) == Some("task.removed")
                && subject_id(event) == Some(timer_id.as_str())
        })
    });
    host.disable("strata.startup").unwrap();
    let _ = fs::remove_dir_all(&root);
}

#[cfg(target_os = "linux")]
struct RemoveOnDrop {
    paths: Vec<PathBuf>,
}

#[cfg(target_os = "linux")]
impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use winstrata_core::{Core, EventBus, Limits};
use winstrata_protocols::{Attribution, Completeness, Event, Record};

struct Held(Option<Child>);

impl Held {
    fn spawn() -> Self {
        let mut command = hold_command();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Self(Some(command.spawn().expect("start a real child process")))
    }

    fn id(&self) -> u32 {
        self.0.as_ref().expect("child is running").id()
    }

    fn finish(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(target_os = "linux")]
fn hold_command() -> Command {
    let mut command = Command::new("/bin/sleep");
    command.arg("120");
    command
}

#[cfg(windows)]
fn hold_command() -> Command {
    let mut command = Command::new("ping.exe");
    command.args(["-n", "40", "127.0.0.1"]);
    command
}

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "winstrata-process-{}-{}",
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
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-process"))
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
                panic!("process event is not valid protocol JSON: {error}")
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

fn identity(event: &Event) -> Option<(u32, i64)> {
    match &event.record {
        Record::Observation {
            process: Some(process),
            ..
        } => Some((process.pid, process.started_unix_ns.get())),
        _ => None,
    }
}

fn count_kind(events: &[Event], kind: &str, pid: u32, start: i64) -> usize {
    events
        .iter()
        .filter(|event| event_type(event) == Some(kind) && identity(event) == Some((pid, start)))
        .count()
}

fn evidence(event: &Event) -> Option<&str> {
    match &event.record {
        Record::Observation {
            attribution: Attribution::Observed { evidence } | Attribution::Inferred { evidence },
            ..
        } => Some(evidence.as_str()),
        _ => None,
    }
}

fn wait_until(deadline: Duration, mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < deadline,
            "timed out waiting for the process sample"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn settle(state: &Path, stable_for: Duration) -> u64 {
    let start = Instant::now();
    let mut last = samples(state);
    let mut since = Instant::now();
    loop {
        thread::sleep(Duration::from_millis(40));
        let now = samples(state);
        if now != last {
            last = now;
            since = Instant::now();
        } else if since.elapsed() >= stable_for {
            return last;
        }
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "sample count did not settle"
        );
    }
}

#[test]
fn real_process_thread_module_tree_and_release_on_disable() {
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
        .join("strata.process");

    let before = host.report("strata.process").unwrap();
    assert_eq!(before.kind, "builtin");
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    let running = host.enable("strata.process").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");
    let plugin_pid = running.pid.expect("collector process");
    assert!(plugin_pid > 0);
    let status = host
        .call("strata.process", "process.observe", "status")
        .unwrap();
    assert!(status.starts_with("samples="), "{status}");

    let mut child = Held::spawn();
    let child_pid = child.id();
    let parent_pid = std::process::id();
    let mut start = 0i64;
    wait_until(Duration::from_secs(8), || {
        let events = read_events(&state);
        let Some(event) = events.iter().find(|event| {
            event_type(event) == Some("process.started")
                && identity(event).is_some_and(|(pid, _)| pid == child_pid)
        }) else {
            return false;
        };
        start = identity(event).unwrap().1;
        let observed = matches!(
            event.record,
            Record::Observation {
                attribution: Attribution::Observed { .. },
                completeness: Completeness::Complete,
                ..
            }
        );
        let source = event.source.provider.as_str() == "winstrata.process"
            && matches!(evidence(event), Some("procfs" | "toolhelp"));
        observed && source && count_kind(&events, "process.started", child_pid, start) == 1
    });
    wait_until(Duration::from_secs(8), || {
        let events = read_events(&state);
        count_kind(&events, "process.thread.started", child_pid, start) >= 1
            && count_kind(&events, "process.module.loaded", child_pid, start) >= 1
    });
    let tree: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(state.join("tree.json")).unwrap()).unwrap();
    let nodes = tree["nodes"].as_array().expect("tree nodes");
    let child_node = nodes
        .iter()
        .find(|node| node["pid"].as_u64() == Some(u64::from(child_pid)))
        .expect("child is in the tree");
    assert_eq!(
        child_node["parent_pid"].as_u64(),
        Some(u64::from(parent_pid))
    );
    assert_eq!(
        child_node["start_unix_ns"].as_str(),
        Some(start.to_string().as_str())
    );
    let parent_start = child_node["parent_start_unix_ns"]
        .as_str()
        .expect("parent start");
    assert!(nodes.iter().any(|node| {
        node["pid"].as_u64() == Some(u64::from(parent_pid))
            && node["start_unix_ns"].as_str() == Some(parent_start)
    }));

    let after_start = samples(&state);
    wait_until(Duration::from_secs(3), || samples(&state) > after_start);
    assert_eq!(
        count_kind(&read_events(&state), "process.started", child_pid, start),
        1
    );

    host.pause("strata.process").unwrap();
    assert_eq!(host.report("strata.process").unwrap().phase, "paused");
    let paused = settle(&state, Duration::from_millis(450));
    thread::sleep(Duration::from_millis(450));
    assert_eq!(samples(&state), paused);
    host.resume("strata.process").unwrap();
    wait_until(Duration::from_secs(3), || samples(&state) > paused);

    child.finish();
    wait_until(Duration::from_secs(8), || {
        count_kind(&read_events(&state), "process.exited", child_pid, start) == 1
    });
    let exit = read_events(&state)
        .into_iter()
        .find(|event| {
            event_type(event) == Some("process.exited")
                && identity(event) == Some((child_pid, start))
        })
        .expect("exit");
    assert!(matches!(
        exit.record,
        Record::Observation {
            completeness: Completeness::Partial { .. },
            ..
        }
    ));
    let sampled = samples(&state);
    wait_until(Duration::from_secs(3), || samples(&state) > sampled);
    assert_eq!(
        count_kind(&read_events(&state), "process.exited", child_pid, start),
        1
    );
    let tree: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(state.join("tree.json")).unwrap()).unwrap();
    assert!(tree["nodes"].as_array().unwrap().iter().all(|node| {
        node["pid"].as_u64() != Some(u64::from(child_pid))
            || node["start_unix_ns"].as_str() != Some(start.to_string().as_str())
    }));

    let disabled = host.disable("strata.process").unwrap();
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
    assert_eq!(
        count_kind(&read_events(&state), "process.exited", child_pid, start),
        1
    );
    let _ = fs::remove_dir_all(&root);
}

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use winstrata_core::{Core, EventBus, Limits};
use winstrata_protocols::Id;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "winstrata-pulse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn collector() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-pulse"))
}

fn samples(state: &Path) -> u64 {
    fs::read_to_string(state.join("samples"))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

fn wait_until(deadline: Duration, mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < deadline,
            "timed out waiting for a pulse sample"
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

fn latest(state: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(state.join("latest.json")).unwrap()).unwrap()
}

fn reading<'a>(document: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    document["readings"]
        .as_array()
        .expect("readings")
        .iter()
        .find(|reading| reading["id"] == id)
        .unwrap_or_else(|| panic!("missing reading {id}"))
}

fn history(state: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(state.join("history.jsonl")).unwrap_or_default();
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).expect("history line"))
        .collect()
}

#[test]
fn counters_history_and_unsupported_readings_survive_pause_and_disable() {
    let scratch = Scratch::new();
    let locator = scratch.0.join("locator");
    let data = scratch.0.join("data");
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
        .join("strata.pulse");

    let before = host.report("strata.pulse").unwrap();
    assert_eq!(before.kind, "builtin");
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    assert_eq!(before.availability, "ready");
    assert_eq!(
        host.uninstall("strata.pulse").unwrap_err().code(),
        "builtin_retained"
    );

    let running = host.enable("strata.pulse").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");
    let status = host
        .call("strata.pulse", "pulse.observe", "status")
        .unwrap();
    assert!(status.starts_with("samples="), "{status}");

    wait_until(Duration::from_secs(8), || {
        if samples(&state) < 2 {
            return false;
        }
        let document = latest(&state);
        reading(&document, "memory.total.bytes")["status"] == "measured"
            && reading(&document, "cpu.usage.ppm")["status"] == "measured"
    });

    let document = latest(&state);
    assert_eq!(document["store"], "committed");
    let total = reading(&document, "memory.total.bytes")["value"]
        .as_i64()
        .expect("memory total");
    let available = reading(&document, "memory.available.bytes")["value"]
        .as_i64()
        .expect("memory available");
    let used = reading(&document, "memory.used.bytes")["value"]
        .as_i64()
        .expect("memory used");
    assert!(total > 0);
    assert!(available >= 0);
    assert!(available <= total);
    assert_eq!(used, total - available);
    let cpu = reading(&document, "cpu.usage.ppm")["value"]
        .as_i64()
        .expect("cpu");
    assert!((0..=1_000_000).contains(&cpu));
    let count = reading(&document, "process.count")["value"]
        .as_i64()
        .expect("process count");
    assert!(count >= 1);

    for item in document["readings"].as_array().unwrap() {
        match item["status"].as_str() {
            Some("measured") => assert!(item["value"].is_i64(), "{item}"),
            Some("baseline" | "unsupported" | "failed") => {
                assert!(item.get("value").is_none(), "{item}");
                assert!(item["reason"].is_string(), "{item}");
            }
            other => panic!("unexpected status {other:?} in {item}"),
        }
    }
    let gpu = reading(&document, "gpu.usage.ppm");
    if gpu["status"] == "unsupported" {
        assert!(gpu.get("value").is_none());
        assert!(gpu["reason"].as_str().is_some());
    }

    let lines = history(&state);
    assert!(lines.len() >= 2);
    let first_cpu = reading(&lines[0], "cpu.usage.ppm");
    assert_eq!(first_cpu["status"], "baseline");
    assert!(first_cpu.get("value").is_none());
    let peaks: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(state.join("peaks.json")).unwrap()).unwrap();
    assert!(peaks["pulse.memory.total.bytes"].as_i64().unwrap() > 0);

    host.pause("strata.pulse").unwrap();
    assert_eq!(host.report("strata.pulse").unwrap().phase, "paused");
    let paused = settle(&state, Duration::from_millis(450));
    thread::sleep(Duration::from_millis(450));
    assert_eq!(samples(&state), paused);

    let store = core.open_hybrid().unwrap();
    let series = Id::new("pulse.memory.total.bytes").unwrap();
    let points = store.metrics(&series, 0, i64::MAX).unwrap();
    assert!(points.iter().any(|point| point.value == total));
    assert!(points.iter().all(|point| point.value > 0));
    if gpu["status"] == "unsupported" {
        let gpu_series = Id::new("pulse.gpu.usage.ppm").unwrap();
        assert!(store.metrics(&gpu_series, 0, i64::MAX).unwrap().is_empty());
    }
    drop(store);

    host.resume("strata.pulse").unwrap();
    wait_until(Duration::from_secs(3), || samples(&state) > paused);
    let frozen = samples(&state);
    host.disable("strata.pulse").unwrap();
    assert_eq!(host.report("strata.pulse").unwrap().phase, "disabled");
    assert!(host.report("strata.pulse").unwrap().pid.is_none());
    assert_eq!(fs::read(state.join("released")).unwrap(), b"joined");
    thread::sleep(Duration::from_millis(300));
    assert_eq!(samples(&state), frozen);
}

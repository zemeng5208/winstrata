use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use winstrata_protocols::{Event, ProtocolError, Version};

fn replay() -> Value {
    json!({
        "version": {"major": 1, "minor": 0},
        "source": {"device": "fixture-device", "provider": "test.provider", "epoch": "session-a"},
        "sequence": "9007199254740993",
        "time": {"unix_ns": "-1", "precision_ns": "100"},
        "record": {
            "kind": "observation", "event_type": "process.observed",
            "subject": {"kind": "process_observation", "id": "sample"},
            "process": {"boot": "fixture-boot", "pid": 321, "started_unix_ns": "-2"},
            "attribution": {"kind": "observed", "evidence": "test.fixture"},
            "completeness": {"kind": "complete"}
        }
    })
}

fn decode(value: &Value) -> Event {
    Event::from_json(&value.to_string()).unwrap()
}

#[test]
fn node_observes_its_actual_pid_and_exchanges_versioned_records() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/interop.mjs");
    let mut child = Command::new("node")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start the workspace Node.js runtime");
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // 仅知道实际子进程的 PID 和观测时刻，不能将观测时刻当作进程启动时间。
    let live = decode(&json!({
        "version": {"major": 1, "minor": 0},
        "source": {
            "device": "interop-fixture-device", "provider": "rust.child_observer",
            "epoch": format!("test-{}-{time}", std::process::id())
        },
        "sequence": "1", "time": {"unix_ns": time.to_string()},
        "record": {
            "kind": "observation", "event_type": "process.observed",
            "subject": {"kind": "pid_observation", "id": child.id().to_string()},
            "attribution": {"kind": "observed", "evidence": "std.process.Child.id"},
            "completeness": {"kind": "partial", "reason": "process_start_unavailable"}
        }
    }));
    let fixture = replay();
    let mut input = child.stdin.take().unwrap();
    for event in [&live, &decode(&fixture)] {
        writeln!(input, "{}", event.to_json().unwrap()).unwrap();
    }
    let diagnostic = Event::from_json("{").unwrap_err();
    writeln!(input, "{}", diagnostic.to_json().unwrap()).unwrap();
    drop(input);

    let deadline = Instant::now() + Duration::from_secs(15);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().expect("reap the timed-out Node.js process");
            panic!("Node.js protocol peer timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "Node.js exited with {}",
        output.status
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let replies: Vec<_> = text.lines().collect();
    assert_eq!(replies.len(), 5);

    assert_eq!(Event::from_json(replies[0]).unwrap(), live);
    let mut expected = fixture;
    expected["version"]["minor"] = json!(1);
    expected["sequence"] = json!("9007199254740994");
    let mut future = Event::from_json(replies[1]).unwrap();
    assert_eq!(future, decode(&expected));
    assert_eq!(
        future.to_json().unwrap_err().code.as_str(),
        "unsupported_version"
    );
    assert!(serde_json::to_value(&future).is_err());
    future.version = Version::CURRENT;
    assert_eq!(
        Event::from_json(&future.to_json().unwrap()).unwrap(),
        future
    );
    for (wire, code) in [
        (replies[2], "unsupported_version"),
        (replies[3], "invalid_record"),
        ("{", "invalid_json"),
    ] {
        assert_eq!(Event::from_json(wire).unwrap_err().code.as_str(), code);
    }
    let diagnostic = ProtocolError::from_json(replies[4]).unwrap();
    assert_eq!(diagnostic.code.as_str(), "source.access_denied");
    let native = diagnostic.native.unwrap();
    assert_eq!(native.domain.as_str(), "win32");
    assert_eq!(native.code.as_str(), "5");
}

#[test]
fn replay_preserves_clock_rollback_epoch_restart_and_reused_pid() {
    let first = decode(&replay());
    let mut row = replay();
    row["sequence"] = json!("9007199254740994");
    row["time"]["unix_ns"] = json!("-10");
    let second = decode(&row);
    assert_eq!(first.source, second.source);
    assert!(first.sequence < second.sequence);
    assert_ne!(first.key(), second.key());
    assert!(first.time.unix_ns > second.time.unix_ns);

    row["source"]["epoch"] = json!("session-b");
    row["sequence"] = json!("1");
    row["time"]["unix_ns"] = json!("10");
    let after_restart = decode(&row);
    assert_ne!(first.source.epoch, after_restart.source.epoch);
    let mut repeated_sequence = first.clone();
    repeated_sequence.sequence = after_restart.sequence;
    assert_ne!(repeated_sequence.key(), after_restart.key());
    assert_eq!(after_restart.sequence.get(), 1);
    assert_eq!(first.record, after_restart.record);

    row["sequence"] = json!("2");
    row["time"]["unix_ns"] = json!("100");
    row["record"]["process"]["started_unix_ns"] = json!("80");
    let reused = decode(&row);
    assert_eq!(reused.source, after_restart.source);
    let before: Value = serde_json::from_str(&after_restart.to_json().unwrap()).unwrap();
    let after: Value = serde_json::from_str(&reused.to_json().unwrap()).unwrap();
    let old_process = &before["record"]["process"];
    let new_process = &after["record"]["process"];
    assert_eq!(before["source"]["device"], after["source"]["device"]);
    assert_eq!(old_process["boot"], new_process["boot"]);
    assert_eq!(old_process["pid"], new_process["pid"]);
    assert_ne!(
        old_process["started_unix_ns"],
        new_process["started_unix_ns"]
    );
}

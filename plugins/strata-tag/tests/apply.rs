use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use winstrata_core::{flow_event, Core, EventBus, Filter, FlowFact, Limits};
use winstrata_kernel::Observer;
use winstrata_protocols::{Attribution, Event, Record};

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "winstrata-tag-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn programs() -> (PathBuf, PathBuf) {
    let tag = PathBuf::from(env!("CARGO_BIN_EXE_winstrata-strata-tag"));
    let runtime_name = if cfg!(windows) {
        "winstrata-runtime.exe"
    } else {
        "winstrata-runtime"
    };
    let script_name = if cfg!(windows) {
        "winstrata-strata-script.exe"
    } else {
        "winstrata-strata-script"
    };
    let runtime = tag
        .parent()
        .expect("tag program directory")
        .join(runtime_name);
    let script = tag
        .parent()
        .expect("tag program directory")
        .join(script_name);
    assert!(runtime.is_file(), "runtime is not beside the tag program");
    assert!(
        script.is_file(),
        "script program is not beside the tag program"
    );
    (runtime, tag)
}

fn call(
    host: &mut winstrata_core::PluginHost,
    id: &str,
    capability: &str,
    payload: Value,
) -> Value {
    let body = payload.to_string();
    let detail = host
        .call(id, capability, &body)
        .unwrap_or_else(|error| panic!("{}: {}", error.code(), error.detail()));
    serde_json::from_str(&detail).unwrap_or_else(|_| panic!("tag result is not json: {detail}"))
}

fn marker_files(root: &Path, marker: &[u8]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if let Ok(bytes) = fs::read(&path) {
                if bytes.windows(marker.len()).any(|window| window == marker) {
                    found.push(path);
                }
            }
        }
    }
    found
}

fn evidence_of(event: &Event) -> String {
    match &event.record {
        Record::Observation { attribution, .. } => match attribution {
            Attribution::Observed { evidence } | Attribution::Inferred { evidence } => {
                evidence.as_str().to_owned()
            }
            Attribution::Unknown => "unknown".to_owned(),
        },
        Record::Gap { .. } => panic!("flow became a gap"),
    }
}

fn basis_of(event: &Event) -> &'static str {
    match &event.record {
        Record::Observation { attribution, .. } => match attribution {
            Attribution::Observed { .. } => "observed",
            Attribution::Inferred { .. } => "inferred",
            Attribution::Unknown => "unknown",
        },
        Record::Gap { .. } => "unknown",
    }
}

#[test]
fn tag_stays_absent_until_the_binary_is_beside_the_runtime() {
    let root = scratch("absent");
    let locator = root.join("locator");
    let data = root.join("data");
    let runtime = root.join("runtime");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    fs::write(&runtime, b"runtime").unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(&runtime).unwrap();
    let report = host.report("strata.tag").unwrap();
    assert_eq!(report.phase, "implementation_absent");
    assert_eq!(report.availability, "implementation_absent");
    assert!(!report.enabled);
    assert_eq!(
        host.enable("strata.tag").unwrap_err().code(),
        "implementation_absent"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn manual_script_and_plugin_tags_keep_untagged_packets_off_disk() {
    let root = scratch("apply");
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let (runtime, _) = programs();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let user = core.current_user().to_owned();
    let mut host = core.open_plugins(&runtime).unwrap();
    let bus = EventBus::new(Limits::standard()).unwrap();
    host.attach_bus(bus.clone());
    let subscription = bus.subscribe(8, Filter::default()).unwrap();
    let running = host.enable("strata.tag").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");
    let state = data
        .join(&user)
        .join("plugins")
        .join("state")
        .join("strata.tag");

    let probe = root.join("probe.bin");
    fs::write(&probe, b"file").unwrap();
    let length = fs::metadata(&probe).unwrap().len();
    let manual = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "file",
            "subject": "probe-file",
            "reason": "risk.file",
            "source": "desk",
            "confidence": "stated",
            "evidence": format!("len={length}"),
            "attribution": "observed",
            "actor": "agent",
            "safety": "safe"
        }),
    );
    assert_eq!(manual["code"], "applied");
    assert_eq!(manual["actor"], "user");
    assert_eq!(manual["actor_id"], user);
    assert_eq!(manual["safety"], "none");
    assert_eq!(manual["wrote"], false);
    assert!(manual["time_unix_ns"].as_i64().unwrap_or(0) > 0);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server = listener.local_addr().unwrap();
    let observer = Observer::open();
    let mut client = std::net::TcpStream::connect(server).unwrap();
    let (mut accepted, _) = listener.accept().unwrap();
    let payload = b"WS029FLOW";
    client.write_all(payload).unwrap();
    let mut received = [0u8; 16];
    let size = accepted.read(&mut received).unwrap();
    assert_eq!(&received[..size], payload);
    let snapshot = observer.collect(Duration::from_millis(400), 32).unwrap();
    let client_addr = client.local_addr().unwrap();
    let outbound = snapshot
        .endpoints
        .iter()
        .find(|endpoint| endpoint.local == client_addr && endpoint.remote == server)
        .expect("real outbound flow");
    let fact = FlowFact {
        transport: outbound.transport.as_str(),
        local: outbound.local,
        remote: outbound.remote,
        direction: outbound.direction.as_str(),
        basis: outbound.direction.basis(),
        pid: outbound.pid,
        payload: outbound.payload,
    };
    let flow = flow_event("local-tag", "run-029", 1, 10, &fact).unwrap();
    bus.publish(flow.clone()).unwrap();
    let evidence = evidence_of(&flow);
    assert!(
        evidence.len() <= 256,
        "flow evidence does not fit a tag: {evidence}"
    );
    let tagged_flow = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "event",
            "subject": "flow-1",
            "reason": "risk.connection",
            "source": "manual",
            "confidence": "stated",
            "evidence": evidence,
            "attribution": basis_of(&flow)
        }),
    );
    assert_eq!(tagged_flow["code"], "applied");
    assert_eq!(tagged_flow["actor"], "user");
    assert_eq!(tagged_flow["evidence"], evidence);
    assert_eq!(tagged_flow["safety"], "none");

    let rejected = host
        .call("strata.tag", "tag.policy", r#"{"mode":"all","cap":4096}"#)
        .unwrap_err();
    assert_eq!(rejected.code(), "policy_rejected");
    let marker = b"WS029TAG-PACKET";
    let packet = host.stage_packet(marker.to_vec()).unwrap();
    let held = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "packet",
            "subject": "packet-held",
            "packet": packet,
            "reason": "risk.connection",
            "source": "manual",
            "confidence": "stated",
            "evidence": "memory",
            "attribution": "observed"
        }),
    );
    assert_eq!(held["code"], "memory_only");
    assert_eq!(held["wrote"], false);
    assert_eq!(held["safety"], "none");
    assert!(marker_files(&root, marker).is_empty());

    let switched = call(
        &mut host,
        "strata.tag",
        "tag.policy",
        json!({"mode": "tagged_only"}),
    );
    assert_eq!(switched["code"], "policy");
    assert_eq!(switched["wrote"], false);
    let untagged = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "packet",
            "subject": "packet-open",
            "packet": packet,
            "reason": "",
            "source": "",
            "confidence": "",
            "evidence": "none",
            "attribution": "unknown"
        }),
    );
    assert_eq!(untagged["code"], "untagged");
    assert_eq!(untagged["safety"], "none");
    assert_eq!(untagged["wrote"], false);
    let queried = call(
        &mut host,
        "strata.tag",
        "tag.query",
        json!({"subject": "packet-open"}),
    );
    assert_eq!(queried["tagged"], false);
    assert_eq!(queried["safety"], "none");
    assert_eq!(queried["code"], "untagged");
    let bad = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "packet",
            "subject": "packet-bad",
            "packet": packet,
            "reason": " padded",
            "source": "manual",
            "confidence": "stated",
            "evidence": "bad",
            "attribution": "observed"
        }),
    );
    assert_eq!(bad["code"], "tag_rejected");
    assert!(marker_files(&root, marker).is_empty());
    let stored = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "packet",
            "subject": "packet-kept",
            "packet": packet,
            "reason": "risk.connection",
            "source": "manual",
            "confidence": "stated",
            "evidence": "kept",
            "attribution": "observed"
        }),
    );
    assert_eq!(stored["code"], "applied");
    assert_eq!(stored["wrote"], true);
    let captures = state.join("captures");
    let files: Vec<_> = fs::read_dir(&captures)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(fs::read(&files[0]).unwrap(), marker);
    assert_eq!(marker_files(&root, marker).len(), 1);

    let late_a = b"LATE029A";
    let late_b = b"LATE029B";
    let policy = call(
        &mut host,
        "strata.tag",
        "tag.policy",
        json!({"mode": "tagged_only", "cap": 8}),
    );
    assert_eq!(policy["code"], "policy");
    let first = host.stage_packet(late_a.to_vec()).unwrap();
    let _second = host.stage_packet(late_b.to_vec()).unwrap();
    let late = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "packet",
            "subject": "packet-late",
            "packet": first,
            "reason": "risk.connection",
            "source": "manual",
            "confidence": "stated",
            "evidence": "late",
            "attribution": "observed"
        }),
    );
    assert_eq!(late["code"], "unavailable");
    assert_eq!(late["wrote"], false);
    assert!(marker_files(&root, late_a).is_empty());
    assert!(marker_files(&root, late_b).is_empty());
    assert_eq!(marker_files(&root, marker).len(), 1);

    host.pause("strata.tag").unwrap();
    let paused = call(
        &mut host,
        "strata.tag",
        "tag.apply",
        json!({
            "kind": "event",
            "subject": "paused-event",
            "reason": "risk.file",
            "source": "manual",
            "confidence": "stated",
            "evidence": "paused",
            "attribution": "observed"
        }),
    );
    assert_eq!(paused["code"], "paused");
    host.resume("strata.tag").unwrap();

    host.enable("strata.script").unwrap();
    let created = call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({
            "op": "create",
            "language": "python",
            "name": "tagger",
            "data": "cGFzcw=="
        }),
    );
    let script_id = created["id"].as_str().unwrap().to_owned();
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "enable-script", "id": script_id}),
    );
    let denied = call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({
            "op": "tag",
            "id": script_id,
            "kind": "event",
            "subject": "script-denied",
            "reason": "risk.connection",
            "source": "script",
            "confidence": "stated",
            "evidence": "denied",
            "attribution": "observed"
        }),
    );
    assert_eq!(denied["code"], "permission_denied");
    let lines_before = fs::read_to_string(state.join("tags.jsonl"))
        .unwrap()
        .lines()
        .count();
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({
            "op": "declare",
            "id": script_id,
            "permissions": ["plugin.call"]
        }),
    );
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "grant", "id": script_id, "permission": "plugin.call"}),
    );
    let scripted = call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({
            "op": "tag",
            "id": script_id,
            "kind": "event",
            "subject": "script-flow",
            "reason": "risk.connection",
            "source": "script",
            "confidence": "inferred",
            "evidence": "script",
            "attribution": "inferred"
        }),
    );
    assert_eq!(scripted["code"], "applied");
    assert_eq!(scripted["actor"], "script");
    assert_eq!(scripted["actor_id"], script_id);
    assert_eq!(scripted["safety"], "none");
    let lines_after = fs::read_to_string(state.join("tags.jsonl"))
        .unwrap()
        .lines()
        .count();
    assert_eq!(lines_after, lines_before + 1);
    assert_eq!(
        fs::read_to_string(
            data.join(&user)
                .join("plugins")
                .join("state")
                .join("strata.script")
                .join("preset.count"),
        )
        .unwrap(),
        "0"
    );

    let package = root.join("fixture");
    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("manifest.json"),
        br#"{"version":{"major":1,"minor":0},"id":"fixture.tagger","name":"fixture.tagger","plugin_version":{"major":1,"minor":0,"patch":0},"kind":"third_party","runtime":"native","capabilities":["fixture.ready"],"permissions":["plugin.call"],"dependencies":[],"events":[],"rescue":false}"#,
    )
    .unwrap();
    host.install(&package).unwrap();
    host.enable("fixture.tagger").unwrap();
    let plugin_tag = call(
        &mut host,
        "fixture.tagger",
        "tag.apply",
        json!({
            "kind": "process",
            "subject": "plugin-proc",
            "reason": "risk.process",
            "source": "plugin",
            "confidence": "stated",
            "evidence": "plugin",
            "attribution": "observed",
            "actor": "user"
        }),
    );
    assert_eq!(plugin_tag["actor"], "plugin");
    assert_eq!(plugin_tag["actor_id"], "fixture.tagger");
    assert_eq!(plugin_tag["code"], "applied");
    assert_eq!(plugin_tag["safety"], "none");

    let events = subscription
        .recv_batch(64, Duration::from_millis(200))
        .unwrap();
    let tag_events: Vec<Arc<Event>> = events
        .into_iter()
        .filter(|event| event.source.provider.as_str() == "strata.tag")
        .collect();
    assert!(tag_events
        .iter()
        .any(|event| event_type(event) == "tag.applied"));
    assert!(tag_events
        .iter()
        .any(|event| event_type(event) == "tag.rejected"));
    assert!(tag_events
        .iter()
        .any(|event| event_type(event) == "tag.late"));
    assert!(tag_events.iter().any(|event| match &event.record {
        Record::Observation {
            user: Some(user_id),
            attribution,
            ..
        } => {
            user_id.as_str() == user
                && matches!(
                    attribution,
                    Attribution::Observed { .. } | Attribution::Inferred { .. }
                )
        }
        _ => false,
    }));

    let disabled = host.disable("strata.tag").unwrap();
    assert!(!disabled.enabled);
    assert_eq!(
        fs::read_to_string(state.join("released")).unwrap(),
        "joined"
    );
    assert_eq!(
        host.call("strata.tag", "tag.apply", r#"{"kind":"event"}"#)
            .unwrap_err()
            .code(),
        "not_running"
    );
    assert_eq!(marker_files(&root, marker).len(), 1);
    assert!(marker_files(&root, late_a).is_empty());
    assert_eq!(
        host.uninstall("strata.tag").unwrap_err().code(),
        "builtin_retained"
    );
    let _ = (client, accepted);
    let _ = fs::remove_dir_all(&root);
}

fn event_type(event: &Event) -> &str {
    match &event.record {
        Record::Observation { event_type, .. } => event_type.as_str(),
        Record::Gap { .. } => "",
    }
}

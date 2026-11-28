use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use winstrata_core::{Batch, CaptureWrite, Confidence, Core, EventBus, HybridRiskTag, Limits};
use winstrata_protocols::Id;

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "winstrata-storage-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn programs() -> PathBuf {
    let storage = PathBuf::from(env!("CARGO_BIN_EXE_winstrata-strata-storage"));
    let dir = storage.parent().expect("storage program directory");
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
    let runtime = dir.join(runtime_name);
    let script = dir.join(script_name);
    assert!(
        runtime.is_file(),
        "runtime is not beside the storage program"
    );
    assert!(
        script.is_file(),
        "script program is not beside the storage program"
    );
    runtime
}

fn ident(value: &str) -> Id {
    Id::new(value).unwrap()
}

fn capture(id: &str, reason: &str, link_type: u16) -> CaptureWrite {
    CaptureWrite {
        id: ident(id),
        tag: Some(HybridRiskTag {
            source: ident("strata-tag"),
            reason: ident(reason),
            unix_ns: 50,
            confidence: Confidence::Stated,
        }),
        link_type,
        timestamp_ns: 2_000,
        bytes: format!("\x45\x00{id}").into_bytes(),
    }
}

fn open(root: &Path) -> (Core, PathBuf) {
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir_all(&locator).unwrap();
    fs::create_dir_all(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    (core, locator)
}

fn notice(core: &mut Core) -> Value {
    serde_json::from_str(&core.storage_notice().unwrap()).unwrap()
}

fn call(
    host: &mut winstrata_core::PluginHost,
    id: &str,
    capability: &str,
    payload: Value,
) -> Value {
    let detail = host
        .call(id, capability, &payload.to_string())
        .unwrap_or_else(|error| panic!("{}: {}", error.code(), error.detail()));
    serde_json::from_str(&detail).unwrap_or_else(|_| panic!("storage result is not json: {detail}"))
}

#[test]
fn thresholds_survive_restart_and_confirmed_deletion_updates_indexes() {
    let root = scratch("cleanup");
    let _ = fs::remove_dir_all(&root);
    let (mut core, locator) = open(&root);
    let user = root.join("data").join(core.current_user());
    let outside = user.join("outside.log");
    fs::write(&outside, b"leave-this-file").unwrap();
    {
        let mut store = core.open_hybrid().unwrap();
        let mut batch = Batch::new();
        batch
            .append_capture(capture("pkt-a", "risk.drop", 1))
            .unwrap();
        core.commit_hybrid(&mut store, batch).unwrap();
        let mut batch = Batch::new();
        batch
            .append_capture(capture("pkt-b", "risk.drop", 101))
            .unwrap();
        core.commit_hybrid(&mut store, batch).unwrap();
        let mut batch = Batch::new();
        batch
            .append_capture(capture("pkt-c", "risk.keep", 201))
            .unwrap();
        core.commit_hybrid(&mut store, batch).unwrap();
    }
    core.append_saved_log("kept.log", b"saved log line\n")
        .unwrap();
    let before = notice(&mut core);
    assert_eq!(before["storage"], "ready");
    assert_eq!(before["pending"], false);
    assert_eq!(before["capture"]["first"], 1_000_000_000);
    assert_eq!(before["logs"]["first"], 2_000_000_000);
    assert_eq!(before["logs"]["repeat"], 500_000_000);
    let capture_usage = before["capture"]["usage"].as_u64().unwrap();
    let log_usage = before["logs"]["usage"].as_u64().unwrap();
    assert!(capture_usage >= 2, "{capture_usage}");
    assert!(log_usage >= 2, "{log_usage}");

    let runtime = programs();
    let mut host = core.open_plugins(&runtime).unwrap();
    let bus = EventBus::new(Limits::standard()).unwrap();
    host.attach_bus(bus);
    let seeded = host.report("strata.storage").unwrap();
    assert!(!seeded.enabled);
    assert_eq!(seeded.phase, "disabled");
    assert_eq!(
        host.uninstall("strata.storage").unwrap_err().code(),
        "builtin_retained"
    );
    host.enable("strata.storage").unwrap();
    host.enable("strata.script").unwrap();
    let created = call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "create", "language": "python", "name": "storage", "data": "cGFzcw=="}),
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
            "op": "storage",
            "id": script_id,
            "action": "configure",
            "capture_first": 1,
            "capture_repeat": 1,
            "log_first": 1,
            "log_repeat": 1
        }),
    );
    assert_eq!(denied["code"], "permission_denied");
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "declare", "id": script_id, "permissions": ["plugin.call"]}),
    );
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "grant", "id": script_id, "permission": "plugin.call"}),
    );
    let configured = call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({
            "op": "storage",
            "id": script_id,
            "action": "configure",
            "capture_first": 1,
            "capture_repeat": 1,
            "log_first": 1,
            "log_repeat": 1
        }),
    );
    assert_eq!(configured["code"], "configured");
    assert_eq!(configured["deleted"], false);
    let forged = call(
        &mut host,
        "strata.storage",
        "storage.configure",
        json!({
            "op": "confirm",
            "confirm": true,
            "capture_first": 1,
            "capture_repeat": 1,
            "log_first": 1,
            "log_repeat": 1
        }),
    );
    assert_eq!(forged["code"], "confirmation_required");
    assert_eq!(forged["deleted"], false);
    drop(host);

    let pending = notice(&mut core);
    assert_eq!(pending["pending"], true);
    assert_eq!(pending["capture"]["first"], 1);
    assert_eq!(pending["capture"]["declines"], 0);
    assert!(pending["capture"]["crossed"].as_u64().unwrap() >= 2);
    assert!(pending["logs"]["crossed"].as_u64().unwrap() >= 2);
    drop(core);
    let mut core = Core::open_at(&locator, None).unwrap();
    let restarted = notice(&mut core);
    assert_eq!(restarted["pending"], true);
    assert_eq!(restarted["capture"]["declines"], 0);
    assert_eq!(restarted["capture"]["armed"], 1);
    assert!(
        user.join("data")
            .join("captures")
            .read_dir()
            .unwrap()
            .next()
            .is_some()
    );
    assert!(user.join("logs").join("kept.log").is_file());

    let declined =
        serde_json::from_str::<Value>(&core.decline_storage("capture").unwrap()).unwrap();
    assert_eq!(declined["declined"], true);
    assert_eq!(declined["capture"]["declines"], 1);
    assert_eq!(declined["capture"]["armed"], 2);
    assert_eq!(declined["capture"]["pending"], true);
    assert_eq!(declined["deleted"], false);
    drop(core);
    let mut core = Core::open_at(&locator, None).unwrap();
    let remembered = notice(&mut core);
    assert_eq!(remembered["capture"]["declines"], 1);
    assert_eq!(remembered["capture"]["pending"], true);

    let raised = serde_json::from_str::<Value>(
        &core
            .configure_storage(capture_usage + 10, 1, log_usage + 10, 1)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(raised["pending"], false);
    assert_eq!(raised["capture"]["first"], capture_usage + 10);
    let lowered =
        serde_json::from_str::<Value>(&core.configure_storage(1, 1, 1, 1).unwrap()).unwrap();
    assert_eq!(lowered["pending"], true);
    assert_eq!(lowered["deleted"], false);

    let expired =
        serde_json::from_str::<Value>(&core.propose_storage(r#"{"op":"expiration"}"#).unwrap())
            .unwrap();
    assert_eq!(expired["code"], "expiration_unconfirmed");
    assert_eq!(expired["deleted"], false);
    let capacity =
        serde_json::from_str::<Value>(&core.propose_storage(r#"{"op":"capacity"}"#).unwrap())
            .unwrap();
    assert_eq!(capacity["code"], "selection_required");
    assert_eq!(capacity["deleted"], false);
    let blocked = serde_json::from_str::<Value>(
        &core
            .propose_storage(r#"{"op":"selection","ids":["pkt-a"],"confirm":true}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(blocked["code"], "confirmation_required");
    {
        let store = core.open_hybrid().unwrap();
        assert!(store.capture(&ident("pkt-a")).unwrap().is_some());
        assert!(store.capture(&ident("pkt-b")).unwrap().is_some());
    }
    assert_eq!(fs::read(&outside).unwrap(), b"leave-this-file");

    let proposal = serde_json::from_str::<Value>(
        &core
            .propose_storage(r#"{"op":"selection","ids":["pkt-a"]}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(proposal["code"], "proposal");
    assert_eq!(proposal["deleted"], false);
    assert_eq!(proposal["deletable"].as_array().unwrap().len(), 1);
    let proposal_id = proposal["id"].as_str().unwrap().to_owned();
    let held =
        serde_json::from_str::<Value>(&core.confirm_storage(&proposal_id, false).unwrap()).unwrap();
    assert_eq!(held["code"], "confirmation_required");
    assert_eq!(held["deleted"], false);
    let stale = serde_json::from_str::<Value>(&core.confirm_storage("00", true).unwrap()).unwrap();
    assert_eq!(stale["code"], "stale");
    assert_eq!(stale["deleted"], false);
    let deleted =
        serde_json::from_str::<Value>(&core.confirm_storage(&proposal_id, true).unwrap()).unwrap();
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted["code"], "deleted");
    {
        let store = core.open_hybrid().unwrap();
        assert!(store.capture(&ident("pkt-a")).unwrap().is_none());
        assert!(store.lookup(&ident("pkt-a")).unwrap().is_empty());
        assert!(store.capture(&ident("pkt-b")).unwrap().is_some());
        assert!(store.capture(&ident("pkt-c")).unwrap().is_some());
    }
    drop(core);
    let mut core = Core::open_at(&locator, None).unwrap();
    {
        let store = core.open_hybrid().unwrap();
        assert!(store.capture(&ident("pkt-a")).unwrap().is_none());
        assert!(store.capture(&ident("pkt-b")).unwrap().is_some());
    }
    assert_eq!(fs::read(&outside).unwrap(), b"leave-this-file");
    assert!(user.join("logs").join("kept.log").is_file());
    let audit = fs::read_to_string(user.join("storage-audit.jsonl")).unwrap();
    assert!(audit.contains("captures"));
    assert!(audit.contains(&proposal_id));

    let protected = serde_json::from_str::<Value>(
        &core
            .propose_storage(
                r#"{"op":"selection","ids":["pkt-b","pkt-c"],"protected":["risk.keep"]}"#,
            )
            .unwrap(),
    )
    .unwrap();
    assert!(
        protected["held"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["reason"] == "protected")
    );
    let protected_id = protected["id"].as_str().unwrap();
    let removed =
        serde_json::from_str::<Value>(&core.confirm_storage(protected_id, true).unwrap()).unwrap();
    assert_eq!(removed["deleted"], true);
    {
        let store = core.open_hybrid().unwrap();
        assert!(store.capture(&ident("pkt-b")).unwrap().is_none());
        assert!(store.capture(&ident("pkt-c")).unwrap().is_some());
    }

    let logs = serde_json::from_str::<Value>(
        &core
            .propose_storage(r#"{"op":"selection","ids":["kept.log"]}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(logs["logs"][0], "kept.log");
    let log_id = logs["id"].as_str().unwrap();
    core.confirm_storage(log_id, true).unwrap();
    assert!(!user.join("logs").join("kept.log").exists());
    assert_eq!(fs::read(&outside).unwrap(), b"leave-this-file");

    let mut host = core.open_plugins(&runtime).unwrap();
    let bus = EventBus::new(Limits::standard()).unwrap();
    host.attach_bus(bus);
    host.enable("strata.storage").unwrap();
    host.disable("strata.storage").unwrap();
    drop(host);
    let alerts = notice(&mut core);
    assert_eq!(alerts["storage"], "ready");
    assert_eq!(alerts["code"], "notice");
    let _ = fs::remove_dir_all(&root);
}

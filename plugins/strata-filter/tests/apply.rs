use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use winstrata_core::{Core, EventBus, LanguageRequirement, Limits, PluginHost};

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "winstrata-filter-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn filter_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-filter"))
}

fn script_bin() -> PathBuf {
    let name = if cfg!(windows) {
        "winstrata-strata-script.exe"
    } else {
        "winstrata-strata-script"
    };
    filter_bin().parent().unwrap().join(name)
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn requirement() -> LanguageRequirement {
    let root = workspace();
    let package: Value =
        serde_json::from_str(&fs::read_to_string(root.join("package.json")).unwrap()).unwrap();
    let declared: Value =
        serde_json::from_str(&fs::read_to_string(root.join("toolchains/runtimes.json")).unwrap())
            .unwrap();
    LanguageRequirement {
        python_minimum: version(declared["python_minimum"].as_str().unwrap()),
        node_exact: version(package["engines"]["node"].as_str().unwrap()),
    }
}

fn version(text: &str) -> (u16, u16, u16) {
    let numbers: Vec<u16> = text
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().unwrap())
        .collect();
    (numbers[0], numbers[1], numbers[2])
}

fn program(env: &str, fallbacks: &[&str]) -> PathBuf {
    if let Some(requested) = std::env::var_os(env) {
        let requested = PathBuf::from(requested);
        if requested.is_absolute() || requested.is_file() {
            return requested;
        }
        if let Some(found) = on_path(&[requested.to_str().unwrap_or("")]) {
            return found;
        }
    }
    on_path(fallbacks).unwrap_or_else(|| PathBuf::from(fallbacks[0]))
}

fn on_path(names: &[&str]) -> Option<PathBuf> {
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for name in names {
            if name.is_empty() {
                continue;
            }
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let mut value = 0u32;
        for (index, byte) in chunk.iter().enumerate() {
            value |= u32::from(*byte) << (16 - index * 8);
        }
        for index in 0..4 {
            if index > chunk.len() {
                out.push('=');
            } else {
                out.push(TABLE[((value >> (18 - index * 6)) & 0x3f) as usize] as char);
            }
        }
    }
    out
}

fn call(host: &mut PluginHost, id: &str, capability: &str, payload: Value) -> Value {
    let body = serde_json::to_string(&payload).unwrap();
    assert!(body.len() <= 1024, "{body}");
    assert!(!body.chars().any(char::is_control), "{body}");
    let detail = host
        .call(id, capability, &body)
        .unwrap_or_else(|error| panic!("{} {}", error.code(), error.detail()));
    serde_json::from_str(&detail).unwrap_or_else(|error| panic!("{error}: {detail}"))
}

fn item(actor: &str, kind: &str, event: &str, path: &str, seq: u64) -> Value {
    json!({
        "actor": actor,
        "kind": kind,
        "event": event,
        "path": path,
        "process": "vim",
        "app": "editor",
        "protocol": "tcp",
        "ip": "10.1.0.8",
        "port": "443",
        "iface": "eth0",
        "critical": true,
        "seq": seq,
    })
}

#[test]
fn gui_script_batch_io_and_self_recursion() {
    let script = script_bin();
    assert!(
        script.is_file(),
        "script workbench is not beside the filter program"
    );
    #[cfg(unix)]
    {
        let shim = filter_bin().parent().unwrap().join("libwinstrata_shim.so");
        assert!(
            shim.is_file(),
            "isolation shim is not beside the filter program"
        );
    }
    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(filter_bin()).unwrap();
    host.attach_bus(EventBus::new(Limits::standard()).unwrap());
    assert!(!host.protect_observation(r#"{"actor":"self","kind":"ordinary","event":"storage.write","path":"/data/WinStrata/events/a","seq":1}"#).unwrap());
    assert!(host.protect_observation(r#"{"actor":"other","kind":"outside","event":"file.modified","path":"/data/WinStrata/settings.json","seq":2}"#).unwrap());
    assert!(
        !host
            .protect_observation(
                r#"{"actor":"other","kind":"traffic","event":"packet.raw","path":"","seq":3}"#
            )
            .unwrap()
    );
    let before = host.report("strata.filter").unwrap();
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    assert_eq!(
        host.uninstall("strata.filter").unwrap_err().code(),
        "builtin_retained"
    );
    let state = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.filter");
    assert!(!state.join("batches").exists());

    host.bind_languages(
        &program(
            "WINSTRATA_PYTHON",
            if cfg!(windows) {
                &["python.exe", "python3.exe", "python3"]
            } else {
                &["python3"]
            },
        ),
        &program(
            "WINSTRATA_NODE",
            if cfg!(windows) {
                &["node.exe", "node"]
            } else {
                &["node"]
            },
        ),
        &workspace().join("sdk"),
        &requirement(),
    )
    .unwrap_or_else(|error| panic!("bind toolchain: {} {}", error.code(), error.detail()));
    let running = host.enable("strata.filter").unwrap();
    assert_eq!(running.phase, "running");
    host.enable("strata.script").unwrap();
    let source = b"from pathlib import Path\nrows = Path('batch.txt').read_text(encoding='utf-8').splitlines()\nprint('\\n'.join('drop' if 'noise' in row else 'keep' for row in rows))\n";
    let created = call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "create", "language": "python", "name": "batchfilter", "data": encode(source)}),
    );
    let script_id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["presets"], 0, "{created}");
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "declare", "id": script_id, "permissions": ["storage.private"]}),
    );
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "grant", "id": script_id, "permission": "storage.private"}),
    );
    call(
        &mut host,
        "strata.script",
        "script.workbench",
        json!({"op": "enable-script", "id": script_id}),
    );

    let paused = host.pause("strata.filter").unwrap();
    assert_eq!(paused.phase, "paused");
    let held = call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "apply", "items": [item("other", "outside", "file.modified", "/data/WinStrata/settings.json", 1)]}),
    );
    assert_eq!(held["phase"], "paused", "{held}");
    assert_eq!(held["batches"], 0);
    host.resume("strata.filter").unwrap();

    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "tune", "buffer": 8, "compress": true, "dedup": true, "sample": 1, "archive": "keep"}),
    );
    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "rule", "field": "path", "mode": "exclude", "value": "/tmp/gui-noise"}),
    );
    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "rule", "field": "event", "mode": "include", "value": "file.modified"}),
    );
    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "script", "id": script_id}),
    );
    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "apply", "items": [
            item("self", "ordinary", "storage.write", "/data/WinStrata/events/seg", 1),
            item("self", "ordinary", "log.appended", "/data/WinStrata/logs/core", 2),
            item("self", "ordinary", "ipc.frame", "", 3),
            item("other", "outside", "file.modified", "/tmp/gui-noise", 4),
            item("other", "traffic", "packet.raw", "", 5),
        ]}),
    );
    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "apply", "items": [
            item("other", "outside", "file.modified", "/data/WinStrata/settings.json", 6),
            item("other", "outside", "file.modified", "/data/WinStrata/settings.json", 7),
            item("other", "outside", "file.modified", "/data/noise-script", 8),
            item("other", "outside", "file.added", "/data/WinStrata/skipped", 9),
        ]}),
    );
    let done = call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "flush"}),
    );
    assert_eq!(done["phase"], "verdict", "{done}");
    assert_eq!(done["runs"], 1, "{done}");
    assert_eq!(done["batches"], 1, "{done}");
    assert_eq!(done["kept"], 1, "{done}");
    assert_eq!(done["suppressed"], 4, "{done}");
    assert!(done["excluded"].as_u64().unwrap() >= 2, "{done}");
    assert_eq!(done["deduped"], 1, "{done}");
    assert_eq!(done["scripted"], 1, "{done}");
    assert_eq!(
        done["avoided"].as_u64().unwrap() + done["kept"].as_u64().unwrap(),
        done["seen"].as_u64().unwrap()
    );
    let files = batch_files(&state);
    assert_eq!(files.len(), 1, "one batch write for the whole flush");
    let stored = fs::read(&files[0]).unwrap();
    assert_eq!(done["overhead"].as_u64().unwrap(), stored.len() as u64);
    assert!(stored.windows(4).any(|window| window == b"WFB1"));
    let text = String::from_utf8_lossy(&stored);
    assert!(text.contains("settings.json"), "{text}");
    assert!(!text.contains("gui-noise"), "{text}");
    assert!(!text.contains("noise-script"), "{text}");
    assert!(!text.contains("events/seg"), "{text}");
    assert!(!text.contains("logs/core"), "{text}");

    let recursion = call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "apply", "items": [item("self", "outside", "file.modified", "/data/WinStrata/batches/b0001.wfb", 10)]}),
    );
    assert_eq!(recursion["batches"], 1, "{recursion}");
    assert_eq!(recursion["suppressed"], 5, "{recursion}");
    assert_eq!(batch_files(&state).len(), 1);

    let preserved = call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "apply", "items": [item("other", "outside", "file.modified", "/data/WinStrata/settings.json", 11)]}),
    );
    assert_eq!(preserved["buffered"], 1, "{preserved}");
    let second = call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "flush"}),
    );
    assert_eq!(second["runs"], 2, "{second}");
    assert_eq!(second["batches"], 2, "{second}");
    assert_eq!(second["kept"], 2, "{second}");
    assert_eq!(batch_files(&state).len(), 2);

    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "script", "id": "missing"}),
    );
    call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "apply", "items": [item("other", "outside", "file.modified", "/data/WinStrata/kept-basic", 12)]}),
    );
    let fell = call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "flush"}),
    );
    assert_eq!(fell["phase"], "fallback", "{fell}");
    assert_eq!(fell["fallbacks"], 1, "{fell}");
    assert_eq!(fell["kept"], 3, "{fell}");
    assert!(fell["runs"].as_u64().unwrap() == 2, "{fell}");
    let log = fs::read_to_string(state.join("fallback.log")).unwrap();
    assert!(log.contains("code="), "{log}");
    let basic = fs::read(batch_files(&state).last().unwrap()).unwrap();
    assert!(String::from_utf8_lossy(&basic).contains("kept-basic"));
    assert_eq!(batch_files(&state).len(), 3);

    let show = call(
        &mut host,
        "strata.filter",
        "filter.apply",
        json!({"op": "show"}),
    );
    assert!(show["overhead"].as_u64().unwrap() > 0, "{show}");
    assert!(show["raw"].as_u64().unwrap() > 0, "{show}");
    host.disable("strata.filter").unwrap();
    assert_eq!(fs::read(state.join("released")).unwrap(), b"joined");
    let script_presets = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.script")
        .join("presets");
    assert!(fs::read_dir(&script_presets).unwrap().next().is_none());
    let _ = fs::remove_dir_all(&root);
}

fn batch_files(state: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(state.join("batches"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    files
}

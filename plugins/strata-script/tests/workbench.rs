use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use winstrata_core::{Core, EventBus, LanguageRequirement, Limits, PluginHost};

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "winstrata-script-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn script_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_winstrata-strata-script"))
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

fn python_program() -> PathBuf {
    program(
        "WINSTRATA_PYTHON",
        if cfg!(windows) {
            &["python.exe", "python3.exe", "python3"]
        } else {
            &["python3"]
        },
    )
}

fn node_program() -> PathBuf {
    program(
        "WINSTRATA_NODE",
        if cfg!(windows) {
            &["node.exe", "node"]
        } else {
            &["node"]
        },
    )
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

fn decode(text: &str) -> Vec<u8> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let mut value = 0u32;
        let mut pads = 0;
        for shift in (0..4).rev() {
            let byte = bytes[index];
            index += 1;
            if byte == b'=' {
                pads += 1;
                continue;
            }
            let digit = TABLE.iter().position(|item| *item == byte).unwrap();
            value |= (digit as u32) << (shift * 6);
        }
        out.push((value >> 16) as u8);
        if pads < 2 {
            out.push((value >> 8) as u8);
        }
        if pads < 1 {
            out.push(value as u8);
        }
    }
    out
}

fn text_of(value: &Value) -> String {
    String::from_utf8(decode(value["output"].as_str().unwrap_or(""))).unwrap_or_default()
}

fn call(host: &mut PluginHost, payload: Value) -> Value {
    let body = serde_json::to_string(&payload).unwrap();
    assert!(body.len() <= 1024, "{body}");
    let detail = host
        .call("strata.script", "script.workbench", &body)
        .unwrap_or_else(|error| panic!("{} {}", error.code(), error.detail()));
    serde_json::from_str(&detail).unwrap_or_else(|error| panic!("{error}: {detail}"))
}

fn open() -> (Core, PluginHost, PathBuf) {
    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let mut host = core.open_plugins(script_bin()).unwrap();
    host.attach_bus(EventBus::new(Limits::standard()).unwrap());
    host.bind_languages(
        &python_program(),
        &node_program(),
        &workspace().join("sdk"),
        &requirement(),
    )
    .unwrap_or_else(|error| panic!("bind toolchain: {} {}", error.code(), error.detail()));
    let state = data
        .join(core.current_user())
        .join("plugins")
        .join("state")
        .join("strata.script");
    (core, host, state)
}

fn assert_no_shipped_presets() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    walk_no_scripts(crate_dir);
    let sdk = workspace().join("sdk");
    assert!(!sdk.join("presets").exists());
    walk_no_preset_dir(&sdk);
}

fn walk_no_scripts(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk_no_scripts(&path);
            continue;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        assert!(
            !name.ends_with(".py")
                && !name.ends_with(".js")
                && !name.ends_with(".mjs")
                && !name.ends_with(".ts"),
            "shipped script {}",
            path.display()
        );
    }
}

fn walk_no_preset_dir(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some("presets") {
            panic!("sdk preset directory {}", path.display());
        }
        if path.is_dir() {
            walk_no_preset_dir(&path);
        }
    }
}

#[test]
fn editor_languages_debug_triggers_logs_permissions_and_empty_presets() {
    assert_no_shipped_presets();
    // Linux 工作区检查会先构建工作区，因此垫片在脚本程序旁边。`cargo test` 本身不编译只有 cdylib 的垫片包。Windows 不加载这个文件。
    #[cfg(unix)]
    {
        let shim = script_bin().parent().unwrap().join("libwinstrata_shim.so");
        assert!(shim.is_file(), "isolation shim is not beside the script program");
    }
    let (_core, mut host, state) = open();
    let before = host.report("strata.script").unwrap();
    assert!(!before.enabled);
    assert_eq!(before.phase, "disabled");
    assert_eq!(before.availability, "ready");
    assert_eq!(
        host.uninstall("strata.script").unwrap_err().code(),
        "builtin_retained"
    );
    let running = host.enable("strata.script").unwrap();
    assert!(running.enabled);
    assert_eq!(running.phase, "running");

    let package = state
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("packages")
        .join("strata.script");
    let names: Vec<_> = fs::read_dir(&package)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["manifest.json".to_owned()]);
    let empty = call(&mut host, json!({"op": "empty"}));
    assert_eq!(empty["presets"], 0);
    assert_eq!(empty["scripts"], 0);
    assert!(fs::read_dir(state.join("presets")).unwrap().next().is_none());
    assert_eq!(
        host.read_private("strata.script", "preset.count").unwrap(),
        "0"
    );

    let created = call(
        &mut host,
        json!({"op": "create", "language": "python", "name": "hello", "part": 0, "end": false, "data": encode(b"print(")}),
    );
    assert_eq!(created["pending"], true);
    let created = call(
        &mut host,
        json!({"op": "create", "language": "python", "name": "hello", "part": 1, "end": true, "data": encode(b"6)\n")}),
    );
    let python = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["enabled"], false);
    assert_eq!(created["presets"], 0);
    let blocked = call(&mut host, json!({"op": "run", "id": python}));
    assert_eq!(blocked["code"], "disabled");

    let diagnosis = call(&mut host, json!({"op": "diagnose", "id": python}));
    assert!(diagnosis["items"].as_array().unwrap().is_empty(), "{diagnosis}");
    let tokens = call(&mut host, json!({"op": "highlight", "id": python}));
    assert!(tokens["tokens"].as_array().unwrap().iter().any(|token| token[0] == "kw"));
    let completion = call(&mut host, json!({"op": "complete", "id": python, "prefix": "pri"}));
    assert!(completion["items"].as_array().unwrap().iter().any(|item| item == "print"));
    let broken = call(
        &mut host,
        json!({"op": "create", "language": "javascript", "name": "broken", "data": encode(b"const value = \"open")}),
    );
    let broken_id = broken["id"].as_str().unwrap();
    let faults = call(&mut host, json!({"op": "diagnose", "id": broken_id}));
    assert!(faults["items"].as_array().unwrap().iter().any(|item| item["code"] == "unclosed_string"));

    call(&mut host, json!({"op": "enable-script", "id": python}));
    let first = call(&mut host, json!({"op": "run", "id": python}));
    assert_eq!(first["code"], "ok", "{first}");
    assert!(text_of(&first).contains('6'), "{}", text_of(&first));
    call(
        &mut host,
        json!({"op": "hot-reload", "id": python, "data": encode(b"print(2)\n")}),
    );
    let reloaded = call(&mut host, json!({"op": "run", "id": python}));
    assert!(reloaded["reloaded"].as_bool().unwrap());
    assert!(text_of(&reloaded).contains('2'), "{reloaded}");
    let history = call(&mut host, json!({"op": "history", "id": python}));
    assert!(history["versions"].as_array().unwrap().len() >= 2);

    let denied = call(&mut host, json!({"op": "run", "id": python, "emit": "script.finished"}));
    assert_eq!(denied["code"], "permission_denied");
    assert!(!state.join("emit.txt").exists());
    call(
        &mut host,
        json!({"op": "declare", "id": python, "permissions": ["event.publish", "storage.private", "event.subscribe"]}),
    );
    let pending = call(&mut host, json!({"op": "grant", "id": python, "permission": "event.publish"}));
    assert!(pending["grants"].as_array().unwrap().iter().any(|item| item == "event.publish"));
    let emitted = call(&mut host, json!({"op": "run", "id": python, "emit": "script.finished"}));
    assert_eq!(emitted["code"], "ok", "{emitted}");
    assert_eq!(
        host.read_private("strata.script", "emit.txt").unwrap(),
        "script.finished"
    );
    call(&mut host, json!({"op": "revoke", "id": python, "permission": "event.publish"}));
    let revoked = call(&mut host, json!({"op": "run", "id": python, "emit": "script.finished"}));
    assert_eq!(revoked["code"], "permission_denied");

    // Linux 子进程有 Landlock。Windows 的 AppContainer 仍是 OI-045，这里不把它记成通过。
    #[cfg(unix)]
    {
        call(&mut host, json!({"op": "grant", "id": python, "permission": "storage.private"}));
        call(
            &mut host,
            json!({"op": "edit", "id": python, "data": encode(b"try:\n    open('../presets/evil','w').write('x')\n    print('wrote')\nexcept OSError:\n    print('kept')\n")}),
        );
        let kept = call(&mut host, json!({"op": "run", "id": python}));
        assert!(text_of(&kept).contains("kept"), "{kept}");
        assert!(fs::read_dir(state.join("presets")).unwrap().next().is_none());
        assert_eq!(call(&mut host, json!({"op": "empty"}))["presets"], 0);
        call(
            &mut host,
            json!({"op": "edit", "id": python, "data": encode(b"try:\n    print(open('/etc/passwd').read(16))\nexcept OSError:\n    print('denied')\n")}),
        );
        let outside = call(&mut host, json!({"op": "run", "id": python}));
        let outside_text = text_of(&outside);
        assert!(outside_text.contains("denied"), "{outside}");
        assert!(!outside_text.contains("root:"), "{outside}");
    }

    call(&mut host, json!({"op": "modules", "id": python, "modules": ["winstrata_absent_mod"]}));
    call(&mut host, json!({"op": "edit", "id": python, "data": encode(b"print('ran')\n")}));
    let missing = call(&mut host, json!({"op": "run", "id": python}));
    assert_eq!(missing["code"], "dependency_missing", "{missing}");
    call(&mut host, json!({"op": "modules", "id": python, "modules": ["json"]}));
    call(&mut host, json!({"op": "edit", "id": python, "data": encode(b"import json\nprint(6)\n")}));
    let present = call(&mut host, json!({"op": "run", "id": python}));
    assert_eq!(present["code"], "ok", "{present}");
    assert!(text_of(&present).contains('6'));

    call(
        &mut host,
        json!({"op": "edit", "id": python, "data": encode(b"while True:\n    pass\n")}),
    );
    let timed = call(&mut host, json!({"op": "run", "id": python}));
    assert_eq!(timed["code"], "timeout", "{timed}");
    call(&mut host, json!({"op": "modules", "id": python, "modules": []}));

    call(
        &mut host,
        json!({"op": "edit", "id": python, "data": encode(b"value = 1\nvalue = 2\nprint(value)\n")}),
    );
    let paused = call(&mut host, json!({"op": "debug-start", "id": python, "line": 2}));
    assert_eq!(paused["state"], "paused", "{paused}");
    assert_eq!(paused["line"], 2);
    let stepped = call(&mut host, json!({"op": "debug-step"}));
    assert_eq!(stepped["state"], "paused", "{stepped}");
    assert_eq!(stepped["line"], 3);
    let finished = call(&mut host, json!({"op": "debug-continue"}));
    assert_eq!(finished["state"], "finished", "{finished}");
    assert!(text_of(&finished).contains('2'), "{finished}");

    let javascript = call(
        &mut host,
        json!({"op": "create", "language": "javascript", "name": "adder", "data": encode(b"console.log(6)\n")}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(&mut host, json!({"op": "enable-script", "id": javascript}));
    let js = call(&mut host, json!({"op": "run", "id": javascript}));
    assert_eq!(js["code"], "ok", "{js}");
    assert!(text_of(&js).contains('6'), "{js}");
    call(
        &mut host,
        json!({"op": "edit", "id": javascript, "data": encode(b"let value = 1\nvalue = 2\nconsole.log(value)\n")}),
    );
    let js_paused = call(&mut host, json!({"op": "debug-start", "id": javascript, "line": 2}));
    assert_eq!(js_paused["line"], 2, "{js_paused}");
    let js_done = call(&mut host, json!({"op": "debug-continue"}));
    assert_eq!(js_done["state"], "finished", "{js_done}");
    assert!(text_of(&js_done).contains('2'), "{js_done}");

    let typescript = call(
        &mut host,
        json!({"op": "create", "language": "typescript", "name": "typed", "data": encode("const value: number = 4\nconsole.log(value + 2)\n".as_bytes())}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(&mut host, json!({"op": "enable-script", "id": typescript}));
    let ts = call(&mut host, json!({"op": "run", "id": typescript}));
    assert_eq!(ts["code"], "ok", "{ts}");
    assert!(text_of(&ts).contains('6'), "{ts}");
    let completion = call(&mut host, json!({"op": "complete", "id": typescript, "prefix": "num"}));
    assert!(completion["items"].as_array().unwrap().iter().any(|item| item == "number"));

    call(&mut host, json!({"op": "disable-script", "id": python}));
    let watcher = call(
        &mut host,
        json!({"op": "create", "language": "python", "name": "watcher", "data": encode(b"print(9)\n")}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(
        &mut host,
        json!({"op": "declare", "id": watcher, "permissions": ["event.subscribe"]}),
    );
    call(&mut host, json!({"op": "grant", "id": watcher, "permission": "event.subscribe"}));
    call(&mut host, json!({"op": "trigger", "id": watcher, "kind": "event", "event": "script.tick"}));
    call(&mut host, json!({"op": "enable-script", "id": watcher}));
    let skipped = call(&mut host, json!({"op": "ingest", "event": "script.other"}));
    assert_eq!(skipped["ran"], 0);
    let ingested = call(&mut host, json!({"op": "ingest", "event": "script.tick"}));
    assert_eq!(ingested["ran"], 1, "{ingested}");
    call(&mut host, json!({"op": "disable-script", "id": watcher}));

    let conditional = call(
        &mut host,
        json!({"op": "create", "language": "python", "name": "threshold", "data": encode(b"print(8)\n")}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(
        &mut host,
        json!({"op": "trigger", "id": conditional, "kind": "condition", "field": "count", "cmp": "gte", "value": 2}),
    );
    call(&mut host, json!({"op": "enable-script", "id": conditional}));
    assert_eq!(call(&mut host, json!({"op": "ingest", "event": "script.tick"}))["ran"], 0);
    let fired = call(&mut host, json!({"op": "ingest", "event": "script.tick"}));
    assert_eq!(fired["ran"], 1, "{fired}");
    assert_eq!(call(&mut host, json!({"op": "ingest", "event": "script.tick"}))["ran"], 0);
    call(&mut host, json!({"op": "disable-script", "id": conditional}));

    let scheduled = call(
        &mut host,
        json!({"op": "create", "language": "python", "name": "ticker", "data": encode(b"print(7)\n")}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(&mut host, json!({"op": "trigger", "id": scheduled, "kind": "schedule", "every_ms": 200}));
    call(&mut host, json!({"op": "enable-script", "id": scheduled}));
    let started = Instant::now();
    let mut saw = false;
    while started.elapsed() < Duration::from_secs(3) {
        let rows = call(&mut host, json!({"op": "logs", "id": scheduled}));
        if rows["rows"].as_array().unwrap().iter().any(|row| row["trigger"] == "schedule" && row["status"] == "ok") {
            saw = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(saw, "scheduled script did not run");
    call(&mut host, json!({"op": "disable-script", "id": scheduled}));
    thread::sleep(Duration::from_millis(400));
    let settled = call(&mut host, json!({"op": "logs", "id": scheduled}))["rows"].as_array().unwrap().len();
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        call(&mut host, json!({"op": "logs", "id": scheduled}))["rows"].as_array().unwrap().len(),
        settled
    );

    host.pause("strata.script").unwrap();
    assert_eq!(host.report("strata.script").unwrap().phase, "paused");
    let held = call(&mut host, json!({"op": "run", "id": javascript}));
    assert_eq!(held["code"], "paused", "{held}");
    host.resume("strata.script").unwrap();

    let exported = call(&mut host, json!({"op": "export", "id": javascript}));
    let imported = call(&mut host, json!({"op": "import", "data": exported["data"].as_str().unwrap()}));
    assert_eq!(imported["enabled"], false);
    assert_eq!(imported["presets"], 0);
    assert_ne!(imported["id"].as_str().unwrap(), javascript);
    assert_eq!(call(&mut host, json!({"op": "empty"}))["presets"], 0);

    let logs = call(&mut host, json!({"op": "logs", "id": python}));
    assert!(logs["rows"].as_array().unwrap().iter().any(|row| row["status"] == "timeout" || row["status"] == "ok"));

    let disabled = host.disable("strata.script").unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.phase, "disabled");
    assert_eq!(fs::read(state.join("released")).unwrap(), b"joined");
    assert_eq!(call_absent(&mut host), "not_running");
    assert!(fs::read_dir(state.join("presets")).unwrap().next().is_none());
}

fn call_absent(host: &mut PluginHost) -> String {
    host.call("strata.script", "script.workbench", r#"{"op":"empty"}"#)
        .unwrap_err()
        .code()
        .to_owned()
}

#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use winstrata_core::{Core, EventBus, LanguageRequirement, Limits};

const PYTHON_PROBE: &str = r#"
def invoke(capability, payload):
    operation, _, payload = payload.partition(" ")
    if operation == "module":
        try:
            module = __import__(payload)
            return module.VALUE
        except Exception:
            return "missing"
    if operation == "read":
        try:
            with open(payload, "rb") as handle:
                handle.read(4)
            return "opened"
        except OSError as exc:
            return f"denied:{exc.winerror or exc.errno}"
    if operation == "net":
        import socket
        try:
            sock = socket.socket()
            sock.settimeout(1)
            sock.connect(("127.0.0.1", 9))
            return "opened"
        except OSError as exc:
            return f"denied:{exc.winerror or exc.errno}"
    if operation == "registry":
        import winreg
        try:
            winreg.OpenKey(winreg.HKEY_CURRENT_USER, payload)
            return "opened"
        except OSError as exc:
            return f"denied:{exc.winerror}"
    if operation == "child":
        import subprocess
        try:
            subprocess.run(["cmd.exe", "/c", "exit", "0"], check=False)
            return "spawned"
        except OSError as exc:
            return f"denied:{exc.winerror or exc.errno}"
    return payload
"#;

fn scratch() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "winstrata-lang-win-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn requirement() -> LanguageRequirement {
    let root = workspace();
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("package.json")).unwrap()).unwrap();
    let declared: serde_json::Value =
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

fn on_path(names: &[&str]) -> PathBuf {
    if let Some(explicit) = std::env::var_os("WINSTRATA_PYTHON") {
        if names[0].contains("python") {
            return PathBuf::from(explicit);
        }
    }
    if let Some(explicit) = std::env::var_os("WINSTRATA_NODE") {
        if names[0].contains("node") {
            return PathBuf::from(explicit);
        }
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for name in names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    panic!("project interpreter was not found: {names:?}");
}

#[test]
fn windows_language_plugins_enforce_the_container_boundary() {
    let root = scratch();
    let locator = root.join("locator");
    let data = root.join("data");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&data).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&data).unwrap();
    let plugins = data.join(core.current_user()).join("plugins");
    let mut host = core
        .open_plugins(Path::new(env!("CARGO_BIN_EXE_winstrata-runtime")))
        .unwrap();
    host.attach_bus(EventBus::new(Limits::standard()).unwrap());
    host.bind_languages(
        &on_path(&["python.exe", "python3.exe"]),
        &on_path(&["node.exe"]),
        &workspace().join("sdk"),
        &requirement(),
    )
    .unwrap_or_else(|error| panic!("bind toolchain: {error}"));
    let package = root.join("python");
    fs::create_dir(&package).unwrap();
    fs::write(
        package.join("manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": {"major": 1, "minor": 0},
            "id": "fixture.pywin",
            "name": "fixture.pywin",
            "plugin_version": {"major": 1, "minor": 0, "patch": 0},
            "kind": "third_party",
            "runtime": "python",
            "capabilities": ["probe.pywin"],
            "permissions": ["storage.private"],
            "dependencies": [],
            "events": [],
            "rescue": false
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(package.join("plugin.py"), PYTHON_PROBE).unwrap();
    fs::write(
        package.join("modules.json"),
        br#"{"modules":["sharedmod"]}"#,
    )
    .unwrap();
    host.install(&package).unwrap();
    fs::create_dir_all(plugins.join("catalog/python")).unwrap();
    fs::write(
        plugins.join("catalog/python/sharedmod.py"),
        "VALUE = 'shared'\n",
    )
    .unwrap();
    fs::create_dir_all(plugins.join("state/fixture.pywin/py")).unwrap();
    fs::write(
        plugins.join("state/fixture.pywin/py/localmod.py"),
        "VALUE = 'local'\n",
    )
    .unwrap();
    let secret = root.join("secret.txt");
    fs::write(&secret, "secret-value").unwrap();
    let key = r"Software\WinStrata015Probe";
    let _ = Command::new("reg.exe")
        .args(["add", &format!(r"HKCU\{key}"), "/f"])
        .status();
    if let Err(error) = host.enable("fixture.pywin") {
        let log =
            fs::read_to_string(plugins.join("state/fixture.pywin/stderr.log")).unwrap_or_default();
        panic!("{error}\n{log}");
    }
    assert_eq!(
        host.call("fixture.pywin", "probe.pywin", "module localmod")
            .unwrap(),
        "local"
    );
    assert_eq!(
        host.call("fixture.pywin", "probe.pywin", "module sharedmod")
            .unwrap(),
        "shared"
    );
    assert!(
        host.call(
            "fixture.pywin",
            "probe.pywin",
            &format!("read {}", secret.display())
        )
        .unwrap()
        .starts_with("denied:")
    );
    assert!(
        host.call("fixture.pywin", "probe.pywin", "net x")
            .unwrap()
            .starts_with("denied:")
    );
    assert!(
        host.call("fixture.pywin", "probe.pywin", &format!("registry {key}"))
            .unwrap()
            .starts_with("denied:")
    );
    assert!(
        host.call("fixture.pywin", "probe.pywin", "child x")
            .unwrap()
            .starts_with("denied:")
    );
    fs::write(plugins.join("state/fixture.pywin/owned.txt"), "private").unwrap();
    assert_eq!(
        host.read_private("fixture.pywin", "owned.txt").unwrap(),
        "private"
    );
    let pid = host.report("fixture.pywin").unwrap().pid.unwrap();
    host.disable("fixture.pywin").unwrap();
    assert!(host.report("fixture.pywin").unwrap().pid.is_none());
    assert!(!plugins.join("state/fixture.pywin/env").exists());
    let listed = Command::new("tasklist.exe")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&listed.stdout);
    assert!(!text.contains(&pid.to_string()));
    let _ = Command::new("reg.exe")
        .args(["delete", &format!(r"HKCU\{key}"), "/f"])
        .status();
    let _ = fs::remove_dir_all(root);
}

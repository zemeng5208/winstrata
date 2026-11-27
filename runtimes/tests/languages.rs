#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use winstrata_core::{Core, EventBus, LanguageRequirement, Limits, PluginHost};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "winstrata-lang-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn runtime() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_winstrata-runtime"))
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn requirement() -> LanguageRequirement {
    let root = workspace();
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("package.json")).unwrap()).unwrap();
    let node = package["engines"]["node"].as_str().unwrap();
    let declared: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("toolchains/runtimes.json")).unwrap())
            .unwrap();
    let python = declared["python_minimum"].as_str().unwrap();
    LanguageRequirement {
        python_minimum: version(python),
        node_exact: version(node),
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

fn program(env: &str, fallback: &str) -> PathBuf {
    let requested = std::env::var_os(env)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback));
    if requested.is_absolute() {
        return requested;
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return requested;
    };
    for directory in std::env::split_paths(&paths) {
        let candidate = directory.join(&requested);
        if candidate.is_file() {
            return candidate;
        }
    }
    requested
}

fn session(scratch: &Scratch) -> (Core, PathBuf) {
    let locator = scratch.dir("locator");
    let root = scratch.dir("data");
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&root).unwrap();
    let plugins = root.join(core.current_user()).join("plugins");
    (core, plugins)
}

fn host(core: &mut Core) -> PluginHost {
    let mut host = core.open_plugins(runtime()).unwrap();
    host.attach_bus(EventBus::new(Limits::standard()).unwrap());
    host.bind_languages(
        &program("WINSTRATA_PYTHON", "python3"),
        &program("WINSTRATA_NODE", "node"),
        &workspace().join("sdk"),
        &requirement(),
    )
    .unwrap_or_else(|error| panic!("bind toolchain: {error}"));
    host
}

fn manifest(
    id: &str,
    runtime_name: &str,
    capabilities: &[&str],
    permissions: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "version": {"major": 1, "minor": 0},
        "id": id,
        "name": id,
        "plugin_version": {"major": 1, "minor": 0, "patch": 0},
        "kind": "third_party",
        "runtime": runtime_name,
        "capabilities": capabilities,
        "permissions": permissions,
        "dependencies": [],
        "events": ["fixture.tick"],
        "rescue": false
    })
}

fn write_plugin(
    dir: &Path,
    body: &serde_json::Value,
    entry: &str,
    source: &str,
    modules: Option<&[&str]>,
) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("manifest.json"), serde_json::to_vec(body).unwrap()).unwrap();
    fs::write(dir.join(entry), source).unwrap();
    if let Some(modules) = modules {
        fs::write(
            dir.join("modules.json"),
            serde_json::to_vec(&serde_json::json!({"modules": modules})).unwrap(),
        )
        .unwrap();
    }
}

fn enable(host: &mut PluginHost, plugins: &Path, id: &str) {
    if let Err(error) = host.enable(id) {
        let log =
            fs::read_to_string(plugins.join(format!("state/{id}/stderr.log"))).unwrap_or_default();
        panic!("{error}\n{log}");
    }
}

const PYTHON_PROBE: &str = r#"
HELD = None

def invoke(capability, payload):
    global HELD
    operation, _, payload = payload.partition(" ")
    if operation == "module":
        try:
            module = __import__(payload)
            return module.VALUE
        except Exception:
            return "missing"
    if operation == "pip":
        try:
            __import__("pip")
            return "present"
        except Exception:
            return "missing"
    if operation == "read":
        try:
            with open(payload, "rb") as handle:
                handle.read(4)
            return "opened"
        except OSError as exc:
            return f"denied:{exc.errno}"
    if operation == "net":
        import socket
        try:
            sock = socket.socket()
            sock.settimeout(1)
            sock.connect(("127.0.0.1", 9))
            return "opened"
        except OSError as exc:
            return f"denied:{exc.errno}"
    if operation == "unix":
        import socket
        try:
            sock = socket.socket(socket.AF_UNIX)
            sock.connect(payload)
            return "opened"
        except OSError as exc:
            return f"denied:{exc.errno}"
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
            subprocess.run(["/bin/true"], check=False)
            return "spawned"
        except OSError as exc:
            return f"denied:{exc.errno}"
    if operation == "fd":
        import os
        try:
            os.read(int(payload), 1)
            return "opened"
        except OSError as exc:
            return f"denied:{exc.errno}"
    if operation == "hold":
        HELD = open(payload, "rb")
        return "held"
    if operation == "held":
        HELD.seek(0)
        return HELD.read(64).decode()
    return payload
"#;

const NODE_PROBE: &str = r#"
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { connect } from "node:net";

let held;

export async function invoke(capability, payload) {
  if (capability === "probe.module") {
    try {
      const loaded = await globalThis.winstrata.loadModule(payload);
      return loaded.VALUE;
    } catch {
      return "missing";
    }
  }
  if (capability === "probe.read") {
    try {
      readFileSync(payload);
      return "opened";
    } catch (error) {
      return `denied:${error.errno ?? error.code}`;
    }
  }
  if (capability === "probe.net") {
    try {
      await new Promise((resolve, reject) => {
        const socket = connect(9, "127.0.0.1", () => resolve());
        socket.on("error", reject);
      });
      return "opened";
    } catch (error) {
      return `denied:${error.errno ?? error.code}`;
    }
  }
  if (capability === "probe.child") {
    const child = spawnSync("/bin/true");
    if (child.error) return `denied:${child.error.errno ?? child.error.code}`;
    return "spawned";
  }
  if (capability === "probe.hold") {
    held = readFileSync(payload);
    return "held";
  }
  if (capability === "probe.held") return held.toString("utf8");
  return `js:${payload}`;
}
"#;

const TYPESCRIPT_PROBE: &str = r#"
export function invoke(capability: string, payload: string): string {
  return `ts:${capability}:${payload}`;
}
"#;

#[test]
fn project_toolchain_rejects_a_different_node_version() {
    let scratch = Scratch::new();
    let script = scratch.0.join("fake-node");
    fs::write(&script, "#!/bin/sh\nprintf '%s\\n' v1.2.3\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
    }
    let (mut core, _) = session(&scratch);
    let mut host = core.open_plugins(runtime()).unwrap();
    let error = host
        .bind_languages(
            &program("WINSTRATA_PYTHON", "python3"),
            &script,
            &workspace().join("sdk"),
            &requirement(),
        )
        .unwrap_err();
    assert_eq!(error.code(), "toolchain_mismatch");
}

#[test]
fn language_plugins_run_revoke_release_and_stay_inside_the_project_boundary() {
    let scratch = Scratch::new();
    let (mut core, plugins) = session(&scratch);
    let mut host = host(&mut core);
    let first = scratch.dir("python-a");
    write_plugin(
        &first,
        &manifest(
            "fixture.pya",
            "python",
            &["probe.pya"],
            &["event.publish", "event.subscribe", "storage.private"],
        ),
        "plugin.py",
        PYTHON_PROBE,
        Some(&["sharedmod"]),
    );
    let second = scratch.dir("python-b");
    write_plugin(
        &second,
        &manifest(
            "fixture.pyb",
            "python",
            &["probe.pyb"],
            &["storage.private"],
        ),
        "plugin.py",
        PYTHON_PROBE,
        None,
    );
    let ruby = scratch.dir("ruby");
    write_plugin(
        &ruby,
        &manifest("fixture.ruby", "ruby", &[], &[]),
        "plugin.py",
        "def invoke(capability, payload):\n    return payload\n",
        None,
    );
    assert_eq!(
        host.install(&ruby).unwrap_err().code(),
        "unsupported_runtime"
    );
    host.install(&first).unwrap();
    host.install(&second).unwrap();
    fs::create_dir_all(plugins.join("catalog/python")).unwrap();
    fs::write(
        plugins.join("catalog/python/sharedmod.py"),
        "VALUE = 'shared'\n",
    )
    .unwrap();
    fs::create_dir_all(plugins.join("state/fixture.pya/py")).unwrap();
    fs::create_dir_all(plugins.join("state/fixture.pyb/py")).unwrap();
    fs::write(
        plugins.join("state/fixture.pya/py/localmod.py"),
        "VALUE = 'alpha'\n",
    )
    .unwrap();
    fs::write(
        plugins.join("state/fixture.pyb/py/localmod.py"),
        "VALUE = 'beta'\n",
    )
    .unwrap();
    fs::write(plugins.join("state/fixture.pya/note.txt"), "kept-open").unwrap();
    let secret = scratch.0.join("secret.txt");
    fs::write(&secret, "secret-value").unwrap();
    let inherited = unsafe { libc::open(c_path(&secret).as_ptr(), libc::O_RDONLY) };
    assert!(inherited >= 0);
    assert!(unsafe { libc::dup2(inherited, 200) } == 200);
    unsafe { libc::close(inherited) };

    enable(&mut host, &plugins, "fixture.pya");
    enable(&mut host, &plugins, "fixture.pyb");
    unsafe { libc::close(200) };
    assert_eq!(
        host.call("fixture.pya", "probe.pya", "module localmod")
            .unwrap(),
        "alpha"
    );
    assert_eq!(
        host.call("fixture.pyb", "probe.pyb", "module localmod")
            .unwrap(),
        "beta"
    );
    assert_eq!(
        host.call("fixture.pya", "probe.pya", "module sharedmod")
            .unwrap(),
        "shared"
    );
    assert_eq!(
        host.call("fixture.pyb", "probe.pyb", "module sharedmod")
            .unwrap(),
        "missing"
    );
    assert_eq!(
        host.call("fixture.pya", "probe.pya", "pip x").unwrap(),
        "missing"
    );
    let other = plugins.join("state/fixture.pyb/py/localmod.py");
    assert!(
        host.call(
            "fixture.pya",
            "probe.pya",
            &format!("read {}", other.display())
        )
        .unwrap()
        .starts_with("denied:")
    );
    assert!(
        host.call(
            "fixture.pya",
            "probe.pya",
            &format!("read {}", secret.display())
        )
        .unwrap()
        .starts_with("denied:")
    );
    assert!(
        host.call("fixture.pya", "probe.pya", "net x")
            .unwrap()
            .starts_with("denied:")
    );
    assert!(
        host.call(
            "fixture.pya",
            "probe.pya",
            &format!("unix {}", scratch.0.join("peer.sock").display())
        )
        .unwrap()
        .starts_with("denied:")
    );
    assert!(
        host.call("fixture.pya", "probe.pya", "child x")
            .unwrap()
            .starts_with("denied:")
    );
    assert_eq!(
        host.call("fixture.pya", "probe.pya", "fd 200").unwrap(),
        "denied:9"
    );
    assert_eq!(
        host.call(
            "fixture.pya",
            "probe.pya",
            &format!(
                "hold {}",
                plugins.join("state/fixture.pya/note.txt").display()
            )
        )
        .unwrap(),
        "held"
    );
    assert_eq!(
        host.call("fixture.pya", "probe.pya", "held x").unwrap(),
        "kept-open"
    );
    fs::write(plugins.join("state/fixture.pya/owned.txt"), "private").unwrap();
    assert_eq!(
        host.read_private("fixture.pya", "owned.txt").unwrap(),
        "private"
    );
    host.emit("fixture.pya", "fixture.tick").unwrap();
    assert_eq!(host.poll("fixture.pya").unwrap().len(), 1);
    host.revoke("fixture.pya", "event.subscribe").unwrap();
    assert_eq!(
        host.poll("fixture.pya").unwrap_err().code(),
        "permission_denied"
    );

    let pid = host.report("fixture.pya").unwrap().pid.unwrap();
    host.disable("fixture.pya").unwrap();
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    assert!(!plugins.join("state/fixture.pya/env").exists());
    assert!(plugins.join("history/fixture.pya.jsonl").is_file());
    enable(&mut host, &plugins, "fixture.pya");
    assert_eq!(
        host.call("fixture.pya", "probe.pya", "module localmod")
            .unwrap(),
        "alpha"
    );
    let restarted = host.report("fixture.pya").unwrap().pid.unwrap();
    assert_ne!(restarted, pid);
    host.disable("fixture.pya").unwrap();
    host.disable("fixture.pyb").unwrap();

    let javascript = scratch.dir("javascript");
    write_plugin(
        &javascript,
        &manifest(
            "fixture.js",
            "javascript",
            &["probe.module", "probe.read", "probe.net", "probe.child"],
            &[],
        ),
        "plugin.mjs",
        NODE_PROBE,
        Some(&["sharedmod"]),
    );
    host.install(&javascript).unwrap();
    fs::create_dir_all(plugins.join("catalog/node")).unwrap();
    fs::write(
        plugins.join("catalog/node/sharedmod.mjs"),
        "export const VALUE = 'shared-js';\n",
    )
    .unwrap();
    fs::create_dir_all(plugins.join("state/fixture.js/node")).unwrap();
    fs::write(
        plugins.join("state/fixture.js/node/localmod.mjs"),
        "export const VALUE = 'alpha-js';\n",
    )
    .unwrap();
    enable(&mut host, &plugins, "fixture.js");
    assert_eq!(
        host.call("fixture.js", "probe.module", "localmod").unwrap(),
        "alpha-js"
    );
    assert_eq!(
        host.call("fixture.js", "probe.module", "sharedmod")
            .unwrap(),
        "shared-js"
    );
    assert!(
        host.call("fixture.js", "probe.read", secret.to_str().unwrap())
            .unwrap()
            .starts_with("denied:")
    );
    assert!(
        host.call("fixture.js", "probe.net", " ")
            .unwrap()
            .starts_with("denied:")
    );
    assert!(
        host.call("fixture.js", "probe.child", " ")
            .unwrap()
            .starts_with("denied:")
    );
    let js_pid = host.report("fixture.js").unwrap().pid.unwrap();
    host.disable("fixture.js").unwrap();
    assert!(!Path::new(&format!("/proc/{js_pid}")).exists());

    let typescript = scratch.dir("typescript");
    write_plugin(
        &typescript,
        &manifest("fixture.ts", "typescript", &["probe.echo"], &[]),
        "plugin.ts",
        TYPESCRIPT_PROBE,
        None,
    );
    host.install(&typescript).unwrap();
    enable(&mut host, &plugins, "fixture.ts");
    assert_eq!(
        host.call("fixture.ts", "probe.echo", "ok").unwrap(),
        "ts:probe.echo:ok"
    );
    let ts_pid = host.report("fixture.ts").unwrap().pid.unwrap();
    host.disable("fixture.ts").unwrap();
    assert!(!Path::new(&format!("/proc/{ts_pid}")).exists());
}

fn c_path(path: &Path) -> std::ffi::CString {
    use std::os::unix::ffi::OsStrExt;
    let mut bytes = path.as_os_str().as_bytes().to_vec();
    bytes.push(0);
    std::ffi::CString::from_vec_with_nul(bytes).unwrap()
}

#[test]
fn declared_node_binary_matches_package_json() {
    let requirement = requirement();
    let output = Command::new(program("WINSTRATA_NODE", "node"))
        .arg("--version")
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(version(text.trim()), requirement.node_exact);
}

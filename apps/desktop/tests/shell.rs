use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use winstrata_desktop::{Desktop, package_root, tray_probe};
use winstrata_service::{Endpoint, Service};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "winstrata-desktop-{}-{id}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        if let Err(error) = fs::create_dir(&path) {
            assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        }
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn start(
    scratch: &Scratch,
    service: Option<String>,
    runtime: Option<PathBuf>,
    bundle: &Path,
) -> winstrata_desktop::Running {
    Desktop {
        prefs: scratch.0.join("gui.json"),
        locator: Some(scratch.dir("locator")),
        service,
        runtime,
        ui_root: package_root(),
        bundle: bundle.to_path_buf(),
        tray: false,
    }
    .serve()
    .unwrap()
}

fn exchange(url: &str, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let without = url.trim_start_matches("http://");
    let (address, rest) = without.split_once('/').unwrap();
    let token = rest.trim_end_matches('/').trim_end_matches("?probe=1");
    let target = if path.is_empty() {
        format!("/{token}/")
    } else {
        format!("/{token}/{path}")
    };
    let payload = body.unwrap_or("");
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: {address}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status: u16 = response.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = response.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    (status, body)
}

fn state(url: &str) -> Value {
    let (status, body) = exchange(url, "GET", "api/state", None);
    assert_eq!(status, 200, "{body}");
    serde_json::from_str(&body).unwrap()
}

fn command(url: &str, body: &str) -> Value {
    let (status, response) = exchange(url, "POST", "api/command", Some(body));
    assert_eq!(status, 200, "{response}");
    serde_json::from_str(&response).unwrap()
}

#[test]
fn theme_language_and_close_preference_persist_without_a_default_theme() {
    let scratch = Scratch::new();
    let mut desktop = start(&scratch, None, None, Path::new("missing-bundle"));
    let url = desktop.url(false);
    let first = state(&url);
    assert!(first["theme"].is_null());
    assert_eq!(first["language"], "en");
    assert_eq!(first["close"], "ask");
    if cfg!(windows) {
        assert!(matches!(
            first["privilege"].as_str(),
            Some("windows_ordinary") | Some("windows_elevated") | Some("unknown")
        ));
    } else {
        assert_eq!(first["privilege"], "not_windows");
    }
    assert_eq!(first["service"]["state"], "unavailable");
    assert!(first["service"]["drivers"].is_null());
    assert_eq!(first["plugins"].as_array().unwrap().len(), 25);
    assert_eq!(first["pluginHost"], "runtime_missing");
    assert_eq!(first["plugins"][0]["availability"], "implementation_absent");
    let rejected = command(&url, r#"{"action":"set_theme","value":"fluent"}"#);
    assert_eq!(rejected["error"], "invalid_theme");
    assert!(rejected["theme"].is_null());
    let saved = command(&url, r#"{"action":"set_theme","value":"black-acrylic"}"#);
    assert_eq!(saved["theme"], "black-acrylic");
    assert!(saved["error"].is_null());
    command(&url, r#"{"action":"set_language","value":"zh-CN"}"#);
    command(&url, r#"{"action":"set_close","value":"exit"}"#);
    let unknown = command(&url, r#"{"action":"navigate","value":"dashboard-2"}"#);
    assert_eq!(unknown["error"], "unknown_page");
    desktop.shutdown();

    let mut again = start(&scratch, None, None, Path::new("missing-bundle"));
    let restored = state(&again.url(false));
    assert_eq!(restored["theme"], "black-acrylic");
    assert_eq!(restored["language"], "zh-CN");
    assert_eq!(restored["close"], "exit");
    again.shutdown();
}

#[test]
fn gui_status_reads_the_live_service_and_does_not_invent_drivers() {
    let scratch = Scratch::new();
    let service = Service::listen_catalog(Endpoint::unique().unwrap()).unwrap();
    let mut desktop = start(
        &scratch,
        Some(service.address().to_owned()),
        None,
        Path::new("missing"),
    );
    let view = state(&desktop.url(false));
    assert_eq!(view["service"]["state"], "ready");
    let drivers = view["service"]["drivers"].as_array().unwrap();
    assert_eq!(drivers.len(), 4);
    assert!(
        drivers
            .iter()
            .all(|driver| driver["phase"] == "idle" && driver["subscriptions"] == 0)
    );
    desktop.shutdown();
}

#[test]
fn storage_selection_opens_the_real_plugin_host() {
    let scratch = Scratch::new();
    let runtime = scratch.0.join("winstrata-runtime");
    fs::write(&runtime, b"").unwrap();
    let data = scratch.dir("data");
    let mut desktop = start(&scratch, None, Some(runtime), Path::new("missing"));
    let url = desktop.url(false);
    assert_eq!(state(&url)["storage"]["state"], "needs_directory");
    let rejected = command(&url, r#"{"action":"set_storage","value":"relative"}"#);
    assert!(rejected["error"].is_string());
    assert_eq!(rejected["storage"]["state"], "needs_directory");
    let body = format!(
        r#"{{"action":"set_storage","value":"{}"}}"#,
        data.display().to_string().replace('\\', "\\\\")
    );
    let selected = command(&url, &body);
    assert!(selected["error"].is_null(), "{selected}");
    assert_eq!(selected["storage"]["state"], "ready");
    assert_eq!(selected["storage"]["run"], "stopped");
    assert_eq!(selected["pluginHost"], "host");
    assert_eq!(selected["plugins"].as_array().unwrap().len(), 25);
    assert!(
        selected["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .any(|plugin| {
                plugin["id"] == "strata.rescue" && plugin["availability"] == "implementation_absent"
            })
    );
    let page = command(
        &url,
        r#"{"action":"navigate","value":"plugin:strata.rescue"}"#,
    );
    assert_eq!(page["page"], "plugin:strata.rescue");
    desktop.shutdown();
}

#[test]
fn loopback_requests_without_the_session_token_are_rejected() {
    let scratch = Scratch::new();
    let desktop = start(&scratch, None, None, Path::new("missing"));
    let url = desktop.url(false);
    let (status, _) = exchange(&url, "GET", "../api/state", None);
    assert_ne!(status, 200);
    let without = url.trim_start_matches("http://");
    let address = without.split_once('/').unwrap().0;
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .write_all(b"GET /api/state HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 404"), "{response}");
}

#[test]
fn upstream_bytes_and_locale_catalog_match_their_pins() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("node")
        .arg("--test")
        .arg("packages/ui/model.test.mjs")
        .arg("packages/ui/upstream.test.mjs")
        .current_dir(&root)
        .status()
        .expect("node");
    assert!(status.success());
}

fn build_bundle() -> PathBuf {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/plan016-ui");
    if !output.join("NOTICES.txt").is_file() {
        let status = Command::new("node")
            .arg("packages/ui/build.mjs")
            .arg(&output)
            .current_dir(&root)
            .status()
            .expect("node");
        assert!(status.success(), "UI bundle build failed");
    }
    output
}

#[test]
fn upstream_components_bundle_from_the_pinned_packages() {
    let output = build_bundle();
    let notices = fs::read_to_string(output.join("NOTICES.txt")).unwrap();
    assert!(notices.contains("MIT") && notices.contains("Amicro") && notices.contains("Glasswork"));
    assert!(notices.contains("shadcn-admin"));
    assert!(!notices.contains("@fluentui/react-components"));
    assert!(!notices.contains("astryx"));
    let amicro = fs::read_to_string(output.join("amicro.js")).unwrap();
    assert!(amicro.contains("mountAmicro"));
    let started = Instant::now();
    let scratch = Scratch::new();
    let desktop = start(&scratch, None, None, &output);
    let (status, page) = exchange(&desktop.url(false), "GET", "", None);
    assert_eq!(status, 200);
    assert!(page.contains("shell.js"));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn tray_matches_the_operating_system() {
    let result = tray_probe();
    if cfg!(windows) {
        result.expect("ordinary Windows tray");
    } else {
        let error = result.expect_err("Linux has no WinStrata tray");
        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    }
}

#[cfg(windows)]
#[test]
fn windows_webview_exercises_the_three_themes() {
    let output = build_bundle();
    let scratch = Scratch::new();
    let desktop = start(&scratch, None, None, &output);
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("windows/host.ps1");
    let report = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Sta",
            "-File",
            script.to_str().unwrap(),
            "-Url",
            &desktop.url(true),
            "-Probe",
        ])
        .output()
        .unwrap();
    if report.status.code() == Some(2) {
        eprintln!(
            "WebView2 environment unverified\n{}",
            String::from_utf8_lossy(&report.stderr)
        );
        return;
    }
    assert!(
        report.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&report.stdout),
        String::from_utf8_lossy(&report.stderr)
    );
    let parsed: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(parsed["ok"], true);
    for name in [
        "themes",
        "themePersisted",
        "language",
        "shadcnLayout",
        "shadcnResize",
        "glassRefraction",
        "motionSuspended",
        "licenses",
    ] {
        assert_eq!(parsed["checks"][name], true, "{name}");
    }
    assert_eq!(parsed["frameSamples"], 30);
}

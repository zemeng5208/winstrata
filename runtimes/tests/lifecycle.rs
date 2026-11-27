use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use winstrata_core::{Core, EventBus, Limits, PluginHost};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, Id, Record, SignedNanos, Source, Version,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "winstrata-plugin-{}-{}-{}",
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

fn manifest(
    id: &str,
    version: (u16, u16, u16),
    capabilities: &[&str],
    permissions: &[&str],
    dependencies: &[(&str, &str, u16, u16, u16)],
    events: &[&str],
    rescue: bool,
) -> serde_json::Value {
    serde_json::json!({
        "version": {"major": 1, "minor": 0},
        "id": id,
        "name": id,
        "plugin_version": {"major": version.0, "minor": version.1, "patch": version.2},
        "kind": "third_party",
        "runtime": "native",
        "capabilities": capabilities,
        "permissions": permissions,
        "dependencies": dependencies.iter().map(|(dep, range, major, minor, patch)| {
            serde_json::json!({
                "id": dep,
                "range": range,
                "version": {"major": major, "minor": minor, "patch": patch}
            })
        }).collect::<Vec<_>>(),
        "events": events,
        "rescue": rescue
    })
}

fn write_package(dir: &Path, body: &serde_json::Value, decline: bool) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("manifest.json"), serde_json::to_vec(body).unwrap()).unwrap();
    if decline {
        fs::write(
            dir.join("startup.json"),
            br#"{"version":{"major":1,"minor":0},"ready":false,"reason":"unavailable"}"#,
        )
        .unwrap();
    }
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
    let bus = EventBus::new(Limits::standard()).unwrap();
    let mut host = core.open_plugins(runtime()).unwrap();
    host.attach_bus(bus.clone());
    host
}

#[test]
fn storage_must_be_selected_before_plugins_open() {
    let scratch = Scratch::new();
    let mut core = Core::open_at(&scratch.dir("locator"), None).unwrap();
    match core.open_plugins(runtime()) {
        Ok(_) => panic!("plugins opened before storage was selected"),
        Err(error) => assert_eq!(error.code(), "storage_unavailable"),
    }
}

fn collector_present(stem: &str) -> bool {
    let name = if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    };
    runtime()
        .parent()
        .is_some_and(|dir| dir.join(name).is_file())
}

#[test]
fn official_plugins_stay_absent_until_their_own_plans() {
    let scratch = Scratch::new();
    let (mut core, _) = session(&scratch);
    let mut host = host(&mut core);
    let reports = host.list();
    let builtins: Vec<_> = reports
        .iter()
        .filter(|report| report.kind == "builtin")
        .collect();
    assert_eq!(builtins.len(), 25);
    let process_seeded = collector_present("winstrata-strata-process");
    let services_seeded = collector_present("winstrata-strata-services");
    let startup_seeded = collector_present("winstrata-strata-startup");
    let health_seeded = collector_present("winstrata-strata-health");
    let registry = builtins
        .iter()
        .find(|report| report.id == "strata.registry")
        .expect("strata.registry");
    assert_eq!(registry.phase, "disabled");
    assert_eq!(registry.availability, "ready");
    assert!(!registry.enabled);
    let environment = builtins
        .iter()
        .find(|report| report.id == "strata.env")
        .expect("strata.env");
    assert_eq!(environment.phase, "disabled");
    assert_eq!(environment.availability, "ready");
    assert!(!environment.enabled);
    let trace = builtins
        .iter()
        .find(|report| report.id == "strata.trace")
        .expect("strata.trace");
    assert_eq!(trace.phase, "disabled");
    assert_eq!(trace.availability, "ready");
    assert!(!trace.enabled);
    assert!(builtins.iter().all(|report| {
        if report.id == "strata.registry"
            || report.id == "strata.env"
            || report.id == "strata.trace"
        {
            true
        } else if report.id == "strata.process" && process_seeded {
            !report.enabled && report.phase == "disabled" && report.availability == "ready"
        } else if report.id == "strata.services" && services_seeded {
            !report.enabled && report.phase == "disabled" && report.availability == "ready"
        } else if report.id == "strata.startup" && startup_seeded {
            !report.enabled && report.phase == "disabled" && report.availability == "ready"
        } else if report.id == "strata.health" && health_seeded {
            !report.enabled && report.phase == "disabled" && report.availability == "ready"
        } else {
            report.phase == "implementation_absent"
        }
    }));
    assert_eq!(
        host.uninstall("strata.registry").unwrap_err().code(),
        "builtin_retained"
    );
    assert_eq!(
        host.uninstall("strata.env").unwrap_err().code(),
        "builtin_retained"
    );
    assert_eq!(
        host.uninstall("strata.trace").unwrap_err().code(),
        "builtin_retained"
    );
    if process_seeded {
        assert_eq!(
            host.uninstall("strata.process").unwrap_err().code(),
            "builtin_retained"
        );
    }
    if services_seeded {
        assert_eq!(
            host.uninstall("strata.services").unwrap_err().code(),
            "builtin_retained"
        );
    }
    if startup_seeded {
        assert_eq!(
            host.uninstall("strata.startup").unwrap_err().code(),
            "builtin_retained"
        );
    }
    if health_seeded {
        assert_eq!(
            host.uninstall("strata.health").unwrap_err().code(),
            "builtin_retained"
        );
    }
    assert_eq!(
        builtins
            .iter()
            .filter(|report| !report.batch)
            .map(|report| report.id.as_str())
            .collect::<Vec<_>>(),
        vec!["strata.rescue"]
    );
    assert_eq!(
        host.uninstall("strata.rescue").unwrap_err().code(),
        "builtin_retained"
    );
    assert_eq!(
        host.enable("strata.pulse").unwrap_err().code(),
        "implementation_absent"
    );
    let reserved = scratch.dir("reserved");
    write_package(
        &reserved,
        &manifest("strata.pulse", (1, 0, 0), &[], &[], &[], &[], false),
        false,
    );
    assert_eq!(
        host.install(&reserved).unwrap_err().code(),
        "builtin_closed"
    );
}

#[test]
fn load_disable_update_isolate_and_recover_real_plugins() {
    let scratch = Scratch::new();
    let (mut core, plugins) = session(&scratch);
    let mut host = host(&mut core);
    let base_dir = scratch.dir("base");
    let user_dir = scratch.dir("user");
    let other_dir = scratch.dir("other");
    let client_dir = scratch.dir("client");
    write_package(
        &base_dir,
        &manifest(
            "fixture.base",
            (1, 0, 0),
            &["fixture.base"],
            &["storage.private"],
            &[],
            &[],
            false,
        ),
        false,
    );
    write_package(
        &user_dir,
        &manifest(
            "fixture.user",
            (1, 0, 0),
            &["fixture.echo"],
            &[
                "event.publish",
                "event.subscribe",
                "plugin.call",
                "storage.private",
            ],
            &[("fixture.base", "compatible", 1, 0, 0)],
            &["fixture.tick"],
            false,
        ),
        false,
    );
    write_package(
        &other_dir,
        &manifest(
            "fixture.other",
            (1, 0, 0),
            &["fixture.echo"],
            &["storage.private"],
            &[],
            &[],
            false,
        ),
        false,
    );
    write_package(
        &client_dir,
        &manifest(
            "fixture.client",
            (1, 0, 0),
            &[],
            &["plugin.call"],
            &[],
            &[],
            false,
        ),
        false,
    );

    let python = scratch.dir("python");
    write_package(
        &python,
        &serde_json::json!({
            "version": {"major": 1, "minor": 0},
            "id": "fixture.python",
            "name": "Fixture Python",
            "plugin_version": {"major": 1, "minor": 0, "patch": 0},
            "kind": "third_party",
            "runtime": "python",
            "capabilities": [],
            "permissions": [],
            "dependencies": [],
            "events": [],
            "rescue": false
        }),
        false,
    );
    assert_eq!(
        host.install(&python).unwrap_err().code(),
        "toolchain_missing"
    );
    let rescue = scratch.dir("rescue");
    let mut rescue_body = manifest("fixture.rescue", (1, 0, 0), &[], &[], &[], &[], true);
    rescue_body["rescue"] = serde_json::json!(true);
    write_package(&rescue, &rescue_body, false);
    assert_eq!(host.install(&rescue).unwrap_err().code(), "rescue_isolated");

    assert_eq!(host.install(&user_dir).unwrap().phase, "disabled");
    assert_eq!(
        host.load("fixture.user").unwrap_err().code(),
        "dependency_missing"
    );
    host.install(&base_dir).unwrap();
    assert_eq!(host.load("fixture.user").unwrap().phase, "loaded");
    assert!(host.report("fixture.user").unwrap().pid.is_none());
    assert_eq!(
        host.enable("fixture.user").unwrap_err().code(),
        "dependency_unhealthy"
    );
    host.enable("fixture.base").unwrap();
    let base = host.ping("fixture.base").unwrap();
    host.enable("fixture.user").unwrap();
    let user = host.ping("fixture.user").unwrap();
    assert_ne!(base.0, user.0);
    fs::write(plugins.join("state/fixture.base/note.txt"), "base-secret").unwrap();
    fs::write(plugins.join("state/fixture.user/note.txt"), "user-note").unwrap();
    assert_eq!(
        host.read_private("fixture.base", "note.txt").unwrap(),
        "base-secret"
    );
    assert_eq!(
        host.read_private("fixture.user", "note.txt").unwrap(),
        "user-note"
    );
    assert_eq!(
        host.read_private("fixture.user", "note.txt")
            .unwrap()
            .contains("base-secret"),
        false
    );
    assert_eq!(
        host.read_private("fixture.user", "../fixture.base/note.txt")
            .unwrap_err()
            .code(),
        "rejected"
    );

    host.install(&client_dir).unwrap();
    host.enable("fixture.client").unwrap();
    let forwarded = host.call("fixture.client", "fixture.base", "ping").unwrap();
    assert!(forwarded.starts_with(&format!("{}:", base.0)));
    host.revoke("fixture.client", "plugin.call").unwrap();
    assert_eq!(
        host.call("fixture.client", "fixture.base", "ping")
            .unwrap_err()
            .code(),
        "permission_denied"
    );
    host.grant("fixture.client", "plugin.call").unwrap();
    assert!(
        host.call("fixture.client", "fixture.base", "ping")
            .unwrap()
            .starts_with(&format!("{}:", base.0))
    );

    assert_eq!(host.subscriber_count(), 1);
    let bus = EventBus::new(Limits::standard()).unwrap();
    // 订阅挂在宿主打开时的总线上。这里重新打开一个宿主会丢掉订阅，所以直接使用已附加的总线。
    let _ = bus;
    let event = host.emit("fixture.user", "fixture.tick").unwrap();
    assert_eq!(event.source.provider.as_str(), "fixture.user");
    assert!(matches!(event.record, Record::Observation { .. }));
    host.pause("fixture.user").unwrap();
    assert_eq!(host.report("fixture.user").unwrap().phase, "paused");
    assert_eq!(host.subscriber_count(), 0);
    host.resume("fixture.user").unwrap();
    assert_eq!(host.subscriber_count(), 1);

    host.fault("fixture.base").unwrap();
    assert_eq!(host.report("fixture.base").unwrap().phase, "faulted");
    assert_eq!(host.ping("fixture.user").unwrap().0, user.0);
    assert_eq!(
        host.call("fixture.user", "fixture.base", "again")
            .unwrap_err()
            .code(),
        "dependency_unhealthy"
    );
    fs::remove_file(plugins.join("packages/fixture.base/manifest.json")).unwrap();
    host.recover("fixture.base").unwrap();
    let restored = host.ping("fixture.base").unwrap();
    assert_ne!(restored.0, 0);
    let after = host.call("fixture.user", "fixture.base", "again").unwrap();
    assert!(after.starts_with(&format!("{}:", restored.0)));

    let history_before = host.history("fixture.user").unwrap();
    assert!(
        history_before
            .iter()
            .any(|line| line.contains("\"action\":\"enable\""))
    );
    let declined = scratch.dir("declined");
    write_package(
        &declined,
        &manifest(
            "fixture.user",
            (1, 1, 0),
            &["fixture.echo"],
            &[
                "event.publish",
                "event.subscribe",
                "plugin.call",
                "storage.private",
                "history.read",
            ],
            &[("fixture.base", "compatible", 1, 0, 0)],
            &["fixture.tick"],
            false,
        ),
        true,
    );
    assert_eq!(
        host.update("fixture.user", &declined).unwrap_err().code(),
        "unavailable"
    );
    assert_eq!(host.ping("fixture.user").unwrap().1.to_string(), "1.0.0");
    let broken = scratch.dir("broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("manifest.json"), b"{").unwrap();
    assert_eq!(
        host.update("fixture.user", &broken).unwrap_err().code(),
        "invalid_manifest"
    );
    assert_eq!(
        host.report("fixture.user")
            .unwrap()
            .version
            .unwrap()
            .to_string(),
        "1.0.0"
    );

    host.pin(
        "fixture.user",
        winstrata_protocols::PluginVersion::new(1, 0, 0),
    )
    .unwrap();
    let next = scratch.dir("next");
    write_package(
        &next,
        &manifest(
            "fixture.user",
            (1, 2, 0),
            &["fixture.echo"],
            &[
                "event.publish",
                "event.subscribe",
                "plugin.call",
                "storage.private",
                "history.read",
            ],
            &[("fixture.base", "compatible", 1, 0, 0)],
            &["fixture.tick"],
            false,
        ),
        false,
    );
    assert_eq!(
        host.update("fixture.user", &next).unwrap_err().code(),
        "pinned"
    );
    host.unpin("fixture.user").unwrap();
    let updated = host.update("fixture.user", &next).unwrap();
    assert_eq!(updated.version.unwrap().to_string(), "1.2.0");
    assert_eq!(host.ping("fixture.user").unwrap().1.to_string(), "1.2.0");
    assert!(
        host.report("fixture.user")
            .unwrap()
            .pending
            .contains(&"history.read".to_owned())
    );
    assert_eq!(
        host.history("fixture.user").unwrap().len() > history_before.len(),
        true
    );
    assert!(
        host.history("fixture.user")
            .unwrap()
            .iter()
            .any(|line| line.contains("\"action\":\"enable\""))
    );

    host.install(&other_dir).unwrap();
    assert_eq!(
        host.enable("fixture.other").unwrap_err().code(),
        "capability_conflict"
    );
    host.disable("fixture.user").unwrap();
    assert!(host.report("fixture.user").unwrap().pid.is_none());
    assert_eq!(host.subscriber_count(), 0);
    assert!(
        host.history("fixture.user")
            .unwrap()
            .iter()
            .any(|line| line.contains("\"action\":\"disable\""))
    );
    host.enable("fixture.other").unwrap();
    let other = host.ping("fixture.other").unwrap();
    let replaced = host.call("fixture.client", "fixture.echo", "x").unwrap();
    assert!(replaced.starts_with(&format!("{}:", other.0)));
    host.disable("fixture.other").unwrap();
    host.uninstall("fixture.other").unwrap();
    assert!(
        host.history("fixture.other")
            .unwrap()
            .iter()
            .any(|line| line.contains("\"action\":\"uninstall\""))
    );
    assert_eq!(
        host.report("fixture.other").unwrap_err().code(),
        "not_installed"
    );

    let stopped = host.enable_batch().unwrap();
    assert!(
        stopped.contains(&"fixture.base".to_owned())
            || host.report("fixture.base").unwrap().phase == "running"
    );
    host.stop("fixture.base").unwrap();
    let restarted = host.enable_batch().unwrap();
    assert!(restarted.contains(&"fixture.base".to_owned()));
    host.enable("fixture.user").unwrap();
    host.disable("fixture.base").unwrap();
    assert!(
        !host
            .enable_batch()
            .unwrap()
            .contains(&"fixture.base".to_owned())
    );
    assert_eq!(
        host.report("strata.rescue").unwrap().phase,
        "implementation_absent"
    );
    drop(host);

    let mut reopened = core.open_plugins(runtime()).unwrap();
    reopened.attach_bus(EventBus::new(Limits::standard()).unwrap());
    let persisted = reopened.report("fixture.user").unwrap();
    assert!(persisted.enabled);
    assert_eq!(persisted.phase, "stopped");
    assert_eq!(persisted.version.unwrap().to_string(), "1.2.0");
    assert!(reopened.history("fixture.user").unwrap().len() > history_before.len());
    reopened.enable("fixture.base").unwrap();
    reopened.start("fixture.user").unwrap();
    assert_eq!(
        reopened.ping("fixture.user").unwrap().1.to_string(),
        "1.2.0"
    );
    reopened.disable("fixture.base").unwrap();
    fs::remove_dir_all(plugins.join("recovery/fixture.base")).unwrap();
    fs::remove_file(plugins.join("packages/fixture.base/manifest.json")).unwrap();
    assert_eq!(
        reopened.recover("fixture.base").unwrap_err().code(),
        "dependency_unrecoverable"
    );
    drop(reopened);
}

#[test]
fn a_dependency_cycle_does_not_leave_either_process_running() {
    let scratch = Scratch::new();
    let (mut core, _) = session(&scratch);
    let mut host = host(&mut core);
    let left = scratch.dir("left");
    let right = scratch.dir("right");
    write_package(
        &left,
        &manifest(
            "fixture.left",
            (1, 0, 0),
            &[],
            &[],
            &[("fixture.right", "exact", 1, 0, 0)],
            &[],
            false,
        ),
        false,
    );
    write_package(
        &right,
        &manifest(
            "fixture.right",
            (1, 0, 0),
            &[],
            &[],
            &[("fixture.left", "exact", 1, 0, 0)],
            &[],
            false,
        ),
        false,
    );
    host.install(&left).unwrap();
    host.install(&right).unwrap();
    assert_eq!(
        host.enable("fixture.left").unwrap_err().code(),
        "dependency_unhealthy"
    );
    assert_eq!(
        host.enable("fixture.right").unwrap_err().code(),
        "dependency_cycle"
    );
    assert!(host.report("fixture.left").unwrap().pid.is_none());
    assert!(host.report("fixture.right").unwrap().pid.is_none());
}

#[test]
fn an_external_event_reaches_only_the_subscribed_plugin() {
    let scratch = Scratch::new();
    let (mut core, _) = session(&scratch);
    let bus = EventBus::new(Limits::standard()).unwrap();
    let mut host = core.open_plugins(runtime()).unwrap();
    host.attach_bus(bus.clone());
    let dir = scratch.dir("listener");
    write_package(
        &dir,
        &manifest(
            "fixture.listen",
            (1, 0, 0),
            &[],
            &["event.subscribe"],
            &[],
            &["fixture.tick"],
            false,
        ),
        false,
    );
    host.install(&dir).unwrap();
    host.enable("fixture.listen").unwrap();
    bus.publish(observation("fixture.tick", 1)).unwrap();
    let received = host.poll("fixture.listen").unwrap();
    assert_eq!(received.len(), 1);
    bus.publish(observation("fixture.other", 2)).unwrap();
    let second = host.poll("fixture.listen").unwrap();
    assert!(second.iter().all(|event| !matches!(
        &event.record,
        Record::Observation { event_type, .. } if event_type.as_str() == "fixture.other"
    )));
    drop(host);
}

fn observation(event_type: &str, sequence: u64) -> Event {
    Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new("device-1").unwrap(),
            provider: Id::new("tests.plugins").unwrap(),
            epoch: Id::new("run-1").unwrap(),
        },
        sequence: Counter::new(sequence).unwrap(),
        time: EventTime {
            unix_ns: SignedNanos::new(10),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new(event_type).unwrap(),
            subject: None,
            user: None,
            process: None,
            attribution: Attribution::Unknown,
            completeness: Completeness::Complete,
        },
    }
}

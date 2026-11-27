//! `strata.registry` 在插件宿主进程内运行。
//!
//! 官方目录里的其余身份仍然没有包。这个插件不启动 `winstrata-runtime`，
//! 也不写入已安装包清单。范围在监视运行时不能改；停机后的下一次启动才使用新范围。

use std::sync::Arc;

use winstrata_protocols::{Event, PluginVersion};
use winstrata_strata_registry::{PLUGIN_ID, RegistryError, Scope, Session};

use super::{PluginError, PluginHost, PluginReport, ready_availability};

impl PluginHost {
    pub fn set_registry_scope(&mut self, keys: &[(&str, &str)]) -> Result<(), PluginError> {
        if self.registry.is_some() {
            return Err(PluginError::new(
                "running",
                "registry scope changes after the watch stops",
            ));
        }
        let scope = Scope::from_pairs(keys).map_err(map_registry)?;
        self.registry_scope = scope
            .keys()
            .iter()
            .map(|key| (key.hive.as_str().to_owned(), key.subkey.clone()))
            .collect();
        self.save()
    }

    pub fn registry_query(&self) -> Result<Vec<Event>, PluginError> {
        self.registry
            .as_ref()
            .map(Session::query)
            .ok_or_else(|| PluginError::new("not_running", "registry watch is not running"))
    }

    pub(super) fn enable_registry(&mut self) -> Result<PluginReport, PluginError> {
        self.registry_enabled = true;
        self.save()?;
        match self.start_registry() {
            Ok(report) => {
                self.audit(PLUGIN_ID, "enable", "ok", "running")?;
                Ok(report)
            }
            Err(error) => {
                let _ = self.audit(PLUGIN_ID, "enable", "error", error.code());
                Err(error)
            }
        }
    }

    pub(super) fn start_registry(&mut self) -> Result<PluginReport, PluginError> {
        if !self.registry_enabled {
            return Err(PluginError::new("disabled", "plugin is disabled"));
        }
        if self.registry.is_some() {
            return self.report(PLUGIN_ID);
        }
        let scope = self.current_scope()?;
        let epoch = self.registry_epoch.saturating_add(1);
        if epoch == 0 {
            return Err(PluginError::new(
                "collector_fault",
                "invalid registry epoch",
            ));
        }
        self.registry_epoch = epoch;
        let user = crate::platform::current_id().ok();
        let bus = self.bus.clone();
        let on_event = bus.map(|bus| {
            Arc::new(move |event: &Event| bus.publish(event.clone()).is_ok())
                as Arc<dyn Fn(&Event) -> bool + Send + Sync>
        });
        match Session::start(&scope, user.as_deref(), epoch, on_event) {
            Ok(session) => {
                self.registry = Some(session);
                self.registry_paused = false;
                self.registry_availability = ready_availability();
                self.save()?;
                Ok(self.report_registry())
            }
            Err(error) => {
                self.registry_availability = error.code().to_owned();
                self.save()?;
                Err(map_registry(error))
            }
        }
    }

    pub(super) fn pause_registry(&mut self) -> Result<PluginReport, PluginError> {
        let session = self
            .registry
            .as_ref()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.pause().map_err(map_registry)?;
        self.registry_paused = true;
        self.audit(PLUGIN_ID, "pause", "ok", "paused")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn resume_registry(&mut self) -> Result<PluginReport, PluginError> {
        let session = self
            .registry
            .as_ref()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.resume().map_err(map_registry)?;
        self.registry_paused = false;
        self.audit(PLUGIN_ID, "resume", "ok", "running")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn stop_registry(&mut self) -> Result<PluginReport, PluginError> {
        let mut session = self
            .registry
            .take()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.stop();
        self.registry_paused = false;
        self.audit(PLUGIN_ID, "stop", "ok", "stopped")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn disable_registry(&mut self) -> Result<PluginReport, PluginError> {
        if let Some(mut session) = self.registry.take() {
            session.stop();
        }
        self.registry_enabled = false;
        self.registry_paused = false;
        self.registry_availability = ready_availability();
        self.save()?;
        self.audit(PLUGIN_ID, "disable", "ok", "disabled")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn report_registry(&self) -> PluginReport {
        let phase = if !self.registry_enabled {
            "disabled"
        } else if self.registry_availability != "ready" {
            "unavailable"
        } else if self.registry.is_some() && self.registry_paused {
            "paused"
        } else if self.registry.is_some() {
            "running"
        } else {
            "stopped"
        };
        let availability = if self.registry_enabled {
            self.registry_availability.clone()
        } else {
            ready_availability()
        };
        PluginReport {
            id: PLUGIN_ID.to_owned(),
            name: "StrataRegistry".to_owned(),
            kind: "builtin".to_owned(),
            phase: phase.to_owned(),
            enabled: self.registry_enabled,
            version: Some(PluginVersion::new(0, 1, 0)),
            pid: None,
            grants: vec!["event.publish".to_owned()],
            pending: Vec::new(),
            pin: None,
            availability,
            batch: true,
        }
    }

    fn current_scope(&self) -> Result<Scope, PluginError> {
        if self.registry_scope.is_empty() {
            return Ok(Scope::product());
        }
        let pairs: Vec<(&str, &str)> = self
            .registry_scope
            .iter()
            .map(|(hive, subkey)| (hive.as_str(), subkey.as_str()))
            .collect();
        Scope::from_pairs(&pairs).map_err(map_registry)
    }
}

fn map_registry(error: RegistryError) -> PluginError {
    PluginError::new(error.code(), error.detail())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::{Core, EventBus, Limits, platform};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "winstrata-registry-{}",
                platform::fresh_id().unwrap()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn directory(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).unwrap();
            path
        }

        fn runtime(&self) -> PathBuf {
            let path = self.0.join("runtime");
            if !path.exists() {
                fs::write(&path, b"runtime").unwrap();
            }
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn open_host(scratch: &Scratch) -> (Core, PluginHost, EventBus) {
        let locator = scratch.directory("locator");
        let root = scratch.directory("data");
        let mut core = Core::open_at(&locator, None).unwrap();
        core.select_directory(&root).unwrap();
        let bus = EventBus::new(Limits::standard()).unwrap();
        let mut host = core.open_plugins(&scratch.runtime()).unwrap();
        host.attach_bus(bus.clone());
        (core, host, bus)
    }

    #[test]
    fn registry_is_in_process_and_other_official_plugins_stay_absent() {
        let scratch = Scratch::new();
        let (core, mut host, bus) = open_host(&scratch);
        #[cfg(not(windows))]
        let mut core = core;
        let report = host.report(PLUGIN_ID).unwrap();
        assert_eq!(report.kind, "builtin");
        assert_eq!(report.phase, "disabled");
        assert_eq!(report.availability, "ready");
        assert!(!report.enabled);
        assert!(report.pid.is_none());
        assert!(report.batch);
        assert_eq!(report.grants, vec!["event.publish".to_owned()]);
        assert!(
            host.list()
                .iter()
                .filter(|item| item.id != PLUGIN_ID)
                .all(|item| item.phase == "implementation_absent"
                    && item.availability == "implementation_absent")
        );
        assert_eq!(
            host.enable("strata.pulse").unwrap_err().code(),
            "implementation_absent"
        );
        assert_eq!(
            host.uninstall(PLUGIN_ID).unwrap_err().code(),
            "builtin_retained"
        );
        assert_eq!(host.ping(PLUGIN_ID).unwrap_err().code(), "builtin_closed");
        assert_eq!(
            host.set_registry_scope(&[("HKCU", "Software")])
                .unwrap_err()
                .code(),
            "collector_fault"
        );
        assert_eq!(
            host.set_registry_scope(&[("HKCU", r"..\Software"), ("HKLM", "SOFTWARE")])
                .unwrap_err()
                .detail(),
            "invalid registry scope"
        );
        host.set_registry_scope(&[
            ("HKCU", r"Software\WinStrata"),
            ("HKLM", r"SOFTWARE\WinStrata"),
        ])
        .unwrap();

        #[cfg(not(windows))]
        {
            let _ = bus;
            let error = host.enable(PLUGIN_ID).unwrap_err();
            assert_eq!(error.code(), "unsupported");
            let report = host.report(PLUGIN_ID).unwrap();
            assert!(report.enabled);
            assert_eq!(report.phase, "unavailable");
            assert_eq!(report.availability, "unsupported");
            assert_eq!(host.registry_query().unwrap_err().code(), "not_running");
            drop(host);
            let mut host = core.open_plugins(&scratch.runtime()).unwrap();
            let report = host.report(PLUGIN_ID).unwrap();
            assert!(report.enabled);
            assert_eq!(report.availability, "unsupported");
            let started = host.enable_batch().unwrap();
            assert!(!started.iter().any(|id| id == PLUGIN_ID));
            let report = host.disable(PLUGIN_ID).unwrap();
            assert!(!report.enabled);
            assert_eq!(report.phase, "disabled");
            assert_eq!(report.availability, "ready");
        }

        #[cfg(windows)]
        {
            let _ = core;
            windows::watch_real_keys(&mut host, &bus);
        }
    }

    #[cfg(windows)]
    mod windows {
        use std::time::Duration;

        use windows_sys::Win32::System::Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, KEY_WOW64_64KEY, REG_DWORD,
            REG_OPTION_VOLATILE, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
        };
        use winstrata_protocols::{Attribution, Completeness, Id, Record};

        use super::*;
        use crate::Filter;

        struct Probe {
            hive: HKEY,
            subkey: String,
            wide: Vec<u16>,
            write: HKEY,
        }

        impl Probe {
            fn create(hive: HKEY, subkey: &str) -> Self {
                let wide = subkey
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<_>>();
                let mut write = std::ptr::null_mut();
                let status = unsafe {
                    RegCreateKeyExW(
                        hive,
                        wide.as_ptr(),
                        0,
                        std::ptr::null(),
                        REG_OPTION_VOLATILE,
                        KEY_SET_VALUE | KEY_WOW64_64KEY,
                        std::ptr::null(),
                        &mut write,
                        std::ptr::null_mut(),
                    )
                };
                assert_eq!(status, 0, "create {subkey} failed with {status}");
                Self {
                    hive,
                    subkey: subkey.to_owned(),
                    wide,
                    write,
                }
            }

            fn set_dword(&self, name: &str, value: u32) {
                let name = name
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<_>>();
                let bytes = value.to_le_bytes();
                let status = unsafe {
                    RegSetValueExW(self.write, name.as_ptr(), 0, REG_DWORD, bytes.as_ptr(), 4)
                };
                assert_eq!(status, 0, "set failed with {status}");
            }
        }

        impl Drop for Probe {
            fn drop(&mut self) {
                unsafe {
                    if !self.write.is_null() {
                        RegCloseKey(self.write);
                    }
                    RegDeleteTreeW(self.hive, self.wide.as_ptr());
                }
            }
        }

        pub(super) fn watch_real_keys(host: &mut PluginHost, bus: &EventBus) {
            let pid = std::process::id();
            let hkcu = Probe::create(
                HKEY_CURRENT_USER,
                &format!(r"Software\WinStrataHost019u{pid}"),
            );
            let hklm = Probe::create(
                HKEY_LOCAL_MACHINE,
                &format!(r"SOFTWARE\WinStrataHost019m{pid}"),
            );
            hkcu.set_dword("Probe", 0x11);
            hklm.set_dword("Probe", 0x11);
            host.set_registry_scope(&[
                ("HKCU", hkcu.subkey.as_str()),
                ("HKLM", hklm.subkey.as_str()),
            ])
            .unwrap();
            let subscription = bus.subscribe(0, Filter::default()).unwrap();
            let report = host.enable(PLUGIN_ID).unwrap();
            assert_eq!(report.phase, "running");
            assert_eq!(
                host.set_registry_scope(&[
                    ("HKCU", hkcu.subkey.as_str()),
                    ("HKLM", hklm.subkey.as_str()),
                ])
                .unwrap_err()
                .code(),
                "running"
            );
            hkcu.set_dword("Probe", 0x22);
            let started = std::time::Instant::now();
            let events = loop {
                let events = host.registry_query().unwrap();
                if events
                    .iter()
                    .any(|event| evidence(event) == Some("t=4;b=11000000;a=22000000"))
                    || started.elapsed() > Duration::from_secs(5)
                {
                    break events;
                }
                std::thread::sleep(Duration::from_millis(50));
            };
            let changed = events
                .iter()
                .find(|event| evidence(event) == Some("t=4;b=11000000;a=22000000"))
                .unwrap_or_else(|| panic!("missing host HKCU change: {events:?}"));
            let expected = format!(r"HKCU\{}\Probe", hkcu.subkey);
            assert_eq!(subject(changed), Some(expected.as_str()));
            assert_provenance(changed);
            let delivered = subscription.recv_batch(32, Duration::from_secs(2)).unwrap();
            assert!(
                delivered
                    .iter()
                    .any(|event| evidence(event) == Some("t=4;b=11000000;a=22000000"))
            );
            hklm.set_dword("Probe", 0x33);
            let started = std::time::Instant::now();
            let events = loop {
                let events = host.registry_query().unwrap();
                if events
                    .iter()
                    .any(|event| evidence(event) == Some("t=4;b=11000000;a=33000000"))
                    || started.elapsed() > Duration::from_secs(5)
                {
                    break events;
                }
                std::thread::sleep(Duration::from_millis(50));
            };
            let changed = events
                .iter()
                .find(|event| evidence(event) == Some("t=4;b=11000000;a=33000000"))
                .unwrap_or_else(|| panic!("missing host HKLM change: {events:?}"));
            assert_eq!(
                subject(changed).unwrap(),
                format!(r"HKLM\{}\Probe", hklm.subkey)
            );
            assert_provenance(changed);
            let report = host.disable(PLUGIN_ID).unwrap();
            assert_eq!(report.phase, "disabled");
        }

        fn evidence(event: &Event) -> Option<&str> {
            match &event.record {
                Record::Observation {
                    attribution: Attribution::Observed { evidence },
                    completeness: Completeness::Complete,
                    ..
                } => Some(evidence.as_str()),
                _ => None,
            }
        }

        fn subject(event: &Event) -> Option<&str> {
            match &event.record {
                Record::Observation {
                    subject: Some(subject),
                    ..
                } => Some(subject.id.as_str()),
                _ => None,
            }
        }

        fn assert_provenance(event: &Event) {
            assert_eq!(event.source.device.as_str(), PLUGIN_ID);
            assert_eq!(event.source.provider.as_str(), "windows.reg_notify");
            assert!(event.time.precision_ns.is_none());
            let current = platform::current_id().unwrap();
            let expected = Id::new(&current).ok();
            match &event.record {
                Record::Observation { user, process, .. } => {
                    assert!(process.is_none());
                    assert_eq!(
                        user.as_ref().map(Id::as_str),
                        expected.as_ref().map(Id::as_str)
                    );
                }
                other => panic!("unexpected record {other:?}"),
            }
        }
    }
}

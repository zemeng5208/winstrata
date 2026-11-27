//! `strata.env` 在插件宿主进程内运行。
//!
//! 它只观察用户和系统环境存储，不复用注册表插件的范围。
//! 范围在监视运行时不能改；停机后的下一次启动才使用新范围。

use std::sync::Arc;

use winstrata_protocols::{Event, PluginVersion};
use winstrata_strata_env::{EnvError, PLUGIN_ID, Scope, Session};

use super::{PluginError, PluginHost, PluginReport, ready_availability};

impl PluginHost {
    pub fn set_env_scope(&mut self, keys: &[(&str, &str)]) -> Result<(), PluginError> {
        if self.environment.is_some() {
            return Err(PluginError::new(
                "running",
                "environment scope changes after the watch stops",
            ));
        }
        let scope = Scope::from_pairs(keys).map_err(map_env)?;
        self.env_scope = scope
            .keys()
            .iter()
            .map(|key| (key.hive.as_str().to_owned(), key.subkey.clone()))
            .collect();
        self.save()
    }

    pub fn env_query(&self) -> Result<Vec<Event>, PluginError> {
        self.environment
            .as_ref()
            .map(Session::query)
            .ok_or_else(|| PluginError::new("not_running", "environment watch is not running"))
    }

    pub(super) fn enable_environment(&mut self) -> Result<PluginReport, PluginError> {
        self.env_enabled = true;
        self.save()?;
        match self.start_environment() {
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

    pub(super) fn start_environment(&mut self) -> Result<PluginReport, PluginError> {
        if !self.env_enabled {
            return Err(PluginError::new("disabled", "plugin is disabled"));
        }
        if self.environment.is_some() {
            return self.report(PLUGIN_ID);
        }
        let scope = self.current_env_scope()?;
        let epoch = self.env_epoch.saturating_add(1);
        if epoch == 0 {
            return Err(PluginError::new("collector_fault", "invalid env epoch"));
        }
        self.env_epoch = epoch;
        let user = crate::platform::current_id().ok();
        let bus = self.bus.clone();
        let on_event = bus.map(|bus| {
            Arc::new(move |event: &Event| bus.publish(event.clone()).is_ok())
                as Arc<dyn Fn(&Event) -> bool + Send + Sync>
        });
        match Session::start(&scope, user.as_deref(), epoch, on_event) {
            Ok(session) => {
                self.environment = Some(session);
                self.env_paused = false;
                self.env_availability = ready_availability();
                self.save()?;
                Ok(self.report_environment())
            }
            Err(error) => {
                self.env_availability = error.code().to_owned();
                self.save()?;
                Err(map_env(error))
            }
        }
    }

    pub(super) fn pause_environment(&mut self) -> Result<PluginReport, PluginError> {
        let session = self
            .environment
            .as_ref()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.pause().map_err(map_env)?;
        self.env_paused = true;
        self.audit(PLUGIN_ID, "pause", "ok", "paused")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn resume_environment(&mut self) -> Result<PluginReport, PluginError> {
        let session = self
            .environment
            .as_ref()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.resume().map_err(map_env)?;
        self.env_paused = false;
        self.audit(PLUGIN_ID, "resume", "ok", "running")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn stop_environment(&mut self) -> Result<PluginReport, PluginError> {
        let mut session = self
            .environment
            .take()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.stop();
        self.env_paused = false;
        self.audit(PLUGIN_ID, "stop", "ok", "stopped")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn disable_environment(&mut self) -> Result<PluginReport, PluginError> {
        if let Some(mut session) = self.environment.take() {
            session.stop();
        }
        self.env_enabled = false;
        self.env_paused = false;
        self.env_availability = ready_availability();
        self.save()?;
        self.audit(PLUGIN_ID, "disable", "ok", "disabled")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn report_environment(&self) -> PluginReport {
        let phase = if !self.env_enabled {
            "disabled"
        } else if self.env_availability != "ready" {
            "unavailable"
        } else if self.environment.is_some() && self.env_paused {
            "paused"
        } else if self.environment.is_some() {
            "running"
        } else {
            "stopped"
        };
        let availability = if self.env_enabled {
            self.env_availability.clone()
        } else {
            ready_availability()
        };
        PluginReport {
            id: PLUGIN_ID.to_owned(),
            name: "StrataEnv".to_owned(),
            kind: "builtin".to_owned(),
            phase: phase.to_owned(),
            enabled: self.env_enabled,
            version: Some(PluginVersion::new(0, 1, 0)),
            pid: None,
            grants: vec!["event.publish".to_owned()],
            pending: Vec::new(),
            pin: None,
            availability,
            batch: true,
        }
    }

    fn current_env_scope(&self) -> Result<Scope, PluginError> {
        if self.env_scope.is_empty() {
            return Ok(Scope::product());
        }
        let pairs: Vec<(&str, &str)> = self
            .env_scope
            .iter()
            .map(|(hive, subkey)| (hive.as_str(), subkey.as_str()))
            .collect();
        Scope::from_pairs(&pairs).map_err(map_env)
    }
}

fn map_env(error: EnvError) -> PluginError {
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
            let path = std::env::temp_dir()
                .join(format!("winstrata-env-{}", platform::fresh_id().unwrap()));
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
    fn environment_is_in_process_and_linux_does_not_publish() {
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
        assert_eq!(
            host.uninstall(PLUGIN_ID).unwrap_err().code(),
            "builtin_retained"
        );
        assert_eq!(host.ping(PLUGIN_ID).unwrap_err().code(), "builtin_closed");
        assert_eq!(
            host.set_env_scope(&[("HKCU", "Environment")])
                .unwrap_err()
                .detail(),
            "invalid env scope"
        );
        host.set_env_scope(&[
            ("HKCU", "Environment"),
            (
                "HKLM",
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            ),
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
            assert_eq!(host.env_query().unwrap_err().code(), "not_running");
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
            windows::watch_real_environment(&mut host, &bus);
        }
    }

    #[cfg(windows)]
    mod windows {
        use std::time::{Duration, Instant};

        use windows_sys::Win32::System::Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE,
            KEY_WOW64_64KEY, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumValueW, RegOpenKeyExW,
            RegQueryValueExW, RegSetValueExW,
        };
        use winstrata_protocols::{Attribution, Completeness, Id, Record};

        use super::*;
        use crate::Filter;

        const ERROR_FILE_NOT_FOUND: u32 = 2;
        const ERROR_NO_MORE_ITEMS: u32 = 259;

        struct OpenKey {
            hive: HKEY,
            subkey: Vec<u16>,
            key: HKEY,
        }

        impl OpenKey {
            fn open(hive: HKEY, subkey: &str) -> Self {
                let wide = wide(subkey);
                let mut key = std::ptr::null_mut();
                let status = unsafe {
                    RegOpenKeyExW(
                        hive,
                        wide.as_ptr(),
                        0,
                        KEY_QUERY_VALUE | KEY_SET_VALUE | KEY_WOW64_64KEY,
                        &mut key,
                    )
                };
                assert_eq!(status, 0, "open {subkey} failed with {status}");
                Self {
                    hive,
                    subkey: wide,
                    key,
                }
            }
        }

        impl Drop for OpenKey {
            fn drop(&mut self) {
                unsafe {
                    if !self.key.is_null() {
                        RegCloseKey(self.key);
                    }
                }
            }
        }

        struct ValueRestore {
            hive: HKEY,
            subkey: Vec<u16>,
            name: Vec<u16>,
            previous: Option<(u32, Vec<u8>)>,
        }

        impl ValueRestore {
            fn prepare(store: &OpenKey, name: &str) -> Self {
                let wide_name = wide(name);
                let previous = query_value(store.key, &wide_name);
                Self {
                    hive: store.hive,
                    subkey: store.subkey.clone(),
                    name: wide_name,
                    previous,
                }
            }

            fn set(&self, text: &str) {
                self.write(REG_SZ, &utf16_bytes(text));
            }

            fn write(&self, kind: u32, data: &[u8]) {
                let status = self.with_key(|key| unsafe {
                    RegSetValueExW(
                        key,
                        self.name.as_ptr(),
                        0,
                        kind,
                        data.as_ptr(),
                        data.len() as u32,
                    )
                });
                assert_eq!(status, 0, "set environment value failed with {status}");
            }

            fn with_key(&self, body: impl FnOnce(HKEY) -> u32) -> u32 {
                let mut key = std::ptr::null_mut();
                let status = unsafe {
                    RegOpenKeyExW(
                        self.hive,
                        self.subkey.as_ptr(),
                        0,
                        KEY_SET_VALUE | KEY_WOW64_64KEY,
                        &mut key,
                    )
                };
                if status != 0 {
                    return status;
                }
                let result = body(key);
                unsafe { RegCloseKey(key) };
                result
            }
        }

        impl Drop for ValueRestore {
            fn drop(&mut self) {
                let _ = self.with_key(|key| unsafe {
                    match &self.previous {
                        Some((kind, data)) => RegSetValueExW(
                            key,
                            self.name.as_ptr(),
                            0,
                            *kind,
                            data.as_ptr(),
                            data.len() as u32,
                        ),
                        None => windows_sys::Win32::System::Registry::RegDeleteValueW(
                            key,
                            self.name.as_ptr(),
                        ),
                    }
                });
            }
        }

        struct PathEdit {
            restore: ValueRestore,
            segment: String,
            applied: bool,
        }

        impl PathEdit {
            fn prepare(store: &OpenKey, segment: &str) -> Self {
                let found = find_path(store.key);
                let (name, previous) = match found {
                    Some((name, kind, data)) => (name, Some((kind, data))),
                    None => (wide("Path"), None),
                };
                Self {
                    restore: ValueRestore {
                        hive: store.hive,
                        subkey: store.subkey.clone(),
                        name,
                        previous,
                    },
                    segment: segment.to_owned(),
                    applied: false,
                }
            }

            fn apply(&mut self) {
                let (kind, text) = match &self.restore.previous {
                    Some((kind, data)) if *kind == REG_SZ || *kind == REG_EXPAND_SZ => {
                        let text = decode_text(data).expect("existing PATH is text");
                        (*kind, text)
                    }
                    Some(_) => panic!("existing PATH is not text"),
                    None => (REG_SZ, String::new()),
                };
                let mut next = text;
                if !next.is_empty() && !next.ends_with(';') {
                    next.push(';');
                }
                next.push_str(&self.segment);
                self.restore.write(kind, &utf16_bytes(&next));
                self.applied = true;
            }
        }

        fn wide(text: &str) -> Vec<u16> {
            text.encode_utf16().chain(std::iter::once(0)).collect()
        }

        fn utf16_bytes(text: &str) -> Vec<u8> {
            wide(text)
                .into_iter()
                .flat_map(|unit| unit.to_le_bytes())
                .collect()
        }

        fn decode_text(data: &[u8]) -> Option<String> {
            if data.len() % 2 != 0 {
                return None;
            }
            let mut units = Vec::with_capacity(data.len() / 2);
            for chunk in data.chunks_exact(2) {
                units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
            }
            if units.last() == Some(&0) {
                units.pop();
            }
            String::from_utf16(&units).ok()
        }

        fn query_value(key: HKEY, name: &[u16]) -> Option<(u32, Vec<u8>)> {
            let mut kind = 0u32;
            let mut len = 0u32;
            let status = unsafe {
                RegQueryValueExW(
                    key,
                    name.as_ptr(),
                    std::ptr::null_mut(),
                    &mut kind,
                    std::ptr::null_mut(),
                    &mut len,
                )
            };
            if status == ERROR_FILE_NOT_FOUND {
                return None;
            }
            assert_eq!(status, 0, "query size failed with {status}");
            let mut data = vec![0u8; len as usize];
            let mut len = data.len() as u32;
            let status = unsafe {
                RegQueryValueExW(
                    key,
                    name.as_ptr(),
                    std::ptr::null_mut(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut len,
                )
            };
            assert_eq!(status, 0, "query value failed with {status}");
            data.truncate(len as usize);
            Some((kind, data))
        }

        fn find_path(key: HKEY) -> Option<(Vec<u16>, u32, Vec<u8>)> {
            let mut index = 0u32;
            loop {
                let mut name = vec![0u16; 256];
                let mut name_chars = name.len() as u32;
                let mut kind = 0u32;
                let mut data = vec![0u8; 32768];
                let mut data_len = data.len() as u32;
                let status = unsafe {
                    RegEnumValueW(
                        key,
                        index,
                        name.as_mut_ptr(),
                        &mut name_chars,
                        std::ptr::null_mut(),
                        &mut kind,
                        data.as_mut_ptr(),
                        &mut data_len,
                    )
                };
                if status == ERROR_NO_MORE_ITEMS {
                    return None;
                }
                assert_eq!(status, 0, "enum environment value failed with {status}");
                let Ok(text) = String::from_utf16(&name[..name_chars as usize]) else {
                    index += 1;
                    continue;
                };
                if text.eq_ignore_ascii_case("path") {
                    name.truncate(name_chars as usize);
                    name.push(0);
                    data.truncate(data_len as usize);
                    return Some((name, kind, data));
                }
                index += 1;
            }
        }

        fn hex_text(text: &str) -> String {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            let mut out = String::new();
            for byte in text.as_bytes() {
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0f) as usize] as char);
            }
            out
        }

        fn evidence(event: &Event) -> Option<&str> {
            match &event.record {
                Record::Observation {
                    attribution: Attribution::Observed { evidence },
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

        fn event_type(event: &Event) -> Option<&str> {
            match &event.record {
                Record::Observation { event_type, .. } => Some(event_type.as_str()),
                _ => None,
            }
        }

        fn wait_until(host: &PluginHost, mut found: impl FnMut(&Event) -> bool) -> Vec<Event> {
            let started = Instant::now();
            loop {
                let events = host.env_query().unwrap();
                if events.iter().any(|event| found(event))
                    || started.elapsed() > Duration::from_secs(8)
                {
                    return events;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        fn assert_provenance(event: &Event) {
            assert_eq!(event.source.device.as_str(), PLUGIN_ID);
            assert_eq!(event.source.provider.as_str(), "windows.env_notify");
            assert!(event.source.epoch.as_str().starts_with("env-"));
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

        pub(super) fn watch_real_environment(host: &mut PluginHost, bus: &EventBus) {
            let pid = std::process::id();
            let user_name = format!("WinStrataEnv020U{pid}");
            let system_name = format!("WinStrataEnv020S{pid}");
            let segment = format!(r"C:\WinStrataEnv020{pid}");
            let user_store = OpenKey::open(HKEY_CURRENT_USER, "Environment");
            let system_store = OpenKey::open(
                HKEY_LOCAL_MACHINE,
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            );
            let user_probe = ValueRestore::prepare(&user_store, &user_name);
            let system_probe = ValueRestore::prepare(&system_store, &system_name);
            let mut user_path = PathEdit::prepare(&user_store, &segment);
            let mut system_path = PathEdit::prepare(&system_store, &segment);

            let subscription = bus.subscribe(0, Filter::default()).unwrap();
            let report = host.enable(PLUGIN_ID).unwrap();
            assert_eq!(report.phase, "running");
            assert!(host.env_query().unwrap().iter().all(|event| {
                subject(event).is_none_or(|id| !id.ends_with(user_name.as_str()))
            }));
            assert_eq!(
                host.set_env_scope(&[
                    ("HKCU", "Environment"),
                    (
                        "HKLM",
                        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
                    ),
                ])
                .unwrap_err()
                .code(),
                "running"
            );

            user_probe.set("alpha");
            let added = hex_text("alpha");
            let events = wait_until(host, |event| {
                evidence(event).is_some_and(|text| text.contains(&added))
                    && subject(event).is_some_and(|id| id == format!("user\\{user_name}"))
            });
            let event = events
                .iter()
                .find(|event| evidence(event).is_some_and(|text| text == format!("t=sz;a={added}")))
                .unwrap_or_else(|| panic!("missing user add: {events:?}"));
            assert_eq!(event_type(event), Some("env.variable_added"));
            assert_provenance(event);
            let delivered = subscription.recv_batch(64, Duration::from_secs(2)).unwrap();
            assert!(delivered.iter().any(|event| {
                evidence(event).is_some_and(|text| text == format!("t=sz;a={added}"))
            }));

            user_probe.set("beta");
            let changed = hex_text("beta");
            let events = wait_until(host, |event| {
                evidence(event).is_some_and(|text| text == format!("t=sz;b={added};a={changed}"))
            });
            let event = events
                .iter()
                .find(|event| {
                    evidence(event)
                        .is_some_and(|text| text == format!("t=sz;b={added};a={changed}"))
                })
                .unwrap_or_else(|| panic!("missing user change: {events:?}"));
            assert_eq!(event_type(event), Some("env.variable_changed"));
            let user_subject = format!("user\\{user_name}");
            assert_eq!(subject(event), Some(user_subject.as_str()));

            system_probe.set("sys-a");
            let system_added = hex_text("sys-a");
            let events = wait_until(host, |event| {
                evidence(event).is_some_and(|text| text == format!("t=sz;a={system_added}"))
            });
            let event = events
                .iter()
                .find(|event| {
                    evidence(event).is_some_and(|text| text == format!("t=sz;a={system_added}"))
                })
                .unwrap_or_else(|| panic!("missing system add: {events:?}"));
            assert_eq!(event_type(event), Some("env.variable_added"));
            let system_subject = format!("system\\{system_name}");
            assert_eq!(subject(event), Some(system_subject.as_str()));
            assert_provenance(event);

            user_path.apply();
            let segment_hex = hex_text(&segment);
            let events = wait_until(host, |event| {
                evidence(event).is_some_and(|text| text.contains(&segment_hex))
                    && subject(event)
                        .is_some_and(|id| id.to_ascii_lowercase().starts_with("user\\path"))
            });
            let event = events
                .iter()
                .find(|event| {
                    evidence(event).is_some_and(|text| text.contains(&segment_hex))
                        && matches!(
                            event_type(event),
                            Some("env.path_added" | "env.path_changed")
                        )
                })
                .unwrap_or_else(|| panic!("missing user PATH change: {events:?}"));
            assert_provenance(event);
            assert!(matches!(
                &event.record,
                Record::Observation {
                    completeness: Completeness::Complete,
                    ..
                }
            ));

            system_path.apply();
            let events = wait_until(host, |event| {
                evidence(event).is_some_and(|text| text.contains(&segment_hex))
                    && subject(event)
                        .is_some_and(|id| id.to_ascii_lowercase().starts_with("system\\path"))
            });
            assert!(
                events.iter().any(|event| {
                    evidence(event).is_some_and(|text| text.contains(&segment_hex))
                        && subject(event)
                            .is_some_and(|id| id.to_ascii_lowercase().starts_with("system\\path"))
                }),
                "missing system PATH change: {events:?}"
            );

            let report = host.pause(PLUGIN_ID).unwrap();
            assert_eq!(report.phase, "paused");
            user_probe.set("gamma");
            let hidden = hex_text("gamma");
            let events = wait_until(host, |event| {
                matches!(
                    &event.record,
                    Record::Gap {
                        reason: winstrata_protocols::GapReason::Paused,
                        ..
                    }
                )
            });
            assert!(
                events.iter().any(|event| matches!(
                    &event.record,
                    Record::Gap {
                        reason: winstrata_protocols::GapReason::Paused,
                        ..
                    }
                )),
                "missing paused gap: {events:?}"
            );
            assert!(
                events
                    .iter()
                    .all(|event| evidence(event).is_none_or(|text| !text.contains(&hidden))),
                "paused value was published: {events:?}"
            );
            let report = host.resume(PLUGIN_ID).unwrap();
            assert_eq!(report.phase, "running");
            user_probe.set("delta");
            let visible = hex_text("delta");
            let events = wait_until(host, |event| {
                evidence(event).is_some_and(|text| text.contains(&visible))
            });
            assert!(
                events.iter().any(|event| {
                    evidence(event).is_some_and(|text| text.contains(&visible))
                        && event_type(event) == Some("env.variable_changed")
                }),
                "missing resumed change: {events:?}"
            );

            let report = host.stop(PLUGIN_ID).unwrap();
            assert!(report.enabled);
            assert_eq!(report.phase, "stopped");
            user_probe.set("after-stop");
            std::thread::sleep(Duration::from_millis(300));
            assert_eq!(host.env_query().unwrap_err().code(), "not_running");
            let report = host.start(PLUGIN_ID).unwrap();
            assert_eq!(report.phase, "running");
            let stopped = hex_text("after-stop");
            assert!(
                host.env_query()
                    .unwrap()
                    .iter()
                    .all(|event| { evidence(event).is_none_or(|text| !text.contains(&stopped)) }),
                "stopped change was replayed"
            );
            user_probe.set("after-start");
            let restarted = hex_text("after-start");
            let events = wait_until(host, |event| {
                evidence(event).is_some_and(|text| text.contains(&restarted))
            });
            assert!(events.iter().any(|event| {
                evidence(event).is_some_and(|text| text.contains(&restarted))
                    && event_type(event) == Some("env.variable_changed")
            }));

            let report = host.disable(PLUGIN_ID).unwrap();
            assert!(!report.enabled);
            assert_eq!(report.phase, "disabled");
            assert_eq!(report.availability, "ready");
            let _ = (user_path.applied, system_path.applied);
        }
    }
}

//! `strata.trace` 在插件宿主进程内运行。这是计划 021，没有安装包。
//!
//! 这次监视不加载驱动，不读取文件字节，也不把用户态证据当成内核归因。
//! 范围在监视运行时不能改。停机后的下一次启动才使用新范围。
//! 投影出来的历史只返回给调用方，不再次发布到事件总线。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use winstrata_protocols::{Event, PluginVersion};
use winstrata_strata_trace::{
    ForeignObservation, PLUGIN_ID, Scope, Session, TraceError, project_history,
};

use super::{PluginError, PluginHost, PluginReport, ready_availability};

impl PluginHost {
    pub fn set_trace_scope(&mut self, paths: &[PathBuf]) -> Result<(), PluginError> {
        if self.trace.is_some() {
            return Err(PluginError::new(
                "running",
                "trace scope changes after the watch stops",
            ));
        }
        let scope = Scope::from_paths(paths).map_err(map_trace)?;
        let profile = trace_profile();
        for path in paths {
            // 符号链接、非目录和其他用户的私有目录都不能进入范围。错误里不写路径。
            if trace_path_rejected(path, &profile) {
                return Err(PluginError::new("collector_fault", "invalid trace scope"));
            }
        }
        let mut saved = Vec::with_capacity(scope.paths().len());
        for path in scope.paths() {
            let Some(text) = path.to_str() else {
                return Err(PluginError::new("collector_fault", "invalid trace scope"));
            };
            saved.push(text.to_owned());
        }
        self.trace_scope = saved;
        self.save()
    }

    pub fn trace_query(&self) -> Result<Vec<Event>, PluginError> {
        self.trace
            .as_ref()
            .map(Session::query)
            .ok_or_else(|| PluginError::new("not_running", "trace watch is not running"))
    }

    pub fn trace_history(
        &self,
        path: &str,
        baseline_paths: &[String],
        foreign: &[ForeignObservation],
    ) -> Result<Vec<Event>, PluginError> {
        let session = self
            .trace
            .as_ref()
            .ok_or_else(|| PluginError::new("not_running", "trace watch is not running"))?;
        // 投影只描述已有观察和外部证据，不把结果再发布一次。
        Ok(project_history(
            path,
            &session.query(),
            baseline_paths,
            foreign,
        ))
    }

    pub(super) fn enable_trace(&mut self) -> Result<PluginReport, PluginError> {
        self.trace_enabled = true;
        self.save()?;
        match self.start_trace() {
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

    pub(super) fn start_trace(&mut self) -> Result<PluginReport, PluginError> {
        if !self.trace_enabled {
            return Err(PluginError::new("disabled", "plugin is disabled"));
        }
        if self.trace.is_some() {
            return self.report(PLUGIN_ID);
        }
        let scope = self.current_trace_scope()?;
        let epoch = self.trace_epoch.saturating_add(1);
        if epoch == 0 {
            return Err(PluginError::new("collector_fault", "invalid trace epoch"));
        }
        self.trace_epoch = epoch;
        let bus = self.bus.clone();
        // 回调只转发采集器已经生成的用户态事件。宿主不读取文件内容，也不补写内核归因。
        let on_event = bus.map(|bus| {
            Arc::new(move |event: &Event| bus.publish(event.clone()).is_ok())
                as Arc<dyn Fn(&Event) -> bool + Send + Sync>
        });
        match Session::start(&scope, epoch, on_event) {
            Ok(session) => {
                self.trace = Some(session);
                self.trace_paused = false;
                self.trace_availability = ready_availability();
                self.save()?;
                Ok(self.report_trace())
            }
            Err(error) => {
                self.trace_availability = error.code().to_owned();
                self.save()?;
                Err(map_trace(error))
            }
        }
    }

    pub(super) fn pause_trace(&mut self) -> Result<PluginReport, PluginError> {
        let session = self
            .trace
            .as_ref()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.pause().map_err(map_trace)?;
        self.trace_paused = true;
        self.audit(PLUGIN_ID, "pause", "ok", "paused")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn resume_trace(&mut self) -> Result<PluginReport, PluginError> {
        let session = self
            .trace
            .as_ref()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.resume().map_err(map_trace)?;
        self.trace_paused = false;
        self.audit(PLUGIN_ID, "resume", "ok", "running")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn stop_trace(&mut self) -> Result<PluginReport, PluginError> {
        let mut session = self
            .trace
            .take()
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        session.stop();
        self.trace_paused = false;
        self.audit(PLUGIN_ID, "stop", "ok", "stopped")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn disable_trace(&mut self) -> Result<PluginReport, PluginError> {
        if let Some(mut session) = self.trace.take() {
            session.stop();
        }
        self.trace_enabled = false;
        self.trace_paused = false;
        self.trace_availability = ready_availability();
        self.save()?;
        self.audit(PLUGIN_ID, "disable", "ok", "disabled")?;
        self.report(PLUGIN_ID)
    }

    pub(super) fn report_trace(&self) -> PluginReport {
        let phase = if !self.trace_enabled {
            "disabled"
        } else if self.trace_availability != "ready" {
            "unavailable"
        } else if self.trace.is_some() && self.trace_paused {
            "paused"
        } else if self.trace.is_some() {
            "running"
        } else {
            "stopped"
        };
        let availability = if self.trace_enabled {
            self.trace_availability.clone()
        } else {
            ready_availability()
        };
        PluginReport {
            id: PLUGIN_ID.to_owned(),
            name: "StrataTrace".to_owned(),
            kind: "builtin".to_owned(),
            phase: phase.to_owned(),
            enabled: self.trace_enabled,
            version: Some(PluginVersion::new(0, 1, 0)),
            pid: None,
            grants: vec!["event.publish".to_owned()],
            pending: Vec::new(),
            pin: None,
            availability,
            batch: true,
        }
    }

    fn current_trace_scope(&self) -> Result<Scope, PluginError> {
        let paths: Vec<PathBuf> = self.trace_scope.iter().map(PathBuf::from).collect();
        Scope::from_paths(&paths).map_err(map_trace)
    }
}

fn map_trace(error: TraceError) -> PluginError {
    PluginError::new(error.code(), error.detail())
}

fn trace_profile() -> PathBuf {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key).map(PathBuf::from).unwrap_or_default()
}

fn trace_path_rejected(path: &Path, profile: &Path) -> bool {
    if path.to_str().is_none() {
        return true;
    }
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return true;
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return true;
    }
    crate::scope::foreign_private(path, Some(profile))
}

#[cfg(all(test, not(windows)))]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use winstrata_protocols::PluginVersion;

    use super::*;
    use crate::{Core, EventBus, Limits, platform};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("winstrata-trace-{}", platform::fresh_id().unwrap()));
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
    fn trace_is_in_process_and_linux_cannot_start_it() {
        let scratch = Scratch::new();
        let watched = scratch.directory("watch");
        let (mut core, mut host, _bus) = open_host(&scratch);
        let report = host.report(PLUGIN_ID).unwrap();
        assert_eq!(report.kind, "builtin");
        assert_eq!(report.phase, "disabled");
        assert_eq!(report.availability, "ready");
        assert!(!report.enabled);
        assert!(report.pid.is_none());
        assert!(report.batch);
        assert_eq!(report.version, Some(PluginVersion::new(0, 1, 0)));
        assert_eq!(report.grants, vec!["event.publish".to_owned()]);
        assert_eq!(host.ping(PLUGIN_ID).unwrap_err().code(), "builtin_closed");
        assert_eq!(
            host.uninstall(PLUGIN_ID).unwrap_err().code(),
            "builtin_retained"
        );

        let relative = host
            .set_trace_scope(&[PathBuf::from("relative-trace-scope")])
            .unwrap_err();
        assert_eq!(relative.detail(), "invalid trace scope");
        assert!(!relative.detail().contains("relative-trace-scope"));

        let marker = "secret-marker";
        let dotted = watched.join("..").join(marker);
        let shown = dotted.to_string_lossy().into_owned();
        let rejected = host.set_trace_scope(&[dotted]).unwrap_err();
        assert_eq!(rejected.detail(), "invalid trace scope");
        assert!(!rejected.detail().contains(marker));
        assert!(!rejected.detail().contains(&shown));

        host.set_trace_scope(&[watched.clone()]).unwrap();
        let error = host.enable(PLUGIN_ID).unwrap_err();
        assert_eq!(error.code(), "unsupported");
        let report = host.report(PLUGIN_ID).unwrap();
        assert!(report.enabled);
        assert_eq!(report.phase, "unavailable");
        assert_eq!(report.availability, "unsupported");
        assert_eq!(host.trace_query().unwrap_err().code(), "not_running");

        // Linux 上这次启动不会进入运行，所以范围仍可改。
        host.set_trace_scope(&[watched]).unwrap();
        let still = host.set_trace_scope(&[PathBuf::from("..")]).unwrap_err();
        assert_eq!(still.detail(), "invalid trace scope");
        assert_ne!(still.code(), "running");

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
}

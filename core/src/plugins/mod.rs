//! 插件清单、权限、依赖和进程生命周期。
//!
//! 官方 25 个身份保留在目录中。第三方安装不能占用这些身份。
//! `strata.process`、`strata.services`、`strata.startup`、`strata.pulse`、`strata.health` 与 `strata.budget` 只在同目录存在对应程序时登记为已安装且默认禁用。
//! `strata.script` 只在同目录存在 `winstrata-strata-script` 时登记，并且在启用前保持禁用。
//! `strata.filter` 只在同目录存在 `winstrata-strata-filter` 时登记，并且在启用前保持禁用。
//! `strata.tag` 只在同目录存在 `winstrata-strata-tag` 时登记，并且在启用前保持禁用。
//! `strata.storage` 只在同目录存在 `winstrata-strata-storage` 时登记，并且在启用前保持禁用。
//! `strata.registry` 由计划 019 在进程内启动，没有包。
//! `strata.env` 由计划 020 在进程内启动，没有包。
//! `strata.trace` 由计划 021 在进程内启动，没有包。
//! 原生包和 Python、TypeScript、JavaScript 包都以独立进程运行。

mod budget;
mod env;
mod filter;
mod health;
mod language;
mod process;
mod registry;
mod sandbox;
mod storage;
mod tag;
mod trace;

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Dependency, Event, EventTime, Id, Manifest, Permission,
    PluginFrame, PluginKind, PluginVersion, Record as EventRecord, SignedNanos, Source, Version,
};

use winstrata_strata_env::PLUGIN_ID as ENV_PLUGIN_ID;
use winstrata_strata_registry::PLUGIN_ID as REGISTRY_PLUGIN_ID;
use winstrata_strata_trace::PLUGIN_ID as TRACE_ID;

use crate::bus::{EventBus, Filter};
use crate::files;
use crate::network::{DEFAULT_PACKET_CAP, PacketLedger};
use crate::platform::Anchor;
use crate::schedule::SchedulerHandle;

pub use language::LanguageRequirement;
use language::{Toolchain, entry_name, language_runtime, shim_beside, supported_runtime};
use process::{Live, native_command};

const MAX_INSTALLED: usize = 256;
const MAX_RUNNING: usize = 64;
const MAX_PACKAGE_FILES: usize = 16;
const MAX_PACKAGE_FILE: u64 = 64 * 1024;
const MAX_HISTORY_LINES: usize = 1024;
const PROCESS_PLUGIN: &str = "strata.process";
const PULSE_PLUGIN: &str = "strata.pulse";
const SCRIPT_PLUGIN: &str = "strata.script";
const SERVICES_PLUGIN: &str = "strata.services";
const STARTUP_PLUGIN: &str = "strata.startup";

// 清单与采集程序共同描述计划 018。宿主不通过第三方安装写入这个身份。
const PROCESS_MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.process","name":"StrataProcess","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["process.observe"],"permissions":["event.publish","storage.private"],"#,
    r#""dependencies":[],"events":["process.started","process.exited","process.thread.started","#,
    r#""process.thread.exited","process.module.loaded","process.module.unloaded","#,
    r#""process.service","process.unresolved"],"rescue":false}"#,
);

// 清单与采集程序共同描述计划 024。宿主不通过第三方安装写入这个身份。
const PULSE_MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.pulse","name":"StrataPulse","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["pulse.observe"],"permissions":["event.publish","storage.private"],"#,
    r#""dependencies":[],"events":["pulse.sample"],"rescue":false}"#,
);

// 清单与工作台程序共同描述计划 027。宿主不通过第三方安装写入这个身份。
const SCRIPT_MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.script","name":"StrataScript","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["script.workbench"],"permissions":["event.subscribe","event.publish","#,
    r#""storage.private","history.read"],"dependencies":[],"events":["script.started","#,
    r#""script.finished","script.faulted"],"rescue":false}"#,
);

// 计划 022 的服务与驱动观察。事件名沿用插件令牌，不含下划线。
const SERVICES_MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.services","name":"StrataServices","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["services.observe"],"permissions":["event.publish","storage.private"],"#,
    r#""dependencies":[],"events":["service.state","service.created","service.deleted","#,
    r#""service.started","service.stopped","service.start-type","service.config","#,
    r#""service.failure","service.permission","driver.state","driver.loaded","#,
    r#""driver.unloaded","driver.removed"],"rescue":false}"#,
);

// 计划 023 的启动项和计划任务观察。事件名沿用插件令牌，不含下划线。
const STARTUP_MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.startup","name":"StrataStartup","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["startup.observe"],"permissions":["event.publish","storage.private"],"#,
    r#""dependencies":[],"events":["startup.state","startup.added","startup.removed","#,
    r#""startup.changed","startup.enabled","startup.disabled","startup.permission","#,
    r#""task.state","task.added","task.removed","task.changed","task.enabled","#,
    r#""task.disabled","task.permission"],"rescue":false}"#,
);

#[derive(Debug)]
pub struct PluginError {
    code: &'static str,
    detail: String,
}

impl PluginError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for PluginError {}

impl From<io::Error> for PluginError {
    fn from(error: io::Error) -> Self {
        let code = match error.kind() {
            io::ErrorKind::NotFound => "not_found",
            io::ErrorKind::PermissionDenied => "permission_denied",
            io::ErrorKind::InvalidInput => "invalid_input",
            io::ErrorKind::InvalidData => "invalid_record",
            _ => "io",
        };
        Self::new(code, error.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfficialPlugin {
    pub id: &'static str,
    pub name: &'static str,
    pub plan: u16,
    pub batch: bool,
}

pub const OFFICIAL: &[OfficialPlugin] = &[
    OfficialPlugin {
        id: "strata.pulse",
        name: "StrataPulse",
        plan: 24,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.net",
        name: "StrataNet",
        plan: 31,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.trace",
        name: "StrataTrace",
        plan: 21,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.apps",
        name: "StrataApps",
        plan: 35,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.registry",
        name: "StrataRegistry",
        plan: 19,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.env",
        name: "StrataEnv",
        plan: 20,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.process",
        name: "StrataProcess",
        plan: 18,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.services",
        name: "StrataServices",
        plan: 22,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.startup",
        name: "StrataStartup",
        plan: 23,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.security",
        name: "StrataSecurity",
        plan: 37,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.snapshot",
        name: "StrataSnapshot",
        plan: 38,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.rules",
        name: "StrataRules",
        plan: 39,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.health",
        name: "StrataHealth",
        plan: 25,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.timeline",
        name: "StrataTimeline",
        plan: 41,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.budget",
        name: "StrataBudget",
        plan: 26,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.mcp",
        name: "StrataMCP",
        plan: 45,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.rescue",
        name: "StrataRescue",
        plan: 47,
        batch: false,
    },
    OfficialPlugin {
        id: "strata.filter",
        name: "StrataFilter",
        plan: 28,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.disk",
        name: "StrataDisk",
        plan: 32,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.script",
        name: "StrataScript",
        plan: 27,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.dependency",
        name: "StrataDependency",
        plan: 34,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.trust",
        name: "StrataTrust",
        plan: 33,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.update",
        name: "StrataUpdate",
        plan: 36,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.tag",
        name: "StrataTag",
        plan: 29,
        batch: true,
    },
    OfficialPlugin {
        id: "strata.storage",
        name: "StrataStorage",
        plan: 30,
        batch: true,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginReport {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub phase: String,
    pub enabled: bool,
    pub version: Option<PluginVersion>,
    pub pid: Option<u32>,
    pub grants: Vec<String>,
    pub pending: Vec<String>,
    pub pin: Option<PluginVersion>,
    pub availability: String,
    pub batch: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependencyStatus {
    pub id: String,
    pub required: PluginVersion,
    pub range: String,
    pub found: Option<PluginVersion>,
    pub status: String,
}

struct Installed {
    id: String,
    name: String,
    kind: PluginKind,
    version: PluginVersion,
    enabled: bool,
    pin: Option<PluginVersion>,
    grants: Vec<Permission>,
    pending: Vec<Permission>,
    loaded: bool,
    availability: String,
}

#[derive(Serialize, Deserialize)]
struct StoredRegistryKey {
    hive: String,
    subkey: String,
}

fn ready_availability() -> String {
    "ready".to_owned()
}

#[derive(Serialize, Deserialize)]
struct StoredHost {
    version: Version,
    entries: Vec<StoredEntry>,
    #[serde(default)]
    registry_enabled: bool,
    #[serde(default = "ready_availability")]
    registry_availability: String,
    #[serde(default)]
    registry_scope: Vec<StoredRegistryKey>,
    #[serde(default)]
    env_enabled: bool,
    #[serde(default = "ready_availability")]
    env_availability: String,
    #[serde(default)]
    env_scope: Vec<StoredRegistryKey>,
    #[serde(default)]
    trace_enabled: bool,
    #[serde(default = "ready_availability")]
    trace_availability: String,
    #[serde(default)]
    trace_scope: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    id: String,
    name: String,
    kind: String,
    version: PluginVersion,
    enabled: bool,
    #[serde(default)]
    pin: Option<PluginVersion>,
    grants: Vec<String>,
    pending: Vec<String>,
    loaded: bool,
    availability: String,
}

pub struct PluginHost {
    root: Anchor,
    runtime: PathBuf,
    user: String,
    epoch: String,
    packets: PacketLedger,
    packet_cap: usize,
    entries: Vec<Installed>,
    live: HashMap<String, Live>,
    bus: Option<EventBus>,
    scheduler: Option<SchedulerHandle>,
    budget_paused: bool,
    budget_sequence: u64,
    budget_plugin: Option<String>,
    budget_cpu: Option<budget::CpuBase>,
    pending_budget: Option<budget::LimitUpdate>,
    starting: Vec<String>,
    languages: Option<Toolchain>,
    registry: Option<winstrata_strata_registry::Session>,
    registry_enabled: bool,
    registry_paused: bool,
    registry_availability: String,
    registry_epoch: u64,
    registry_scope: Vec<(String, String)>,
    environment: Option<winstrata_strata_env::Session>,
    env_enabled: bool,
    env_paused: bool,
    env_availability: String,
    env_epoch: u64,
    env_scope: Vec<(String, String)>,
    trace: Option<winstrata_strata_trace::Session>,
    trace_enabled: bool,
    trace_paused: bool,
    trace_availability: String,
    trace_epoch: u64,
    trace_scope: Vec<String>,
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        let ids: Vec<String> = self.live.keys().cloned().collect();
        for id in ids {
            if let Some(live) = self.live.remove(&id) {
                live.finish();
            }
            self.release_environment(&id);
        }
    }
}

impl PluginHost {
    pub(crate) fn open(dir: Anchor, runtime: &Path, user: &str) -> Result<Self, PluginError> {
        let runtime = fs::canonicalize(runtime).map_err(|error| {
            PluginError::new(
                "runtime_missing",
                format!("plugin runtime is not available: {error}"),
            )
        })?;
        if !runtime.is_file() {
            return Err(PluginError::new(
                "runtime_missing",
                "plugin runtime is not a file",
            ));
        }
        // 目录已经由调用方固定。再按路径打开会与现有共享模式冲突，Windows 返回占用错误。
        let epoch = crate::platform::fresh_id()?;
        let mut host = Self {
            root: dir,
            runtime,
            user: user.to_owned(),
            epoch,
            packets: PacketLedger::memory_only(),
            packet_cap: DEFAULT_PACKET_CAP,
            entries: Vec::new(),
            live: HashMap::new(),
            bus: None,
            scheduler: None,
            budget_paused: false,
            budget_sequence: 0,
            budget_plugin: None,
            budget_cpu: None,
            pending_budget: None,
            starting: Vec::new(),
            languages: None,
            registry: None,
            registry_enabled: false,
            registry_paused: false,
            registry_availability: ready_availability(),
            registry_epoch: 0,
            registry_scope: Vec::new(),
            environment: None,
            env_enabled: false,
            env_paused: false,
            env_availability: ready_availability(),
            env_epoch: 0,
            env_scope: Vec::new(),
            trace: None,
            trace_enabled: false,
            trace_paused: false,
            trace_availability: ready_availability(),
            trace_epoch: 0,
            trace_scope: Vec::new(),
        };
        host.read_entries()?;
        host.seed_process_collector()?;
        host.seed_script_workbench()?;
        host.seed_filter()?;
        host.seed_services_collector()?;
        host.seed_startup_collector()?;
        host.seed_pulse_collector()?;
        host.seed_health_collector()?;
        host.seed_budget_collector()?;
        host.seed_tag_collector()?;
        host.seed_storage_collector()?;
        Ok(host)
    }

    /// 把原始字节留在内存。这不会创建文件。
    pub fn stage_packet(&mut self, bytes: Vec<u8>) -> Result<u64, PluginError> {
        if bytes.is_empty() || bytes.len() > 65_536 {
            return Err(PluginError::new(
                "invalid_input",
                "packet bytes are empty or too large",
            ));
        }
        Ok(self
            .packets
            .stage(crate::network::PacketClass::Plain, bytes))
    }

    pub fn attach_bus(&mut self, bus: EventBus) {
        self.bus = Some(bus);
    }

    /// 只保存读取计数的句柄。丢弃句柄不会关闭调度器，也不改变任务队列。
    pub fn attach_scheduler(&mut self, scheduler: SchedulerHandle) {
        self.scheduler = Some(scheduler);
    }

    /// 绑定项目声明的解释器和 SDK。语言插件在绑定前不能安装。
    pub fn bind_languages(
        &mut self,
        python: &Path,
        node: &Path,
        sdk: &Path,
        requirement: &LanguageRequirement,
    ) -> Result<(), PluginError> {
        let shim = shim_beside(&self.runtime);
        self.languages = Some(Toolchain::bind(python, node, sdk, &shim, requirement)?);
        Ok(())
    }

    pub fn list(&self) -> Vec<PluginReport> {
        let mut reports: Vec<_> = OFFICIAL
            .iter()
            .map(|plugin| self.report_official(plugin))
            .collect();
        let mut extra: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| official(entry.id.as_str()).is_none())
            .map(|entry| self.report_entry(entry))
            .collect();
        extra.sort_by(|left, right| left.id.cmp(&right.id));
        reports.extend(extra);
        reports
    }

    pub fn report(&self, id: &str) -> Result<PluginReport, PluginError> {
        if let Some(plugin) = official(id) {
            return Ok(self.report_official(plugin));
        }
        let entry = self.entry(id)?;
        Ok(self.report_entry(entry))
    }

    pub fn install(&mut self, package: &Path) -> Result<PluginReport, PluginError> {
        let files = read_package(package)?;
        let manifest = manifest_from(&files)?;
        self.reject_install(&manifest)?;
        self.require_entry(&manifest, &files)?;
        if self.entries.len() >= MAX_INSTALLED {
            return Err(PluginError::new(
                "resource_limit",
                "installed plugin limit has been reached",
            ));
        }
        self.write_tree("packages", manifest.id.as_str(), &files)?;
        self.write_tree("recovery", manifest.id.as_str(), &files)?;
        self.ensure_dir("state", manifest.id.as_str())?;
        self.entries.push(Installed {
            id: manifest.id.as_str().to_owned(),
            name: manifest.name.as_str().to_owned(),
            kind: manifest.kind,
            version: manifest.plugin_version,
            enabled: false,
            pin: None,
            grants: manifest.permissions.clone(),
            pending: Vec::new(),
            loaded: false,
            availability: "ready".to_owned(),
        });
        self.save()?;
        self.audit(manifest.id.as_str(), "install", "ok", "installed")?;
        self.report(manifest.id.as_str())
    }

    pub fn load(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        self.absent(id)?;
        self.ensure_dependencies(id, false)?;
        let entry = self.entry_mut(id)?;
        entry.loaded = true;
        entry.availability = "ready".to_owned();
        self.save()?;
        self.audit(id, "load", "ok", "loaded")?;
        self.report(id)
    }

    pub fn enable(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        if id == REGISTRY_PLUGIN_ID {
            return self.enable_registry();
        }
        if id == ENV_PLUGIN_ID {
            return self.enable_environment();
        }
        if id == TRACE_ID {
            return self.enable_trace();
        }
        self.absent(id)?;
        {
            let entry = self.entry_mut(id)?;
            entry.enabled = true;
        }
        self.save()?;
        match self.start(id) {
            Ok(report) => {
                self.audit(id, "enable", "ok", "running")?;
                Ok(report)
            }
            Err(error) => {
                let _ = self.audit(id, "enable", "error", error.code());
                Err(error)
            }
        }
    }

    pub fn start(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        if id == REGISTRY_PLUGIN_ID {
            return self.start_registry();
        }
        if id == ENV_PLUGIN_ID {
            return self.start_environment();
        }
        if id == TRACE_ID {
            return self.start_trace();
        }
        self.absent(id)?;
        if self.live.contains_key(id) {
            return self.report(id);
        }
        if !self.entry(id)?.enabled {
            return Err(PluginError::new("disabled", "plugin is disabled"));
        }
        if self.starting.iter().any(|item| item == id) {
            return Err(PluginError::new(
                "dependency_cycle",
                "plugin dependencies cycle",
            ));
        }
        self.starting.push(id.to_owned());
        let started = self.start_unlocked(id);
        self.starting.pop();
        started?;
        self.report(id)
    }

    pub fn pause(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        if id == REGISTRY_PLUGIN_ID {
            return self.pause_registry();
        }
        if id == ENV_PLUGIN_ID {
            return self.pause_environment();
        }
        if id == TRACE_ID {
            return self.pause_trace();
        }
        self.roundtrip(id, "pause")?;
        self.drop_subscriptions(id);
        if let Some(live) = self.live.get_mut(id) {
            live.paused = true;
        }
        self.audit(id, "pause", "ok", "paused")?;
        self.report(id)
    }

    pub fn resume(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        if id == REGISTRY_PLUGIN_ID {
            return self.resume_registry();
        }
        if id == ENV_PLUGIN_ID {
            return self.resume_environment();
        }
        if id == TRACE_ID {
            return self.resume_trace();
        }
        self.roundtrip(id, "resume")?;
        self.subscribe(id)?;
        if let Some(live) = self.live.get_mut(id) {
            live.paused = false;
        }
        self.audit(id, "resume", "ok", "running")?;
        self.report(id)
    }

    pub fn stop(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        if id == REGISTRY_PLUGIN_ID {
            return self.stop_registry();
        }
        if id == ENV_PLUGIN_ID {
            return self.stop_environment();
        }
        if id == TRACE_ID {
            return self.stop_trace();
        }
        self.finish_process(id, "stop", "ready")?;
        self.audit(id, "stop", "ok", "stopped")?;
        self.report(id)
    }

    pub fn disable(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        if id == REGISTRY_PLUGIN_ID {
            return self.disable_registry();
        }
        if id == ENV_PLUGIN_ID {
            return self.disable_environment();
        }
        if id == TRACE_ID {
            return self.disable_trace();
        }
        self.absent(id)?;
        if self.live.contains_key(id) {
            self.finish_process(id, "disable", "ready")?;
        }
        let entry = self.entry_mut(id)?;
        entry.enabled = false;
        entry.loaded = false;
        entry.availability = "ready".to_owned();
        self.save()?;
        self.audit(id, "disable", "ok", "disabled")?;
        self.report(id)
    }

    pub fn update(&mut self, id: &str, package: &Path) -> Result<PluginReport, PluginError> {
        self.absent(id)?;
        let files = read_package(package)?;
        let manifest = manifest_from(&files)?;
        self.require_entry(&manifest, &files)?;
        self.prepare_update(id, &manifest)?;
        self.write_tree("staging", id, &files)?;
        if self.live.contains_key(id) {
            match self.probe_staged(&manifest) {
                Ok(candidate) => {
                    let old = self.live.remove(id).expect("running plugin");
                    let code = Self::stop_live(old, "stop");
                    if code != 0 {
                        candidate.finish();
                        let _ = self.remove_tree("staging", id);
                        let _ = self.audit(id, "update", "error", "unhealthy");
                        return Err(PluginError::new(
                            "unhealthy",
                            format!("previous plugin exited with status {code}"),
                        ));
                    }
                    if let Err(error) = self.write_tree("packages", id, &files) {
                        candidate.finish();
                        let _ = self.restore_package(id);
                        let _ = self.remove_tree("staging", id);
                        return Err(error);
                    }
                    let _ = self.write_tree("recovery", id, &files);
                    if let Err(error) = self.adopt(id, &manifest, candidate) {
                        let _ = self.remove_tree("staging", id);
                        return Err(error);
                    }
                }
                Err(error) => {
                    let _ = self.remove_tree("staging", id);
                    let _ = self.audit(id, "update", "error", error.code());
                    return Err(error);
                }
            }
        } else {
            self.write_tree("packages", id, &files)?;
            self.write_tree("recovery", id, &files)?;
            self.apply_manifest(id, &manifest)?;
        }
        let _ = self.remove_tree("staging", id);
        self.audit(id, "update", "ok", &manifest.plugin_version.to_string())?;
        self.report(id)
    }

    pub fn fault(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        self.running(id)?;
        let live = self.live.remove(id).expect("running plugin");
        let code = Self::stop_live(live, "fault");
        self.entry_mut(id)?.availability = "faulted".to_owned();
        self.save()?;
        self.audit(id, "fault", "ok", &format!("exit={code}"))?;
        if code != 7 {
            return Err(PluginError::new(
                "unhealthy",
                format!("plugin fault exited with status {code}"),
            ));
        }
        self.report(id)
    }

    pub fn ping(&mut self, id: &str) -> Result<(u32, PluginVersion), PluginError> {
        let reply = self.roundtrip(id, "ping")?;
        let (pid, version) = self
            .live
            .get(id)
            .map(|live| (live.pid(), live.version))
            .ok_or_else(|| PluginError::new("not_running", "plugin is not running"))?;
        if reply.ok == Some(true) && reply.pid == Some(pid) && reply.plugin_version == Some(version)
        {
            Ok((pid, version))
        } else {
            Err(PluginError::new(
                "untrusted_peer",
                "plugin process identity did not match the host",
            ))
        }
    }

    pub fn pin(&mut self, id: &str, version: PluginVersion) -> Result<(), PluginError> {
        self.absent(id)?;
        let entry = self.entry_mut(id)?;
        if entry.version != version {
            return Err(PluginError::new(
                "pinned",
                "pin must match the installed version",
            ));
        }
        entry.pin = Some(version);
        self.save()?;
        self.audit(id, "pin", "ok", &version.to_string())?;
        Ok(())
    }

    pub fn unpin(&mut self, id: &str) -> Result<(), PluginError> {
        self.absent(id)?;
        self.entry_mut(id)?.pin = None;
        self.save()?;
        self.audit(id, "unpin", "ok", "unpinned")?;
        Ok(())
    }

    pub fn grant(&mut self, id: &str, permission: &str) -> Result<(), PluginError> {
        self.absent(id)?;
        let permission = Permission::parse(permission).map_err(|_| {
            PluginError::new("undeclared_permission", "permission is not recognized")
        })?;
        let entry = self.entry_mut(id)?;
        if !entry.pending.contains(&permission) {
            return Err(PluginError::new(
                "undeclared_permission",
                "permission was not requested by the installed manifest",
            ));
        }
        entry.pending.retain(|item| *item != permission);
        if !entry.grants.contains(&permission) {
            entry.grants.push(permission);
        }
        self.save()?;
        self.audit(id, "grant", "ok", permission.as_str())?;
        Ok(())
    }

    pub fn revoke(&mut self, id: &str, permission: &str) -> Result<(), PluginError> {
        self.absent(id)?;
        let permission = Permission::parse(permission).map_err(|_| {
            PluginError::new("undeclared_permission", "permission is not recognized")
        })?;
        {
            let entry = self.entry_mut(id)?;
            if !entry.grants.contains(&permission) {
                return Err(PluginError::new(
                    "permission_denied",
                    "permission is not granted",
                ));
            }
            entry.grants.retain(|item| *item != permission);
        }
        let declared = self
            .manifest_file(id)
            .ok()
            .is_some_and(|manifest| manifest.permissions.contains(&permission));
        if declared {
            let entry = self.entry_mut(id)?;
            if !entry.pending.contains(&permission) {
                entry.pending.push(permission);
            }
        }
        self.save()?;
        if permission == Permission::EventSubscribe {
            self.drop_subscriptions(id);
        }
        self.audit(id, "revoke", "ok", permission.as_str())?;
        Ok(())
    }

    pub fn uninstall(&mut self, id: &str) -> Result<(), PluginError> {
        if official(id).is_some() {
            return Err(PluginError::new(
                "builtin_retained",
                "built-in plugins stay installed",
            ));
        }
        if self.entry(id).is_err() {
            return Err(PluginError::new("not_installed", "plugin is not installed"));
        }
        if self.live.contains_key(id) {
            self.finish_process(id, "stop", "ready")?;
        }
        self.remove_tree("packages", id)?;
        self.remove_tree("recovery", id)?;
        self.remove_tree("state", id)?;
        self.remove_tree("staging", id)?;
        self.entries.retain(|entry| entry.id != id);
        self.save()?;
        self.audit(id, "uninstall", "ok", "uninstalled")?;
        Ok(())
    }

    pub fn recover(&mut self, id: &str) -> Result<PluginReport, PluginError> {
        self.absent(id)?;
        let live_ok = self.manifest_file(id).is_ok();
        if live_ok && self.entry(id)?.availability != "faulted" {
            return self.report(id);
        }
        let files = self.read_tree("recovery", id).map_err(|_| {
            PluginError::new(
                "dependency_unrecoverable",
                "no healthy package copy is available",
            )
        })?;
        let manifest = manifest_from(&files).map_err(|_| {
            PluginError::new(
                "dependency_unrecoverable",
                "the saved package copy is not valid",
            )
        })?;
        if manifest.id.as_str() != id {
            return Err(PluginError::new(
                "dependency_unrecoverable",
                "the saved package copy is a different plugin",
            ));
        }
        self.write_tree("packages", id, &files)?;
        {
            let entry = self.entry_mut(id)?;
            entry.availability = "ready".to_owned();
            entry.version = manifest.plugin_version;
            entry.name = manifest.name.as_str().to_owned();
        }
        self.save()?;
        if self.entry(id)?.enabled {
            self.start(id)?;
        }
        self.audit(id, "recover", "ok", "restored")?;
        self.report(id)
    }

    pub fn call(
        &mut self,
        id: &str,
        capability: &str,
        payload: &str,
    ) -> Result<String, PluginError> {
        if capability == "filter.apply" {
            return self.filter_call(id, payload);
        }
        self.running(id)?;
        let provider = self.resolve_provider(id, capability)?;
        if provider == tag::PLUGIN && capability == "tag.apply" {
            let (actor, actor_id) = tag::actor_for(id, &self.user);
            return self.tag_apply(actor, &actor_id, payload);
        }
        if provider == tag::PLUGIN && capability == "tag.policy" {
            let (actor, actor_id) = tag::actor_for(id, &self.user);
            return self.tag_policy(actor, &actor_id, payload);
        }
        let mut frame = PluginFrame::request("invoke");
        frame.capability = Some(capability.to_owned());
        frame.payload = Some(self.invoke_payload(&provider, capability, payload)?);
        let reply = self.exchange(&provider, &frame)?;
        if reply.pid != Some(self.live.get(&provider).map(Live::pid).unwrap_or(0)) {
            return Err(PluginError::new(
                "untrusted_peer",
                "plugin process identity did not match the host",
            ));
        }
        if reply.ok != Some(true) {
            return Err(PluginError::new(
                static_code(reply.code.as_deref()),
                reply
                    .detail
                    .unwrap_or_else(|| "plugin rejected the call".to_owned()),
            ));
        }
        let detail = reply
            .detail
            .ok_or_else(|| PluginError::new("invalid_record", "plugin returned no result"))?;
        if provider == budget::PLUGIN && capability == "budget.regulate" {
            self.apply_budget(&detail)?;
        }
        if provider == SCRIPT_PLUGIN && capability == "script.workbench" {
            if let Some(done) = self.forward_script_tag(&detail)? {
                return Ok(done);
            }
            if let Some(done) = self.forward_script_storage(&detail)? {
                return Ok(done);
            }
        }
        Ok(detail)
    }

    pub fn read_private(&mut self, id: &str, name: &str) -> Result<String, PluginError> {
        self.running(id)?;
        self.require_grant(id, Permission::StoragePrivate)?;
        let mut frame = PluginFrame::request("read_state");
        frame.name = Some(name.to_owned());
        let reply = self.exchange(id, &frame)?;
        if reply.ok != Some(true) {
            return Err(PluginError::new(
                static_code(reply.code.as_deref()),
                reply
                    .detail
                    .unwrap_or_else(|| "plugin rejected the read".to_owned()),
            ));
        }
        reply
            .detail
            .ok_or_else(|| PluginError::new("missing", "plugin file is absent"))
    }

    pub fn emit(&mut self, id: &str, event_type: &str) -> Result<Event, PluginError> {
        self.running(id)?;
        if self.budget_paused {
            return Err(PluginError::new(
                "budget_paused",
                "resource budget has paused publication",
            ));
        }
        self.require_grant(id, Permission::EventPublish)?;
        let manifest = self.manifest_file(id)?;
        if !manifest
            .events
            .iter()
            .any(|item| item.as_str() == event_type)
        {
            return Err(PluginError::new(
                "undeclared_permission",
                "event type is not declared",
            ));
        }
        let mut frame = PluginFrame::request("emit");
        frame.event_type = Some(event_type.to_owned());
        let reply = self.exchange(id, &frame)?;
        let pid = self.live.get(id).map(Live::pid).unwrap_or(0);
        if reply.ok != Some(true) || reply.pid != Some(pid) {
            return Err(PluginError::new(
                "untrusted_peer",
                "plugin process identity did not match the host",
            ));
        }
        let bus = self
            .bus
            .clone()
            .ok_or_else(|| PluginError::new("bus_unavailable", "plugin host has no event bus"))?;
        let sequence = {
            let live = self.live.get_mut(id).expect("running plugin");
            live.sequence += 1;
            live.sequence
        };
        let event = plugin_event(&self.epoch, id, sequence, event_type, pid)?;
        bus.publish(event.clone())
            .map_err(|error| PluginError::new(error.error.code(), error.error.detail()))?;
        Ok(event)
    }

    pub fn poll(&mut self, id: &str) -> Result<Vec<std::sync::Arc<Event>>, PluginError> {
        self.running(id)?;
        self.require_grant(id, Permission::EventSubscribe)?;
        let live = self.live.get(id).expect("running plugin");
        let mut events = Vec::new();
        for subscription in &live.subscriptions {
            events.extend(
                subscription
                    .recv_batch(64, std::time::Duration::from_millis(1))
                    .map_err(|error| PluginError::new(error.code(), error.detail()))?,
            );
        }
        Ok(events)
    }

    pub fn dependencies(&self, id: &str) -> Result<Vec<DependencyStatus>, PluginError> {
        self.absent(id)?;
        let manifest = self.manifest_file(id)?;
        Ok(manifest
            .dependencies
            .iter()
            .map(|dependency| self.dependency_status(dependency))
            .collect())
    }

    pub fn history(&self, id: &str) -> Result<Vec<String>, PluginError> {
        let path = self.history_path(id)?;
        match fs::read_to_string(&path) {
            Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn enable_batch(&mut self) -> Result<Vec<String>, PluginError> {
        let ids: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| {
                entry.enabled && batch_allowed(&entry.id) && !self.live.contains_key(&entry.id)
            })
            .map(|entry| entry.id.clone())
            .collect();
        let mut started = Vec::new();
        for id in ids {
            self.start(&id)?;
            started.push(id);
        }
        if self.registry_enabled && self.registry.is_none() && self.registry_availability == "ready"
        {
            self.start_registry()?;
            started.push(REGISTRY_PLUGIN_ID.to_owned());
        }
        if self.env_enabled && self.environment.is_none() && self.env_availability == "ready" {
            self.start_environment()?;
            started.push(ENV_PLUGIN_ID.to_owned());
        }
        if self.trace_enabled && self.trace.is_none() && self.trace_availability == "ready" {
            self.start_trace()?;
            started.push(TRACE_ID.to_owned());
        }
        Ok(started)
    }

    pub fn subscriber_count(&self) -> usize {
        self.bus
            .as_ref()
            .map(|bus| bus.stats().subscribers)
            .unwrap_or(0)
    }

    fn start_unlocked(&mut self, id: &str) -> Result<(), PluginError> {
        if self.live.len() >= MAX_RUNNING {
            return Err(PluginError::new(
                "resource_limit",
                "running plugin limit has been reached",
            ));
        }
        self.ensure_dependencies(id, true)?;
        let manifest = self.manifest_file(id)?;
        self.capability_free(&manifest)?;
        if manifest.events.iter().any(|_| true)
            && manifest.permissions.contains(&Permission::EventSubscribe)
            && self.bus.is_none()
        {
            return Err(PluginError::new(
                "bus_unavailable",
                "plugin host has no event bus",
            ));
        }
        let manifest_path = self.member_path("packages", id, "manifest.json")?;
        let state_path = self.tree_path("state", id)?;
        let mut live = self.spawn_plugin(&manifest, &manifest_path, &state_path)?;
        let reply = match live.exchange(&PluginFrame::request("ping")) {
            Ok(reply) => reply,
            Err(error) => {
                let code = live.wait_exit().unwrap_or(1);
                live.finish();
                let availability = if code == 3 { "unavailable" } else { "faulted" };
                self.entry_mut(id)?.availability = availability.to_owned();
                self.save()?;
                return Err(if code == 3 {
                    PluginError::new("unavailable", "plugin declined to start")
                } else {
                    error
                });
            }
        };
        if reply.ok != Some(true) || reply.pid != Some(live.pid()) {
            let code = live.wait_exit().unwrap_or(1);
            live.finish();
            self.entry_mut(id)?.availability = "unavailable".to_owned();
            self.save()?;
            return Err(PluginError::new(
                if code == 3 {
                    "unavailable"
                } else {
                    "untrusted_peer"
                },
                "plugin did not complete its startup handshake",
            ));
        }
        self.live.insert(id.to_owned(), live);
        if let Err(error) = self.subscribe(id) {
            self.finish_process(id, "stop", "ready")?;
            return Err(error);
        }
        let entry = self.entry_mut(id)?;
        entry.loaded = true;
        entry.availability = "ready".to_owned();
        entry.version = manifest.plugin_version;
        self.save()?;
        Ok(())
    }

    fn probe_staged(&self, manifest: &Manifest) -> Result<Live, PluginError> {
        let manifest_path = self.member_path("staging", manifest.id.as_str(), "manifest.json")?;
        let state_path = self.tree_path("state", manifest.id.as_str())?;
        let mut live = self.spawn_plugin(manifest, &manifest_path, &state_path)?;
        match live.exchange(&PluginFrame::request("ping")) {
            Ok(reply) if reply.ok == Some(true) && reply.pid == Some(live.pid()) => Ok(live),
            Ok(_) | Err(_) => {
                let code = live.wait_exit().unwrap_or(1);
                live.finish();
                Err(if code == 3 {
                    PluginError::new("unavailable", "replacement plugin declined to start")
                } else {
                    PluginError::new(
                        "unhealthy",
                        "replacement plugin failed its startup handshake",
                    )
                })
            }
        }
    }

    fn prepare_update(&self, id: &str, manifest: &Manifest) -> Result<(), PluginError> {
        if manifest.id.as_str() != id {
            return Err(PluginError::new(
                "invalid_manifest",
                "update package is a different plugin",
            ));
        }
        if !supported_runtime(manifest.runtime.as_str()) {
            return Err(PluginError::new(
                "unsupported_runtime",
                "plugin runtime is not supported",
            ));
        }
        let installed = self.manifest_file(id)?;
        if installed.runtime.as_str() != manifest.runtime.as_str() {
            return Err(PluginError::new(
                "invalid_manifest",
                "plugin runtime cannot change in an update",
            ));
        }
        if language_runtime(manifest.runtime.as_str()) && self.languages.is_none() {
            return Err(PluginError::new(
                "toolchain_missing",
                "language toolchain is not bound",
            ));
        }
        let entry = self.entry(id)?;
        if manifest.plugin_version == entry.version {
            return Err(PluginError::new(
                "same_version",
                "plugin version did not change",
            ));
        }
        if manifest.plugin_version < entry.version {
            return Err(PluginError::new(
                "incompatible_version",
                "plugin version is older than the installed version",
            ));
        }
        if entry.kind != manifest.kind {
            return Err(PluginError::new(
                "invalid_manifest",
                "plugin kind cannot change in an update",
            ));
        }
        if entry.pin.is_some_and(|pin| pin != manifest.plugin_version) {
            return Err(PluginError::new("pinned", "installed version is pinned"));
        }
        self.dependents_accept(id, manifest.plugin_version)?;
        for dependency in &manifest.dependencies {
            let status = self.dependency_status(dependency);
            if status.status == "missing" {
                return Err(PluginError::new(
                    "dependency_missing",
                    format!("dependency {} is not installed", dependency.id.as_str()),
                ));
            }
            if status.status == "incompatible" {
                return Err(PluginError::new(
                    "dependency_incompatible",
                    format!(
                        "dependency {} does not satisfy the required version",
                        dependency.id.as_str()
                    ),
                ));
            }
        }
        Ok(())
    }

    fn adopt(&mut self, id: &str, manifest: &Manifest, live: Live) -> Result<(), PluginError> {
        self.live.insert(id.to_owned(), live);
        self.subscribe(id)?;
        self.apply_manifest(id, manifest)
    }

    fn apply_manifest(&mut self, id: &str, manifest: &Manifest) -> Result<(), PluginError> {
        self.sync_permissions(id, manifest)?;
        let entry = self.entry_mut(id)?;
        entry.version = manifest.plugin_version;
        entry.name = manifest.name.as_str().to_owned();
        entry.availability = "ready".to_owned();
        self.save()
    }

    fn sync_permissions(&mut self, id: &str, manifest: &Manifest) -> Result<(), PluginError> {
        let entry = self.entry_mut(id)?;
        entry
            .grants
            .retain(|grant| manifest.permissions.contains(grant));
        entry
            .pending
            .retain(|grant| manifest.permissions.contains(grant));
        for permission in &manifest.permissions {
            if !entry.grants.contains(permission) && !entry.pending.contains(permission) {
                entry.pending.push(*permission);
            }
        }
        Ok(())
    }

    fn finish_process(
        &mut self,
        id: &str,
        op: &str,
        availability: &str,
    ) -> Result<(), PluginError> {
        let Some(live) = self.live.remove(id) else {
            return Ok(());
        };
        let code = Self::stop_live(live, op);
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.availability = availability.to_owned();
        }
        self.save()?;
        self.release_environment(id);
        if code != 0 {
            return Err(PluginError::new(
                "unhealthy",
                format!("plugin exited with status {code}"),
            ));
        }
        Ok(())
    }

    fn release_environment(&self, id: &str) {
        let Ok(state) = self.tree_path("state", id) else {
            return;
        };
        let environment = state.join("env");
        if environment.exists() {
            let _ = fs::remove_dir_all(environment);
        }
    }

    fn require_entry(
        &self,
        manifest: &Manifest,
        files: &[(String, Vec<u8>)],
    ) -> Result<(), PluginError> {
        if let Some(entry) = entry_name(manifest.runtime.as_str()) {
            if !files.iter().any(|(name, _)| name == entry) {
                return Err(PluginError::new(
                    "invalid_manifest",
                    "language package is missing its entry",
                ));
            }
        }
        Ok(())
    }

    fn spawn_plugin(
        &self,
        manifest: &Manifest,
        manifest_path: &Path,
        state: &Path,
    ) -> Result<Live, PluginError> {
        if manifest.runtime.as_str() == "native" {
            let program = if manifest.id.as_str() == PROCESS_PLUGIN {
                collector_beside(&self.runtime, "winstrata-strata-process").ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "process collector is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == SCRIPT_PLUGIN {
                collector_beside(&self.runtime, "winstrata-strata-script").ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "script workbench is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == SERVICES_PLUGIN {
                collector_beside(&self.runtime, "winstrata-strata-services").ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "services collector is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == STARTUP_PLUGIN {
                collector_beside(&self.runtime, "winstrata-strata-startup").ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "startup collector is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == PULSE_PLUGIN {
                pulse_beside(&self.runtime).ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "pulse collector is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == health::PLUGIN {
                health::beside(&self.runtime).ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "health collector is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == budget::PLUGIN {
                budget::beside(&self.runtime).ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "budget collector is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == filter::PLUGIN {
                filter::beside(&self.runtime).ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "filter program is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == tag::PLUGIN {
                tag::beside(&self.runtime).ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "tag program is not beside the plugin runtime",
                    )
                })?
            } else if manifest.id.as_str() == storage::PLUGIN {
                storage::beside(&self.runtime).ok_or_else(|| {
                    PluginError::new(
                        "runtime_missing",
                        "storage program is not beside the plugin runtime",
                    )
                })?
            } else {
                self.runtime.clone()
            };
            let mut command = native_command(&program, manifest_path, state);
            if manifest.id.as_str() == SCRIPT_PLUGIN {
                self.pass_script_toolchain(&mut command);
            }
            return Live::spawn(command, manifest.plugin_version);
        }
        let tools = self.languages.as_ref().ok_or_else(|| {
            PluginError::new("toolchain_missing", "language toolchain is not bound")
        })?;
        language::launch(tools, manifest, manifest_path, state, self.root.io_path())
    }

    fn stop_live(mut live: Live, op: &str) -> i32 {
        let frame = PluginFrame::request(op);
        let _ = live.exchange(&frame);
        let code = live.wait_exit().unwrap_or(1);
        live.finish();
        code
    }

    fn roundtrip(&mut self, id: &str, op: &str) -> Result<PluginFrame, PluginError> {
        self.running(id)?;
        self.exchange(id, &PluginFrame::request(op))
    }

    fn exchange(&mut self, id: &str, frame: &PluginFrame) -> Result<PluginFrame, PluginError> {
        let live = self
            .live
            .get_mut(id)
            .ok_or_else(|| PluginError::new("not_running", "plugin process is not running"))?;
        live.exchange(frame)
    }

    fn subscribe(&mut self, id: &str) -> Result<(), PluginError> {
        self.drop_subscriptions(id);
        let Some(bus) = self.bus.clone() else {
            return Ok(());
        };
        let manifest = self.manifest_file(id)?;
        if !self.granted(id, Permission::EventSubscribe) {
            return Ok(());
        }
        let mut subscriptions = Vec::new();
        for event_type in &manifest.events {
            let filter = Filter {
                device: None,
                provider: None,
                event_type: Some(event_type.clone()),
            };
            subscriptions.push(
                bus.subscribe(1, filter)
                    .map_err(|error| PluginError::new(error.code(), error.detail()))?,
            );
        }
        if let Some(live) = self.live.get_mut(id) {
            live.subscriptions = subscriptions;
        }
        Ok(())
    }

    fn drop_subscriptions(&mut self, id: &str) {
        if let Some(live) = self.live.get_mut(id) {
            live.subscriptions.clear();
        }
    }

    fn ensure_dependencies(&mut self, id: &str, start_enabled: bool) -> Result<(), PluginError> {
        let dependencies = self.manifest_file(id)?.dependencies;
        for dependency in dependencies {
            let status = self.dependency_status(&dependency);
            if status.status == "missing" {
                return Err(PluginError::new(
                    "dependency_missing",
                    format!("dependency {} is not installed", dependency.id.as_str()),
                ));
            }
            if status.status == "incompatible" {
                return Err(PluginError::new(
                    "dependency_incompatible",
                    format!(
                        "dependency {} does not satisfy the required version",
                        dependency.id.as_str()
                    ),
                ));
            }
            if !start_enabled {
                continue;
            }
            if status.status != "ready" {
                return Err(PluginError::new(
                    "dependency_unhealthy",
                    format!("dependency {} is not healthy", dependency.id.as_str()),
                ));
            }
            let dep_id = dependency.id.as_str().to_owned();
            if !self.live.contains_key(&dep_id) {
                self.start(&dep_id)?;
            }
        }
        Ok(())
    }

    fn dependency_status(&self, dependency: &Dependency) -> DependencyStatus {
        let found = self
            .entries
            .iter()
            .find(|entry| entry.id == dependency.id.as_str());
        let (found_version, status) = match found {
            None => (None, "missing"),
            Some(entry) if !dependency.matches(entry.version) => {
                (Some(entry.version), "incompatible")
            }
            Some(entry)
                if !entry.enabled
                    || entry.availability == "faulted"
                    || entry.availability == "unavailable" =>
            {
                (Some(entry.version), "unhealthy")
            }
            Some(entry) => (Some(entry.version), "ready"),
        };
        DependencyStatus {
            id: dependency.id.as_str().to_owned(),
            required: dependency.version,
            range: dependency.range.as_str().to_owned(),
            found: found_version,
            status: status.to_owned(),
        }
    }

    fn dependents_accept(&self, id: &str, version: PluginVersion) -> Result<(), PluginError> {
        for entry in &self.entries {
            if entry.id == id {
                continue;
            }
            let manifest = match self.manifest_file(&entry.id) {
                Ok(manifest) => manifest,
                Err(_) => continue,
            };
            for dependency in &manifest.dependencies {
                if dependency.id.as_str() == id && !dependency.matches(version) {
                    return Err(PluginError::new(
                        "dependency_incompatible",
                        format!("{} requires a different version", entry.id),
                    ));
                }
            }
        }
        Ok(())
    }

    fn resolve_provider(&self, caller: &str, capability: &str) -> Result<String, PluginError> {
        let caller_manifest = self.manifest_file(caller)?;
        if caller_manifest
            .capabilities
            .iter()
            .any(|item| item.as_str() == capability)
        {
            return Ok(caller.to_owned());
        }
        self.require_grant(caller, Permission::PluginCall)?;
        let mut provider = None;
        for id in self.live.keys() {
            let manifest = self.manifest_file(id)?;
            if manifest
                .capabilities
                .iter()
                .any(|item| item.as_str() == capability)
            {
                if provider.is_some() {
                    return Err(PluginError::new(
                        "capability_conflict",
                        "more than one running plugin provides this capability",
                    ));
                }
                provider = Some(id.clone());
            }
        }
        if let Some(id) = provider {
            return Ok(id);
        }
        for entry in &self.entries {
            let Ok(manifest) = self.manifest_file(&entry.id) else {
                continue;
            };
            if manifest
                .capabilities
                .iter()
                .any(|item| item.as_str() == capability)
            {
                return Err(PluginError::new(
                    "dependency_unhealthy",
                    "the providing plugin is not running",
                ));
            }
        }
        Err(PluginError::new(
            "dependency_missing",
            "no installed plugin provides this capability",
        ))
    }

    fn capability_free(&self, manifest: &Manifest) -> Result<(), PluginError> {
        for capability in &manifest.capabilities {
            for id in self.live.keys() {
                if id == manifest.id.as_str() {
                    continue;
                }
                let other = self.manifest_file(id)?;
                if other.capabilities.iter().any(|item| item == capability) {
                    return Err(PluginError::new(
                        "capability_conflict",
                        format!("{} is already provided", capability.as_str()),
                    ));
                }
            }
        }
        Ok(())
    }

    fn reject_install(&self, manifest: &Manifest) -> Result<(), PluginError> {
        if !supported_runtime(manifest.runtime.as_str()) {
            return Err(PluginError::new(
                "unsupported_runtime",
                "plugin runtime is not supported",
            ));
        }
        if language_runtime(manifest.runtime.as_str()) && self.languages.is_none() {
            return Err(PluginError::new(
                "toolchain_missing",
                "language toolchain is not bound",
            ));
        }
        if manifest.kind != PluginKind::ThirdParty {
            return Err(PluginError::new(
                "builtin_closed",
                "built-in implementations are not installed by this package",
            ));
        }
        if official(manifest.id.as_str()).is_some() {
            return Err(PluginError::new(
                "builtin_closed",
                "official plugin ids stay in the catalog",
            ));
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.id == manifest.id.as_str())
        {
            return Err(PluginError::new(
                "duplicate_plugin",
                "plugin is already installed",
            ));
        }
        Ok(())
    }

    fn restore_package(&mut self, id: &str) -> Result<(), PluginError> {
        let files = self.read_tree("recovery", id)?;
        self.write_tree("packages", id, &files)
    }

    fn running(&self, id: &str) -> Result<(), PluginError> {
        self.absent(id)?;
        if self.live.contains_key(id) {
            Ok(())
        } else {
            Err(PluginError::new(
                "not_running",
                "plugin process is not running",
            ))
        }
    }

    fn require_grant(&self, id: &str, permission: Permission) -> Result<(), PluginError> {
        if self.granted(id, permission) {
            Ok(())
        } else {
            Err(PluginError::new(
                "permission_denied",
                "permission is not granted",
            ))
        }
    }

    fn granted(&self, id: &str, permission: Permission) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .is_some_and(|entry| entry.grants.contains(&permission))
    }

    fn absent(&self, id: &str) -> Result<(), PluginError> {
        if id == REGISTRY_PLUGIN_ID {
            return Err(PluginError::new(
                "builtin_closed",
                "strata.registry runs in process and has no package",
            ));
        }
        if id == ENV_PLUGIN_ID {
            return Err(PluginError::new(
                "builtin_closed",
                "strata.env runs in process and has no package",
            ));
        }
        if id == TRACE_ID {
            return Err(PluginError::new(
                "builtin_closed",
                "strata.trace runs in process and has no package",
            ));
        }
        if official(id).is_some() && self.entry(id).is_err() {
            return Err(PluginError::new(
                "implementation_absent",
                "official plugin implementation is not part of this plan",
            ));
        }
        self.entry(id).map(|_| ())
    }

    fn entry(&self, id: &str) -> Result<&Installed, PluginError> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| PluginError::new("not_installed", "plugin is not installed"))
    }

    fn entry_mut(&mut self, id: &str) -> Result<&mut Installed, PluginError> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| PluginError::new("not_installed", "plugin is not installed"))
    }

    fn manifest_file(&self, id: &str) -> Result<Manifest, PluginError> {
        let path = self.member_path("packages", id, "manifest.json")?;
        let text = fs::read_to_string(path)?;
        Manifest::from_json(&text).map_err(manifest_error)
    }

    fn report_official(&self, plugin: &OfficialPlugin) -> PluginReport {
        if plugin.id == REGISTRY_PLUGIN_ID {
            return self.report_registry();
        }
        if plugin.id == ENV_PLUGIN_ID {
            return self.report_environment();
        }
        if plugin.id == TRACE_ID {
            return self.report_trace();
        }
        if let Ok(entry) = self.entry(plugin.id) {
            let mut report = self.report_entry(entry);
            report.batch = plugin.batch;
            report.kind = "builtin".to_owned();
            return report;
        }
        PluginReport {
            id: plugin.id.to_owned(),
            name: plugin.name.to_owned(),
            kind: "builtin".to_owned(),
            phase: "implementation_absent".to_owned(),
            enabled: false,
            version: None,
            pid: None,
            grants: Vec::new(),
            pending: Vec::new(),
            pin: None,
            availability: "implementation_absent".to_owned(),
            batch: plugin.batch,
        }
    }

    fn report_entry(&self, entry: &Installed) -> PluginReport {
        let phase = if self.live.get(&entry.id).is_some_and(|live| live.paused) {
            "paused"
        } else if self.live.contains_key(&entry.id) {
            "running"
        } else if entry.availability == "faulted" {
            "faulted"
        } else if entry.availability == "unavailable" {
            "unavailable"
        } else if entry.loaded && !entry.enabled {
            "loaded"
        } else if entry.enabled {
            "stopped"
        } else {
            "disabled"
        };
        PluginReport {
            id: entry.id.clone(),
            name: entry.name.clone(),
            kind: entry.kind.as_str().to_owned(),
            phase: phase.to_owned(),
            enabled: entry.enabled,
            version: Some(entry.version),
            pid: self.live.get(&entry.id).map(Live::pid),
            grants: entry
                .grants
                .iter()
                .map(|item| item.as_str().to_owned())
                .collect(),
            pending: entry
                .pending
                .iter()
                .map(|item| item.as_str().to_owned())
                .collect(),
            pin: entry.pin,
            availability: entry.availability.clone(),
            batch: batch_allowed(&entry.id),
        }
    }

    fn read_entries(&mut self) -> Result<(), PluginError> {
        let anchor = self.anchor();
        let stored = match files::read_json::<StoredHost>(&anchor, "host.json") {
            Ok(stored) => stored,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.save()?;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        if stored.version != Version::CURRENT {
            return Err(PluginError::new(
                "unsupported_version",
                "plugin host state version is not supported",
            ));
        }
        for entry in stored.entries {
            self.entries.push(Installed {
                id: entry.id,
                name: entry.name,
                kind: match entry.kind.as_str() {
                    "builtin" => PluginKind::Builtin,
                    "third_party" => PluginKind::ThirdParty,
                    _ => {
                        return Err(PluginError::new(
                            "invalid_record",
                            "stored plugin kind is not recognized",
                        ));
                    }
                },
                version: entry.version,
                enabled: entry.enabled,
                pin: entry.pin,
                grants: parse_permissions(&entry.grants)?,
                pending: parse_permissions(&entry.pending)?,
                loaded: entry.loaded,
                availability: entry.availability,
            });
        }
        self.registry_enabled = stored.registry_enabled;
        self.registry_availability = if matches!(
            stored.registry_availability.as_str(),
            "ready" | "unsupported" | "collector_fault" | "collector_os"
        ) {
            stored.registry_availability
        } else {
            ready_availability()
        };
        if !stored.registry_scope.is_empty() {
            let owned: Vec<(String, String)> = stored
                .registry_scope
                .iter()
                .map(|key| (key.hive.clone(), key.subkey.clone()))
                .collect();
            let pairs: Vec<(&str, &str)> = owned
                .iter()
                .map(|(hive, subkey)| (hive.as_str(), subkey.as_str()))
                .collect();
            winstrata_strata_registry::Scope::from_pairs(&pairs)
                .map_err(|error| PluginError::new("invalid_record", error.detail()))?;
            self.registry_scope = owned;
        }
        self.env_enabled = stored.env_enabled;
        self.env_availability = if matches!(
            stored.env_availability.as_str(),
            "ready" | "unsupported" | "collector_fault" | "collector_os"
        ) {
            stored.env_availability
        } else {
            ready_availability()
        };
        if !stored.env_scope.is_empty() {
            let owned: Vec<(String, String)> = stored
                .env_scope
                .iter()
                .map(|key| (key.hive.clone(), key.subkey.clone()))
                .collect();
            let pairs: Vec<(&str, &str)> = owned
                .iter()
                .map(|(hive, subkey)| (hive.as_str(), subkey.as_str()))
                .collect();
            winstrata_strata_env::Scope::from_pairs(&pairs)
                .map_err(|error| PluginError::new("invalid_record", error.detail()))?;
            self.env_scope = owned;
        }
        self.trace_enabled = stored.trace_enabled;
        self.trace_availability = if matches!(
            stored.trace_availability.as_str(),
            "ready" | "unsupported" | "collector_fault" | "collector_os"
        ) {
            stored.trace_availability
        } else {
            ready_availability()
        };
        if !stored.trace_scope.is_empty() {
            let paths: Vec<PathBuf> = stored.trace_scope.iter().map(PathBuf::from).collect();
            // 无效范围只记记录错误。被拒绝的路径不进入错误文本。
            winstrata_strata_trace::Scope::from_paths(&paths)
                .map_err(|error| PluginError::new("invalid_record", error.detail()))?;
            self.trace_scope = stored.trace_scope;
        }
        Ok(())
    }

    fn seed_process_collector(&mut self) -> Result<(), PluginError> {
        self.seed_builtin(
            PROCESS_PLUGIN,
            PROCESS_MANIFEST,
            "winstrata-strata-process",
            "process collector manifest is not the official plugin",
        )
    }

    fn seed_services_collector(&mut self) -> Result<(), PluginError> {
        self.seed_builtin(
            SERVICES_PLUGIN,
            SERVICES_MANIFEST,
            "winstrata-strata-services",
            "services collector manifest is not the official plugin",
        )
    }

    fn seed_startup_collector(&mut self) -> Result<(), PluginError> {
        self.seed_builtin(
            STARTUP_PLUGIN,
            STARTUP_MANIFEST,
            "winstrata-strata-startup",
            "startup collector manifest is not the official plugin",
        )
    }

    fn seed_builtin(
        &mut self,
        id: &str,
        manifest_json: &str,
        stem: &str,
        mismatch: &str,
    ) -> Result<(), PluginError> {
        if collector_beside(&self.runtime, stem).is_none() || self.entry(id).is_ok() {
            return Ok(());
        }
        let manifest = Manifest::from_json(manifest_json).map_err(manifest_error)?;
        if manifest.id.as_str() != id || manifest.kind != PluginKind::Builtin {
            return Err(PluginError::new("invalid_manifest", mismatch));
        }
        let files = vec![(
            "manifest.json".to_owned(),
            manifest_json.as_bytes().to_vec(),
        )];
        self.write_tree("packages", id, &files)?;
        self.write_tree("recovery", id, &files)?;
        self.ensure_dir("state", id)?;
        self.entries.push(Installed {
            id: id.to_owned(),
            name: manifest.name.as_str().to_owned(),
            kind: PluginKind::Builtin,
            version: manifest.plugin_version,
            enabled: false,
            pin: None,
            grants: manifest.permissions.clone(),
            pending: Vec::new(),
            loaded: false,
            availability: "ready".to_owned(),
        });
        self.save()?;
        self.audit(id, "seed", "ok", "present")?;
        Ok(())
    }

    fn seed_pulse_collector(&mut self) -> Result<(), PluginError> {
        if pulse_beside(&self.runtime).is_none() || self.entry(PULSE_PLUGIN).is_ok() {
            return Ok(());
        }
        let manifest = Manifest::from_json(PULSE_MANIFEST).map_err(manifest_error)?;
        if manifest.id.as_str() != PULSE_PLUGIN || manifest.kind != PluginKind::Builtin {
            return Err(PluginError::new(
                "invalid_manifest",
                "pulse collector manifest is not the official plugin",
            ));
        }
        let files = vec![(
            "manifest.json".to_owned(),
            PULSE_MANIFEST.as_bytes().to_vec(),
        )];
        self.write_tree("packages", PULSE_PLUGIN, &files)?;
        self.write_tree("recovery", PULSE_PLUGIN, &files)?;
        self.ensure_dir("state", PULSE_PLUGIN)?;
        self.entries.push(Installed {
            id: PULSE_PLUGIN.to_owned(),
            name: manifest.name.as_str().to_owned(),
            kind: PluginKind::Builtin,
            version: manifest.plugin_version,
            enabled: false,
            pin: None,
            grants: manifest.permissions.clone(),
            pending: Vec::new(),
            loaded: false,
            availability: "ready".to_owned(),
        });
        self.save()?;
        self.audit(PULSE_PLUGIN, "seed", "ok", "present")?;
        Ok(())
    }

    fn seed_health_collector(&mut self) -> Result<(), PluginError> {
        if health::beside(&self.runtime).is_none() || self.entry(health::PLUGIN).is_ok() {
            return Ok(());
        }
        let manifest = Manifest::from_json(health::MANIFEST).map_err(manifest_error)?;
        if manifest.id.as_str() != health::PLUGIN || manifest.kind != PluginKind::Builtin {
            return Err(PluginError::new(
                "invalid_manifest",
                "health collector manifest is not the official plugin",
            ));
        }
        let files = vec![(
            "manifest.json".to_owned(),
            health::MANIFEST.as_bytes().to_vec(),
        )];
        self.write_tree("packages", health::PLUGIN, &files)?;
        self.write_tree("recovery", health::PLUGIN, &files)?;
        self.ensure_dir("state", health::PLUGIN)?;
        self.entries.push(Installed {
            id: health::PLUGIN.to_owned(),
            name: manifest.name.as_str().to_owned(),
            kind: PluginKind::Builtin,
            version: manifest.plugin_version,
            enabled: false,
            pin: None,
            grants: manifest.permissions.clone(),
            pending: Vec::new(),
            loaded: false,
            availability: "ready".to_owned(),
        });
        self.save()?;
        self.audit(health::PLUGIN, "seed", "ok", "present")?;
        Ok(())
    }

    fn seed_script_workbench(&mut self) -> Result<(), PluginError> {
        if collector_beside(&self.runtime, "winstrata-strata-script").is_none()
            || self.entry(SCRIPT_PLUGIN).is_ok()
        {
            return Ok(());
        }
        let manifest = Manifest::from_json(SCRIPT_MANIFEST).map_err(manifest_error)?;
        if manifest.id.as_str() != SCRIPT_PLUGIN || manifest.kind != PluginKind::Builtin {
            return Err(PluginError::new(
                "invalid_manifest",
                "script workbench manifest is not the official plugin",
            ));
        }
        let files = vec![(
            "manifest.json".to_owned(),
            SCRIPT_MANIFEST.as_bytes().to_vec(),
        )];
        self.write_tree("packages", SCRIPT_PLUGIN, &files)?;
        self.write_tree("recovery", SCRIPT_PLUGIN, &files)?;
        self.ensure_dir("state", SCRIPT_PLUGIN)?;
        self.entries.push(Installed {
            id: SCRIPT_PLUGIN.to_owned(),
            name: manifest.name.as_str().to_owned(),
            kind: PluginKind::Builtin,
            version: manifest.plugin_version,
            enabled: false,
            pin: None,
            grants: manifest.permissions.clone(),
            pending: Vec::new(),
            loaded: false,
            availability: "ready".to_owned(),
        });
        self.save()?;
        self.audit(SCRIPT_PLUGIN, "seed", "ok", "present")?;
        Ok(())
    }

    fn pass_script_toolchain(&self, command: &mut std::process::Command) {
        if let Some(tools) = &self.languages {
            command.arg("--python").arg(&tools.python);
            command.arg("--python-prefix").arg(&tools.python_prefix);
            command.arg("--node").arg(&tools.node);
            command.arg("--node-prefix").arg(&tools.node_prefix);
        }
        let shim = shim_beside(&self.runtime);
        if shim.is_file() {
            command.arg("--shim").arg(shim);
        }
    }

    fn seed_filter(&mut self) -> Result<(), PluginError> {
        self.seed_builtin(
            filter::PLUGIN,
            winstrata_strata_filter::MANIFEST,
            "winstrata-strata-filter",
            "filter manifest is not the official plugin",
        )
    }

    /// 插件未启用时仍执行基础保护。不写批量文件，也不启动过滤进程。
    pub fn protect_observation(&self, payload: &str) -> Result<bool, PluginError> {
        winstrata_strata_filter::protect_payload(payload)
            .map_err(|_| PluginError::new("rejected", "observation was rejected"))
    }

    fn filter_call(&mut self, id: &str, payload: &str) -> Result<String, PluginError> {
        self.running(id)?;
        let provider = self.resolve_provider(id, "filter.apply")?;
        if provider != filter::PLUGIN {
            return Err(PluginError::new(
                "rejected",
                "filter.apply is provided by strata.filter",
            ));
        }
        let detail = self.invoke_raw(&provider, "filter.apply", payload)?;
        let Some(request) = filter::script_request(&detail) else {
            return Ok(detail);
        };
        let staged =
            fs::read_to_string(self.tree_path("state", filter::PLUGIN)?.join("staged.txt"))
                .map_err(|_| PluginError::new("missing", "filter batch was not staged"))?;
        let staged_lines = staged.lines().filter(|line| !line.is_empty()).count();
        if staged_lines != request.count {
            return self.invoke_raw(
                &provider,
                "filter.apply",
                &filter::fallback_payload("staged_mismatch"),
            );
        }
        match self.script_batch(&request.id, &staged) {
            Ok(lines) if lines.len() == request.count => {
                let payload = filter::verdict_payload(&lines)?;
                self.invoke_raw(&provider, "filter.apply", &payload)
            }
            Ok(_) => self.invoke_raw(
                &provider,
                "filter.apply",
                &filter::fallback_payload("verdict_mismatch"),
            ),
            Err(code) => {
                self.invoke_raw(&provider, "filter.apply", &filter::fallback_payload(code))
            }
        }
    }

    fn script_batch(&mut self, id: &str, staged: &str) -> Result<Vec<String>, &'static str> {
        if !self.live.contains_key(SCRIPT_PLUGIN) {
            return Err("script_unavailable");
        }
        let private = self
            .tree_path("state", SCRIPT_PLUGIN)
            .map_err(|_| "faulted")?
            .join("private");
        fs::create_dir_all(&private).map_err(|_| "faulted")?;
        fs::write(private.join("batch.txt"), staged).map_err(|_| "faulted")?;
        let payload = format!(r#"{{"op":"run","id":"{id}"}}"#);
        let detail = self
            .call(SCRIPT_PLUGIN, "script.workbench", &payload)
            .map_err(|error| error.code())?;
        let text = filter::decode_output(&detail).ok_or("faulted")?;
        filter::verdict_lines(&text).ok_or("verdict_mismatch")
    }

    fn invoke_raw(
        &mut self,
        provider: &str,
        capability: &str,
        payload: &str,
    ) -> Result<String, PluginError> {
        if payload.len() > 1024 || payload.chars().any(char::is_control) {
            return Err(PluginError::new(
                "invalid_record",
                "filter payload does not fit a plugin frame",
            ));
        }
        let mut frame = PluginFrame::request("invoke");
        frame.capability = Some(capability.to_owned());
        frame.payload = Some(payload.to_owned());
        let reply = self.exchange(provider, &frame)?;
        if reply.pid != Some(self.live.get(provider).map(Live::pid).unwrap_or(0)) {
            return Err(PluginError::new(
                "untrusted_peer",
                "plugin process identity did not match the host",
            ));
        }
        if reply.ok != Some(true) {
            return Err(PluginError::new(
                static_code(reply.code.as_deref()),
                reply
                    .detail
                    .unwrap_or_else(|| "plugin rejected the call".to_owned()),
            ));
        }
        reply
            .detail
            .ok_or_else(|| PluginError::new("invalid_record", "plugin returned no result"))
    }

    fn seed_budget_collector(&mut self) -> Result<(), PluginError> {
        self.seed_builtin(
            budget::PLUGIN,
            budget::MANIFEST,
            "winstrata-strata-budget",
            "budget collector manifest is not the official plugin",
        )
    }

    fn seed_storage_collector(&mut self) -> Result<(), PluginError> {
        self.seed_builtin(
            storage::PLUGIN,
            storage::MANIFEST,
            "winstrata-strata-storage",
            "storage manifest is not the official plugin",
        )
    }

    fn seed_tag_collector(&mut self) -> Result<(), PluginError> {
        self.seed_builtin(
            tag::PLUGIN,
            tag::MANIFEST,
            "winstrata-strata-tag",
            "tag manifest is not the official plugin",
        )
    }

    fn tag_apply(
        &mut self,
        actor: &str,
        actor_id: &str,
        payload: &str,
    ) -> Result<String, PluginError> {
        self.running(tag::PLUGIN)?;
        let paused = self.live.get(tag::PLUGIN).is_some_and(|live| live.paused);
        let capture = tag_capture(payload)
            .map(|id| self.capture_path(id))
            .transpose()?;
        let prepared = tag::prepare_apply(
            actor,
            actor_id,
            payload,
            paused,
            unix_ns(),
            &mut self.packets,
            capture,
        )?;
        let detail = self.invoke_tag("tag.apply", &prepared.request, prepared.path.as_deref())?;
        self.publish_tag(&detail)?;
        Ok(detail)
    }

    fn tag_policy(
        &mut self,
        actor: &str,
        actor_id: &str,
        payload: &str,
    ) -> Result<String, PluginError> {
        self.running(tag::PLUGIN)?;
        let prepared = tag::prepare_policy(actor, actor_id, payload, unix_ns(), self.packet_cap)?;
        let detail = self.invoke_tag("tag.policy", &prepared.request, None)?;
        if let Some((policy, cap)) = prepared.policy {
            self.packets.set_policy(policy);
            if cap != self.packet_cap {
                self.packets.set_cap(cap);
                self.packet_cap = cap;
            }
        }
        Ok(detail)
    }

    fn invoke_tag(
        &mut self,
        capability: &str,
        payload: &str,
        rollback: Option<&std::path::Path>,
    ) -> Result<String, PluginError> {
        let mut frame = PluginFrame::request("invoke");
        frame.capability = Some(capability.to_owned());
        frame.payload = Some(payload.to_owned());
        let reply = match self.exchange(tag::PLUGIN, &frame) {
            Ok(reply) => reply,
            Err(error) => {
                if let Some(path) = rollback {
                    let _ = fs::remove_file(path);
                }
                return Err(error);
            }
        };
        let pid = self.live.get(tag::PLUGIN).map(Live::pid).unwrap_or(0);
        if reply.pid != Some(pid) || reply.ok != Some(true) {
            if let Some(path) = rollback {
                let _ = fs::remove_file(path);
            }
            return Err(PluginError::new(
                static_code(reply.code.as_deref()),
                reply
                    .detail
                    .unwrap_or_else(|| "plugin rejected the call".to_owned()),
            ));
        }
        reply
            .detail
            .ok_or_else(|| PluginError::new("invalid_record", "plugin returned no result"))
    }

    fn forward_script_tag(&mut self, detail: &str) -> Result<Option<String>, PluginError> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(detail) else {
            return Ok(None);
        };
        if value.get("code").and_then(serde_json::Value::as_str) != Some("tag_request") {
            return Ok(None);
        }
        let script = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !self.live.contains_key(tag::PLUGIN) {
            return Ok(Some(
                serde_json::json!({
                    "ok": false,
                    "code": "tag_unavailable",
                    "id": script,
                    "safety": "none"
                })
                .to_string(),
            ));
        }
        let payload = serde_json::json!({
            "kind": value.get("kind").and_then(serde_json::Value::as_str).unwrap_or(""),
            "subject": value.get("subject").and_then(serde_json::Value::as_str).unwrap_or(""),
            "packet": value.get("packet").and_then(serde_json::Value::as_u64).unwrap_or(0),
            "reason": value.get("reason").and_then(serde_json::Value::as_str).unwrap_or(""),
            "source": value.get("source").and_then(serde_json::Value::as_str).unwrap_or(""),
            "confidence": value.get("confidence").and_then(serde_json::Value::as_str).unwrap_or(""),
            "evidence": value.get("evidence").and_then(serde_json::Value::as_str).unwrap_or(""),
            "attribution": value.get("attribution").and_then(serde_json::Value::as_str).unwrap_or(""),
        })
        .to_string();
        if payload.len() > 1024 || payload.chars().any(char::is_control) {
            return Err(PluginError::new(
                "invalid_record",
                "script tag does not fit a plugin frame",
            ));
        }
        Ok(Some(self.tag_apply("script", script, &payload)?))
    }

    fn forward_script_storage(&mut self, detail: &str) -> Result<Option<String>, PluginError> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(detail) else {
            return Ok(None);
        };
        if value.get("code").and_then(serde_json::Value::as_str) != Some("storage_request") {
            return Ok(None);
        }
        if value.get("confirm").and_then(serde_json::Value::as_bool) == Some(true) {
            return Ok(Some(
                serde_json::json!({"ok": true, "code": "confirmation_required", "deleted": false})
                    .to_string(),
            ));
        }
        let action = value
            .get("action")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let capability = match action {
            "configure" => "storage.configure",
            "decline" => "storage.decline",
            "propose" => "storage.propose",
            "status" => "storage.status",
            _ => {
                return Ok(Some(
                    serde_json::json!({"ok": false, "code": "rejected", "deleted": false})
                        .to_string(),
                ));
            }
        };
        if !self.live.contains_key(storage::PLUGIN) {
            return Ok(Some(
                serde_json::json!({"ok": false, "code": "not_running", "deleted": false})
                    .to_string(),
            ));
        }
        let payload = serde_json::json!({
            "op": action,
            "capture_first": value.get("capture_first").cloned().unwrap_or(serde_json::Value::Null),
            "capture_repeat": value.get("capture_repeat").cloned().unwrap_or(serde_json::Value::Null),
            "log_first": value.get("log_first").cloned().unwrap_or(serde_json::Value::Null),
            "log_repeat": value.get("log_repeat").cloned().unwrap_or(serde_json::Value::Null),
            "category": value.get("category").cloned().unwrap_or(serde_json::Value::Null),
            "scope": value.get("scope").cloned().unwrap_or(serde_json::Value::Null),
            "confirm": false
        })
        .to_string();
        if payload.len() > 1024 || payload.chars().any(char::is_control) {
            return Err(PluginError::new(
                "invalid_record",
                "script storage request does not fit a plugin frame",
            ));
        }
        let mut frame = PluginFrame::request("invoke");
        frame.capability = Some(capability.to_owned());
        frame.payload = Some(payload);
        let reply = self.exchange(storage::PLUGIN, &frame)?;
        let pid = self.live.get(storage::PLUGIN).map(Live::pid).unwrap_or(0);
        if reply.pid != Some(pid) || reply.ok != Some(true) {
            return Err(PluginError::new(
                static_code(reply.code.as_deref()),
                reply
                    .detail
                    .unwrap_or_else(|| "storage plugin rejected the call".to_owned()),
            ));
        }
        reply
            .detail
            .map(Some)
            .ok_or_else(|| PluginError::new("invalid_record", "storage plugin returned no result"))
    }

    fn publish_tag(&mut self, detail: &str) -> Result<(), PluginError> {
        let value: serde_json::Value = serde_json::from_str(detail)
            .map_err(|_| PluginError::new("invalid_record", "tag result is not json"))?;
        let code = value
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let Some(event_type) = tag::event_type(code) else {
            return Ok(());
        };
        let Some(bus) = self.bus.clone() else {
            return Ok(());
        };
        let sequence =
            {
                let live = self.live.get_mut(tag::PLUGIN).ok_or_else(|| {
                    PluginError::new("not_running", "plugin process is not running")
                })?;
                let next = live.sequence.checked_add(1).ok_or_else(|| {
                    PluginError::new("resource_limit", "tag sequence is exhausted")
                })?;
                live.sequence = next;
                next
            };
        let event = tag::observation(&self.epoch, sequence, event_type, &value)?;
        bus.publish(event)
            .map_err(|error| PluginError::new(error.error.code(), error.error.detail()))?;
        Ok(())
    }

    fn capture_path(&self, id: u64) -> Result<PathBuf, PluginError> {
        let dir = self.tree_path("state", tag::PLUGIN)?.join("captures");
        fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("p{id}.bin")))
    }

    fn invoke_payload(
        &mut self,
        provider: &str,
        capability: &str,
        payload: &str,
    ) -> Result<String, PluginError> {
        if provider == health::PLUGIN && capability == "health.observe" {
            let text = health::snapshot_text(
                unix_ns(),
                self.plugin_faults(),
                self.bus.as_ref().map(EventBus::stats),
                self.scheduler.as_ref().map(SchedulerHandle::stats),
            );
            if text.len() > 1024 || text.chars().any(char::is_control) {
                return Err(PluginError::new(
                    "invalid_record",
                    "health snapshot does not fit a plugin frame",
                ));
            }
            return Ok(text);
        }
        if provider == budget::PLUGIN && capability == "budget.regulate" {
            return self.budget_payload(payload);
        }
        Ok(payload.to_owned())
    }

    fn budget_payload(&mut self, caller: &str) -> Result<String, PluginError> {
        let update = budget::parse_limits(caller)?;
        let (cpu, base) = budget::sample_cpu(self.budget_cpu.as_ref());
        self.budget_cpu = base;
        let plugin_queued = self.plugin_queue_depth(&update);
        let pressure = budget::Pressure {
            rss_kib: budget::rss_kib(),
            cpu,
            queued: self.bus.as_ref().map(|bus| count_u64(bus.stats().queued)),
            lost: self.bus.as_ref().map(|bus| bus.stats().lost),
            sched_queued: self
                .scheduler
                .as_ref()
                .map(|scheduler| count_u64(scheduler.stats().queued)),
            plugin_queued,
        };
        let text = budget::payload(&update, &pressure)?;
        self.pending_budget = Some(update);
        Ok(text)
    }

    fn plugin_queue_depth(&self, update: &budget::LimitUpdate) -> Option<u64> {
        let id = match &update.plugin {
            budget::PluginLimit::Clear => return None,
            budget::PluginLimit::Set { id, .. } => id.as_str(),
            budget::PluginLimit::Keep => self.budget_plugin.as_deref()?,
        };
        let live = self.live.get(id)?;
        let queued = live
            .subscriptions
            .iter()
            .map(|subscription| subscription.backlog().0)
            .sum::<usize>();
        Some(count_u64(queued))
    }

    fn apply_budget(&mut self, detail: &str) -> Result<(), PluginError> {
        let decision = budget::parse_decision(detail)?;
        let update = self.pending_budget.take();
        match decision.action {
            budget::Action::Keep => Ok(()),
            budget::Action::Hold | budget::Action::Resume => {
                self.remember_budget_plugin(update.as_ref());
                self.budget_paused = false;
                Ok(())
            }
            budget::Action::Pause => {
                if decision.loss {
                    self.publish_budget_loss()?;
                }
                self.remember_budget_plugin(update.as_ref());
                self.budget_paused = true;
                Ok(())
            }
        }
    }

    fn remember_budget_plugin(&mut self, update: Option<&budget::LimitUpdate>) {
        let Some(update) = update else {
            return;
        };
        match &update.plugin {
            budget::PluginLimit::Keep => {}
            budget::PluginLimit::Clear => self.budget_plugin = None,
            budget::PluginLimit::Set { id, .. } => self.budget_plugin = Some(id.clone()),
        }
    }

    fn publish_budget_loss(&mut self) -> Result<(), PluginError> {
        let bus = self
            .bus
            .clone()
            .ok_or_else(|| PluginError::new("bus_unavailable", "plugin host has no event bus"))?;
        let sequence = self.budget_sequence.checked_add(1).ok_or_else(|| {
            PluginError::new("resource_limit", "budget loss sequence is exhausted")
        })?;
        let event = budget::loss_event(&self.epoch, sequence)?;
        bus.publish(event)
            .map_err(|error| PluginError::new(error.error.code(), error.error.detail()))?;
        self.budget_sequence = sequence;
        Ok(())
    }

    fn plugin_faults(&self) -> u64 {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.availability.as_str(), "faulted" | "unavailable"))
            .count() as u64
    }
    fn save(&self) -> Result<(), PluginError> {
        let stored = StoredHost {
            version: Version::CURRENT,
            entries: self
                .entries
                .iter()
                .map(|entry| StoredEntry {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    kind: entry.kind.as_str().to_owned(),
                    version: entry.version,
                    enabled: entry.enabled,
                    pin: entry.pin,
                    grants: entry
                        .grants
                        .iter()
                        .map(|item| item.as_str().to_owned())
                        .collect(),
                    pending: entry
                        .pending
                        .iter()
                        .map(|item| item.as_str().to_owned())
                        .collect(),
                    loaded: entry.loaded,
                    availability: entry.availability.clone(),
                })
                .collect(),
            registry_enabled: self.registry_enabled,
            registry_availability: self.registry_availability.clone(),
            registry_scope: self
                .registry_scope
                .iter()
                .map(|(hive, subkey)| StoredRegistryKey {
                    hive: hive.clone(),
                    subkey: subkey.clone(),
                })
                .collect(),
            env_enabled: self.env_enabled,
            env_availability: self.env_availability.clone(),
            env_scope: self
                .env_scope
                .iter()
                .map(|(hive, subkey)| StoredRegistryKey {
                    hive: hive.clone(),
                    subkey: subkey.clone(),
                })
                .collect(),
            trace_enabled: self.trace_enabled,
            trace_availability: self.trace_availability.clone(),
            trace_scope: self.trace_scope.clone(),
        };
        files::write_json(self.anchor(), "host.json", &stored)?;
        Ok(())
    }

    fn audit(&self, id: &str, action: &str, result: &str, code: &str) -> Result<(), PluginError> {
        let line = serde_json::json!({
            "unix_ns": unix_ns().to_string(),
            "plugin": id,
            "action": action,
            "result": result,
            "code": code,
        });
        let text = serde_json::to_string(&line)
            .map_err(|error| PluginError::new("invalid_record", error.to_string()))?;
        append_line(&self.history_path(id)?, &text)
    }

    fn history_path(&self, id: &str) -> Result<PathBuf, PluginError> {
        if !safe_id(id) {
            return Err(PluginError::new(
                "invalid_input",
                "plugin id is not a safe token",
            ));
        }
        let dir = self
            .anchor()
            .child(std::ffi::OsStr::new("history"), true, true)?;
        // 句柄路径在目录关闭后失效，历史文件必须落在真实目录上。
        Ok(fs::canonicalize(dir.io_path())?.join(format!("{id}.jsonl")))
    }

    fn anchor(&self) -> &Anchor {
        &self.root
    }

    fn ensure_dir(&self, tree: &str, id: &str) -> Result<Anchor, PluginError> {
        if !safe_id(id) || !safe_tree(tree) {
            return Err(PluginError::new("invalid_input", "plugin path is not safe"));
        }
        let parent = self
            .anchor()
            .child(std::ffi::OsStr::new(tree), true, true)?;
        Ok(parent.child(std::ffi::OsStr::new(id), true, true)?)
    }

    fn tree_path(&self, tree: &str, id: &str) -> Result<PathBuf, PluginError> {
        Ok(fs::canonicalize(self.ensure_dir(tree, id)?.io_path())?)
    }

    fn member_path(&self, tree: &str, id: &str, name: &str) -> Result<PathBuf, PluginError> {
        if !allowed_file(name) {
            return Err(PluginError::new(
                "invalid_package",
                "package file name is not allowed",
            ));
        }
        Ok(self.tree_path(tree, id)?.join(name))
    }

    fn write_tree(
        &self,
        tree: &str,
        id: &str,
        files: &[(String, Vec<u8>)],
    ) -> Result<(), PluginError> {
        let dir = fs::canonicalize(self.ensure_dir(tree, id)?.io_path())?;
        if dir.exists() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let metadata = fs::symlink_metadata(entry.path())?;
                if metadata.file_type().is_file() {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        for (name, bytes) in files {
            if !allowed_file(name) {
                return Err(PluginError::new(
                    "invalid_package",
                    "package file name is not allowed",
                ));
            }
            write_private(&dir.join(name), bytes)?;
        }
        Ok(())
    }

    fn read_tree(&self, tree: &str, id: &str) -> Result<Vec<(String, Vec<u8>)>, PluginError> {
        let dir = self.tree_path(tree, id)?;
        read_package(&dir)
    }

    fn remove_tree(&self, tree: &str, id: &str) -> Result<(), PluginError> {
        if !safe_id(id) || !safe_tree(tree) {
            return Err(PluginError::new("invalid_input", "plugin path is not safe"));
        }
        let parent = self
            .anchor()
            .child(std::ffi::OsStr::new(tree), true, true)?;
        let path = fs::canonicalize(parent.io_path())?.join(id);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }
}

fn tag_capture(payload: &str) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    if value.get("kind").and_then(serde_json::Value::as_str) != Some("packet") {
        return None;
    }
    match value.get("packet").and_then(serde_json::Value::as_u64) {
        Some(id) if id > 0 => Some(id),
        _ => None,
    }
}

fn static_code(code: Option<&str>) -> &'static str {
    match code {
        Some("ok") => "ok",
        Some("rejected") => "rejected",
        Some("missing") => "missing",
        Some("unavailable") => "unavailable",
        Some("faulted") => "faulted",
        Some("disabled") => "disabled",
        Some("timeout") => "timeout",
        Some("toolchain_missing") => "toolchain_missing",
        Some("isolation_unavailable") => "isolation_unavailable",
        Some("preset_present") => "preset_present",
        Some("permission_denied") => "permission_denied",
        Some("pending_script") => "pending_script",
        Some("dependency_missing") => "dependency_missing",
        _ => "rejected",
    }
}

fn collector_beside(runtime: &Path, stem: &str) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    };
    let path = runtime.parent()?.join(name);
    path.is_file().then_some(path)
}

fn pulse_beside(runtime: &Path) -> Option<PathBuf> {
    collector_beside(runtime, "winstrata-strata-pulse")
}

fn official(id: &str) -> Option<&'static OfficialPlugin> {
    OFFICIAL.iter().find(|plugin| plugin.id == id)
}

fn batch_allowed(id: &str) -> bool {
    official(id).is_none_or(|plugin| plugin.batch)
}

fn parse_permissions(values: &[String]) -> Result<Vec<Permission>, PluginError> {
    values
        .iter()
        .map(|value| {
            Permission::parse(value).map_err(|_| {
                PluginError::new("invalid_record", "stored permission is not recognized")
            })
        })
        .collect()
}

fn manifest_from(files: &[(String, Vec<u8>)]) -> Result<Manifest, PluginError> {
    let bytes = files
        .iter()
        .find(|(name, _)| name == "manifest.json")
        .map(|(_, bytes)| bytes.as_slice())
        .ok_or_else(|| PluginError::new("invalid_manifest", "manifest.json is missing"))?;
    let text = std::str::from_utf8(bytes)
        .map_err(|_| PluginError::new("invalid_manifest", "manifest.json is not UTF-8"))?;
    match Manifest::from_json(text) {
        Ok(manifest) => Ok(manifest),
        Err(_) if claims_rescue(text) => Err(PluginError::new(
            "rescue_isolated",
            "rescue authority is isolated to strata.rescue",
        )),
        Err(error) => Err(manifest_error(error)),
    }
}

/// 协议层会折叠自定义诊断，宿主仍要单独识别越权的救援标记。
fn claims_rescue(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    value.get("rescue").and_then(serde_json::Value::as_bool) == Some(true)
        && value.get("id").and_then(serde_json::Value::as_str) != Some("strata.rescue")
}

fn manifest_error(error: winstrata_protocols::ProtocolError) -> PluginError {
    let code = if error.to_string().contains("rescue authority") {
        "rescue_isolated"
    } else if error.code.as_str() == "unsupported_version" {
        "unsupported_version"
    } else {
        "invalid_manifest"
    };
    PluginError::new(code, error.to_string())
}

fn read_package(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, PluginError> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).map_err(|error| {
        PluginError::new(
            "invalid_package",
            format!("package directory cannot be read: {error}"),
        )
    })?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(PluginError::new(
                "invalid_package",
                "package file name is not UTF-8",
            ));
        };
        if !allowed_file(name) {
            return Err(PluginError::new(
                "invalid_package",
                "package file name is not allowed",
            ));
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.file_type().is_file() {
            return Err(PluginError::new(
                "invalid_package",
                "package entries must be regular files",
            ));
        }
        if metadata.len() > MAX_PACKAGE_FILE {
            return Err(PluginError::new(
                "invalid_package",
                "package file is larger than 64 KiB",
            ));
        }
        files.push((name.to_owned(), fs::read(entry.path())?));
        if files.len() > MAX_PACKAGE_FILES {
            return Err(PluginError::new(
                "invalid_package",
                "package has too many files",
            ));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn allowed_file(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=48).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b'-')
        })
        && !name.contains("..")
        && !name.starts_with('.')
}

fn safe_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'.' | b'-')
        })
        && !id.contains("..")
        && !id.ends_with('.')
}

fn safe_tree(tree: &str) -> bool {
    matches!(
        tree,
        "packages" | "recovery" | "state" | "staging" | "history"
    )
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), PluginError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)?;
    Ok(())
}

fn append_line(path: &Path, line: &str) -> Result<(), PluginError> {
    let mut existing = match fs::read_to_string(path) {
        Ok(text) => text.lines().map(str::to_owned).collect(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    existing.push(line.to_owned());
    if existing.len() > MAX_HISTORY_LINES {
        let skip = existing.len() - MAX_HISTORY_LINES;
        existing.drain(0..skip);
    }
    let mut body = existing.join("\n");
    body.push('\n');
    write_private(path, body.as_bytes())
}

fn count_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn unix_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn plugin_event(
    epoch: &str,
    plugin: &str,
    sequence: u64,
    event_type: &str,
    pid: u32,
) -> Result<Event, PluginError> {
    let evidence = Id::new(format!("pid={pid}"))
        .map_err(|error| PluginError::new("invalid_record", error.to_string()))?;
    Ok(Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new("local-plugin-host").map_err(manifest_error)?,
            provider: Id::new(plugin).map_err(manifest_error)?,
            epoch: Id::new(epoch).map_err(manifest_error)?,
        },
        sequence: Counter::new(sequence).map_err(manifest_error)?,
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns()),
            precision_ns: None,
        },
        record: EventRecord::Observation {
            event_type: Id::new(event_type).map_err(manifest_error)?,
            subject: Some(winstrata_protocols::ObjectRef {
                kind: Id::new("plugin").map_err(manifest_error)?,
                id: Id::new(plugin).map_err(manifest_error)?,
            }),
            user: None,
            process: None,
            attribution: Attribution::Observed { evidence },
            completeness: Completeness::Complete,
        },
    })
}

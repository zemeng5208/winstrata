//! 官方界面使用的引导和模式命令。
//!
//! 页面只能提交这些选择。订阅、释放、启用、禁用和救援操作仍被桥接拒绝。
//! 历史和停止会询问服务；没有订阅时不调用释放，也不把未发生的卸载写成成功。

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use winstrata_core::{CollectionScope, Core, Onboarding};
use winstrata_service::{CoreClient, Driver, Endpoint, GuiClient};

use crate::observe::describe;

pub enum FollowUp {
    None,
    ReleaseDrivers,
}

pub fn ensure<'a>(slot: &'a mut Option<Core>, locator: &Path) -> io::Result<&'a mut Core> {
    if slot.is_none() {
        *slot = Some(Core::open_at(locator, None)?);
    }
    Ok(slot.as_mut().expect("core session"))
}

pub fn view(
    slot: &mut Option<Core>,
    locator: &Path,
    endpoint: Option<&Endpoint>,
    drivers: &str,
) -> Value {
    let core = match ensure(slot, locator) {
        Ok(core) => core,
        Err(error) => return unavailable(&error.to_string(), endpoint, drivers),
    };
    let observed = describe(core, endpoint);
    let onboarding = core.onboarding().unwrap_or_default();
    let settings = core.settings().ok();
    let (markers, markers_note) = match core.markers() {
        Ok(markers) => (
            markers
                .into_iter()
                .map(|marker| {
                    json!({
                        "sequence": marker.sequence,
                        "activity": marker.activity,
                        "reason": marker.reason,
                        "releaseDrivers": marker.release_drivers,
                    })
                })
                .collect::<Vec<_>>(),
            Value::Null,
        ),
        Err(error) => (Vec::new(), Value::String(error.to_string())),
    };
    json!({
        "user": observed.user,
        "storage": observed.storage,
        "run": observed.run,
        "service": observed.service,
        "plugins": observed.plugins.iter().map(|plugin| json!({
            "id": plugin.id,
            "name": plugin.name,
            "availability": plugin.availability,
            "enabled": plugin.enabled,
        })).collect::<Vec<_>>(),
        "privilege": observed.privilege,
        "paused": core.paused(),
        "collectors": core.collector_note(),
        "drivers": drivers,
        "onboarding": onboarding_json(&onboarding),
        "markers": markers,
        "markersNote": markers_note,
        "settings": settings_json(settings.as_ref()),
    })
}

pub fn apply(core: &mut Core, command: &Value) -> io::Result<FollowUp> {
    let name = command
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("invalid record"))?;
    match name {
        "skip-onboarding" => {
            core.skip_onboarding(&text(command, "step")?)?;
            Ok(FollowUp::None)
        }
        "choose-location" => {
            let language = command
                .get("language")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let skip = command
                .get("skipLanguage")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            core.choose_location(Path::new(&text(command, "root")?), language.as_deref(), skip)?;
            Ok(FollowUp::None)
        }
        "opt-in" => {
            core.opt_in(&string_list(command, "ids")?)?;
            Ok(FollowUp::None)
        }
        "set-rescue" => {
            core.set_rescue_opt_in(flag(command, "enabled")?)?;
            Ok(FollowUp::None)
        }
        "set-scope" => {
            core.set_collection_scope(scope_from(command)?)?;
            Ok(FollowUp::None)
        }
        "confirm-retention" => {
            core.confirm_retention(days(command)?)?;
            Ok(FollowUp::None)
        }
        "set-boot" => {
            core.set_boot_preference(flag(command, "startAtLogin")?)?;
            Ok(FollowUp::None)
        }
        "enable-encryption" => {
            let password = text(command, "password")?;
            let seconds = command
                .get("autoLockSeconds")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid("invalid record"))?;
            let seconds = u32::try_from(seconds).map_err(|_| invalid("invalid record"))?;
            core.enable_onboarding_encryption(&password, seconds)
                .map_err(|error| redact(&error, &password))?;
            Ok(FollowUp::None)
        }
        "set-mcp" => {
            core.set_mcp(flag(command, "enabled")?)?;
            Ok(FollowUp::None)
        }
        "build-baseline" => {
            core.build_baseline()?;
            Ok(FollowUp::None)
        }
        "pause" => {
            core.pause_monitoring()?;
            Ok(FollowUp::None)
        }
        "resume" => {
            core.resume_monitoring()?;
            Ok(FollowUp::None)
        }
        "start-live" => {
            core.begin_live()?;
            Ok(FollowUp::None)
        }
        "enter-history" => {
            core.switch_to_history()?;
            Ok(FollowUp::ReleaseDrivers)
        }
        "stop" => {
            core.stop_monitoring()?;
            Ok(FollowUp::ReleaseDrivers)
        }
        _ => Err(io::Error::new(ErrorKind::NotFound, "not_found")),
    }
}

pub fn release_subscribed(endpoint: &Endpoint) -> String {
    let mut gui = match GuiClient::connect(endpoint) {
        Ok(client) => client,
        Err(error) => return format!("unavailable: {error}"),
    };
    let snapshot = match gui.status() {
        Ok(snapshot) => snapshot,
        Err(error) => return format!("unavailable: {error}"),
    };
    let mut busy = Vec::new();
    for state in &snapshot.drivers {
        if state.subscriptions == 0 {
            continue;
        }
        match Driver::parse(&state.driver) {
            Some(driver) => busy.push(driver),
            None => return format!("unavailable: {}", state.driver),
        }
    }
    if busy.is_empty() {
        return "idle".into();
    }
    let mut core = match CoreClient::connect(endpoint) {
        Ok(client) => client,
        Err(error) => return format!("unavailable: {error}"),
    };
    for driver in busy {
        if let Err(error) = core.release(driver) {
            let code = error.code.as_str();
            if code == "privilege_unavailable" {
                return "privilege_unavailable".into();
            }
            return format!("unavailable: {code}");
        }
    }
    "released".into()
}

fn unavailable(detail: &str, endpoint: Option<&Endpoint>, drivers: &str) -> Value {
    json!({
        "user": "",
        "storage": format!("unavailable: {detail}"),
        "run": "not_observed",
        "service": endpoint.map(|_| "not_observed").unwrap_or("not_configured"),
        "plugins": [],
        "privilege": "not_observed",
        "paused": false,
        "collectors": "",
        "drivers": drivers,
        "onboarding": onboarding_json(&Onboarding::default()),
        "markers": [],
        "markersNote": Value::Null,
        "settings": settings_json(None),
    })
}

fn onboarding_json(record: &Onboarding) -> Value {
    json!({
        "step": record.step,
        "complete": record.complete,
        "skipped": record.skipped,
        "language": record.language,
        "optedIn": record.opted_in,
        "rescue": record.rescue,
        "mcp": record.mcp,
        "encryption": record.encryption,
        "baseline": record.baseline_id,
        "baselineLabel": record.baseline_label,
        "retentionDays": record.retention_days,
        "scripts": record.scripts,
        "automaticSnapshots": record.automatic_snapshots,
        "scope": {
            "roots": path_list(&record.scope.roots),
            "fileTypes": record.scope.file_types,
            "processes": record.scope.processes,
            "software": record.scope.software,
            "registry": record.scope.registry,
            "interfaces": record.scope.interfaces,
            "watchProcesses": record.scope.watch_processes,
        },
    })
}

fn settings_json(settings: Option<&winstrata_core::Settings>) -> Value {
    match settings {
        Some(settings) => json!({
            "startAtLogin": settings.start_at_login,
            "monitoredRoots": path_list(&settings.monitored_roots),
        }),
        None => json!({
            "startAtLogin": false,
            "monitoredRoots": [],
        }),
    }
}

fn scope_from(command: &Value) -> io::Result<CollectionScope> {
    Ok(CollectionScope {
        roots: string_list(command, "roots")?
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        file_types: string_list(command, "fileTypes")?,
        processes: string_list(command, "processes")?,
        software: string_list(command, "software")?,
        registry: string_list(command, "registry")?,
        interfaces: string_list(command, "interfaces")?,
        watch_processes: command
            .get("watchProcesses")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn path_list(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|path| path.display().to_string()).collect()
}

fn string_list(command: &Value, key: &str) -> io::Result<Vec<String>> {
    match command.get(key) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("invalid record"))
            })
            .collect(),
        Some(_) => Err(invalid("invalid record")),
    }
}

fn text(command: &Value, key: &str) -> io::Result<String> {
    command
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid("invalid record"))
}

fn flag(command: &Value, key: &str) -> io::Result<bool> {
    command
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("invalid record"))
}

fn days(command: &Value) -> io::Result<Option<u32>> {
    match command.get("days") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => {
            let days = number.as_u64().ok_or_else(|| invalid("invalid record"))?;
            u32::try_from(days)
                .map(Some)
                .map_err(|_| invalid("invalid record"))
        }
        Some(_) => Err(invalid("invalid record")),
    }
}

fn invalid(detail: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, detail)
}

fn redact(error: &io::Error, secret: &str) -> io::Error {
    let mut detail = error.to_string();
    if !secret.is_empty() {
        detail = detail.replace(secret, "[redacted]");
    }
    io::Error::new(error.kind(), detail)
}

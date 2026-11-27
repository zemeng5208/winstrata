use std::path::{Path, PathBuf};

use winstrata_core::{Core, OFFICIAL, RunState, StorageState};
use winstrata_service::{Endpoint, GuiClient, Privilege, current_privilege};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginView {
    pub id: String,
    pub name: String,
    pub availability: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub user: String,
    pub storage: String,
    pub run: String,
    pub plugins: Vec<PluginView>,
    pub service: String,
    pub privilege: String,
}

pub fn observe_at(locator: &Path, endpoint: Option<&Endpoint>) -> Observation {
    if let Err(error) = current_privilege() {
        return unavailable(&error.to_string());
    }
    match Core::open_at(locator, None) {
        Ok(mut core) => describe(&mut core, endpoint),
        Err(error) => unavailable(&error.to_string()),
    }
}

pub(crate) fn describe(core: &mut Core, endpoint: Option<&Endpoint>) -> Observation {
    let mut observation = core_view(core);
    observation.privilege = match current_privilege() {
        Ok(Privilege::WindowsElevated) => "windows_elevated".to_owned(),
        Ok(Privilege::WindowsOrdinary) => "windows_ordinary".to_owned(),
        Ok(Privilege::NotWindows) => "not_windows".to_owned(),
        Err(error) => format!("unavailable: {error}"),
    };
    observation.service = endpoint
        .map(observe_service)
        .unwrap_or_else(|| "not_configured".to_owned());
    observation
}

pub fn observe_service(endpoint: &Endpoint) -> String {
    let mut client = match GuiClient::connect(endpoint) {
        Ok(client) => client,
        Err(error) => return format!("unavailable: {error}"),
    };
    match client.status() {
        Ok(snapshot) => {
            let drivers = snapshot
                .drivers
                .iter()
                .map(|driver| {
                    format!(
                        "{}:{}:{}",
                        driver.driver, driver.phase, driver.subscriptions
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "connected faults={} audit_dropped={} {drivers}",
                snapshot.faults, snapshot.audit_dropped
            )
        }
        Err(error) => format!("unavailable: {error}"),
    }
}

fn core_view(core: &mut Core) -> Observation {
    let storage = match core.storage_state() {
        StorageState::Ready => "ready".to_owned(),
        StorageState::NeedsDirectory { detail, .. } => format!("needs_directory: {detail}"),
    };
    let run = match core.run_state() {
        RunState::Stopped => "stopped",
        RunState::Live => "live",
        RunState::History => "history",
    };
    let ready = matches!(core.storage_state(), StorageState::Ready);
    let plugins = if ready {
        match runtime_binary().and_then(|path| core.open_plugins(&path).ok()) {
            Some(host) => host
                .list()
                .into_iter()
                .map(|report| PluginView {
                    id: report.id,
                    name: report.name,
                    availability: report.availability,
                    enabled: report.enabled,
                })
                .collect(),
            None => official_views("not_observed"),
        }
    } else {
        official_views("not_observed")
    };
    Observation {
        user: core.current_user().to_owned(),
        storage,
        run: run.to_owned(),
        plugins,
        service: String::new(),
        privilege: String::new(),
    }
}

fn official_views(availability: &str) -> Vec<PluginView> {
    OFFICIAL
        .iter()
        .map(|plugin| PluginView {
            id: plugin.id.to_owned(),
            name: plugin.name.to_owned(),
            availability: availability.to_owned(),
            enabled: false,
        })
        .collect()
}

fn unavailable(detail: &str) -> Observation {
    Observation {
        user: String::new(),
        storage: format!("unavailable: {detail}"),
        run: "not_observed".to_owned(),
        plugins: official_views("not_observed"),
        service: format!("unavailable: {detail}"),
        privilege: "not_observed".to_owned(),
    }
}

fn runtime_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    if dir.file_name().and_then(|name| name.to_str()) == Some("deps") {
        dir.pop();
    }
    let name = if cfg!(windows) {
        "winstrata-runtime.exe"
    } else {
        "winstrata-runtime"
    };
    let path = dir.join(name);
    path.is_file().then_some(path)
}

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use winstrata_core::{Core, OFFICIAL, PluginHost, RunState, StorageState};
use winstrata_service::{Endpoint, GuiClient, Privilege, current_privilege};

use crate::prefs::{CloseChoice, Language, Prefs, Theme};

pub struct Session {
    prefs_path: PathBuf,
    pub prefs: Prefs,
    core: CoreLink,
    runtime: Option<PathBuf>,
    plugins: Option<PluginHost>,
    plugin_note: String,
    service: Option<String>,
    pub bundle: bool,
    pub tray: &'static str,
    page: String,
    window: &'static str,
    error: Option<String>,
}

enum CoreLink {
    Open(Core),
    Failed(String),
}

impl Session {
    pub fn open(
        prefs_path: PathBuf,
        locator: Option<&Path>,
        service: Option<String>,
        runtime: Option<PathBuf>,
        bundle: bool,
        tray: &'static str,
    ) -> Self {
        let prefs = match Prefs::load(&prefs_path) {
            Ok(prefs) => prefs,
            Err(error) => {
                return Self::failed(prefs_path, service, runtime, bundle, tray, error);
            }
        };
        let core = match locator {
            Some(path) => Core::open_at(path, None),
            None => Core::open(),
        };
        let mut session = Self {
            prefs_path,
            prefs,
            core: match core {
                Ok(core) => CoreLink::Open(core),
                Err(error) => CoreLink::Failed(clip(&error.to_string())),
            },
            runtime,
            plugins: None,
            plugin_note: "catalog".to_owned(),
            service,
            bundle,
            tray,
            page: "home".to_owned(),
            window: "visible",
            error: None,
        };
        session.open_plugins();
        session
    }

    fn failed(
        prefs_path: PathBuf,
        service: Option<String>,
        runtime: Option<PathBuf>,
        bundle: bool,
        tray: &'static str,
        error: io::Error,
    ) -> Self {
        Self {
            prefs_path,
            prefs: Prefs::default(),
            core: CoreLink::Failed("gui preferences were not loaded".to_owned()),
            runtime,
            plugins: None,
            plugin_note: "catalog".to_owned(),
            service,
            bundle,
            tray,
            page: "home".to_owned(),
            window: "visible",
            error: Some(clip(&error.to_string())),
        }
    }

    pub fn command(&mut self, action: &str, value: Option<&str>, remember: bool) -> bool {
        self.error = None;
        let result = match action {
            "set_theme" => self.set_theme(value.unwrap_or("")),
            "set_language" => self.set_language(value.unwrap_or("")),
            "set_close" => self.set_close(value.unwrap_or("")),
            "navigate" => self.navigate(value.unwrap_or("")),
            "set_storage" => self.set_storage(value.unwrap_or("")),
            "window_close" => self.window_close(),
            "choose_close" => self.choose_close(value.unwrap_or(""), remember),
            "tray_open" => {
                self.window = "visible";
                Ok(())
            }
            "shutdown" => {
                self.window = "exit";
                Ok(())
            }
            _ => Err("unknown_command".to_owned()),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
        self.window == "exit"
    }

    pub fn observe_tray(&mut self, event: u8) {
        match event {
            1 => self.window = "visible",
            2 => self.window = "exit",
            _ => {}
        }
    }

    pub fn view(&self) -> Value {
        json!({
            "theme": self.prefs.theme.map(theme_name),
            "language": language_name(self.prefs.language),
            "close": close_name(self.prefs.close),
            "privilege": privilege_name(),
            "service": self.service_view(),
            "storage": self.storage_view(),
            "plugins": self.plugin_rows(),
            "pluginHost": self.plugin_note,
            "page": self.page,
            "window": self.window,
            "tray": self.tray,
            "bundle": self.bundle,
            "error": self.error,
        })
    }

    fn set_theme(&mut self, value: &str) -> Result<(), String> {
        self.prefs.theme = Some(parse_theme(value)?);
        self.save()
    }

    fn set_language(&mut self, value: &str) -> Result<(), String> {
        self.prefs.language = match value {
            "en" => Language::En,
            "zh-CN" => Language::Zh,
            _ => return Err("invalid_language".to_owned()),
        };
        self.save()
    }

    fn set_close(&mut self, value: &str) -> Result<(), String> {
        self.prefs.close = parse_close(value)?;
        self.save()
    }

    fn navigate(&mut self, value: &str) -> Result<(), String> {
        if matches!(value, "home" | "settings" | "market" | "onboarding")
            || self
                .plugin_rows()
                .iter()
                .any(|row| row.get("id").and_then(Value::as_str) == value.strip_prefix("plugin:"))
        {
            self.page = value.to_owned();
            return Ok(());
        }
        Err("unknown_page".to_owned())
    }

    fn set_storage(&mut self, value: &str) -> Result<(), String> {
        let CoreLink::Open(core) = &mut self.core else {
            return Err("storage_unavailable".to_owned());
        };
        core.select_directory(Path::new(value))
            .map_err(|error| clip(&error.to_string()))?;
        self.open_plugins();
        Ok(())
    }

    fn window_close(&mut self) -> Result<(), String> {
        match self.prefs.close {
            CloseChoice::Ask => self.window = "prompt",
            CloseChoice::Exit => self.window = "exit",
            CloseChoice::Tray if self.tray == "ready" => self.window = "hidden",
            CloseChoice::Tray => {
                self.window = "prompt";
                return Err("tray_unavailable".to_owned());
            }
        }
        Ok(())
    }

    fn choose_close(&mut self, value: &str, remember: bool) -> Result<(), String> {
        match value {
            "exit" => {
                if remember {
                    self.prefs.close = CloseChoice::Exit;
                    self.save()?;
                }
                self.window = "exit";
                Ok(())
            }
            "tray" if self.tray == "ready" => {
                if remember {
                    self.prefs.close = CloseChoice::Tray;
                    self.save()?;
                }
                self.window = "hidden";
                Ok(())
            }
            "tray" => Err("tray_unavailable".to_owned()),
            _ => Err("invalid_close".to_owned()),
        }
    }

    fn save(&self) -> Result<(), String> {
        self.prefs
            .save(&self.prefs_path)
            .map_err(|error| clip(&error.to_string()))
    }

    fn open_plugins(&mut self) {
        if self.plugins.is_some() {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            self.plugin_note = "runtime_missing".to_owned();
            return;
        };
        let CoreLink::Open(core) = &mut self.core else {
            self.plugin_note = "storage_unavailable".to_owned();
            return;
        };
        if !matches!(core.storage_state(), StorageState::Ready) {
            self.plugin_note = "storage_unavailable".to_owned();
            return;
        }
        match core.open_plugins(&runtime) {
            Ok(host) => {
                self.plugins = Some(host);
                self.plugin_note = "host".to_owned();
            }
            Err(error) => {
                self.plugin_note = error.code().to_owned();
                self.error = Some(clip(error.detail()));
            }
        }
    }

    fn storage_view(&self) -> Value {
        match &self.core {
            CoreLink::Failed(detail) => json!({"state": "unavailable", "detail": detail}),
            CoreLink::Open(core) => match core.storage_state() {
                StorageState::Ready => json!({
                    "state": "ready",
                    "root": core.selected_root().map(|path| path.display().to_string()),
                    "run": match core.run_state() {
                        RunState::Stopped => "stopped",
                        RunState::Live => "live",
                        RunState::History => "history",
                    },
                }),
                StorageState::NeedsDirectory { detail, .. } => {
                    json!({"state": "needs_directory", "detail": detail})
                }
            },
        }
    }

    fn service_view(&self) -> Value {
        let Some(address) = &self.service else {
            return json!({"state": "unavailable", "detail": "service endpoint is not configured"});
        };
        let endpoint = match Endpoint::client(address) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                return json!({"state": "unavailable", "detail": clip(&error.to_string())});
            }
        };
        let mut client = match GuiClient::connect(&endpoint) {
            Ok(client) => client,
            Err(error) => {
                return json!({"state": "unavailable", "detail": clip(&error.to_string())});
            }
        };
        match client.status() {
            Ok(snapshot) => json!({
                "state": "ready",
                "drivers": snapshot.drivers.iter().map(|driver| json!({
                    "driver": driver.driver,
                    "phase": driver.phase,
                    "subscriptions": driver.subscriptions,
                    "failure": driver.failure,
                })).collect::<Vec<_>>(),
            }),
            Err(error) => json!({"state": "unavailable", "detail": clip(&error.detail)}),
        }
    }

    fn plugin_rows(&self) -> Vec<Value> {
        if let Some(host) = &self.plugins {
            return host
                .list()
                .into_iter()
                .map(|report| {
                    json!({
                        "id": report.id,
                        "name": report.name,
                        "plan": OFFICIAL.iter().find(|plugin| plugin.id == report.id).map(|plugin| plugin.plan),
                        "availability": report.availability,
                        "phase": report.phase,
                        "enabled": report.enabled,
                    })
                })
                .collect();
        }
        OFFICIAL
            .iter()
            .map(|plugin| {
                json!({
                    "id": plugin.id,
                    "name": plugin.name,
                    "plan": plugin.plan,
                    "availability": "implementation_absent",
                    "phase": "implementation_absent",
                    "enabled": false,
                })
            })
            .collect()
    }
}

fn parse_theme(value: &str) -> Result<Theme, String> {
    match value {
        "white-acrylic" => Ok(Theme::WhiteAcrylic),
        "black-acrylic" => Ok(Theme::BlackAcrylic),
        "liquid-glass" => Ok(Theme::LiquidGlass),
        _ => Err("invalid_theme".to_owned()),
    }
}

fn parse_close(value: &str) -> Result<CloseChoice, String> {
    match value {
        "ask" => Ok(CloseChoice::Ask),
        "tray" => Ok(CloseChoice::Tray),
        "exit" => Ok(CloseChoice::Exit),
        _ => Err("invalid_close".to_owned()),
    }
}

fn theme_name(theme: Theme) -> &'static str {
    match theme {
        Theme::WhiteAcrylic => "white-acrylic",
        Theme::BlackAcrylic => "black-acrylic",
        Theme::LiquidGlass => "liquid-glass",
    }
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::En => "en",
        Language::Zh => "zh-CN",
    }
}

fn close_name(close: CloseChoice) -> &'static str {
    match close {
        CloseChoice::Ask => "ask",
        CloseChoice::Tray => "tray",
        CloseChoice::Exit => "exit",
    }
}

fn privilege_name() -> &'static str {
    match current_privilege() {
        Ok(Privilege::NotWindows) => "not_windows",
        Ok(Privilege::WindowsOrdinary) => "windows_ordinary",
        Ok(Privilege::WindowsElevated) => "windows_elevated",
        Err(_) => "unknown",
    }
}

fn clip(value: &str) -> String {
    value.chars().take(300).collect()
}

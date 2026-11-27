//! 脚本目录、权限、触发、日志和空预设。
//!
//! 预置目录必须保持为空。用户脚本默认禁用，导入也不会变成预置。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::codec;
use crate::editor::{self, extension};
use crate::execute::{self, ChildIo, Pause, Tools};

const SOURCE_LIMIT: usize = 16 * 1024;
const PERMISSIONS: &[&str] = &[
    "event.publish",
    "event.subscribe",
    "history.read",
    "plugin.call",
    "storage.private",
];

#[derive(Clone, Serialize, Deserialize)]
struct Trigger {
    kind: String,
    #[serde(default)]
    event: String,
    #[serde(default)]
    every_ms: u64,
    #[serde(default)]
    field: String,
    #[serde(default)]
    cmp: String,
    #[serde(default)]
    value: i64,
}

#[derive(Clone, Serialize, Deserialize)]
struct Script {
    id: String,
    name: String,
    language: String,
    enabled: bool,
    paused: bool,
    version: u32,
    versions: Vec<u32>,
    declared: Vec<String>,
    grants: Vec<String>,
    pending: Vec<String>,
    triggers: Vec<Trigger>,
    modules: Vec<String>,
    count: i64,
    condition_fired: bool,
    last_schedule: u64,
    reloaded: bool,
}

#[derive(Serialize, Deserialize)]
struct Catalog {
    next_id: u64,
    scripts: Vec<Script>,
}

pub(crate) struct Bench {
    state: PathBuf,
    tools: Tools,
    paused: bool,
    next_id: u64,
    scripts: Vec<Script>,
    parts: std::collections::HashMap<String, Vec<String>>,
    logs: Vec<Value>,
    child: Option<ChildIo>,
}

impl Bench {
    pub(crate) fn open(state: &Path, tools: Tools) -> std::io::Result<Self> {
        fs::create_dir_all(state.join("presets"))?;
        fs::create_dir_all(state.join("sources"))?;
        fs::create_dir_all(state.join("private"))?;
        fs::create_dir_all(state.join("debug"))?;
        let catalog = state.join("catalog.json");
        let (next_id, scripts) = if catalog.is_file() {
            let text = fs::read_to_string(&catalog)?;
            let stored: Catalog = serde_json::from_str(&text)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            (stored.next_id, stored.scripts)
        } else {
            (1, Vec::new())
        };
        let mut logs = Vec::new();
        if let Ok(text) = fs::read_to_string(state.join("logs.jsonl")) {
            for line in text.lines().filter(|line| !line.is_empty()) {
                if let Ok(value) = serde_json::from_str::<Value>(line) {
                    logs.push(value);
                }
            }
        }
        let bench = Self {
            state: state.to_path_buf(),
            tools,
            paused: false,
            next_id,
            scripts,
            parts: std::collections::HashMap::new(),
            logs,
            child: None,
        };
        bench.write_preset_count()?;
        Ok(bench)
    }

    pub(crate) fn pause(&mut self) {
        self.paused = true;
        self.kill_child();
    }

    pub(crate) fn resume(&mut self) {
        self.paused = false;
    }

    pub(crate) fn kill_child(&mut self) {
        let _ = fs::write(self.state.join("debug").join("stop"), b"stop");
        if let Some(mut child) = self.child.take() {
            let _ = child.child.kill();
            let _ = child.child.wait();
        }
    }

    pub(crate) fn dispatch(&mut self, payload: &str) -> Value {
        let value: Value = match serde_json::from_str::<Value>(payload) {
            Ok(value) if value.is_object() => value,
            _ => return self.fail("rejected"),
        };
        let op = value["op"].as_str().unwrap_or("");
        if self.preset_files() > 0 && !matches!(op, "empty" | "status") {
            return self.fail("preset_present");
        }
        match op {
            "empty" | "status" => self.empty(),
            "list" => self.list(),
            "create" => self.create(&value),
            "edit" => self.edit(&value, false),
            "hot-reload" => self.edit(&value, true),
            "highlight" => self.highlight(&value),
            "complete" => self.complete(&value),
            "diagnose" => self.diagnose(&value),
            "declare" => self.declare(&value),
            "grant" => self.grant(&value),
            "revoke" => self.revoke(&value),
            "enable-script" => self.set_enabled(&value, true),
            "disable-script" => self.set_enabled(&value, false),
            "pause-script" => self.set_paused(&value, true),
            "resume-script" => self.set_paused(&value, false),
            "modules" => self.set_modules(&value),
            "trigger" => self.set_trigger(&value),
            "run" => self.run_manual(&value),
            "debug-start" => self.debug_start(&value),
            "debug-step" => self.debug_control("step"),
            "debug-continue" => self.debug_control("continue"),
            "debug-stop" => self.debug_stop(),
            "logs" => self.logs(&value),
            "history" => self.history(&value),
            "export" => self.export_script(&value),
            "import" => self.import_script(&value),
            "ingest" => self.ingest(&value),
            _ => self.fail("rejected"),
        }
    }

    pub(crate) fn fire_due(&mut self) {
        if self.paused || self.child.is_some() || self.preset_files() > 0 {
            return;
        }
        let now = unix_ms();
        let due: Vec<String> = self
            .scripts
            .iter()
            .filter(|script| script.enabled && !script.paused)
            .filter(|script| {
                script.triggers.iter().any(|trigger| {
                    trigger.kind == "schedule"
                        && trigger.every_ms >= 200
                        && now.saturating_sub(script.last_schedule) >= trigger.every_ms
                })
            })
            .map(|script| script.id.clone())
            .collect();
        for id in due {
            if let Some(script) = self.scripts.iter_mut().find(|script| script.id == id) {
                script.last_schedule = now;
            }
            self.execute(&id, "schedule", "");
        }
    }

    pub(crate) fn fit(&self, value: &Value) -> Result<String, &'static str> {
        let text = serde_json::to_string(value).map_err(|_| "invalid_record")?;
        if text.len() > 1024 || text.chars().any(char::is_control) {
            Err("invalid_record")
        } else {
            Ok(text)
        }
    }

    fn empty(&self) -> Value {
        json!({
            "ok": self.preset_files() == 0,
            "code": if self.preset_files() == 0 { "ok" } else { "preset_present" },
            "presets": self.preset_files(),
            "scripts": self.scripts.len(),
            "paused": self.paused,
        })
    }

    fn list(&self) -> Value {
        let scripts: Vec<Value> = self
            .scripts
            .iter()
            .take(8)
            .map(|script| {
                json!({
                    "id": script.id,
                    "name": script.name,
                    "language": script.language,
                    "enabled": script.enabled,
                    "version": script.version,
                })
            })
            .collect();
        json!({"ok": true, "presets": self.preset_files(), "scripts": scripts})
    }

    fn create(&mut self, value: &Value) -> Value {
        let language = value["language"].as_str().unwrap_or("");
        let name = value["name"].as_str().unwrap_or("");
        if !editor::known_language(language) || !name_token(name) {
            return self.fail("rejected");
        }
        let key = format!("create:{language}:{name}");
        let Some(source) = self.source_from(value, &key) else {
            return self.pending();
        };
        let source = match source {
            Ok(source) => source,
            Err(code) => return self.fail(&code),
        };
        let id = format!("s{}", self.next_id);
        self.next_id += 1;
        let script = Script {
            id: id.clone(),
            name: name.to_owned(),
            language: language.to_owned(),
            enabled: false,
            paused: false,
            version: 1,
            versions: vec![1],
            declared: Vec::new(),
            grants: Vec::new(),
            pending: Vec::new(),
            triggers: Vec::new(),
            modules: Vec::new(),
            count: 0,
            condition_fired: false,
            last_schedule: unix_ms(),
            reloaded: false,
        };
        if let Err(code) = self.write_source(&script, &source) {
            return self.fail(&code);
        }
        self.scripts.push(script);
        if self.save().is_err() {
            return self.fail("faulted");
        }
        json!({
            "ok": true,
            "id": id,
            "version": 1,
            "enabled": false,
            "presets": self.preset_files(),
            "scripts": self.scripts.len(),
        })
    }

    fn edit(&mut self, value: &Value, reload: bool) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        if reload && !self.scripts[index].enabled {
            return self.fail("disabled");
        }
        let key = format!("edit:{id}");
        let Some(source) = self.source_from(value, &key) else {
            return self.pending();
        };
        let source = match source {
            Ok(source) => source,
            Err(code) => return self.fail(&code),
        };
        {
            let script = &mut self.scripts[index];
            script.version += 1;
            script.versions.push(script.version);
            script.versions.truncate(32);
            script.reloaded = reload;
        }
        if let Err(code) = self.write_source(&self.scripts[index], &source) {
            return self.fail(&code);
        }
        let version = self.scripts[index].version;
        if self.save().is_err() {
            return self.fail("faulted");
        }
        json!({"ok": true, "id": id, "version": version, "reloaded": reload, "presets": self.preset_files()})
    }

    fn highlight(&self, value: &Value) -> Value {
        let Some(script) = self.script(value) else {
            return self.fail("missing");
        };
        let source = self.read_source(script);
        let tokens: Vec<Value> = editor::highlight(&script.language, &source)
            .into_iter()
            .map(|(kind, len)| json!([kind, len]))
            .collect();
        json!({"ok": true, "tokens": tokens, "presets": self.preset_files()})
    }

    fn complete(&self, value: &Value) -> Value {
        let Some(script) = self.script(value) else {
            return self.fail("missing");
        };
        let source = self.read_source(script);
        let prefix = value["prefix"].as_str().unwrap_or("");
        json!({
            "ok": true,
            "items": editor::complete(&script.language, &source, prefix),
            "presets": self.preset_files(),
        })
    }

    fn diagnose(&self, value: &Value) -> Value {
        let Some(script) = self.script(value) else {
            return self.fail("missing");
        };
        let source = self.read_source(script);
        let items: Vec<Value> = editor::diagnose(&script.language, &source)
            .into_iter()
            .map(|(line, code)| json!({"line": line, "code": code}))
            .collect();
        json!({"ok": true, "items": items, "presets": self.preset_files()})
    }

    fn declare(&mut self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        let permissions = value["permissions"].as_array();
        let Some(permissions) = permissions else {
            return self.fail("rejected");
        };
        for permission in permissions {
            let Some(permission) = permission.as_str() else {
                return self.fail("rejected");
            };
            if !PERMISSIONS.contains(&permission) {
                return self.fail("undeclared_permission");
            }
        }
        let script = &mut self.scripts[index];
        for permission in permissions {
            let permission = permission.as_str().unwrap();
            if !script.declared.iter().any(|item| item == permission) {
                script.declared.push(permission.to_owned());
            }
            if !script.grants.iter().any(|item| item == permission)
                && !script.pending.iter().any(|item| item == permission)
            {
                script.pending.push(permission.to_owned());
            }
        }
        if self.save().is_err() {
            return self.fail("faulted");
        }
        self.permission_view(&id)
    }

    fn grant(&mut self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let permission = value["permission"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        let script = &mut self.scripts[index];
        if !script.pending.iter().any(|item| item == &permission) {
            return self.fail("undeclared_permission");
        }
        script.pending.retain(|item| item != &permission);
        script.grants.push(permission);
        if self.save().is_err() {
            return self.fail("faulted");
        }
        self.permission_view(&id)
    }

    fn revoke(&mut self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let permission = value["permission"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        let script = &mut self.scripts[index];
        if !script.grants.iter().any(|item| item == &permission) {
            return self.fail("missing");
        }
        script.grants.retain(|item| item != &permission);
        if script.declared.iter().any(|item| item == &permission)
            && !script.pending.iter().any(|item| item == &permission)
        {
            script.pending.push(permission);
        }
        if self.save().is_err() {
            return self.fail("faulted");
        }
        self.permission_view(&id)
    }

    fn set_enabled(&mut self, value: &Value, enabled: bool) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        if !enabled {
            self.kill_child();
        }
        self.scripts[index].enabled = enabled;
        if enabled {
            self.scripts[index].last_schedule = unix_ms();
        }
        if self.save().is_err() {
            return self.fail("faulted");
        }
        json!({"ok": true, "id": id, "enabled": enabled, "presets": self.preset_files()})
    }

    fn set_paused(&mut self, value: &Value, paused: bool) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        self.scripts[index].paused = paused;
        if paused {
            self.kill_child();
        }
        if self.save().is_err() {
            return self.fail("faulted");
        }
        json!({"ok": true, "id": id, "paused": paused, "presets": self.preset_files()})
    }

    fn set_modules(&mut self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        let Some(modules) = value["modules"].as_array() else {
            return self.fail("rejected");
        };
        let mut names = Vec::new();
        for module in modules {
            let Some(name) = module.as_str() else {
                return self.fail("rejected");
            };
            if !module_token(name) {
                return self.fail("rejected");
            }
            names.push(name.to_owned());
        }
        self.scripts[index].modules = names;
        if self.save().is_err() {
            return self.fail("faulted");
        }
        json!({"ok": true, "id": id, "presets": self.preset_files()})
    }

    fn set_trigger(&mut self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        let kind = value["kind"].as_str().unwrap_or("");
        let trigger = match kind {
            "manual" => Trigger {
                kind: kind.to_owned(),
                event: String::new(),
                every_ms: 0,
                field: String::new(),
                cmp: String::new(),
                value: 0,
            },
            "event" => {
                let event = value["event"].as_str().unwrap_or("");
                if !event_token(event) {
                    return self.fail("rejected");
                }
                Trigger {
                    kind: kind.to_owned(),
                    event: event.to_owned(),
                    every_ms: 0,
                    field: String::new(),
                    cmp: String::new(),
                    value: 0,
                }
            }
            "schedule" => {
                let every_ms = value["every_ms"].as_u64().unwrap_or(0);
                if !(200..86_400_000).contains(&every_ms) {
                    return self.fail("rejected");
                }
                Trigger {
                    kind: kind.to_owned(),
                    event: String::new(),
                    every_ms,
                    field: String::new(),
                    cmp: String::new(),
                    value: 0,
                }
            }
            "condition" => {
                let field = value["field"].as_str().unwrap_or("");
                let cmp = value["cmp"].as_str().unwrap_or("");
                if field != "count" || !matches!(cmp, "gte" | "eq") {
                    return self.fail("rejected");
                }
                Trigger {
                    kind: kind.to_owned(),
                    event: String::new(),
                    every_ms: 0,
                    field: field.to_owned(),
                    cmp: cmp.to_owned(),
                    value: value["value"].as_i64().unwrap_or(0),
                }
            }
            _ => return self.fail("rejected"),
        };
        let script = &mut self.scripts[index];
        script.triggers.retain(|item| item.kind != trigger.kind);
        script.triggers.push(trigger);
        script.last_schedule = unix_ms();
        script.condition_fired = false;
        if self.save().is_err() {
            return self.fail("faulted");
        }
        json!({"ok": true, "id": id, "kind": kind, "presets": self.preset_files()})
    }

    fn run_manual(&mut self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let emit = value["emit"].as_str().unwrap_or("");
        if !emit.is_empty() && !event_token(emit) {
            return self.fail("rejected");
        }
        self.execute(&id, "manual", emit)
    }

    fn ingest(&mut self, value: &Value) -> Value {
        let event = value["event"].as_str().unwrap_or("");
        if !event_token(event) {
            return self.fail("rejected");
        }
        if self.paused {
            return self.fail("paused");
        }
        let ids: Vec<String> = self.scripts.iter().map(|script| script.id.clone()).collect();
        let mut ran = 0;
        for id in ids {
            let Some(index) = self.index(&id) else {
                continue;
            };
            let script = &mut self.scripts[index];
            if !script.enabled || script.paused {
                continue;
            }
            let condition = script.triggers.iter().any(|trigger| trigger.kind == "condition");
            if condition {
                script.count += 1;
            }
            let fire_condition = script.triggers.iter().any(|trigger| {
                trigger.kind == "condition"
                    && trigger.field == "count"
                    && !script.condition_fired
                    && match trigger.cmp.as_str() {
                        "gte" => script.count >= trigger.value,
                        "eq" => script.count == trigger.value,
                        _ => false,
                    }
            });
            let subscribed = script.grants.iter().any(|item| item == "event.subscribe");
            let fire_event = subscribed
                && script
                    .triggers
                    .iter()
                    .any(|trigger| trigger.kind == "event" && trigger.event == event);
            if fire_condition {
                script.condition_fired = true;
            }
            if fire_condition {
                self.execute(&id, "condition", "");
                ran += 1;
            }
            if fire_event {
                self.execute(&id, "event", "");
                ran += 1;
            }
        }
        json!({"ok": true, "event": event, "ran": ran, "presets": self.preset_files()})
    }

    fn execute(&mut self, id: &str, trigger: &str, emit: &str) -> Value {
        if self.preset_files() > 0 {
            return self.fail("preset_present");
        }
        if self.paused {
            return self.fail("paused");
        }
        if self.child.is_some() {
            return self.fail("busy");
        }
        let Some(index) = self.index(id) else {
            return self.fail("missing");
        };
        if !self.scripts[index].enabled {
            return self.fail("disabled");
        }
        if self.scripts[index].paused {
            return self.fail("paused");
        }
        if !emit.is_empty() && !self.scripts[index].grants.iter().any(|item| item == "event.publish")
        {
            self.log(id, trigger, "permission_denied", "");
            return self.fail("permission_denied");
        }
        let language = self.scripts[index].language.clone();
        let modules = self.scripts[index].modules.clone();
        let allow_private = self.scripts[index]
            .grants
            .iter()
            .any(|item| item == "storage.private");
        let reloaded = self.scripts[index].reloaded;
        self.scripts[index].reloaded = false;
        let source = self.source_path(&self.scripts[index]);
        let version = self.scripts[index].version;
        let result = execute::run_source(
            &self.tools,
            &self.state,
            &language,
            &source,
            &modules,
            allow_private,
        );
        let (status, output) = match result {
            Ok(captured) if captured.code == 0 => ("ok", text_of(&captured.stdout, &captured.stderr)),
            Ok(captured) => ("faulted", text_of(&captured.stdout, &captured.stderr)),
            Err(code) => {
                self.log(id, trigger, &code, "");
                let _ = self.save();
                return self.fail(&code);
            }
        };
        if status == "ok" && !emit.is_empty() {
            if fs::write(self.state.join("emit.txt"), emit).is_err() {
                self.log(id, trigger, "faulted", "");
                return self.fail("faulted");
            }
        }
        self.log(id, trigger, status, &output);
        let _ = self.save();
        json!({
            "ok": status == "ok",
            "code": status,
            "id": id,
            "trigger": trigger,
            "version": version,
            "reloaded": reloaded,
            "output": codec::encode(&output.into_bytes()),
            "presets": self.preset_files(),
        })
    }

    fn debug_start(&mut self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("").to_owned();
        let line = value["line"].as_u64().unwrap_or(0) as u32;
        if line == 0 {
            return self.fail("rejected");
        }
        if self.paused {
            return self.fail("paused");
        }
        let Some(index) = self.index(&id) else {
            return self.fail("missing");
        };
        if !self.scripts[index].enabled || self.scripts[index].paused {
            return self.fail("disabled");
        }
        self.kill_child();
        let language = self.scripts[index].language.clone();
        let allow_private = self.scripts[index]
            .grants
            .iter()
            .any(|item| item == "storage.private");
        let source = self.source_path(&self.scripts[index]);
        let mut child = match execute::spawn_debug(
            &self.tools,
            &self.state,
            &language,
            &source,
            line,
            allow_private,
        ) {
            Ok(child) => child,
            Err(code) => return self.fail(&code),
        };
        match execute::wait_pause(&self.state, &mut child.child) {
            Ok(Pause::Line(paused)) => {
                self.child = Some(child);
                json!({"ok": true, "state": "paused", "line": paused, "id": id, "presets": self.preset_files()})
            }
            Ok(Pause::Exited(code)) => {
                let captured = execute::read_captured(&child, code);
                let output = text_of(&captured.stdout, &captured.stderr);
                self.log(&id, "debug", if code == 0 { "ok" } else { "faulted" }, &output);
                let _ = self.save();
                json!({
                    "ok": code == 0,
                    "state": "finished",
                    "code": if code == 0 { "ok" } else { "faulted" },
                    "output": codec::encode(output.as_bytes()),
                    "presets": self.preset_files(),
                })
            }
            Err(code) => self.fail(&code),
        }
    }

    fn debug_control(&mut self, action: &str) -> Value {
        let Some(mut child) = self.child.take() else {
            return self.fail("not_debugging");
        };
        if execute::signal_debug(&self.state, action).is_err() {
            self.kill_held(&mut child);
            return self.fail("faulted");
        }
        match execute::wait_pause(&self.state, &mut child.child) {
            Ok(Pause::Line(line)) => {
                self.child = Some(child);
                json!({"ok": true, "state": "paused", "line": line, "presets": self.preset_files()})
            }
            Ok(Pause::Exited(code)) => {
                let captured = execute::read_captured(&child, code);
                let output = text_of(&captured.stdout, &captured.stderr);
                json!({
                    "ok": code == 0,
                    "state": "finished",
                    "code": if code == 0 { "ok" } else { "faulted" },
                    "output": codec::encode(output.as_bytes()),
                    "presets": self.preset_files(),
                })
            }
            Err(code) => self.fail(&code),
        }
    }

    fn debug_stop(&mut self) -> Value {
        self.kill_child();
        json!({"ok": true, "state": "stopped", "presets": self.preset_files()})
    }

    fn logs(&self, value: &Value) -> Value {
        let id = value["id"].as_str().unwrap_or("");
        let rows: Vec<&Value> = self
            .logs
            .iter()
            .rev()
            .filter(|row| id.is_empty() || row["id"].as_str() == Some(id))
            .take(3)
            .collect();
        json!({"ok": true, "rows": rows, "presets": self.preset_files()})
    }

    fn history(&self, value: &Value) -> Value {
        let Some(script) = self.script(value) else {
            return self.fail("missing");
        };
        json!({"ok": true, "versions": script.versions, "presets": self.preset_files()})
    }

    fn export_script(&self, value: &Value) -> Value {
        let Some(script) = self.script(value) else {
            return self.fail("missing");
        };
        let packed = json!({
            "name": script.name,
            "language": script.language,
            "source": self.read_source(script),
            "permissions": script.declared,
            "triggers": script.triggers,
        });
        let data = codec::encode(&serde_json::to_vec(&packed).unwrap_or_default());
        if data.len() > 700 {
            return self.fail("too_large");
        }
        json!({"ok": true, "data": data, "presets": self.preset_files()})
    }

    fn import_script(&mut self, value: &Value) -> Value {
        let data = value["data"].as_str().unwrap_or("");
        let bytes = match codec::decode(data) {
            Ok(bytes) => bytes,
            Err(()) => return self.fail("rejected"),
        };
        let packed: Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => return self.fail("rejected"),
        };
        let name = packed["name"].as_str().unwrap_or("");
        let language = packed["language"].as_str().unwrap_or("");
        let source = packed["source"].as_str().unwrap_or("");
        if !name_token(name) || !editor::known_language(language) || source.len() > SOURCE_LIMIT {
            return self.fail("rejected");
        }
        let id = format!("s{}", self.next_id);
        self.next_id += 1;
        let mut declared = Vec::new();
        if let Some(items) = packed["permissions"].as_array() {
            for item in items {
                let Some(permission) = item.as_str() else {
                    return self.fail("rejected");
                };
                if !PERMISSIONS.contains(&permission) {
                    return self.fail("undeclared_permission");
                }
                declared.push(permission.to_owned());
            }
        }
        let mut triggers = Vec::new();
        if let Ok(parsed) = serde_json::from_value::<Vec<Trigger>>(packed["triggers"].clone()) {
            triggers = parsed;
        }
        let script = Script {
            id: id.clone(),
            name: name.to_owned(),
            language: language.to_owned(),
            enabled: false,
            paused: false,
            version: 1,
            versions: vec![1],
            pending: declared.clone(),
            declared,
            grants: Vec::new(),
            triggers,
            modules: Vec::new(),
            count: 0,
            condition_fired: false,
            last_schedule: unix_ms(),
            reloaded: false,
        };
        if let Err(code) = self.write_source(&script, source) {
            return self.fail(&code);
        }
        self.scripts.push(script);
        if self.save().is_err() {
            return self.fail("faulted");
        }
        json!({
            "ok": true,
            "id": id,
            "enabled": false,
            "presets": self.preset_files(),
            "scripts": self.scripts.len(),
        })
    }

    fn source_from(&mut self, value: &Value, key: &str) -> Option<Result<String, String>> {
        let data = value["data"].as_str().unwrap_or("");
        if data.len() > 800 {
            return Some(Err("rejected".to_owned()));
        }
        let text = match codec::decode(data) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) if !text.contains('\0') => text,
                _ => return Some(Err("rejected".to_owned())),
            },
            Err(()) => return Some(Err("rejected".to_owned())),
        };
        if let Some(part) = value["part"].as_u64() {
            let slot = self.parts.entry(key.to_owned()).or_default();
            if part as usize != slot.len() {
                return Some(Err("rejected".to_owned()));
            }
            slot.push(text);
            if !value["end"].as_bool().unwrap_or(false) {
                return None;
            }
            let full = self.parts.remove(key).unwrap_or_default().join("");
            if full.len() > SOURCE_LIMIT {
                return Some(Err("rejected".to_owned()));
            }
            return Some(Ok(full));
        }
        if text.len() > SOURCE_LIMIT {
            return Some(Err("rejected".to_owned()));
        }
        Some(Ok(text))
    }

    fn write_source(&self, script: &Script, source: &str) -> Result<(), String> {
        let path = self.source_path(script);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| "faulted".to_owned())?;
        }
        fs::write(path, source).map_err(|_| "faulted".to_owned())
    }

    fn read_source(&self, script: &Script) -> String {
        fs::read_to_string(self.source_path(script)).unwrap_or_default()
    }

    fn source_path(&self, script: &Script) -> PathBuf {
        self.state
            .join("sources")
            .join(format!("{}.{}", script.id, extension(&script.language)))
    }

    fn log(&mut self, id: &str, trigger: &str, status: &str, output: &str) {
        let bytes = output.as_bytes();
        let end = bytes.len().min(80);
        let mut end = end;
        while !output.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        self.logs.push(json!({
            "id": id,
            "trigger": trigger,
            "status": status,
            "output": codec::encode(&bytes[..end]),
        }));
        if self.logs.len() > 64 {
            let extra = self.logs.len() - 64;
            self.logs.drain(0..extra);
        }
    }

    fn save(&self) -> std::io::Result<()> {
        let catalog = Catalog {
            next_id: self.next_id,
            scripts: self.scripts.clone(),
        };
        fs::write(
            self.state.join("catalog.json"),
            serde_json::to_vec(&catalog).map_err(std::io::Error::other)?,
        )?;
        let mut lines = String::new();
        for row in &self.logs {
            lines.push_str(&serde_json::to_string(row).unwrap_or_default());
            lines.push('\n');
        }
        fs::write(self.state.join("logs.jsonl"), lines)?;
        self.write_preset_count()
    }

    fn write_preset_count(&self) -> std::io::Result<()> {
        fs::write(
            self.state.join("preset.count"),
            self.preset_files().to_string(),
        )
    }

    fn preset_files(&self) -> usize {
        fs::read_dir(self.state.join("presets"))
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.scripts.iter().position(|script| script.id == id)
    }

    fn script(&self, value: &Value) -> Option<&Script> {
        let id = value["id"].as_str().unwrap_or("");
        self.scripts.iter().find(|script| script.id == id)
    }

    fn permission_view(&self, id: &str) -> Value {
        let Some(script) = self.scripts.iter().find(|script| script.id == id) else {
            return self.fail("missing");
        };
        json!({
            "ok": true,
            "id": id,
            "grants": script.grants,
            "pending": script.pending,
            "presets": self.preset_files(),
        })
    }

    fn pending(&self) -> Value {
        json!({"ok": true, "pending": true, "presets": self.preset_files()})
    }

    fn fail(&self, code: &str) -> Value {
        json!({"ok": false, "code": code, "presets": self.preset_files()})
    }

    fn kill_held(&mut self, child: &mut ChildIo) {
        let _ = child.child.kill();
        let _ = child.child.wait();
    }
}

fn text_of(stdout: &[u8], stderr: &[u8]) -> String {
    let text = if stdout.is_empty() { stderr } else { stdout };
    String::from_utf8_lossy(text).replace(['\u{0000}'], "")
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn name_token(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'_' | b'-'))
}

fn event_token(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && !name.contains("..")
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'.' | b'-')
        })
}

fn module_token(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

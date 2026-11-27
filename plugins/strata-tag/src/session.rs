//! 标记证据。
//!
//! 进程只保存宿主已经判定的记录。它不写原始报文。
//! `safety` 只能是 `none`：没有标记不是安全结论，有标记也不是放行结论。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Value};

const CODES: &[&str] = &[
    "applied",
    "untagged",
    "memory_only",
    "tag_rejected",
    "unknown_packet",
    "unavailable",
    "paused",
    "policy",
    "none",
];

pub(crate) struct Session {
    state: PathBuf,
    paused: Mutex<bool>,
}

impl Session {
    pub(crate) fn start(state: &Path) -> Self {
        let _ = fs::create_dir_all(state);
        Self {
            state: state.to_path_buf(),
            paused: Mutex::new(false),
        }
    }

    pub(crate) fn pause(&self) {
        *self.paused.lock().expect("tag pause") = true;
    }

    pub(crate) fn resume(&self) {
        *self.paused.lock().expect("tag resume") = false;
    }

    pub(crate) fn release(&self) -> std::io::Result<()> {
        write_private(&self.state.join("released"), b"joined")
    }

    pub(crate) fn apply(&self, payload: &str) -> Result<String, &'static str> {
        let record = accept(payload)?;
        let code = record["code"].as_str().unwrap_or("");
        if *self.paused.lock().expect("tag paused") && code != "paused" {
            return Err("paused");
        }
        self.store(&record)
    }

    pub(crate) fn policy(&self, payload: &str) -> Result<String, &'static str> {
        let record = accept(payload)?;
        if record["code"].as_str() != Some("policy") {
            return Err("rejected");
        }
        self.store(&record)
    }

    pub(crate) fn query(&self, payload: &str) -> Result<String, &'static str> {
        let request: Value = serde_json::from_str(payload).map_err(|_| "rejected")?;
        if !request.is_object() {
            return Err("rejected");
        }
        let subject = request["subject"].as_str().unwrap_or("");
        let packet = request["packet"].as_u64().unwrap_or(0);
        if subject.len() > 256 || subject.chars().any(char::is_control) {
            return Err("rejected");
        }
        let text = fs::read_to_string(self.state.join("tags.jsonl")).unwrap_or_default();
        let mut found = None;
        for line in text.lines().filter(|line| !line.is_empty()) {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let subject_match = !subject.is_empty() && row["subject"].as_str() == Some(subject);
            let packet_match = packet > 0 && row["packet"].as_u64() == Some(packet);
            if subject.is_empty() && packet == 0 || subject_match || packet_match {
                found = Some(row);
            }
        }
        let Some(row) = found else {
            return Ok(json!({
                "ok": true,
                "code": "none",
                "safety": "none",
                "tagged": false
            })
            .to_string());
        };
        let tagged = row["code"].as_str() == Some("applied");
        let mut row = row;
        if let Some(object) = row.as_object_mut() {
            object.insert("tagged".to_owned(), Value::Bool(tagged));
            object.insert("safety".to_owned(), Value::String("none".to_owned()));
        }
        fit(&row)
    }

    fn store(&self, record: &Value) -> Result<String, &'static str> {
        let line = fit(record)?;
        append_line(&self.state.join("tags.jsonl"), &line)?;
        write_private(self.state.join("last.json").as_path(), line.as_bytes())
            .map_err(|_| "faulted")?;
        Ok(line)
    }
}

fn accept(payload: &str) -> Result<Value, &'static str> {
    let record: Value = serde_json::from_str(payload).map_err(|_| "rejected")?;
    if !record.is_object() {
        return Err("rejected");
    }
    let actor = record["actor"].as_str().unwrap_or("");
    if !matches!(actor, "user" | "script" | "plugin" | "agent") {
        return Err("rejected");
    }
    let actor_id = record["actor_id"].as_str().unwrap_or("");
    if !token(actor_id, 256) {
        return Err("rejected");
    }
    if record["safety"].as_str() != Some("none") {
        return Err("rejected");
    }
    let code = record["code"].as_str().unwrap_or("");
    if !CODES.contains(&code) || code == "none" {
        return Err("rejected");
    }
    if record["wrote"].as_bool() == Some(true) && code != "applied" {
        return Err("rejected");
    }
    let kind = record["kind"].as_str().unwrap_or("");
    if !matches!(
        kind,
        "event" | "packet" | "file" | "process" | "software" | "registry" | "system" | "policy"
    ) {
        return Err("rejected");
    }
    Ok(record)
}

fn fit(value: &Value) -> Result<String, &'static str> {
    let text = serde_json::to_string(value).map_err(|_| "invalid_record")?;
    if text.len() > 1024 || text.chars().any(char::is_control) {
        return Err("invalid_record");
    }
    Ok(text)
}

fn token(value: &str, max: usize) -> bool {
    (1..=max).contains(&value.len())
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn append_line(path: &Path, line: &str) -> Result<(), &'static str> {
    let mut existing = match fs::read_to_string(path) {
        Ok(text) => text.lines().map(str::to_owned).collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return Err("faulted"),
    };
    existing.push(line.to_owned());
    if existing.len() > 1024 {
        let skip = existing.len() - 1024;
        existing.drain(0..skip);
    }
    let mut body = existing.join("\n");
    body.push('\n');
    write_private(path, body.as_bytes()).map_err(|_| "faulted")
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

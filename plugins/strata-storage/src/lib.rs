//! 计划 030 的存储插件进程。
//!
//! 它只把阈值、拒绝和提案写进自己的私有目录。确认字段不会删除任何文件。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

pub const PLUGIN_ID: &str = "strata.storage";

pub const MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.storage","name":"StrataStorage","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["storage.configure","storage.decline","storage.propose","storage.status"],"#,
    r#""permissions":["storage.private","plugin.call"],"dependencies":[],"events":[],"rescue":false}"#,
);

const MAX_THRESHOLD: u64 = 1_000_000_000_000_000;

pub struct Desk {
    root: PathBuf,
}

impl Desk {
    pub fn open(root: &Path) -> std::io::Result<Self> {
        if !root.is_dir() {
            return Err(std::io::Error::other("storage state is not a directory"));
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn dispatch(&mut self, payload: &str) -> Result<String, &'static str> {
        if payload.len() > 1024 || payload.chars().any(char::is_control) {
            return Err("rejected");
        }
        let value: Value = serde_json::from_str(payload).map_err(|_| "rejected")?;
        if value.get("confirm").and_then(Value::as_bool) == Some(true)
            || value.get("op").and_then(Value::as_str) == Some("confirm")
        {
            return Ok(json!({
                "ok": true,
                "code": "confirmation_required",
                "deleted": false
            })
            .to_string());
        }
        match value.get("op").and_then(Value::as_str).unwrap_or("status") {
            "configure" => self.configure(&value),
            "decline" => self.decline(&value),
            "propose" => self.propose(&value),
            "status" => self.status(),
            _ => Err("rejected"),
        }
    }

    fn configure(&self, value: &Value) -> Result<String, &'static str> {
        let capture_first = number(value, "capture_first")?;
        let capture_repeat = number(value, "capture_repeat")?;
        let log_first = number(value, "log_first")?;
        let log_repeat = number(value, "log_repeat")?;
        let body = json!({
            "capture_first": capture_first,
            "capture_repeat": capture_repeat,
            "log_first": log_first,
            "log_repeat": log_repeat
        });
        write_private(&self.root.join("policy.json"), &body.to_string())?;
        Ok(json!({
            "ok": true,
            "code": "configured",
            "deleted": false,
            "capture_first": capture_first,
            "capture_repeat": capture_repeat,
            "log_first": log_first,
            "log_repeat": log_repeat
        })
        .to_string())
    }

    fn decline(&self, value: &Value) -> Result<String, &'static str> {
        let category = value.get("category").and_then(Value::as_str).unwrap_or("");
        if !matches!(category, "capture" | "logs") {
            return Err("rejected");
        }
        let body = json!({"category": category});
        write_private(&self.root.join("decline.json"), &body.to_string())?;
        Ok(json!({
            "ok": true,
            "code": "decline",
            "deleted": false,
            "category": category
        })
        .to_string())
    }

    fn propose(&self, value: &Value) -> Result<String, &'static str> {
        let Some(scope) = value.get("scope") else {
            return Err("rejected");
        };
        if !scope.is_object() {
            return Err("rejected");
        }
        let text = scope.to_string();
        if text.len() > 900 {
            return Err("rejected");
        }
        write_private(&self.root.join("proposal-request.json"), &text)?;
        Ok(json!({
            "ok": true,
            "code": "proposal",
            "deleted": false
        })
        .to_string())
    }

    fn status(&self) -> Result<String, &'static str> {
        let configured = self.root.join("policy.json").is_file();
        Ok(json!({
            "ok": true,
            "code": "status",
            "deleted": false,
            "configured": configured
        })
        .to_string())
    }
}

fn write_private(path: &Path, text: &str) -> Result<(), &'static str> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| "faulted")?;
    file.write_all(text.as_bytes()).map_err(|_| "faulted")?;
    file.sync_all().map_err(|_| "faulted")?;
    Ok(())
}

fn number(value: &Value, field: &str) -> Result<u64, &'static str> {
    let Some(number) = value.get(field).and_then(Value::as_u64) else {
        return Err("rejected");
    };
    if (1..=MAX_THRESHOLD).contains(&number) {
        Ok(number)
    } else {
        Err("rejected")
    }
}

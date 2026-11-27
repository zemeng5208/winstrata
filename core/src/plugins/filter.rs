//! 计划 028 的过滤宿主辅助。
//!
//! 基础保护在过滤库里，插件进程没启动时也可以调用。
//! 脚本批次由宿主转给已经在运行的 StrataScript，一次一批，失败则回退。

use serde_json::{Value, json};

use super::PluginError;

pub(super) const PLUGIN: &str = winstrata_strata_filter::PLUGIN_ID;

pub(super) fn beside(runtime: &std::path::Path) -> Option<std::path::PathBuf> {
    super::collector_beside(runtime, "winstrata-strata-filter")
}

pub(super) struct ScriptBatch {
    pub(super) id: String,
    pub(super) count: usize,
}

pub(super) fn script_request(detail: &str) -> Option<ScriptBatch> {
    let value: Value = serde_json::from_str(detail).ok()?;
    if value.get("phase").and_then(Value::as_str) != Some("script") {
        return None;
    }
    let id = value.get("id").and_then(Value::as_str)?.to_owned();
    let count = value.get("count").and_then(Value::as_u64)? as usize;
    if !script_id(&id) || count == 0 || count > 32 {
        return None;
    }
    Some(ScriptBatch { id, count })
}

pub(super) fn verdict_payload(lines: &[String]) -> Result<String, PluginError> {
    let text = serde_json::to_string(&json!({"op": "verdict", "lines": lines}))
        .map_err(|_| PluginError::new("invalid_record", "filter verdict does not encode"))?;
    if text.len() > 1024 || text.chars().any(char::is_control) {
        return Err(PluginError::new(
            "invalid_record",
            "filter verdict does not fit a plugin frame",
        ));
    }
    Ok(text)
}

pub(super) fn fallback_payload(code: &str) -> String {
    let code = if code_token(code) { code } else { "faulted" };
    format!(r#"{{"op":"fallback","code":"{code}"}}"#)
}

pub(super) fn verdict_lines(text: &str) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line != "keep" && line != "drop" {
            return None;
        }
        lines.push(line.to_owned());
    }
    if lines.is_empty() { None } else { Some(lines) }
}

pub(super) fn decode_output(detail: &str) -> Option<String> {
    let value: Value = serde_json::from_str(detail).ok()?;
    if value.get("ok") != Some(&Value::Bool(true)) {
        return None;
    }
    let encoded = value.get("output").and_then(Value::as_str)?;
    let bytes = decode_base64(encoded)?;
    String::from_utf8(bytes).ok()
}

fn script_id(text: &str) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 16
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn code_token(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 40
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if text.len() > 2048 || !text.len().is_multiple_of(4) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let mut value = 0u32;
        let mut pads = 0;
        for shift in (0..4).rev() {
            let byte = *bytes.get(index)?;
            index += 1;
            if byte == b'=' {
                pads += 1;
                continue;
            }
            let digit = TABLE.iter().position(|item| *item == byte)? as u32;
            value |= digit << (shift * 6);
        }
        out.push((value >> 16) as u8);
        if pads < 2 {
            out.push((value >> 8) as u8);
        }
        if pads < 1 {
            out.push(value as u8);
        }
    }
    Some(out)
}

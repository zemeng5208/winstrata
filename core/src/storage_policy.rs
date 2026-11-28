//! 计划 030 的容量阶梯和经确认的删除。
//!
//! 小数 GB 与 MB 仍是候选单位。保存的日志只计算用户目录下 `logs` 里的普通文件。
//! 事件、指标、索引和备份是否计入日志限额尚未确认，见 OI-024。
//! 过期授权和默认清理选择也未确认，因此这里不按过期删除，也不自行挑出要删的分段。
//! 插件帧里的 `confirm` 不能删除数据。只有本模块在 `confirm` 为真且提案标识一致时才删除。

use std::fs;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::files::{self, create_private, open_existing};
use crate::hybrid::{Hybrid, Piece};
use crate::platform::Anchor;

pub(crate) const DECIMAL_GB: u64 = 1_000_000_000;
pub(crate) const DECIMAL_MB: u64 = 1_000_000;
pub(crate) const CAPTURE_FIRST_DEFAULT: u64 = DECIMAL_GB;
pub(crate) const CAPTURE_REPEAT_DEFAULT: u64 = DECIMAL_GB;
pub(crate) const LOG_FIRST_DEFAULT: u64 = 2 * DECIMAL_GB;
pub(crate) const LOG_REPEAT_DEFAULT: u64 = 500 * DECIMAL_MB;

const MAX_THRESHOLD: u64 = 1_000_000_000_000_000;
const ALERTS: &str = "storage-alerts.json";
const AUDIT: &str = "storage-audit.jsonl";

#[derive(Clone, Debug)]
struct Rung {
    first: u64,
    repeat: u64,
    declines: u64,
    pending: bool,
}

impl Rung {
    fn defaults(first: u64, repeat: u64) -> Self {
        Self {
            first,
            repeat,
            declines: 0,
            pending: false,
        }
    }

    fn armed(&self) -> u64 {
        self.first
            .saturating_add(self.declines.saturating_mul(self.repeat))
    }

    fn next(&self) -> u64 {
        self.armed().saturating_add(self.repeat)
    }

    fn crossed(&self, usage: u64) -> u64 {
        if usage < self.first {
            return 0;
        }
        if self.repeat == 0 {
            return 1;
        }
        1 + (usage - self.first) / self.repeat
    }

    fn evaluate(&mut self, usage: u64) {
        self.pending = usage >= self.armed();
    }

    fn decline(&mut self, usage: u64) -> bool {
        if !self.pending {
            return false;
        }
        self.declines = self.declines.saturating_add(1);
        self.evaluate(usage);
        true
    }

    fn recalculate(&mut self, usage: u64) {
        self.declines = 0;
        self.pending = false;
        self.evaluate(usage);
    }
}

#[derive(Clone, Debug)]
struct Proposal {
    id: String,
    request: Value,
    segments: Vec<(u8, u32)>,
    logs: Vec<String>,
}

#[derive(Clone, Debug)]
struct Alerts {
    capture: Rung,
    logs: Rung,
    proposal: Option<Proposal>,
}

impl Default for Alerts {
    fn default() -> Self {
        Self {
            capture: Rung::defaults(CAPTURE_FIRST_DEFAULT, CAPTURE_REPEAT_DEFAULT),
            logs: Rung::defaults(LOG_FIRST_DEFAULT, LOG_REPEAT_DEFAULT),
            proposal: None,
        }
    }
}

#[derive(Deserialize)]
struct StoredAlerts {
    version: u32,
    capture_first: u64,
    capture_repeat: u64,
    capture_declines: u64,
    capture_pending: bool,
    log_first: u64,
    log_repeat: u64,
    log_declines: u64,
    log_pending: bool,
    #[serde(default)]
    proposal_id: Option<String>,
    #[serde(default)]
    proposal_request: Option<String>,
    #[serde(default)]
    proposal_segments: Vec<String>,
    #[serde(default)]
    proposal_logs: Vec<String>,
}

#[derive(Deserialize)]
struct PolicyFile {
    capture_first: u64,
    capture_repeat: u64,
    log_first: u64,
    log_repeat: u64,
}

struct Usage {
    capture: u64,
    logs: u64,
}

struct Resolved {
    deletable: Vec<(u8, u32, u64)>,
    held: Vec<Value>,
    logs: Vec<String>,
    excluded: Vec<String>,
    bytes: u64,
}

pub(crate) fn unavailable() -> String {
    json!({
        "ok": true,
        "storage": "unavailable",
        "deleted": false,
        "pending": false,
        "enabled": false
    })
    .to_string()
}

pub(crate) fn notice(user: &Anchor) -> io::Result<String> {
    let (mut alerts, usage) = prepare(user)?;
    if let Some(category) = take_decline(user)? {
        match category.as_str() {
            "capture" => {
                alerts.capture.decline(usage.capture);
            }
            "logs" => {
                alerts.logs.decline(usage.logs);
            }
            _ => {}
        }
    }
    save(user, &alerts)?;
    Ok(render(&alerts, &usage, false, "notice"))
}

pub(crate) fn configure(
    user: &Anchor,
    capture_first: u64,
    capture_repeat: u64,
    log_first: u64,
    log_repeat: u64,
) -> io::Result<String> {
    check_threshold(capture_first)?;
    check_threshold(capture_repeat)?;
    check_threshold(log_first)?;
    check_threshold(log_repeat)?;
    let (mut alerts, usage) = prepare(user)?;
    alerts.capture.first = capture_first;
    alerts.capture.repeat = capture_repeat;
    alerts.logs.first = log_first;
    alerts.logs.repeat = log_repeat;
    alerts.capture.evaluate(usage.capture);
    alerts.logs.evaluate(usage.logs);
    write_policy_if_present(user, capture_first, capture_repeat, log_first, log_repeat)?;
    save(user, &alerts)?;
    Ok(render(&alerts, &usage, false, "configured"))
}

pub(crate) fn decline(user: &Anchor, category: &str) -> io::Result<String> {
    let (mut alerts, usage) = prepare(user)?;
    let changed = match category {
        "capture" => alerts.capture.decline(usage.capture),
        "logs" => alerts.logs.decline(usage.logs),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "storage category is capture or logs",
            ));
        }
    };
    save(user, &alerts)?;
    let mut body: Value = serde_json::from_str(&render(
        &alerts,
        &usage,
        false,
        if changed { "declined" } else { "not_pending" },
    ))
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    if let Some(object) = body.as_object_mut() {
        object.insert("declined".to_owned(), Value::Bool(changed));
    }
    Ok(body.to_string())
}

pub(crate) fn append_log(user: &Anchor, name: &str, bytes: &[u8]) -> io::Result<u64> {
    if !log_name(name) || bytes.is_empty() || bytes.len() > 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "saved log name or body is not valid",
        ));
    }
    let logs = user.child(std::ffi::OsStr::new("logs"), true, true)?;
    let path = logs.io_path().join(name);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "saved log is not a regular file",
            ));
        }
        let mut file = open_existing(&path, true)?;
        file.seek(SeekFrom::End(0))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    } else {
        let mut file = create_private(&path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    directory_bytes(&logs)
}

pub(crate) fn propose(user: &Anchor, store: &Hybrid, request: &str) -> io::Result<String> {
    if request.len() > 4096 || request.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage proposal does not fit",
        ));
    }
    let value: Value = serde_json::from_str(request)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "storage proposal is not JSON"))?;
    if value.get("confirm").and_then(Value::as_bool) == Some(true) {
        return Ok(json!({
            "ok": true,
            "code": "confirmation_required",
            "deleted": false,
            "storage": "ready"
        })
        .to_string());
    }
    let (mut alerts, usage) = prepare(user)?;
    let op = value.get("op").and_then(Value::as_str).unwrap_or("");
    if op == "expiration" {
        alerts.proposal = None;
        save(user, &alerts)?;
        return Ok(json!({
            "ok": true,
            "code": "expiration_unconfirmed",
            "deleted": false,
            "storage": "ready",
            "capture": rung_json(&alerts.capture, usage.capture),
            "logs": rung_json(&alerts.logs, usage.logs)
        })
        .to_string());
    }
    if op == "capacity" {
        alerts.proposal = None;
        save(user, &alerts)?;
        return Ok(json!({
            "ok": true,
            "code": "selection_required",
            "deleted": false,
            "storage": "ready",
            "capture": rung_json(&alerts.capture, usage.capture),
            "logs": rung_json(&alerts.logs, usage.logs)
        })
        .to_string());
    }
    let resolved = resolve(&store.pieces()?, &log_records(user)?, &value)?;
    let id = proposal_id(&resolved);
    alerts.proposal = Some(Proposal {
        id: id.clone(),
        request: value,
        segments: resolved
            .deletable
            .iter()
            .map(|(category, segment, _)| (*category, *segment))
            .collect(),
        logs: resolved.logs.clone(),
    });
    save(user, &alerts)?;
    Ok(proposal_json(&alerts, &usage, &resolved, &id))
}

pub(crate) fn confirm(
    user: &Anchor,
    store: &mut Hybrid,
    proposal: &str,
    confirm: bool,
) -> io::Result<String> {
    if proposal.len() > 64 || !proposal.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage proposal id is not valid",
        ));
    }
    let (mut alerts, usage) = prepare(user)?;
    if !confirm {
        save(user, &alerts)?;
        return Ok(json!({
            "ok": true,
            "code": "confirmation_required",
            "deleted": false,
            "storage": "ready",
            "capture": rung_json(&alerts.capture, usage.capture),
            "logs": rung_json(&alerts.logs, usage.logs)
        })
        .to_string());
    }
    let Some(stored) = alerts.proposal.clone() else {
        return Ok(refusal("missing", &alerts, &usage));
    };
    if stored.id != proposal {
        return Ok(refusal("stale", &alerts, &usage));
    }
    let resolved = resolve(&store.pieces()?, &log_records(user)?, &stored.request)?;
    let fresh = proposal_id(&resolved);
    let same_logs = resolved.logs == stored.logs;
    let same_segments = resolved
        .deletable
        .iter()
        .map(|(category, segment, _)| (*category, *segment))
        .eq(stored.segments.iter().copied());
    if fresh != stored.id || !same_logs || !same_segments {
        return Ok(refusal("stale", &alerts, &usage));
    }
    if stored.segments.is_empty() && stored.logs.is_empty() {
        alerts.proposal = None;
        save(user, &alerts)?;
        return Ok(refusal("empty", &alerts, &usage));
    }
    let commit = if stored.segments.is_empty() {
        store.commits()
    } else {
        store.drop_segments(&stored.segments)?
    };
    for name in &stored.logs {
        let path = user.io_path().join("logs").join(name);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "saved log is not a regular file",
            ));
        }
        fs::remove_file(&path)?;
    }
    append_audit(user, commit, &stored)?;
    let usage = measure(user)?;
    alerts.capture.recalculate(usage.capture);
    alerts.logs.recalculate(usage.logs);
    alerts.proposal = None;
    save(user, &alerts)?;
    Ok(render(&alerts, &usage, true, "deleted"))
}

fn prepare(user: &Anchor) -> io::Result<(Alerts, Usage)> {
    let mut alerts = load(user)?;
    let usage = measure(user)?;
    if let Some(policy) = read_policy(user)? {
        alerts.capture.first = policy.capture_first;
        alerts.capture.repeat = policy.capture_repeat;
        alerts.logs.first = policy.log_first;
        alerts.logs.repeat = policy.log_repeat;
    }
    alerts.capture.evaluate(usage.capture);
    alerts.logs.evaluate(usage.logs);
    Ok((alerts, usage))
}

fn check_threshold(value: u64) -> io::Result<()> {
    if (1..=MAX_THRESHOLD).contains(&value) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage threshold is outside 1 byte through 1 PB",
        ))
    }
}

fn measure(user: &Anchor) -> io::Result<Usage> {
    Ok(Usage {
        capture: optional_directory_bytes(&user.io_path().join("data").join("captures"))?,
        logs: optional_directory_bytes(&user.io_path().join("logs"))?,
    })
}

fn optional_directory_bytes(path: &Path) -> io::Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            continue;
        }
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "storage usage overflow"))?;
    }
    Ok(total)
}

fn directory_bytes(dir: &Anchor) -> io::Result<u64> {
    optional_directory_bytes(dir.io_path())
}

fn log_records(user: &Anchor) -> io::Result<Vec<(String, u64, i64)>> {
    let path = user.io_path().join("logs");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !log_name(name) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            continue;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|time| i64::try_from(time.as_nanos()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        found.push((name.to_owned(), metadata.len(), modified));
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(found)
}

fn log_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (5..=48).contains(&bytes.len())
        && name.ends_with(".log")
        && !name.contains("..")
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn resolve(pieces: &[Piece], logs: &[(String, u64, i64)], request: &Value) -> io::Result<Resolved> {
    let op = request.get("op").and_then(Value::as_str).unwrap_or("");
    if !matches!(op, "selection" | "script" | "type" | "time" | "risk") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage scope is selection, script, type, time, or risk",
        ));
    }
    let protected = string_list(request, "protected", 32, 128)?;
    let ids = string_list(request, "ids", 32, 256)?;
    let class = request.get("class").and_then(Value::as_str).unwrap_or("");
    let start = request.get("start_ns").and_then(Value::as_i64);
    let end = request.get("end_ns").and_then(Value::as_i64);
    let risk_source = request
        .get("risk_source")
        .and_then(Value::as_str)
        .unwrap_or("");
    let risk_reason = request
        .get("risk_reason")
        .and_then(Value::as_str)
        .unwrap_or("");
    if op == "time" && (start.is_none() || end.is_none() || start > end) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage time range is reversed",
        ));
    }
    if op == "risk" && risk_source.is_empty() && risk_reason.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage risk scope needs a source or a reason",
        ));
    }
    if op == "type" && category_code(class).is_none() && class != "logs" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage type is not a data class",
        ));
    }
    if matches!(op, "selection" | "script") && ids.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage selection needs at least one id",
        ));
    }
    let mut groups: Vec<((u8, u32), Vec<&Piece>)> = Vec::new();
    for piece in pieces {
        if let Some((_, items)) = groups
            .iter_mut()
            .find(|(key, _)| *key == (piece.category, piece.segment))
        {
            items.push(piece);
        } else {
            groups.push(((piece.category, piece.segment), vec![piece]));
        }
    }
    let mut deletable = Vec::new();
    let mut held = Vec::new();
    for ((category, segment), items) in groups {
        let protected_hit = items.iter().any(|piece| {
            piece
                .risk_reason
                .as_deref()
                .is_some_and(|value| protected.iter().any(|item| item == value))
                || piece
                    .risk_source
                    .as_deref()
                    .is_some_and(|value| protected.iter().any(|item| item == value))
        });
        let matched = items
            .iter()
            .filter(|piece| {
                record_matches(piece, op, class, &ids, start, end, risk_source, risk_reason)
            })
            .count();
        if matched == 0 {
            continue;
        }
        let bytes = items.first().map(|piece| piece.segment_bytes).unwrap_or(0);
        if protected_hit || matched != items.len() {
            held.push(json!({
                "category": category_name(category),
                "segment": segment,
                "reason": if protected_hit { "protected" } else { "partial" }
            }));
        } else {
            deletable.push((category, segment, bytes));
        }
    }
    deletable.sort_by_key(|(category, segment, _)| (*category, *segment));
    deletable.dedup_by_key(|(category, segment, _)| (*category, *segment));
    let mut chosen_logs = Vec::new();
    let mut excluded = Vec::new();
    if matches!(op, "selection" | "script") {
        for id in &ids {
            if !id.ends_with(".log") {
                continue;
            }
            if !log_name(id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "saved log name is not valid",
                ));
            }
            if logs.iter().any(|(name, _, _)| name == id) {
                chosen_logs.push(id.clone());
            } else {
                excluded.push(id.clone());
            }
        }
    } else if op == "type" && class == "logs" {
        chosen_logs.extend(logs.iter().map(|(name, _, _)| name.clone()));
    } else if op == "time" {
        let start = start.unwrap_or(0);
        let end = end.unwrap_or(0);
        for (name, _, modified) in logs {
            if (start..=end).contains(modified) {
                chosen_logs.push(name.clone());
            }
        }
    }
    chosen_logs.sort();
    chosen_logs.dedup();
    let mut bytes = chosen_logs
        .iter()
        .filter_map(|name| logs.iter().find(|(item, _, _)| item == name))
        .fold(0u64, |total, (_, len, _)| total.saturating_add(*len));
    for (_, _, len) in &deletable {
        bytes = bytes.saturating_add(*len);
    }
    Ok(Resolved {
        deletable,
        held,
        logs: chosen_logs,
        excluded,
        bytes,
    })
}

fn record_matches(
    piece: &Piece,
    op: &str,
    class: &str,
    ids: &[String],
    start: Option<i64>,
    end: Option<i64>,
    risk_source: &str,
    risk_reason: &str,
) -> bool {
    match op {
        "selection" | "script" => ids.iter().any(|id| id == &piece.id),
        "type" => category_code(class) == Some(piece.category),
        "time" => piece
            .unix_ns
            .is_some_and(|time| (start.unwrap_or(0)..=end.unwrap_or(0)).contains(&time)),
        "risk" => {
            let source_ok =
                risk_source.is_empty() || piece.risk_source.as_deref() == Some(risk_source);
            let reason_ok =
                risk_reason.is_empty() || piece.risk_reason.as_deref() == Some(risk_reason);
            piece.category == 3
                && source_ok
                && reason_ok
                && !(risk_source.is_empty() && risk_reason.is_empty())
        }
        _ => false,
    }
}

fn string_list(
    request: &Value,
    field: &str,
    max_items: usize,
    max_len: usize,
) -> io::Result<Vec<String>> {
    let Some(value) = request.get(field) else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage list is not an array",
        ));
    };
    if items.len() > max_items {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage list is too long",
        ));
    }
    let mut out = Vec::new();
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "storage list item is not text",
            ));
        };
        if text.is_empty()
            || text.len() > max_len
            || text.chars().any(char::is_control)
            || text.contains("..")
            || text.contains('/')
            || text.contains('\\')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "storage list item is not valid",
            ));
        }
        out.push(text.to_owned());
    }
    Ok(out)
}

fn category_name(category: u8) -> &'static str {
    match category {
        0 => "structured",
        1 => "events",
        2 => "metrics",
        3 => "captures",
        4 => "snapshots",
        _ => "unknown",
    }
}

fn category_code(name: &str) -> Option<u8> {
    Some(match name {
        "structured" | "databases" => 0,
        "events" => 1,
        "metrics" => 2,
        "captures" => 3,
        "snapshots" => 4,
        _ => return None,
    })
}

fn proposal_id(resolved: &Resolved) -> String {
    let mut text = String::new();
    for (category, segment, bytes) in &resolved.deletable {
        text.push_str(&format!("{category}:{segment}:{bytes},"));
    }
    text.push('|');
    for name in &resolved.logs {
        text.push_str(name);
        text.push(',');
    }
    format!("{:016x}", fnv(text.as_bytes()))
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn proposal_json(alerts: &Alerts, usage: &Usage, resolved: &Resolved, id: &str) -> String {
    json!({
        "ok": true,
        "code": "proposal",
        "deleted": false,
        "storage": "ready",
        "id": id,
        "bytes": resolved.bytes,
        "deletable": resolved.deletable.iter().map(|(category, segment, bytes)| json!({
            "category": category_name(*category),
            "segment": segment,
            "bytes": bytes
        })).collect::<Vec<_>>(),
        "held": resolved.held,
        "logs": resolved.logs,
        "excluded": resolved.excluded,
        "capture": rung_json(&alerts.capture, usage.capture),
        "logs_notice": rung_json(&alerts.logs, usage.logs)
    })
    .to_string()
}

fn render(alerts: &Alerts, usage: &Usage, deleted: bool, code: &str) -> String {
    json!({
        "ok": true,
        "code": code,
        "deleted": deleted,
        "storage": "ready",
        "pending": alerts.capture.pending || alerts.logs.pending,
        "proposal": alerts.proposal.as_ref().map(|item| item.id.clone()),
        "capture": rung_json(&alerts.capture, usage.capture),
        "logs": rung_json(&alerts.logs, usage.logs)
    })
    .to_string()
}

fn refusal(code: &str, alerts: &Alerts, usage: &Usage) -> String {
    json!({
        "ok": true,
        "code": code,
        "deleted": false,
        "storage": "ready",
        "capture": rung_json(&alerts.capture, usage.capture),
        "logs": rung_json(&alerts.logs, usage.logs)
    })
    .to_string()
}

fn rung_json(rung: &Rung, usage: u64) -> Value {
    json!({
        "usage": usage,
        "first": rung.first,
        "repeat": rung.repeat,
        "declines": rung.declines,
        "armed": rung.armed(),
        "pending": rung.pending,
        "crossed": rung.crossed(usage),
        "next": rung.next()
    })
}

fn load(user: &Anchor) -> io::Result<Alerts> {
    match files::read_json::<StoredAlerts>(user, ALERTS) {
        Ok(stored) => {
            if stored.version != 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "storage alert version is not supported",
                ));
            }
            check_threshold(stored.capture_first)?;
            check_threshold(stored.capture_repeat)?;
            check_threshold(stored.log_first)?;
            check_threshold(stored.log_repeat)?;
            let request = match stored.proposal_request {
                Some(text) => Some(serde_json::from_str(&text).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "stored proposal is not JSON")
                })?),
                None => None,
            };
            let proposal = match (stored.proposal_id, request) {
                (Some(id), Some(request)) => Some(Proposal {
                    id,
                    request,
                    segments: stored
                        .proposal_segments
                        .iter()
                        .map(|item| parse_segment(item))
                        .collect::<io::Result<Vec<_>>>()?,
                    logs: stored.proposal_logs,
                }),
                _ => None,
            };
            Ok(Alerts {
                capture: Rung {
                    first: stored.capture_first,
                    repeat: stored.capture_repeat,
                    declines: stored.capture_declines,
                    pending: stored.capture_pending,
                },
                logs: Rung {
                    first: stored.log_first,
                    repeat: stored.log_repeat,
                    declines: stored.log_declines,
                    pending: stored.log_pending,
                },
                proposal,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Alerts::default()),
        Err(error) => Err(error),
    }
}

fn parse_segment(text: &str) -> io::Result<(u8, u32)> {
    let Some((category, segment)) = text.split_once(':') else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stored segment is not valid",
        ));
    };
    let category = category_code(category).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "stored segment category is not valid",
        )
    })?;
    let segment = segment.parse::<u32>().map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidData, "stored segment id is not valid")
    })?;
    if segment == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stored segment id is not valid",
        ));
    }
    Ok((category, segment))
}

fn save(user: &Anchor, alerts: &Alerts) -> io::Result<()> {
    let (proposal_id, proposal_request, proposal_segments, proposal_logs) =
        if let Some(proposal) = &alerts.proposal {
            (
                Some(proposal.id.clone()),
                Some(proposal.request.to_string()),
                proposal
                    .segments
                    .iter()
                    .map(|(category, segment)| format!("{}:{segment}", category_name(*category)))
                    .collect(),
                proposal.logs.clone(),
            )
        } else {
            (None, None, Vec::new(), Vec::new())
        };
    let body = json!({
        "version": 1,
        "capture_first": alerts.capture.first,
        "capture_repeat": alerts.capture.repeat,
        "capture_declines": alerts.capture.declines,
        "capture_pending": alerts.capture.pending,
        "log_first": alerts.logs.first,
        "log_repeat": alerts.logs.repeat,
        "log_declines": alerts.logs.declines,
        "log_pending": alerts.logs.pending,
        "proposal_id": proposal_id,
        "proposal_request": proposal_request,
        "proposal_segments": proposal_segments,
        "proposal_logs": proposal_logs
    });
    files::write_json(user, ALERTS, &body)
}

fn policy_path(user: &Anchor) -> std::path::PathBuf {
    user.io_path()
        .join("plugins")
        .join("state")
        .join("strata.storage")
        .join("policy.json")
}

fn read_policy(user: &Anchor) -> io::Result<Option<PolicyFile>> {
    let path = policy_path(user);
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    if text.len() > 1024 || text.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage policy file is not valid",
        ));
    }
    let policy: PolicyFile = serde_json::from_str(&text).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage policy file is not valid",
        )
    })?;
    check_threshold(policy.capture_first)?;
    check_threshold(policy.capture_repeat)?;
    check_threshold(policy.log_first)?;
    check_threshold(policy.log_repeat)?;
    Ok(Some(policy))
}

fn write_policy_if_present(
    user: &Anchor,
    capture_first: u64,
    capture_repeat: u64,
    log_first: u64,
    log_repeat: u64,
) -> io::Result<()> {
    let path = policy_path(user);
    if !path.is_file() {
        return Ok(());
    }
    let body = json!({
        "capture_first": capture_first,
        "capture_repeat": capture_repeat,
        "log_first": log_first,
        "log_repeat": log_repeat
    });
    let text = body.to_string();
    let mut file = open_existing(&path, true)?;
    file.set_len(0)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn take_decline(user: &Anchor) -> io::Result<Option<String>> {
    let path = user
        .io_path()
        .join("plugins")
        .join("state")
        .join("strata.storage")
        .join("decline.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    fs::remove_file(&path)?;
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(value
        .get("category")
        .and_then(Value::as_str)
        .map(str::to_owned))
}

fn append_audit(user: &Anchor, commit: u64, proposal: &Proposal) -> io::Result<()> {
    let path = user.io_path().join(AUDIT);
    let line = json!({
        "commit": commit,
        "segments": proposal.segments.iter().map(|(category, segment)| json!({
            "category": category_name(*category),
            "segment": segment
        })).collect::<Vec<_>>(),
        "logs": proposal.logs,
        "proposal": proposal.id
    });
    let mut encoded = line.to_string();
    encoded.push('\n');
    if path.is_file() {
        let mut file = open_existing(&path, true)?;
        file.seek(SeekFrom::End(0))?;
        file.write_all(encoded.as_bytes())?;
        file.sync_all()?;
    } else {
        let mut file = create_private(&path)?;
        file.write_all(encoded.as_bytes())?;
        file.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Rung;

    #[test]
    fn decline_moves_one_step_and_keeps_residual_overage() {
        let mut rung = Rung::defaults(1_000, 1_000);
        rung.evaluate(2_500);
        assert!(rung.pending);
        assert_eq!(rung.crossed(2_500), 2);
        assert_eq!(rung.armed(), 1_000);
        assert!(rung.decline(2_500));
        assert_eq!(rung.declines, 1);
        assert_eq!(rung.armed(), 2_000);
        assert!(rung.pending);
        assert!(rung.decline(2_500));
        assert_eq!(rung.armed(), 3_000);
        assert!(!rung.pending);
        assert!(!rung.decline(2_500));
    }

    #[test]
    fn cleanup_recalculates_from_the_first_threshold() {
        let mut rung = Rung::defaults(1_000, 500);
        rung.declines = 4;
        rung.recalculate(1_200);
        assert_eq!(rung.declines, 0);
        assert_eq!(rung.armed(), 1_000);
        assert!(rung.pending);
        rung.recalculate(400);
        assert!(!rung.pending);
    }

    #[test]
    fn defaults_use_the_decimal_candidate() {
        let capture = Rung::defaults(super::CAPTURE_FIRST_DEFAULT, super::CAPTURE_REPEAT_DEFAULT);
        let logs = Rung::defaults(super::LOG_FIRST_DEFAULT, super::LOG_REPEAT_DEFAULT);
        assert_eq!(capture.first, 1_000_000_000);
        assert_eq!(capture.repeat, 1_000_000_000);
        assert_eq!(logs.first, 2_000_000_000);
        assert_eq!(logs.repeat, 500_000_000);
        assert!(!capture.pending);
    }
}

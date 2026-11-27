//! StrataFilter 的基础保护和批量策略。
//!
//! 插件进程没有启动时，`protect` 仍然丢掉自身普通写入、日志、IPC，
//! 以及未标记的原始包，并保留外部进程对 WinStrata 路径的修改。
//! 脚本结论不能把这些已经丢掉的事件加回来。

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub const PLUGIN_ID: &str = "strata.filter";

// 清单与过滤程序共同描述计划 028。宿主不通过第三方安装写入这个身份。
pub const MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.filter","name":"StrataFilter","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["filter.apply"],"permissions":["storage.private","plugin.call"],"#,
    r#""dependencies":[],"events":[],"rescue":false}"#,
);

const MAX_RULES: usize = 16;
const MAX_BUFFER: usize = 32;
const MAX_FIELD: usize = 160;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    SelfProcess,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ordinary,
    Outside,
    Traffic,
    Metric,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub event: String,
    pub path: String,
    pub process: String,
    pub app: String,
    pub protocol: String,
    pub ip: String,
    pub port: String,
    pub iface: String,
    pub actor: Actor,
    pub kind: Kind,
    pub critical: bool,
    pub tagged: bool,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    field: String,
    mode: String,
    value: String,
}

#[derive(Clone, Debug)]
struct Policy {
    rules: Vec<Rule>,
    dedup: bool,
    aggregate: bool,
    sample: u64,
    interval_ms: u64,
    buffer: usize,
    compress: bool,
    archive: String,
    script: String,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            dedup: true,
            aggregate: false,
            sample: 1,
            interval_ms: 0,
            buffer: 8,
            compress: true,
            archive: "keep".to_owned(),
            script: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Counts {
    seen: u64,
    kept: u64,
    suppressed: u64,
    excluded: u64,
    sampled: u64,
    deduped: u64,
    aggregated: u64,
    archived: u64,
    scripted: u64,
    raw: u64,
    stored: u64,
    batches: u64,
    runs: u64,
    fallbacks: u64,
}

#[derive(Clone, Debug)]
struct Staged {
    line: String,
    event: String,
    path: String,
    metric: bool,
}

pub struct Engine {
    root: PathBuf,
    policy: Policy,
    paused: bool,
    buffer: Vec<Staged>,
    seen_keys: BTreeSet<String>,
    counts: Counts,
    batch_no: u32,
    last_batch: String,
    awaiting: bool,
    sample_cursor: u64,
    staged_at: Option<Instant>,
}

impl Engine {
    pub fn open(root: &Path) -> std::io::Result<Self> {
        fs::create_dir_all(root.join("batches"))?;
        let engine = Self {
            root: root.to_path_buf(),
            policy: Policy::default(),
            paused: false,
            buffer: Vec::new(),
            seen_keys: BTreeSet::new(),
            counts: Counts::default(),
            batch_no: 0,
            last_batch: String::new(),
            awaiting: false,
            sample_cursor: 0,
            staged_at: None,
        };
        engine
            .write_stats()
            .map_err(|code| std::io::Error::other(code))?;
        Ok(engine)
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn dispatch(&mut self, payload: &str) -> Result<String, &'static str> {
        let value: Value = serde_json::from_str(payload).map_err(|_| "rejected")?;
        if !value.is_object() {
            return Err("rejected");
        }
        let op = value["op"].as_str().unwrap_or("");
        let result = match op {
            "show" => self.show(),
            "clear" => self.clear_rules(),
            "rule" => self.add_rule(&value),
            "tune" => self.tune(&value),
            "script" => self.set_script(&value),
            "apply" => self.apply(&value),
            "flush" => self.flush_now(),
            "verdict" => self.verdict(&value),
            "fallback" => self.fallback(&value),
            _ => return Err("rejected"),
        }?;
        let text = serde_json::to_string(&result).map_err(|_| "invalid_record")?;
        if text.len() > 1024 || text.chars().any(char::is_control) {
            return Err("invalid_record");
        }
        Ok(text)
    }

    fn show(&self) -> Result<Value, &'static str> {
        Ok(self.stats_json("show"))
    }

    fn clear_rules(&mut self) -> Result<Value, &'static str> {
        self.policy.rules.clear();
        Ok(self.stats_json("clear"))
    }

    fn add_rule(&mut self, value: &Value) -> Result<Value, &'static str> {
        let field = token(value, "field")?;
        let mode = token(value, "mode")?;
        let pattern = field_text(value, "value")?;
        if !matches!(
            field.as_str(),
            "process" | "app" | "path" | "event" | "protocol" | "ip" | "port" | "iface"
        ) || !matches!(mode.as_str(), "include" | "exclude")
            || pattern.is_empty()
        {
            return Err("rejected");
        }
        let same = self
            .policy
            .rules
            .iter()
            .filter(|rule| rule.field == field)
            .count();
        if same >= MAX_RULES {
            return Err("rejected");
        }
        if !self
            .policy
            .rules
            .iter()
            .any(|rule| rule.field == field && rule.mode == mode && rule.value == pattern)
        {
            self.policy.rules.push(Rule {
                field,
                mode,
                value: pattern,
            });
        }
        Ok(self.stats_json("rule"))
    }

    fn tune(&mut self, value: &Value) -> Result<Value, &'static str> {
        if let Some(dedup) = value.get("dedup") {
            self.policy.dedup = dedup.as_bool().ok_or("rejected")?;
        }
        if let Some(aggregate) = value.get("aggregate") {
            self.policy.aggregate = aggregate.as_bool().ok_or("rejected")?;
        }
        if let Some(sample) = value.get("sample") {
            let sample = sample.as_u64().ok_or("rejected")?;
            if !(1..=32).contains(&sample) {
                return Err("rejected");
            }
            self.policy.sample = sample;
        }
        if let Some(interval) = value.get("interval_ms") {
            let interval = interval.as_u64().ok_or("rejected")?;
            if interval > 60_000 {
                return Err("rejected");
            }
            self.policy.interval_ms = interval;
        }
        if let Some(buffer) = value.get("buffer") {
            let buffer = buffer.as_u64().ok_or("rejected")?;
            if !(1..=MAX_BUFFER as u64).contains(&buffer) {
                return Err("rejected");
            }
            self.policy.buffer = buffer as usize;
        }
        if let Some(compress) = value.get("compress") {
            self.policy.compress = compress.as_bool().ok_or("rejected")?;
        }
        if value.get("archive").is_some() {
            let archive = token(value, "archive")?;
            if !matches!(archive.as_str(), "keep" | "drop-metric") {
                return Err("rejected");
            }
            self.policy.archive = archive;
        }
        Ok(self.stats_json("tune"))
    }

    fn set_script(&mut self, value: &Value) -> Result<Value, &'static str> {
        let id = value["id"].as_str().unwrap_or("");
        if !id.is_empty() && !script_id(id) {
            return Err("rejected");
        }
        self.policy.script = id.to_owned();
        Ok(self.stats_json("script"))
    }

    fn apply(&mut self, value: &Value) -> Result<Value, &'static str> {
        if self.paused {
            return Ok(self.stats_json("paused"));
        }
        if self.awaiting {
            return Err("pending_script");
        }
        let items = value["items"].as_array().ok_or("rejected")?;
        if items.is_empty() || items.len() > MAX_BUFFER {
            return Err("rejected");
        }
        if self.interval_due() {
            self.finish_basic()?;
        }
        for item in items {
            self.push_item(&parse_item(item)?)?;
        }
        if self.buffer.len() >= self.policy.buffer {
            return self.finish();
        }
        Ok(self.stats_json("apply"))
    }

    fn flush_now(&mut self) -> Result<Value, &'static str> {
        if self.paused {
            return Ok(self.stats_json("paused"));
        }
        if self.awaiting {
            return Err("pending_script");
        }
        self.finish()
    }

    fn verdict(&mut self, value: &Value) -> Result<Value, &'static str> {
        if !self.awaiting {
            return Err("rejected");
        }
        let lines = value["lines"].as_array().ok_or("rejected")?;
        if lines.len() != self.buffer.len() {
            return Err("rejected");
        }
        let mut kept = Vec::new();
        for (staged, line) in self.buffer.iter().zip(lines) {
            match line.as_str() {
                Some("keep") => kept.push(staged.clone()),
                Some("drop") => self.counts.scripted += 1,
                _ => return Err("rejected"),
            }
        }
        self.buffer = kept;
        self.awaiting = false;
        self.counts.runs += 1;
        self.write_buffer()?;
        Ok(self.stats_json("verdict"))
    }

    fn fallback(&mut self, value: &Value) -> Result<Value, &'static str> {
        if !self.awaiting {
            return Err("rejected");
        }
        let code = token(value, "code")?;
        if code.is_empty() {
            return Err("rejected");
        }
        self.awaiting = false;
        self.counts.fallbacks += 1;
        let line = format!("code={code} count={}", self.buffer.len());
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("fallback.log"))
            .map_err(|_| "faulted")?;
        writeln!(file, "{line}").map_err(|_| "faulted")?;
        self.write_buffer()?;
        Ok(self.stats_json("fallback"))
    }

    fn push_item(&mut self, item: &Item) -> Result<(), &'static str> {
        self.counts.seen += 1;
        if !protect(item) || self.own_batch(item) {
            self.counts.suppressed += 1;
            return Ok(());
        }
        if self.policy.archive == "drop-metric" && item.kind == Kind::Metric && !item.critical {
            self.counts.archived += 1;
            return Ok(());
        }
        if !self.admits(item) {
            self.counts.excluded += 1;
            return Ok(());
        }
        if !item.critical && self.policy.sample > 1 {
            self.sample_cursor += 1;
            if self.sample_cursor % self.policy.sample != 0 {
                self.counts.sampled += 1;
                return Ok(());
            }
        }
        let key = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            item.event,
            item.path,
            item.process,
            item.app,
            item.protocol,
            item.ip,
            item.port,
            item.iface
        );
        if self.policy.dedup && !self.seen_keys.insert(key) {
            self.counts.deduped += 1;
            return Ok(());
        }
        if self.policy.aggregate
            && item.kind == Kind::Metric
            && !item.critical
            && let Some(last) = self.buffer.last_mut()
            && last.metric
            && last.event == item.event
            && last.path == item.path
        {
            self.counts.aggregated += 1;
            return Ok(());
        }
        self.buffer.push(Staged {
            line: format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                item.event,
                item.path,
                item.process,
                item.app,
                item.protocol,
                item.ip,
                item.port,
                item.iface,
                item.seq
            ),
            event: item.event.clone(),
            path: item.path.clone(),
            metric: item.kind == Kind::Metric,
        });
        if self.staged_at.is_none() {
            self.staged_at = Some(Instant::now());
        }
        Ok(())
    }

    fn admits(&self, item: &Item) -> bool {
        for rule in &self.policy.rules {
            if rule.mode == "exclude" && field_matches(item, &rule.field, &rule.value) {
                return false;
            }
        }
        for field in [
            "process", "app", "path", "event", "protocol", "ip", "port", "iface",
        ] {
            let includes: Vec<_> = self
                .policy
                .rules
                .iter()
                .filter(|rule| rule.field == field && rule.mode == "include")
                .collect();
            if includes.is_empty() {
                continue;
            }
            if !includes
                .iter()
                .any(|rule| field_matches(item, field, &rule.value))
            {
                return false;
            }
        }
        true
    }

    fn own_batch(&self, item: &Item) -> bool {
        item.actor == Actor::SelfProcess
            && !self.last_batch.is_empty()
            && item.path.ends_with(&self.last_batch)
    }

    fn interval_due(&self) -> bool {
        self.policy.interval_ms > 0
            && !self.buffer.is_empty()
            && self
                .staged_at
                .is_some_and(|at| at.elapsed() >= Duration::from_millis(self.policy.interval_ms))
    }

    fn finish(&mut self) -> Result<Value, &'static str> {
        if self.buffer.is_empty() {
            return Ok(self.stats_json("flush"));
        }
        if !self.policy.script.is_empty() {
            self.awaiting = true;
            let text = self
                .buffer
                .iter()
                .map(|item| item.line.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(self.root.join("staged.txt"), text).map_err(|_| "faulted")?;
            let mut body = self.stats_json("script");
            body["phase"] = json!("script");
            body["id"] = json!(self.policy.script);
            body["count"] = json!(self.buffer.len());
            return Ok(body);
        }
        self.finish_basic()?;
        Ok(self.stats_json("flush"))
    }

    fn finish_basic(&mut self) -> Result<(), &'static str> {
        self.write_buffer()
    }

    fn write_buffer(&mut self) -> Result<(), &'static str> {
        if self.buffer.is_empty() {
            self.seen_keys.clear();
            self.staged_at = None;
            let _ = fs::remove_file(self.root.join("staged.txt"));
            self.write_stats()?;
            return Ok(());
        }
        let lines: Vec<String> = self.buffer.iter().map(|item| item.line.clone()).collect();
        let bytes = pack(&lines, self.policy.compress);
        self.batch_no = self.batch_no.saturating_add(1);
        let name = format!("b{:04}.wfb", self.batch_no);
        let path = self.root.join("batches").join(&name);
        let mut file = File::create(&path).map_err(|_| "faulted")?;
        file.write_all(&bytes).map_err(|_| "faulted")?;
        file.sync_all().map_err(|_| "faulted")?;
        self.counts.kept += lines.len() as u64;
        self.counts.raw += lines.iter().map(|line| line.len() as u64 + 1).sum::<u64>();
        self.counts.stored += bytes.len() as u64;
        self.counts.batches += 1;
        self.last_batch = name;
        self.buffer.clear();
        self.seen_keys.clear();
        self.staged_at = None;
        self.awaiting = false;
        let _ = fs::remove_file(self.root.join("staged.txt"));
        self.write_stats()?;
        Ok(())
    }

    fn write_stats(&self) -> Result<(), &'static str> {
        let text = serde_json::to_string(&self.stats_json("stats")).map_err(|_| "faulted")?;
        fs::write(self.root.join("stats.json"), text).map_err(|_| "faulted")?;
        Ok(())
    }

    fn stats_json(&self, phase: &str) -> Value {
        let avoided = self
            .counts
            .seen
            .saturating_sub(self.counts.kept + self.buffer.len() as u64);
        json!({
            "ok": true,
            "phase": phase,
            "seen": self.counts.seen,
            "kept": self.counts.kept,
            "suppressed": self.counts.suppressed,
            "excluded": self.counts.excluded,
            "sampled": self.counts.sampled,
            "deduped": self.counts.deduped,
            "aggregated": self.counts.aggregated,
            "archived": self.counts.archived,
            "scripted": self.counts.scripted,
            "avoided": avoided,
            "overhead": self.counts.stored,
            "raw": self.counts.raw,
            "stored": self.counts.stored,
            "batches": self.counts.batches,
            "runs": self.counts.runs,
            "fallbacks": self.counts.fallbacks,
            "buffered": self.buffer.len(),
            "script": self.policy.script,
            "buffer": self.policy.buffer,
            "compress": self.policy.compress,
            "dedup": self.policy.dedup,
            "sample": self.policy.sample,
            "archive": self.policy.archive,
        })
    }
}

/// 插件未启用时仍然生效。返回 true 表示基础保护允许继续。
pub fn protect(item: &Item) -> bool {
    if item.event == "packet.raw" && !item.tagged {
        return false;
    }
    if item.actor == Actor::SelfProcess {
        return false;
    }
    true
}

pub fn protect_payload(payload: &str) -> Result<bool, &'static str> {
    let value: Value = serde_json::from_str(payload).map_err(|_| "rejected")?;
    Ok(protect(&parse_item(&value)?))
}

fn parse_item(value: &Value) -> Result<Item, &'static str> {
    if !value.is_object() {
        return Err("rejected");
    }
    let actor = match value["actor"].as_str().unwrap_or("") {
        "self" => Actor::SelfProcess,
        "other" => Actor::Other,
        _ => return Err("rejected"),
    };
    let kind = match value["kind"].as_str().unwrap_or("") {
        "ordinary" => Kind::Ordinary,
        "outside" => Kind::Outside,
        "traffic" => Kind::Traffic,
        "metric" => Kind::Metric,
        _ => return Err("rejected"),
    };
    let seq = value["seq"].as_u64().ok_or("rejected")?;
    Ok(Item {
        event: field_text(value, "event")?,
        path: field_text(value, "path")?,
        process: field_text(value, "process")?,
        app: field_text(value, "app")?,
        protocol: field_text(value, "protocol")?,
        ip: field_text(value, "ip")?,
        port: field_text(value, "port")?,
        iface: field_text(value, "iface")?,
        actor,
        kind,
        critical: value["critical"].as_bool().unwrap_or(false),
        tagged: value["tagged"].as_bool().unwrap_or(false),
        seq,
    })
}

fn field_text(value: &Value, key: &str) -> Result<String, &'static str> {
    let text = value[key].as_str().unwrap_or("");
    if text.chars().any(char::is_control) || text.contains('|') || text.len() > MAX_FIELD {
        return Err("rejected");
    }
    Ok(text.to_owned())
}

fn token(value: &Value, key: &str) -> Result<String, &'static str> {
    let text = value[key].as_str().unwrap_or("");
    if text.is_empty() {
        return Ok(String::new());
    }
    if !text.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    }) || text.len() > 40
    {
        return Err("rejected");
    }
    Ok(text.to_owned())
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

fn field_of<'a>(item: &'a Item, field: &str) -> &'a str {
    match field {
        "process" => &item.process,
        "app" => &item.app,
        "path" => &item.path,
        "event" => &item.event,
        "protocol" => &item.protocol,
        "ip" => &item.ip,
        "port" => &item.port,
        "iface" => &item.iface,
        _ => "",
    }
}

fn field_matches(item: &Item, field: &str, value: &str) -> bool {
    let got = field_of(item, field);
    if field == "path" && (value.ends_with('/') || value.ends_with('\\')) {
        got.starts_with(value)
    } else {
        got == value
    }
}

fn pack(lines: &[String], compress: bool) -> Vec<u8> {
    let raw = lines.join("\n");
    let packed = if compress {
        compress_lines(lines)
    } else {
        raw.as_bytes().to_vec()
    };
    let (flag, body) = if compress && packed.len() < raw.len() {
        (1u8, packed)
    } else {
        (0u8, raw.into_bytes())
    };
    let mut out = b"WFB1".to_vec();
    out.push(flag);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

fn compress_lines(lines: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let mut count = 1usize;
        while index + count < lines.len() && lines[index + count] == lines[index] && count < 65535 {
            count += 1;
        }
        let bytes = lines[index].as_bytes();
        out.extend_from_slice(&(count as u16).to_le_bytes());
        out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(bytes);
        index += count;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(actor: Actor, kind: Kind, event: &str, path: &str, seq: u64) -> Item {
        Item {
            event: event.to_owned(),
            path: path.to_owned(),
            process: "proc".to_owned(),
            app: "app".to_owned(),
            protocol: "tcp".to_owned(),
            ip: "10.0.0.8".to_owned(),
            port: "443".to_owned(),
            iface: "eth0".to_owned(),
            actor,
            kind,
            critical: false,
            tagged: false,
            seq,
        }
    }

    #[test]
    fn baseline_drops_self_and_untagged_packets_and_keeps_outside_changes() {
        let outside = item(
            Actor::Other,
            Kind::Outside,
            "file.modified",
            "/data/WinStrata/settings.json",
            1,
        );
        let own = item(
            Actor::SelfProcess,
            Kind::Ordinary,
            "storage.write",
            "/data/WinStrata/events/segment",
            2,
        );
        let log = item(
            Actor::SelfProcess,
            Kind::Ordinary,
            "log.appended",
            "/data/WinStrata/logs/core",
            3,
        );
        let ipc = item(Actor::SelfProcess, Kind::Ordinary, "ipc.frame", "", 4);
        let mut packet = item(Actor::Other, Kind::Traffic, "packet.raw", "", 5);
        assert!(protect(&outside));
        assert!(!protect(&own));
        assert!(!protect(&log));
        assert!(!protect(&ipc));
        assert!(!protect(&packet));
        packet.tagged = true;
        assert!(protect(&packet));
        assert!(protect_payload(r#"{"actor":"other","kind":"outside","event":"file.modified","path":"/data/WinStrata/settings.json","seq":1}"#).unwrap());
        assert!(!protect_payload(r#"{"actor":"self","kind":"ordinary","event":"storage.write","path":"/data/WinStrata/events/a","seq":2}"#).unwrap());
    }

    #[test]
    fn gui_rules_dedup_sample_aggregate_and_one_compressed_batch() {
        let root = std::env::temp_dir().join(format!(
            "winstrata-filter-unit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let mut engine = Engine::open(&root).unwrap();
        engine.dispatch(r#"{"op":"tune","buffer":8,"compress":true,"dedup":true,"aggregate":true,"sample":2,"archive":"drop-metric"}"#).unwrap();
        engine
            .dispatch(r#"{"op":"rule","field":"path","mode":"exclude","value":"/tmp/noise"}"#)
            .unwrap();
        engine
            .dispatch(r#"{"op":"rule","field":"event","mode":"include","value":"file.modified"}"#)
            .unwrap();
        engine.dispatch(&serde_json::to_string(&json!({"op":"apply","items":[
            {"actor":"self","kind":"ordinary","event":"storage.write","path":"/data/WinStrata/events/a","seq":1},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/tmp/noise","process":"vim","seq":2},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/WinStrata/settings.json","process":"vim","critical":true,"seq":3},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/WinStrata/settings.json","process":"vim","critical":true,"seq":4},
            {"actor":"other","kind":"metric","event":"file.modified","path":"/data/WinStrata/metrics","seq":5},
            {"actor":"other","kind":"traffic","event":"packet.raw","path":"","seq":6},
            {"actor":"other","kind":"outside","event":"file.added","path":"/data/WinStrata/other","seq":7},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/kept-a","critical":false,"seq":8},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/kept-b","critical":false,"seq":9}
        ]})).unwrap()).unwrap();
        let body = engine.dispatch(r#"{"op":"flush"}"#).unwrap();
        let stats: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(stats["suppressed"], 2, "{stats}");
        assert_eq!(stats["excluded"], 2, "{stats}");
        assert_eq!(stats["deduped"], 1, "{stats}");
        assert_eq!(stats["archived"], 1, "{stats}");
        assert_eq!(stats["sampled"], 1, "{stats}");
        assert_eq!(stats["batches"], 1, "{stats}");
        assert_eq!(stats["kept"], 2, "{stats}");
        assert_eq!(
            stats["avoided"].as_u64().unwrap() + stats["kept"].as_u64().unwrap(),
            stats["seen"].as_u64().unwrap()
        );
        let batches: Vec<_> = fs::read_dir(root.join("batches"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(batches.len(), 1, "one ordered batch file");
        let bytes = fs::read(&batches[0]).unwrap();
        assert!(bytes.starts_with(b"WFB1"));
        assert_eq!(stats["stored"].as_u64().unwrap(), bytes.len() as u64);
        assert!(stats["overhead"].as_u64().unwrap() == bytes.len() as u64);
        let followed = engine.dispatch(&serde_json::to_string(&json!({"op":"apply","items":[
            {"actor":"self","kind":"outside","event":"file.modified","path": batches[0].display().to_string(), "seq": 10}
        ]})).unwrap()).unwrap();
        let followed: Value = serde_json::from_str(&followed).unwrap();
        assert_eq!(followed["batches"], 1, "{followed}");
        assert_eq!(followed["suppressed"], 3, "{followed}");
        engine.dispatch(&serde_json::to_string(&json!({"op":"apply","items":[
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/WinStrata/settings.json","process":"vim","critical":true,"seq":11}
        ]})).unwrap()).unwrap();
        let outside = engine.dispatch(r#"{"op":"flush"}"#).unwrap();
        let outside: Value = serde_json::from_str(&outside).unwrap();
        assert_eq!(outside["batches"], 2, "{outside}");
        assert_eq!(outside["kept"], 3, "{outside}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn script_verdict_cannot_restore_a_suppressed_self_event_and_fallback_keeps_basic_rules() {
        let root = std::env::temp_dir().join(format!(
            "winstrata-filter-script-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let mut engine = Engine::open(&root).unwrap();
        engine.dispatch(r#"{"op":"script","id":"s1"}"#).unwrap();
        engine
            .dispatch(r#"{"op":"rule","field":"path","mode":"exclude","value":"/tmp/noise"}"#)
            .unwrap();
        let pending = engine.dispatch(&serde_json::to_string(&json!({"op":"apply","items":[
            {"actor":"self","kind":"ordinary","event":"log.appended","path":"/data/WinStrata/logs/core","seq":1},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/tmp/noise","seq":2},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/WinStrata/settings.json","seq":3},
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/kept","seq":4}
        ]})).unwrap()).unwrap();
        let pending: Value = serde_json::from_str(&pending).unwrap();
        assert_eq!(pending["phase"], "apply", "{pending}");
        let flushed = engine.dispatch(r#"{"op":"flush"}"#).unwrap();
        let flushed: Value = serde_json::from_str(&flushed).unwrap();
        assert_eq!(flushed["phase"], "script", "{flushed}");
        assert_eq!(flushed["count"], 2, "{flushed}");
        let staged = fs::read_to_string(root.join("staged.txt")).unwrap();
        assert!(!staged.contains("logs/core"));
        assert!(!staged.contains("/tmp/noise"));
        let done = engine
            .dispatch(r#"{"op":"verdict","lines":["drop","keep"]}"#)
            .unwrap();
        let done: Value = serde_json::from_str(&done).unwrap();
        assert_eq!(done["runs"], 1, "{done}");
        assert_eq!(done["scripted"], 1, "{done}");
        assert_eq!(done["kept"], 1, "{done}");
        assert_eq!(done["suppressed"], 1, "{done}");
        assert_eq!(fs::read_dir(root.join("batches")).unwrap().count(), 1);

        engine.dispatch(r#"{"op":"script","id":"s2"}"#).unwrap();
        let again = engine.dispatch(&serde_json::to_string(&json!({"op":"apply","items":[
            {"actor":"other","kind":"outside","event":"file.modified","path":"/data/kept","seq":5},
            {"actor":"self","kind":"ordinary","event":"ipc.frame","path":"","seq":6}
        ]})).unwrap()).unwrap();
        assert!(again.contains("\"buffered\":1"), "{again}");
        engine.dispatch(r#"{"op":"flush"}"#).unwrap();
        let fell = engine
            .dispatch(r#"{"op":"fallback","code":"faulted"}"#)
            .unwrap();
        let fell: Value = serde_json::from_str(&fell).unwrap();
        assert_eq!(fell["fallbacks"], 1, "{fell}");
        assert_eq!(fell["kept"], 2, "{fell}");
        assert_eq!(fell["suppressed"], 2, "{fell}");
        let log = fs::read_to_string(root.join("fallback.log")).unwrap();
        assert!(log.contains("code=faulted"), "{log}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn repeated_lines_store_fewer_bytes_than_the_raw_batch() {
        let lines = vec!["same-line".to_owned(); 8];
        let packed = pack(&lines, true);
        let raw: usize = lines.iter().map(|line| line.len() + 1).sum();
        assert!(packed.len() < raw, "{} vs {raw}", packed.len());
        assert!(packed.starts_with(b"WFB1"));
        assert_eq!(packed[4], 1);
    }
}

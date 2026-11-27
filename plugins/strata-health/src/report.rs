//! 解析宿主快照并写下本地报告。
//!
//! 快照缺总线或调度器时保持 absent，不用 0 代替没读到的计数。
//! 暂停只停止追加报告，不改宿主队列。

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_REPORT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusReading {
    pub queued: u64,
    pub pending: u64,
    pub global: u64,
    pub lost: u64,
    pub filtered: u64,
    pub queue_rejects: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchedReading {
    pub queued: u64,
    pub inflight: u64,
    pub rejected: u64,
    pub timed_out: u64,
    pub panicked: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub sent_ns: i64,
    pub plugin_faults: u64,
    pub bus: Option<BusReading>,
    pub sched: Option<SchedReading>,
}

impl Reading {
    pub fn faults(&self) -> u64 {
        let scheduler = self
            .sched
            .as_ref()
            .map(|item| item.timed_out.saturating_add(item.panicked))
            .unwrap_or(0);
        self.plugin_faults.saturating_add(scheduler)
    }

    pub fn queues(&self) -> Option<u64> {
        fold(
            self.bus.as_ref().map(BusReading::queued),
            self.sched.as_ref().map(SchedReading::queued),
        )
    }

    pub fn loss(&self) -> Option<u64> {
        let bus = self.bus.as_ref().map(|item| {
            item.lost
                .saturating_add(item.filtered)
                .saturating_add(item.queue_rejects)
        });
        let scheduler = self.sched.as_ref().map(|item| item.rejected);
        fold(bus, scheduler)
    }
}

impl BusReading {
    fn queued(&self) -> u64 {
        self.queued
            .saturating_add(self.pending)
            .saturating_add(self.global)
    }
}

impl SchedReading {
    fn queued(&self) -> u64 {
        self.queued.saturating_add(self.inflight)
    }
}

fn fold(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (None, None) => None,
        (Some(value), None) | (None, Some(value)) => Some(value),
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
    }
}

pub struct Session {
    state: PathBuf,
    paused: AtomicBool,
    reports: AtomicU64,
}

impl Session {
    pub fn start(state: &Path) -> Self {
        Self {
            state: state.to_path_buf(),
            paused: AtomicBool::new(false),
            reports: AtomicU64::new(0),
        }
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }

    pub fn observe(&self, payload: &str) -> Result<String, &'static str> {
        let reading = parse_snapshot(payload).ok_or("rejected")?;
        let observed_ns = unix_ns();
        let latency_ns = observed_ns.saturating_sub(reading.sent_ns) as u64;
        if !self.paused.load(Ordering::Acquire) {
            let line = report_line(&reading, observed_ns, latency_ns);
            append_report(&self.state, &line).map_err(|_| "unavailable")?;
            self.reports.fetch_add(1, Ordering::Release);
        }
        Ok(status_line(
            &reading,
            latency_ns,
            self.reports.load(Ordering::Acquire),
        ))
    }

    pub fn release(self) -> io::Result<()> {
        let mut file = open_private(&self.state.join("released"), false)?;
        file.write_all(b"joined")?;
        file.flush()?;
        Ok(())
    }
}

pub fn status_line(reading: &Reading, latency_ns: u64, reports: u64) -> String {
    format!(
        "faults={} queues={} latency_ns={} loss={} reports={}",
        reading.faults(),
        marker(reading.queues()),
        latency_ns,
        marker(reading.loss()),
        reports
    )
}

pub fn report_line(reading: &Reading, observed_ns: i64, latency_ns: u64) -> String {
    let mut parts = vec![
        format!("sent_ns={}", reading.sent_ns),
        format!("observed_ns={observed_ns}"),
        format!("latency_ns={latency_ns}"),
        format!("faults={}", reading.faults()),
        format!("queues={}", marker(reading.queues())),
        format!("loss={}", marker(reading.loss())),
        format!("plugin_faults={}", reading.plugin_faults),
    ];
    match &reading.bus {
        Some(bus) => {
            parts.push("bus=present".to_owned());
            parts.push(format!("bus_queued={}", bus.queued));
            parts.push(format!("bus_pending={}", bus.pending));
            parts.push(format!("bus_global={}", bus.global));
            parts.push(format!("lost={}", bus.lost));
            parts.push(format!("filtered={}", bus.filtered));
            parts.push(format!("queue_rejects={}", bus.queue_rejects));
        }
        None => parts.push("bus=absent".to_owned()),
    }
    match &reading.sched {
        Some(sched) => {
            parts.push("sched=present".to_owned());
            parts.push(format!("sched_queued={}", sched.queued));
            parts.push(format!("sched_inflight={}", sched.inflight));
            parts.push(format!("sched_rejected={}", sched.rejected));
            parts.push(format!("sched_timed_out={}", sched.timed_out));
            parts.push(format!("sched_panicked={}", sched.panicked));
        }
        None => parts.push("sched=absent".to_owned()),
    }
    parts.join(" ")
}

fn marker(value: Option<u64>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "absent".to_owned(),
    }
}

pub fn parse_snapshot(input: &str) -> Option<Reading> {
    if input.is_empty() || input.chars().any(char::is_control) {
        return None;
    }
    let mut fields = BTreeMap::new();
    for token in input.split(' ') {
        let (key, value) = token.split_once('=')?;
        if key.is_empty() || value.is_empty() || fields.insert(key, value).is_some() {
            return None;
        }
    }
    if fields.get("v")? != &"1" {
        return None;
    }
    let sent_ns = parse_sent(fields.get("sent_ns")?)?;
    let plugin_faults = parse_u64(fields.get("plugin_faults")?)?;
    let bus = match *fields.get("bus")? {
        "absent" => {
            if has_any(
                &fields,
                &[
                    "bus_queued",
                    "bus_pending",
                    "bus_global",
                    "lost",
                    "filtered",
                    "queue_rejects",
                ],
            ) {
                return None;
            }
            None
        }
        "present" => Some(BusReading {
            queued: parse_u64(fields.get("bus_queued")?)?,
            pending: parse_u64(fields.get("bus_pending")?)?,
            global: parse_u64(fields.get("bus_global")?)?,
            lost: parse_u64(fields.get("lost")?)?,
            filtered: parse_u64(fields.get("filtered")?)?,
            queue_rejects: parse_u64(fields.get("queue_rejects")?)?,
        }),
        _ => return None,
    };
    let sched = match *fields.get("sched")? {
        "absent" => {
            if has_any(
                &fields,
                &[
                    "sched_queued",
                    "sched_inflight",
                    "sched_rejected",
                    "sched_timed_out",
                    "sched_panicked",
                ],
            ) {
                return None;
            }
            None
        }
        "present" => Some(SchedReading {
            queued: parse_u64(fields.get("sched_queued")?)?,
            inflight: parse_u64(fields.get("sched_inflight")?)?,
            rejected: parse_u64(fields.get("sched_rejected")?)?,
            timed_out: parse_u64(fields.get("sched_timed_out")?)?,
            panicked: parse_u64(fields.get("sched_panicked")?)?,
        }),
        _ => return None,
    };
    let known = [
        "v",
        "sent_ns",
        "plugin_faults",
        "bus",
        "bus_queued",
        "bus_pending",
        "bus_global",
        "lost",
        "filtered",
        "queue_rejects",
        "sched",
        "sched_queued",
        "sched_inflight",
        "sched_rejected",
        "sched_timed_out",
        "sched_panicked",
    ];
    if fields.keys().any(|key| !known.contains(key)) {
        return None;
    }
    Some(Reading {
        sent_ns,
        plugin_faults,
        bus,
        sched,
    })
}

fn has_any(fields: &BTreeMap<&str, &str>, keys: &[&str]) -> bool {
    keys.iter().any(|key| fields.contains_key(key))
}

fn parse_u64(value: &str) -> Option<u64> {
    if value != "0" && value.as_bytes().first() == Some(&b'0') {
        return None;
    }
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn parse_sent(value: &str) -> Option<i64> {
    let number = parse_u64(value)?;
    i64::try_from(number).ok()
}

fn unix_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn append_report(state: &Path, line: &str) -> io::Result<()> {
    let path = state.join("diagnostics.jsonl");
    rotate_if_full(&path)?;
    let mut file = open_private(&path, true)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn rotate_if_full(path: &Path) -> io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < MAX_REPORT_BYTES {
        return Ok(());
    }
    let previous = path
        .parent()
        .unwrap_or(Path::new("."))
        .join("diagnostics.prev.jsonl");
    let _ = fs::remove_file(&previous);
    fs::rename(path, previous)?;
    Ok(())
}

fn open_private(path: &Path, append: bool) -> io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn present() -> &'static str {
        "v=1 sent_ns=10 plugin_faults=2 bus=present bus_queued=3 bus_pending=4 bus_global=5 lost=6 filtered=7 queue_rejects=8 sched=present sched_queued=9 sched_inflight=1 sched_rejected=2 sched_timed_out=3 sched_panicked=4"
    }

    #[test]
    fn present_counters_add_faults_queues_and_loss() {
        let reading = parse_snapshot(present()).unwrap();
        assert_eq!(reading.faults(), 2 + 3 + 4);
        assert_eq!(reading.queues(), Some(3 + 4 + 5 + 9 + 1));
        assert_eq!(reading.loss(), Some(6 + 7 + 8 + 2));
        assert_eq!(
            status_line(&reading, 15, 1),
            "faults=9 queues=22 latency_ns=15 loss=23 reports=1"
        );
    }

    #[test]
    fn absent_runtime_is_not_reported_as_zero() {
        let reading =
            parse_snapshot("v=1 sent_ns=0 plugin_faults=1 bus=absent sched=absent").unwrap();
        assert_eq!(reading.faults(), 1);
        assert_eq!(reading.queues(), None);
        assert_eq!(reading.loss(), None);
        assert_eq!(
            status_line(&reading, 0, 0),
            "faults=1 queues=absent latency_ns=0 loss=absent reports=0"
        );
    }

    #[test]
    fn contradictory_or_padded_snapshots_are_rejected() {
        assert!(parse_snapshot("").is_none());
        assert!(parse_snapshot("v=1 sent_ns=01 plugin_faults=0 bus=absent sched=absent").is_none());
        assert!(
            parse_snapshot("v=1 sent_ns=1 plugin_faults=0 bus=absent bus_queued=1 sched=absent")
                .is_none()
        );
        assert!(parse_snapshot("v=1 sent_ns=1 plugin_faults=0 bus=present bus_queued=1 bus_pending=0 bus_global=0 lost=0 filtered=0 sched=absent").is_none());
        assert!(
            parse_snapshot("v=1 v=1 sent_ns=1 plugin_faults=0 bus=absent sched=absent").is_none()
        );
        assert!(parse_snapshot("v=2 sent_ns=1 plugin_faults=0 bus=absent sched=absent").is_none());
        assert!(
            parse_snapshot("v=1 sent_ns=1 plugin_faults=0 bus=absent sched=absent extra=1")
                .is_none()
        );
    }
}

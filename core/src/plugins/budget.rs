//! 计划 026 的资源限额。
//!
//! 宿主读取本进程的内存和 CPU，以及已经附着的总线和调度器队列。
//! 插件比较这些读数和调用方调整后的限额。固定天花板不在这里放宽。

use std::collections::BTreeMap;
use std::time::Instant;

#[cfg(target_os = "linux")]
use std::fs;

use winstrata_protocols::{
    Counter, Event, EventTime, GapReason, Id, Record, SignedNanos, Source, Version,
};

use super::PluginError;

pub(super) const PLUGIN: &str = "strata.budget";
pub(super) const CPU_CEILING: u64 = 1_000_000;
pub(super) const MEM_CEILING_KIB: u64 = 4_194_304;
pub(super) const QUEUE_CEILING: u64 = 65_536;

// 清单与调节程序共同描述计划 026。宿主不通过第三方安装写入这个身份。
pub(super) const MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.budget","name":"StrataBudget","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["budget.regulate"],"permissions":["storage.private"],"#,
    r#""dependencies":[],"events":[],"rescue":false}"#,
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PluginLimit {
    Keep,
    Clear,
    Set { id: String, queue: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LimitUpdate {
    pub(super) cpu_ppm: Option<u64>,
    pub(super) mem_kib: Option<u64>,
    pub(super) queue: Option<u64>,
    pub(super) plugin: PluginLimit,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CpuBase {
    ticks: u64,
    at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CpuView {
    Absent,
    Baseline,
    Measured(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Pressure {
    pub(super) rss_kib: Option<u64>,
    pub(super) cpu: CpuView,
    pub(super) queued: Option<u64>,
    pub(super) lost: Option<u64>,
    pub(super) sched_queued: Option<u64>,
    pub(super) plugin_queued: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Keep,
    Hold,
    Pause,
    Resume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reason {
    None,
    Queue,
    Memory,
    Cpu,
    Plugin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Decision {
    pub(super) action: Action,
    pub(super) loss: bool,
    pub(super) reason: Reason,
}

pub(super) fn beside(runtime: &std::path::Path) -> Option<std::path::PathBuf> {
    super::collector_beside(runtime, "winstrata-strata-budget")
}

pub(super) fn parse_limits(input: &str) -> Result<LimitUpdate, PluginError> {
    if input.chars().any(char::is_control) {
        return Err(PluginError::new(
            "invalid_input",
            "budget limits contain a control character",
        ));
    }
    let mut fields = BTreeMap::new();
    if !input.is_empty() {
        for token in input.split(' ') {
            let Some((key, value)) = token.split_once('=') else {
                return Err(PluginError::new(
                    "invalid_input",
                    "budget limit field is malformed",
                ));
            };
            if key.is_empty() || value.is_empty() || fields.insert(key, value).is_some() {
                return Err(PluginError::new(
                    "invalid_input",
                    "budget limit field is repeated or empty",
                ));
            }
        }
    }
    let known = ["cpu_ppm", "mem_kib", "queue", "plugin", "plugin_queue"];
    if fields.keys().any(|key| !known.contains(key)) {
        return Err(PluginError::new(
            "invalid_input",
            "budget limit field is not adjustable",
        ));
    }
    let cpu_ppm = optional_limit(fields.get("cpu_ppm").copied(), CPU_CEILING)?;
    let mem_kib = optional_limit(fields.get("mem_kib").copied(), MEM_CEILING_KIB)?;
    let queue = optional_limit(fields.get("queue").copied(), QUEUE_CEILING)?;
    let plugin = match (
        fields.get("plugin").copied(),
        fields.get("plugin_queue").copied(),
    ) {
        (None, None) => PluginLimit::Keep,
        (Some("none"), None) => PluginLimit::Clear,
        (Some(id), Some(queue)) if valid_plugin_id(id) => PluginLimit::Set {
            id: id.to_owned(),
            queue: bounded(queue, QUEUE_CEILING)?,
        },
        _ => {
            return Err(PluginError::new(
                "invalid_input",
                "plugin budget needs an id and a queue limit",
            ));
        }
    };
    Ok(LimitUpdate {
        cpu_ppm,
        mem_kib,
        queue,
        plugin,
    })
}

pub(super) fn payload(update: &LimitUpdate, pressure: &Pressure) -> Result<String, PluginError> {
    let mut parts = Vec::with_capacity(20);
    parts.push("v=1".to_owned());
    parts.push(limit_token("cpu_ppm", update.cpu_ppm));
    parts.push(limit_token("mem_kib", update.mem_kib));
    parts.push(limit_token("queue", update.queue));
    match &update.plugin {
        PluginLimit::Keep => {
            parts.push("plugin=keep".to_owned());
            parts.push("plugin_queue=keep".to_owned());
        }
        PluginLimit::Clear => {
            parts.push("plugin=none".to_owned());
            parts.push("plugin_queue=none".to_owned());
        }
        PluginLimit::Set { id, queue } => {
            parts.push(format!("plugin={id}"));
            parts.push(format!("plugin_queue={queue}"));
        }
    }
    match pressure.rss_kib {
        Some(kib) => {
            parts.push("rss=measured".to_owned());
            parts.push(format!("rss_kib={kib}"));
        }
        None => parts.push("rss=absent".to_owned()),
    }
    match pressure.cpu {
        CpuView::Baseline => parts.push("cpu=baseline".to_owned()),
        CpuView::Measured(ppm) => {
            parts.push("cpu=measured".to_owned());
            parts.push(format!("cpu_used_ppm={ppm}"));
        }
        CpuView::Absent => parts.push("cpu=absent".to_owned()),
    }
    match (pressure.queued, pressure.lost) {
        (Some(queued), Some(lost)) => {
            parts.push("bus=present".to_owned());
            parts.push(format!("queued={queued}"));
            parts.push(format!("lost={lost}"));
        }
        _ => parts.push("bus=absent".to_owned()),
    }
    match pressure.sched_queued {
        Some(queued) => {
            parts.push("sched=present".to_owned());
            parts.push(format!("sched_queued={queued}"));
        }
        None => parts.push("sched=absent".to_owned()),
    }
    match pressure.plugin_queued {
        Some(queued) => {
            parts.push("plugin_state=present".to_owned());
            parts.push(format!("plugin_queued={queued}"));
        }
        None => parts.push("plugin_state=absent".to_owned()),
    }
    let text = parts.join(" ");
    if text.len() > 1024 || text.chars().any(char::is_control) {
        return Err(PluginError::new(
            "invalid_record",
            "budget snapshot does not fit a plugin frame",
        ));
    }
    Ok(text)
}

pub(super) fn parse_decision(input: &str) -> Result<Decision, PluginError> {
    if input.is_empty() || input.len() > 1024 || input.chars().any(char::is_control) {
        return Err(PluginError::new(
            "invalid_record",
            "budget decision does not fit a plugin frame",
        ));
    }
    let mut fields = BTreeMap::new();
    for token in input.split(' ') {
        let Some((key, value)) = token.split_once('=') else {
            return Err(PluginError::new(
                "invalid_record",
                "budget decision field is malformed",
            ));
        };
        if key.is_empty()
            || value.is_empty()
            || fields.insert(key.to_owned(), value.to_owned()).is_some()
        {
            return Err(PluginError::new(
                "invalid_record",
                "budget decision field is repeated or empty",
            ));
        }
    }
    let known = [
        "action",
        "loss",
        "reason",
        "reports",
        "cpu_ppm",
        "mem_kib",
        "queue",
        "rss",
        "rss_kib",
        "cpu",
        "cpu_used_ppm",
        "bus",
        "queued",
        "lost",
        "sched",
        "sched_queued",
        "plugin",
        "plugin_queue",
        "plugin_state",
        "plugin_queued",
    ];
    if fields.keys().any(|key| !known.contains(&key.as_str())) {
        return Err(PluginError::new(
            "invalid_record",
            "budget decision field is not recognized",
        ));
    }
    let action = match fields.get("action").map(String::as_str) {
        Some("keep") => Action::Keep,
        Some("hold") => Action::Hold,
        Some("pause") => Action::Pause,
        Some("resume") => Action::Resume,
        _ => {
            return Err(PluginError::new(
                "invalid_record",
                "budget action is not recognized",
            ));
        }
    };
    let loss = match fields.get("loss").map(String::as_str) {
        Some("0") => false,
        Some("1") => true,
        _ => {
            return Err(PluginError::new(
                "invalid_record",
                "budget loss flag is not 0 or 1",
            ));
        }
    };
    let reason = match fields.get("reason").map(String::as_str) {
        Some("none") => Reason::None,
        Some("queue") => Reason::Queue,
        Some("memory") => Reason::Memory,
        Some("cpu") => Reason::Cpu,
        Some("plugin") => Reason::Plugin,
        _ => {
            return Err(PluginError::new(
                "invalid_record",
                "budget reason is not recognized",
            ));
        }
    };
    let consistent = match action {
        Action::Pause => reason != Reason::None,
        Action::Keep | Action::Hold | Action::Resume => !loss && reason == Reason::None,
    };
    if !consistent {
        return Err(PluginError::new(
            "invalid_record",
            "budget action, loss, and reason do not agree",
        ));
    }
    Ok(Decision {
        action,
        loss,
        reason,
    })
}

pub(super) fn loss_event(epoch: &str, sequence: u64) -> Result<Event, PluginError> {
    let event = Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new("local-plugin-host").map_err(super::manifest_error)?,
            provider: Id::new(PLUGIN).map_err(super::manifest_error)?,
            epoch: Id::new(epoch).map_err(super::manifest_error)?,
        },
        sequence: Counter::new(sequence).map_err(super::manifest_error)?,
        time: EventTime {
            unix_ns: SignedNanos::new(super::unix_ns()),
            precision_ns: None,
        },
        record: Record::Gap {
            reason: GapReason::Lost,
            range: None,
        },
    };
    event
        .validate()
        .map_err(|_| PluginError::new("invalid_record", "budget loss marker is not valid"))?;
    Ok(event)
}

pub(super) fn sample_cpu(previous: Option<&CpuBase>) -> (CpuView, Option<CpuBase>) {
    let Some((ticks, hz)) = process_cpu() else {
        return (CpuView::Absent, previous.cloned());
    };
    let now = CpuBase {
        ticks,
        at: Instant::now(),
    };
    let Some(previous) = previous else {
        return (CpuView::Baseline, Some(now));
    };
    let elapsed = now.at.saturating_duration_since(previous.at).as_nanos();
    if elapsed == 0 || hz == 0 {
        return (CpuView::Baseline, Some(now));
    }
    let delta = now.ticks.saturating_sub(previous.ticks);
    let ppm = u128::from(delta)
        .saturating_mul(1_000_000)
        .saturating_mul(1_000_000_000)
        / (u128::from(hz) * elapsed);
    let ppm = u64::try_from(ppm).unwrap_or(u64::MAX);
    (CpuView::Measured(ppm), Some(now))
}

pub(super) fn rss_kib() -> Option<u64> {
    read_rss_kib()
}

fn limit_token(name: &str, value: Option<u64>) -> String {
    match value {
        Some(value) => format!("{name}={value}"),
        None => format!("{name}=keep"),
    }
}

fn optional_limit(value: Option<&str>, ceiling: u64) -> Result<Option<u64>, PluginError> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(bounded(value, ceiling)?))
}

fn bounded(value: &str, ceiling: u64) -> Result<u64, PluginError> {
    let number = parse_u64(value).ok_or_else(|| {
        PluginError::new("invalid_input", "budget limit is not a decimal integer")
    })?;
    if number > ceiling {
        return Err(PluginError::new(
            "invalid_limit",
            "budget limit is above the fixed ceiling",
        ));
    }
    Ok(number)
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

fn valid_plugin_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 || value == "keep" || value == "none" {
        return false;
    }
    bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'.' | b'-')
        })
        && !value.contains("..")
        && !value.ends_with('.')
}

#[cfg(target_os = "linux")]
fn process_cpu() -> Option<(u64, u64)> {
    let text = fs::read_to_string("/proc/self/stat").ok()?;
    let rest = text.rsplit_once(')')?.1.trim_start();
    let mut fields = rest.split_whitespace();
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if hz <= 0 {
        return None;
    }
    Some((utime.saturating_add(stime), u64::try_from(hz).ok()?))
}

#[cfg(target_os = "linux")]
fn read_rss_kib() -> Option<u64> {
    let text = fs::read_to_string("/proc/self/status").ok()?;
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        return parse_u64(rest.split_whitespace().next()?);
    }
    None
}

#[cfg(target_os = "windows")]
fn process_cpu() -> Option<(u64, u64)> {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let mut created = zero_filetime();
    let mut exited = zero_filetime();
    let mut kernel = zero_filetime();
    let mut user = zero_filetime();
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    if ok == 0 {
        return None;
    }
    let ticks = filetime_u64(kernel).saturating_add(filetime_u64(user));
    Some((ticks, 10_000_000))
}

#[cfg(target_os = "windows")]
fn read_rss_kib() -> Option<u64> {
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { zeroed() };
    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    if ok == 0 {
        return None;
    }
    u64::try_from(counters.WorkingSetSize / 1024).ok()
}

#[cfg(target_os = "windows")]
fn zero_filetime() -> windows_sys::Win32::Foundation::FILETIME {
    windows_sys::Win32::Foundation::FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    }
}

#[cfg(target_os = "windows")]
fn filetime_u64(value: windows_sys::Win32::Foundation::FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn process_cpu() -> Option<(u64, u64)> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn read_rss_kib() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_measurements_are_rejected_and_ceilings_hold() {
        assert_eq!(
            parse_limits("rss_kib=1").unwrap_err().code(),
            "invalid_input"
        );
        assert_eq!(
            parse_limits("mem_kib=4194305").unwrap_err().code(),
            "invalid_limit"
        );
        assert_eq!(
            parse_limits("queue=65537").unwrap_err().code(),
            "invalid_limit"
        );
        assert_eq!(
            parse_limits("cpu_ppm=01").unwrap_err().code(),
            "invalid_input"
        );
        let update = parse_limits("mem_kib=1 plugin=strata.health plugin_queue=2").unwrap();
        assert_eq!(update.mem_kib, Some(1));
        assert_eq!(update.cpu_ppm, None);
        match update.plugin {
            PluginLimit::Set { id, queue } => {
                assert_eq!(id, "strata.health");
                assert_eq!(queue, 2);
            }
            other => panic!("plugin limit was not stored: {other:?}"),
        }
    }

    #[test]
    fn widest_snapshot_fits_a_plugin_frame() {
        let update = LimitUpdate {
            cpu_ppm: Some(CPU_CEILING),
            mem_kib: Some(MEM_CEILING_KIB),
            queue: Some(QUEUE_CEILING),
            plugin: PluginLimit::Set {
                id: "p".repeat(64),
                queue: QUEUE_CEILING,
            },
        };
        let pressure = Pressure {
            rss_kib: Some(u64::MAX),
            cpu: CpuView::Measured(u64::MAX),
            queued: Some(u64::MAX),
            lost: Some(u64::MAX),
            sched_queued: Some(u64::MAX),
            plugin_queued: Some(u64::MAX),
        };
        let text = payload(&update, &pressure).unwrap();
        assert!(text.len() <= 1024, "snapshot length {}", text.len());
        assert_eq!(
            QUEUE_CEILING,
            u64::try_from(crate::runtime::MAX_QUEUE).unwrap()
        );
        assert!(text.starts_with("v=1 "));
        assert!(text.contains("rss=measured"));
        assert!(text.contains("cpu=measured"));
        assert!(text.contains("bus=present"));
        assert!(text.contains("sched=present"));
        let decision = parse_decision(
            "action=pause loss=1 reason=memory cpu_ppm=1000000 mem_kib=1 queue=65536 reports=1",
        )
        .unwrap();
        assert_eq!(decision.action, Action::Pause);
        assert!(decision.loss);
        assert_eq!(decision.reason, Reason::Memory);
        assert!(parse_decision("action=pause loss=0 reason=none").is_err());
        assert!(parse_decision("action=resume loss=1 reason=none").is_err());
        let event = loss_event("epoch-budget", 1).unwrap();
        assert!(matches!(
            event.record,
            Record::Gap {
                reason: GapReason::Lost,
                range: None
            }
        ));
    }

    #[test]
    fn this_process_reports_memory_without_inventing_a_zero() {
        let rss = rss_kib();
        assert!(rss.is_some_and(|kib| kib > 0), "rss was {rss:?}");
        let (first, base) = sample_cpu(None);
        assert!(matches!(first, CpuView::Baseline | CpuView::Absent));
        if let Some(base) = base.as_ref() {
            let (second, _) = sample_cpu(Some(base));
            assert!(matches!(
                second,
                CpuView::Measured(_) | CpuView::Baseline | CpuView::Absent
            ));
        }
    }
}

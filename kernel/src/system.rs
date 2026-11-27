//! 进程、线程、模块与服务进程关联的一次采样。
//!
//! 采样只报告调用当时能够读到的记录。两次采样之间出现并退出的进程不会留下事件，
//! 调用方也不能把缺席当成已经观察到退出时刻。

use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
#[path = "system/linux.rs"]
mod host;

#[cfg(windows)]
#[path = "system/windows.rs"]
mod host;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("process sampling supports Windows and the Linux development host");

pub const MAX_PROCESSES: usize = 4_096;
pub const MAX_THREADS: usize = 1_024;
pub const MAX_MODULES: usize = 64;
pub const MAX_SERVICES: usize = 1_024;
const MAX_NAME: usize = 240;
const MAX_IMAGE: usize = 1_024;

/// 一次用户态采样。`device` 和 `boot` 在同一次启动中保持不变。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemSnapshot {
    pub device: String,
    pub boot: String,
    pub sampled_unix_ns: i64,
    pub precision_ns: Option<u64>,
    pub processes: Vec<ProcessRow>,
    pub services: Vec<ServiceRow>,
    pub processes_truncated: bool,
    pub services_truncated: bool,
    pub services_unavailable: bool,
    pub evidence: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessRow {
    pub pid: u32,
    pub parent_pid: u32,
    pub start_unix_ns: Option<i64>,
    pub name: String,
    pub image: Option<String>,
    pub threads: Vec<u32>,
    pub modules: Vec<ModuleRow>,
    pub modules_truncated: bool,
    pub threads_truncated: bool,
    pub limited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleRow {
    pub name: String,
    pub base: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceRow {
    pub name: String,
    pub pid: u32,
}

pub(crate) struct HostSample {
    pub device: String,
    pub boot: String,
    pub precision_ns: Option<u64>,
    pub processes: Vec<ProcessRow>,
    pub services: Vec<ServiceRow>,
    pub services_unavailable: bool,
    pub evidence: &'static str,
}

pub fn capture() -> io::Result<SystemSnapshot> {
    let mut sample = host::sample()?;
    sample.device = identity(&sample.device)?;
    sample.boot = identity(&sample.boot)?;
    let processes_truncated = finish_processes(&mut sample.processes);
    let services_truncated = finish_services(&mut sample.services);
    Ok(SystemSnapshot {
        device: sample.device,
        boot: sample.boot,
        sampled_unix_ns: unix_now()?,
        precision_ns: sample.precision_ns.filter(|value| *value > 0),
        processes: sample.processes,
        services: sample.services,
        processes_truncated,
        services_truncated,
        services_unavailable: sample.services_unavailable,
        evidence: sample.evidence,
    })
}

pub(crate) fn unix_now() -> io::Result<i64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            "system clock is before the Unix epoch",
        )
    })?;
    i64::try_from(elapsed.as_nanos()).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            "system clock does not fit in signed nanoseconds",
        )
    })
}

pub(crate) fn identity(value: &str) -> io::Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 256
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "machine or boot identity is empty or not a single token",
        ));
    }
    Ok(value.to_owned())
}

pub(crate) fn bounded_text(value: &str, max: usize) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_control() {
            continue;
        }
        let mut encoded = [0u8; 4];
        let bytes = ch.encode_utf8(&mut encoded);
        if out.len() + bytes.len() > max {
            break;
        }
        out.push(ch);
    }
    out.trim().to_owned()
}

pub(crate) fn process_name(value: &str, pid: u32) -> String {
    let name = bounded_text(value, MAX_NAME);
    if name.is_empty() {
        pid.to_string()
    } else {
        name
    }
}

pub(crate) fn image_text(value: &str) -> Option<String> {
    let image = bounded_text(value, MAX_IMAGE);
    if image.is_empty() { None } else { Some(image) }
}

pub(crate) fn limit_modules(
    mut modules: BTreeMap<String, u64>,
    preferred: Option<&str>,
) -> (Vec<ModuleRow>, bool) {
    let truncated = modules.len() > MAX_MODULES;
    let mut kept = Vec::new();
    if let Some(preferred) = preferred {
        let key = modules
            .keys()
            .find(|name| module_names_match(name, preferred))
            .cloned();
        if let Some(key) = key
            && let Some(base) = modules.remove(&key)
        {
            kept.push(ModuleRow { name: key, base });
        }
    }
    for (name, base) in modules {
        if kept.len() == MAX_MODULES {
            break;
        }
        kept.push(ModuleRow { name, base });
    }
    (kept, truncated)
}

fn module_names_match(left: &str, right: &str) -> bool {
    #[cfg(windows)]
    {
        left.eq_ignore_ascii_case(right)
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

pub(crate) fn service_name(value: &str) -> Option<String> {
    let name = bounded_text(value, MAX_NAME);
    if name.is_empty() { None } else { Some(name) }
}

fn finish_processes(processes: &mut Vec<ProcessRow>) -> bool {
    processes.sort_by_key(|row| row.pid);
    let mut kept: Vec<ProcessRow> = Vec::with_capacity(processes.len());
    for mut row in processes.drain(..) {
        if row.pid == 0 {
            continue;
        }
        row.threads.sort_unstable();
        row.threads.dedup();
        row.modules.sort();
        row.modules.dedup();
        if let Some(previous) = kept.last_mut()
            && previous.pid == row.pid
        {
            if previous.start_unix_ns.is_none() && row.start_unix_ns.is_some() {
                *previous = row;
            }
            continue;
        }
        kept.push(row);
    }
    let truncated = kept.len() > MAX_PROCESSES;
    kept.truncate(MAX_PROCESSES);
    *processes = kept;
    truncated
}

fn finish_services(services: &mut Vec<ServiceRow>) -> bool {
    services.retain(|row| row.pid != 0 && !row.name.is_empty());
    services.sort();
    services.dedup();
    let truncated = services.len() > MAX_SERVICES;
    services.truncate(MAX_SERVICES);
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_reads_this_process_and_keeps_identities_stable() {
        let first = capture().expect("first capture");
        let second = capture().expect("second capture");
        assert_eq!(first.device, second.device);
        assert_eq!(first.boot, second.boot);
        assert_eq!(first.evidence, second.evidence);
        assert!(!first.processes_truncated, "process list was truncated");
        let pid = std::process::id();
        let row = first
            .processes
            .iter()
            .find(|row| row.pid == pid)
            .expect("current pid is in the snapshot");
        let again = second
            .processes
            .iter()
            .find(|row| row.pid == pid)
            .expect("current pid remains in the snapshot");
        assert_eq!(row.start_unix_ns, again.start_unix_ns);
        assert!(
            row.start_unix_ns
                .is_some_and(|start| start > 0 && start <= first.sampled_unix_ns + 5_000_000_000)
        );
        assert!(!row.threads.is_empty(), "current process has no thread");
        assert!(row.image.as_ref().is_some_and(|image| !image.is_empty()));
        assert!(!row.modules.is_empty(), "current process has no module");
        assert!(!row.limited, "current process was only partially readable");
        let exe = std::env::current_exe().expect("test executable");
        let file = exe.file_name().expect("executable name");
        let file = file.to_string_lossy();
        assert!(
            row.modules
                .iter()
                .any(|module| { module.name.contains(file.as_ref()) }),
            "modules do not include the test executable"
        );
        let mut pids = first
            .processes
            .iter()
            .map(|row| row.pid)
            .collect::<Vec<_>>();
        let before = pids.clone();
        pids.sort_unstable();
        pids.dedup();
        assert_eq!(pids, before);
    }
}

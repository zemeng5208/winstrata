//! Linux 只读取此刻正在运行的 cgroup 服务单元和已加载的内核模块。
//!
//! 不调用 systemctl，也不把单元文件当成正在运行的服务。pid 1 不是 systemd
//! 时，空列表是一次成功的空观察，不是权限失败。

use std::fs;
use std::io::{self, ErrorKind};

use super::{Coverage, Fact, Kind, RunState, ServiceSnapshot, StartType, clean_name};

const MAX_EACH: usize = 1024;

pub(super) fn capture() -> io::Result<ServiceSnapshot> {
    let device = identity(&fs::read_to_string("/etc/machine-id").map_err(|error| {
        io::Error::new(error.kind(), "cannot read the machine id")
    })?)?;
    let boot = identity(
        &fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|error| io::Error::new(error.kind(), "cannot read the boot id"))?,
    )?;
    let (services, services_unreadable) = read_services();
    let (drivers, driver_permission_denied, source_failure) = read_modules();
    let mut facts = services;
    facts.extend(drivers);
    let (facts, truncated) = finish(facts);
    Ok(ServiceSnapshot {
        device,
        boot,
        sampled_unix_ns: super::unix_now()?,
        facts,
        coverage: Coverage::ActiveOnly,
        truncated,
        permission_denied: false,
        services_unreadable,
        driver_permission_denied,
        source_failure,
        evidence: "procfs",
    })
}

fn identity(value: &str) -> io::Result<String> {
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

fn read_services() -> (Vec<Fact>, bool) {
    let Ok(entries) = fs::read_dir("/proc") else {
        return (Vec::new(), true);
    };
    let mut facts: Vec<Fact> = Vec::new();
    let mut saw_cgroup = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(text) = name.to_str() else {
            continue;
        };
        if text.parse::<u32>().ok().is_none_or(|pid| pid == 0) {
            continue;
        }
        let Some(cgroup) = fs::read_to_string(entry.path().join("cgroup")).ok() else {
            continue;
        };
        saw_cgroup = true;
        let Ok(pid) = text.parse::<u32>() else {
            continue;
        };
        for unit in service_units(&cgroup) {
            let Some(unit_name) = clean_name(&unit) else {
                continue;
            };
            match facts
                .iter_mut()
                .find(|fact| fact.kind == Kind::Service && fact.name == unit_name)
            {
                Some(fact) => {
                    if fact.pid.is_none_or(|current| pid < current) {
                        fact.pid = Some(pid);
                    }
                }
                None => facts.push(service_fact(unit_name, pid)),
            }
        }
    }
    (facts, !saw_cgroup)
}

fn service_fact(name: String, pid: u32) -> Fact {
    Fact {
        name,
        kind: Kind::Service,
        state: RunState::Running,
        start_type: StartType::Unknown,
        binary: None,
        pid: Some(pid),
        win32_exit: 0,
        service_exit: 0,
        config_error: None,
    }
}

fn service_units(cgroup: &str) -> Vec<String> {
    let mut units = Vec::new();
    for line in cgroup.lines() {
        let path = line.rsplit_once(':').map(|(_, path)| path).unwrap_or(line);
        for part in path.split('/') {
            let Some(unit) = part.strip_suffix(".service") else {
                continue;
            };
            if unit.is_empty() || unit.starts_with("user@") {
                continue;
            }
            units.push(format!("{unit}.service"));
        }
    }
    units
}

fn read_modules() -> (Vec<Fact>, bool, Option<String>) {
    match fs::read_to_string("/proc/modules") {
        Ok(text) => (parse_modules(&text), false, None),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => (Vec::new(), true, None),
        Err(error) => (
            Vec::new(),
            false,
            Some(format!(
                "modules:{}",
                error.raw_os_error().unwrap_or(0)
            )),
        ),
    }
}

fn parse_modules(text: &str) -> Vec<Fact> {
    let mut facts = Vec::new();
    for line in text.lines() {
        let mut columns = line.split_whitespace();
        let Some(name) = columns.next().and_then(clean_name) else {
            continue;
        };
        // 列序是 size、instances、deps、state。地址不是映像路径。
        let state = match columns.nth(3) {
            Some("Live") => RunState::Running,
            Some("Loading") => RunState::StartPending,
            Some("Unloading") => RunState::StopPending,
            _ => RunState::Unknown,
        };
        facts.push(Fact {
            name,
            kind: Kind::Driver,
            state,
            start_type: StartType::Unknown,
            binary: None,
            pid: None,
            win32_exit: 0,
            service_exit: 0,
            config_error: None,
        });
    }
    facts
}

fn finish(mut facts: Vec<Fact>) -> (Vec<Fact>, bool) {
    facts.sort_by(|left, right| {
        (left.kind as u8, left.name.as_str()).cmp(&(right.kind as u8, right.name.as_str()))
    });
    facts.dedup_by(|left, right| left.kind == right.kind && left.name == right.name);
    let services = facts
        .iter()
        .filter(|fact| fact.kind == Kind::Service)
        .count();
    let drivers = facts.len().saturating_sub(services);
    let truncated = services > MAX_EACH || drivers > MAX_EACH;
    let mut kept_services = 0usize;
    let mut kept_drivers = 0usize;
    facts.retain(|fact| match fact.kind {
        Kind::Service => {
            kept_services += 1;
            kept_services <= MAX_EACH
        }
        Kind::Driver => {
            kept_drivers += 1;
            kept_drivers <= MAX_EACH
        }
    });
    (facts, truncated)
}

//! Linux 通过 procfs 读取进程、线程、可执行映射和服务单元。
//!
//! `/proc/<pid>/stat` 的启动时间以时钟滴答计。用户态服务关联只接受 cgroup 路径里的
//! `.service` 单元，并跳过包裹用户会话的 `user@*.service`。

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use super::{
    HostSample, MAX_THREADS, ModuleRow, ProcessRow, ServiceRow, image_text, process_name,
    service_name,
};

pub(super) fn sample() -> io::Result<HostSample> {
    let device = fs::read_to_string("/etc/machine-id")
        .map_err(|error| io::Error::new(error.kind(), "cannot read /etc/machine-id"))?;
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|error| io::Error::new(error.kind(), "cannot read the kernel boot id"))?;
    let btime = boot_unix_seconds()?;
    let tick_ns = tick_ns()?;
    let mut processes = Vec::new();
    let mut services = Vec::new();
    let mut saw_cgroup = false;
    let mut entries = fs::read_dir("/proc")?;
    while let Some(entry) = entries.next() {
        let entry = entry?;
        let name = entry.file_name();
        let Some(pid_text) = name.to_str() else {
            continue;
        };
        let Ok(pid) = pid_text.parse::<u32>() else {
            continue;
        };
        if pid == 0 {
            continue;
        }
        let dir = entry.path();
        if let Some(row) = read_process(&dir, pid, btime, tick_ns) {
            if let Some(cgroup) = fs::read_to_string(dir.join("cgroup")).ok() {
                saw_cgroup = true;
                for unit in service_units(&cgroup) {
                    if let Some(name) = service_name(&unit) {
                        services.push(ServiceRow { name, pid });
                    }
                }
            }
            processes.push(row);
        }
    }
    Ok(HostSample {
        device,
        boot,
        precision_ns: Some(tick_ns),
        processes,
        services,
        services_unavailable: !saw_cgroup,
        evidence: "procfs",
    })
}

fn boot_unix_seconds() -> io::Result<u64> {
    let stat = fs::read_to_string("/proc/stat")?;
    for line in stat.lines() {
        if let Some(value) = line.strip_prefix("btime ") {
            return value.trim().parse().map_err(|_| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "kernel boot time is not a decimal integer",
                )
            });
        }
    }
    Err(io::Error::new(
        ErrorKind::NotFound,
        "kernel boot time is unavailable",
    ))
}

fn tick_ns() -> io::Result<u64> {
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if ticks <= 0 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "clock tick rate is unavailable",
        ));
    }
    Ok(1_000_000_000 / ticks as u64)
}

fn read_process(dir: &Path, pid: u32, btime: u64, tick_ns: u64) -> Option<ProcessRow> {
    let stat = fs::read_to_string(dir.join("stat")).ok()?;
    let parsed = parse_stat(&stat)?;
    if parsed.pid != pid {
        return None;
    }
    let start_unix_ns = start_unix_ns(btime, parsed.start_ticks, tick_ns);
    let comm = fs::read_to_string(dir.join("comm")).unwrap_or_default();
    let image = fs::read_link(dir.join("exe"))
        .ok()
        .and_then(|path| image_text(&path.to_string_lossy()));
    let (threads, threads_truncated) = read_threads(&dir.join("task"));
    let (modules, modules_truncated) = read_modules(&dir.join("maps"), image.as_deref());
    let limited = start_unix_ns.is_none() || image.is_none() || threads.is_empty();
    Some(ProcessRow {
        pid,
        parent_pid: parsed.parent_pid,
        start_unix_ns,
        name: process_name(comm.trim_end_matches('\n'), pid),
        image,
        threads,
        modules,
        modules_truncated,
        threads_truncated,
        limited,
    })
}

struct ParsedStat {
    pid: u32,
    parent_pid: u32,
    start_ticks: u64,
}

fn parse_stat(text: &str) -> Option<ParsedStat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    if close < open {
        return None;
    }
    let pid = text[..open].trim().parse().ok()?;
    let fields = text[close + 1..].split_whitespace().collect::<Vec<_>>();
    let parent_pid = fields.get(1)?.parse().ok()?;
    let start_ticks = fields.get(19)?.parse().ok()?;
    Some(ParsedStat {
        pid,
        parent_pid,
        start_ticks,
    })
}

fn start_unix_ns(btime: u64, start_ticks: u64, tick_ns: u64) -> Option<i64> {
    let boot_ns = btime.checked_mul(1_000_000_000)?;
    let offset = start_ticks.checked_mul(tick_ns)?;
    i64::try_from(boot_ns.checked_add(offset)?).ok()
}

fn read_threads(dir: &Path) -> (Vec<u32>, bool) {
    let Ok(entries) = fs::read_dir(dir) else {
        return (Vec::new(), false);
    };
    let mut threads = Vec::new();
    let mut truncated = false;
    for entry in entries.flatten() {
        let Ok(tid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if threads.len() == MAX_THREADS {
            truncated = true;
            break;
        }
        threads.push(tid);
    }
    (threads, truncated)
}

fn read_modules(path: &Path, preferred: Option<&str>) -> (Vec<ModuleRow>, bool) {
    let Ok(text) = fs::read_to_string(path) else {
        return (Vec::new(), false);
    };
    let mut bases = BTreeMap::<String, u64>::new();
    for line in text.lines() {
        let Some((base, name)) = executable_mapping(line) else {
            continue;
        };
        bases
            .entry(name)
            .and_modify(|current| *current = (*current).min(base))
            .or_insert(base);
    }
    super::limit_modules(bases, preferred)
}

fn executable_mapping(line: &str) -> Option<(u64, String)> {
    let mut columns = line.split_whitespace();
    let range = columns.next()?;
    let perms = columns.next()?;
    columns.next()?;
    columns.next()?;
    columns.next()?;
    if !perms.contains('x') {
        return None;
    }
    let path = columns.collect::<Vec<_>>().join(" ");
    if !path.starts_with('/') {
        return None;
    }
    let start = range.split_once('-')?.0;
    let base = u64::from_str_radix(start, 16).ok()?;
    Some((base, path))
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
    units.sort();
    units.dedup();
    units
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_parser_matches_this_process() {
        let text = fs::read_to_string("/proc/self/stat").expect("stat");
        let parsed = parse_stat(&text).expect("parse");
        assert_eq!(parsed.pid, std::process::id());
        assert!(parsed.start_ticks > 0);
    }

    #[test]
    fn service_units_keep_workload_units_and_skip_the_user_manager() {
        let cgroup = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/editor.service\n";
        assert_eq!(service_units(cgroup), vec!["editor.service".to_owned()]);
        assert!(service_units("0::/system.slice/pod/init").is_empty());
    }
}

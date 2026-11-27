//! Linux 上的 CPU、内存、GPU 和硬件计数器。
//!
//! 只解析 procfs、sysfs 和 perf 这次读到的文本或计数。文件不存在就是不支持，
//! 读失败就是失败。两种情况都不写成 0。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::sample::{
    BATTERY_CAPACITY, BATTERY_POWER, CPU_FREQUENCY_MAX, CPU_FREQUENCY_MIN, CPU_LOGICAL,
    CPU_TEMPERATURE, CPU_USAGE, GPU_USAGE, GPU_VRAM_TOTAL, GPU_VRAM_USED, LOAD, MEMORY_AVAILABLE,
    MEMORY_SWAP_TOTAL, MEMORY_TOTAL, MEMORY_USED, PERF_CPUS, PERF_CYCLES, PERF_INSTRUCTIONS,
    PROCESS_COUNT, ProcUse, Processes, RAPL_ENERGY, Reading, SENSOR_TEMPERATURE, Sample,
    bytes_from_kib, clean_name, clock_unix_ns, frequency_extent, memory_used, rank_processes,
    ratio_ppm,
};

const MAX_PROCESSES: usize = 4_096;

struct CpuTicks {
    busy: u64,
    total: u64,
}

struct ProcBase {
    start: u64,
    ticks: u64,
}

struct Counter {
    fd: i32,
    previous: Option<u64>,
}

impl Drop for Counter {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

struct PerfSource {
    cycles: Vec<Counter>,
    instructions: Vec<Counter>,
    source: &'static str,
    absent: Option<&'static str>,
}

pub struct Sampler {
    cpu: Option<CpuTicks>,
    rapl: Option<u64>,
    processes: BTreeMap<u32, ProcBase>,
    perf: PerfSource,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            cpu: None,
            rapl: None,
            processes: BTreeMap::new(),
            perf: open_perf(),
        }
    }

    pub fn sample(&mut self) -> Sample {
        let unix_ns = clock_unix_ns().unwrap_or(-1);
        let mut readings = Vec::new();
        match fs::read_to_string("/proc/stat") {
            Ok(text) => readings.extend(cpu_readings(&mut self.cpu, &text)),
            Err(_) => {
                readings.push(Reading::failed(
                    CPU_USAGE,
                    "ppm",
                    "procfs.stat",
                    "proc_stat",
                ));
                readings.push(Reading::failed(
                    CPU_LOGICAL,
                    "count",
                    "procfs.stat",
                    "proc_stat",
                ));
            }
        }
        match fs::read_to_string("/proc/meminfo") {
            Ok(text) => readings.extend(memory_readings(&text)),
            Err(_) => readings.extend(memory_readings("")),
        }
        readings.push(load_reading());
        readings.extend(frequency_readings());
        readings.extend(thermal_readings());
        readings.extend(battery_readings());
        readings.push(rapl_reading(&mut self.rapl));
        readings.extend(gpu_readings());
        readings.extend(self.perf.readings());
        let processes = process_readings(&mut self.processes);
        if let Some(count) = processes.count {
            readings.push(Reading::measured(
                PROCESS_COUNT,
                i64::try_from(count).unwrap_or(i64::MAX),
                "count",
                "procfs",
            ));
        } else {
            readings.push(Reading::failed(PROCESS_COUNT, "count", "procfs", "proc"));
        }
        Sample {
            unix_ns,
            readings,
            processes,
        }
    }
}

fn cpu_readings(previous: &mut Option<CpuTicks>, text: &str) -> Vec<Reading> {
    let Some(now) = parse_aggregate_cpu(text) else {
        return vec![
            Reading::failed(CPU_USAGE, "ppm", "procfs.stat", "proc_stat"),
            Reading::failed(CPU_LOGICAL, "count", "procfs.stat", "proc_stat"),
        ];
    };
    let logical = match logical_cpus(text) {
        Some(count) => Reading::measured(CPU_LOGICAL, count, "count", "procfs.stat"),
        None => Reading::failed(CPU_LOGICAL, "count", "procfs.stat", "proc_stat"),
    };
    let usage = match previous.take() {
        None => {
            *previous = Some(now);
            Reading::baseline(CPU_USAGE, "ppm", "procfs.stat")
        }
        Some(old) if now.busy < old.busy || now.total < old.total => {
            *previous = Some(now);
            Reading::failed(CPU_USAGE, "ppm", "procfs.stat", "counter_reset")
        }
        Some(old) => {
            let busy = now.busy - old.busy;
            let total = now.total - old.total;
            *previous = Some(now);
            match ratio_ppm(busy, total) {
                Ok(ppm) => Reading::measured(CPU_USAGE, ppm, "ppm", "procfs.stat"),
                Err(reason) => Reading::failed(CPU_USAGE, "ppm", "procfs.stat", reason),
            }
        }
    };
    vec![usage, logical]
}

fn parse_aggregate_cpu(text: &str) -> Option<CpuTicks> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("cpu") {
            continue;
        }
        let numbers: Vec<u64> = parts.map(|part| part.parse().ok()).collect::<Option<_>>()?;
        if numbers.len() < 4 {
            return None;
        }
        let user = numbers[0];
        let nice = numbers[1];
        let system = numbers[2];
        let idle = numbers[3];
        let iowait = numbers.get(4).copied().unwrap_or(0);
        let irq = numbers.get(5).copied().unwrap_or(0);
        let softirq = numbers.get(6).copied().unwrap_or(0);
        let steal = numbers.get(7).copied().unwrap_or(0);
        let busy = user
            .saturating_add(nice)
            .saturating_add(system)
            .saturating_add(irq)
            .saturating_add(softirq)
            .saturating_add(steal);
        let total = busy.saturating_add(idle.saturating_add(iowait));
        return Some(CpuTicks { busy, total });
    }
    None
}

fn logical_cpus(text: &str) -> Option<i64> {
    let count = text
        .lines()
        .filter(|line| {
            let name = line.split_whitespace().next().unwrap_or("");
            let Some(suffix) = name.strip_prefix("cpu") else {
                return false;
            };
            !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
        })
        .count();
    if count == 0 {
        None
    } else {
        i64::try_from(count).ok()
    }
}

fn memory_readings(text: &str) -> Vec<Reading> {
    let total = meminfo_kib(text, "MemTotal:");
    let available = meminfo_kib(text, "MemAvailable:");
    let swap = meminfo_kib(text, "SwapTotal:");
    let mut readings = Vec::new();
    readings.push(kib_reading(MEMORY_TOTAL, total, true));
    readings.push(match available {
        Some(kib) => kib_reading(MEMORY_AVAILABLE, Some(kib), false),
        None => Reading::unsupported(MEMORY_AVAILABLE, "bytes", "absent", "memavailable_absent"),
    });
    readings.push(match (total, available) {
        (Some(total), Some(available)) => {
            match (bytes_from_kib(total), bytes_from_kib(available)) {
                (Some(total_bytes), Some(available_bytes)) => {
                    match memory_used(total_bytes as u64, available_bytes as u64) {
                        Ok(used) => Reading::measured(MEMORY_USED, used, "bytes", "procfs.meminfo"),
                        Err(reason) => {
                            Reading::failed(MEMORY_USED, "bytes", "procfs.meminfo", reason)
                        }
                    }
                }
                _ => Reading::failed(
                    MEMORY_USED,
                    "bytes",
                    "procfs.meminfo",
                    "counter_inconsistent",
                ),
            }
        }
        (Some(_), None) => {
            Reading::unsupported(MEMORY_USED, "bytes", "absent", "memavailable_absent")
        }
        _ => Reading::failed(MEMORY_USED, "bytes", "procfs.meminfo", "meminfo"),
    });
    readings.push(match swap {
        Some(kib) => kib_reading(MEMORY_SWAP_TOTAL, Some(kib), false),
        None => Reading::unsupported(MEMORY_SWAP_TOTAL, "bytes", "absent", "no_swap"),
    });
    readings
}

fn kib_reading(id: &'static str, kib: Option<u64>, require_positive: bool) -> Reading {
    match kib.and_then(bytes_from_kib) {
        Some(bytes) if !require_positive || bytes > 0 => {
            Reading::measured(id, bytes, "bytes", "procfs.meminfo")
        }
        Some(_) => Reading::failed(id, "bytes", "procfs.meminfo", "counter_inconsistent"),
        None => Reading::failed(id, "bytes", "procfs.meminfo", "meminfo"),
    }
}

fn meminfo_kib(text: &str, key: &str) -> Option<u64> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() == Some(key) {
            return parts.next()?.parse().ok();
        }
    }
    None
}

fn load_reading() -> Reading {
    match fs::read_to_string("/proc/loadavg") {
        Ok(text) => match load_milli(&text) {
            Some(value) => Reading::measured(LOAD, value, "milli", "procfs.loadavg"),
            None => Reading::failed(LOAD, "milli", "procfs.loadavg", "loadavg"),
        },
        Err(_) => Reading::unsupported(LOAD, "milli", "absent", "no_loadavg"),
    }
}

fn load_milli(text: &str) -> Option<i64> {
    let first = text.split_whitespace().next()?;
    let value: f64 = first.parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let milli = (value * 1000.0).round();
    if milli > i64::MAX as f64 {
        return None;
    }
    Some(milli as i64)
}

fn frequency_readings() -> Vec<Reading> {
    let root = Path::new("/sys/devices/system/cpu");
    let Ok(entries) = fs::read_dir(root) else {
        return frequency_absent();
    };
    let mut values = Vec::new();
    let mut saw_cpu = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(suffix) = name.strip_prefix("cpu") else {
            continue;
        };
        if suffix.is_empty() || !suffix.chars().all(|character| character.is_ascii_digit()) {
            continue;
        }
        saw_cpu = true;
        let scaling = entry.path().join("cpufreq/scaling_cur_freq");
        let info = entry.path().join("cpufreq/cpuinfo_cur_freq");
        if let Some(value) = read_u64(&scaling).or_else(|| read_u64(&info)) {
            values.push(value);
        }
    }
    if !saw_cpu {
        return frequency_absent();
    }
    match frequency_extent(&values) {
        Some((min, max)) => vec![
            Reading::measured(CPU_FREQUENCY_MIN, min, "khz", "sysfs.cpufreq"),
            Reading::measured(CPU_FREQUENCY_MAX, max, "khz", "sysfs.cpufreq"),
        ],
        None => vec![
            Reading::unsupported(CPU_FREQUENCY_MIN, "khz", "absent", "no_cpufreq"),
            Reading::unsupported(CPU_FREQUENCY_MAX, "khz", "absent", "no_cpufreq"),
        ],
    }
}

fn frequency_absent() -> Vec<Reading> {
    vec![
        Reading::unsupported(CPU_FREQUENCY_MIN, "khz", "absent", "no_cpufreq"),
        Reading::unsupported(CPU_FREQUENCY_MAX, "khz", "absent", "no_cpufreq"),
    ]
}

fn thermal_readings() -> Vec<Reading> {
    let zones = thermal_zones();
    let hwmon = hwmon_millic();
    let cpu_zones: Vec<i64> = zones
        .iter()
        .filter(|(kind, _)| is_cpu_zone(kind))
        .map(|(_, value)| *value)
        .collect();
    let cpu = if !cpu_zones.is_empty() {
        Reading::measured(
            CPU_TEMPERATURE,
            cpu_zones.iter().copied().max().unwrap_or(0),
            "millic",
            "sysfs.thermal",
        )
    } else if zones.is_empty() && !thermal_directory_exists() {
        Reading::unsupported(CPU_TEMPERATURE, "millic", "absent", "no_thermal_zone")
    } else {
        Reading::unsupported(CPU_TEMPERATURE, "millic", "sysfs.thermal", "no_cpu_zone")
    };
    let sensor = if let Some(max) = hwmon.iter().copied().max() {
        Reading::measured(SENSOR_TEMPERATURE, max, "millic", "sysfs.hwmon")
    } else if let Some(max) = zones.iter().map(|(_, value)| *value).max() {
        Reading::measured(SENSOR_TEMPERATURE, max, "millic", "sysfs.thermal")
    } else {
        Reading::unsupported(SENSOR_TEMPERATURE, "millic", "absent", "no_sensor")
    };
    vec![cpu, sensor]
}

fn thermal_directory_exists() -> bool {
    Path::new("/sys/class/thermal").is_dir()
}

fn is_cpu_zone(kind: &str) -> bool {
    let kind = kind.to_ascii_lowercase();
    kind.contains("cpu") || kind.contains("x86_pkg") || kind.contains("pkg")
}

fn thermal_zones() -> Vec<(String, i64)> {
    let Ok(entries) = fs::read_dir("/sys/class/thermal") else {
        return Vec::new();
    };
    let mut zones = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let Some(temp) = read_i64(&entry.path().join("temp")) else {
            continue;
        };
        let kind = fs::read_to_string(entry.path().join("type"))
            .unwrap_or_default()
            .trim()
            .to_owned();
        zones.push((kind, temp));
    }
    zones
}

fn hwmon_millic() -> Vec<i64> {
    let Ok(entries) = fs::read_dir("/sys/class/hwmon") else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for entry in entries.flatten() {
        let Ok(files) = fs::read_dir(entry.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with("temp") && name.ends_with("_input")) {
                continue;
            }
            if let Some(value) = read_i64(&file.path()) {
                values.push(value);
            }
            if values.len() == 64 {
                return values;
            }
        }
    }
    values
}

fn battery_readings() -> Vec<Reading> {
    let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
        return vec![
            Reading::unsupported(BATTERY_CAPACITY, "percent", "absent", "no_battery"),
            Reading::unsupported(BATTERY_POWER, "microwatts", "absent", "no_battery"),
        ];
    };
    let mut capacity = None;
    let mut power = None;
    let mut saw_battery = false;
    for entry in entries.flatten() {
        let kind = fs::read_to_string(entry.path().join("type")).unwrap_or_default();
        if !kind.to_ascii_lowercase().contains("battery") {
            continue;
        }
        saw_battery = true;
        if capacity.is_none() {
            capacity = read_i64(&entry.path().join("capacity"));
        }
        if power.is_none() {
            power = read_i64(&entry.path().join("power_now")).or_else(|| {
                let current = read_i64(&entry.path().join("current_now"))?;
                let voltage = read_i64(&entry.path().join("voltage_now"))?;
                current.checked_mul(voltage)?.checked_div(1_000_000)
            });
        }
    }
    if !saw_battery {
        return vec![
            Reading::unsupported(BATTERY_CAPACITY, "percent", "absent", "no_battery"),
            Reading::unsupported(BATTERY_POWER, "microwatts", "absent", "no_battery"),
        ];
    }
    let capacity = match capacity {
        Some(value) if (0..=100).contains(&value) => {
            Reading::measured(BATTERY_CAPACITY, value, "percent", "sysfs.power_supply")
        }
        Some(_) => Reading::failed(
            BATTERY_CAPACITY,
            "percent",
            "sysfs.power_supply",
            "counter_inconsistent",
        ),
        None => Reading::failed(
            BATTERY_CAPACITY,
            "percent",
            "sysfs.power_supply",
            "battery_read",
        ),
    };
    let power = match power {
        Some(value) if value >= 0 => {
            Reading::measured(BATTERY_POWER, value, "microwatts", "sysfs.power_supply")
        }
        Some(_) => Reading::failed(
            BATTERY_POWER,
            "microwatts",
            "sysfs.power_supply",
            "counter_inconsistent",
        ),
        None => Reading::unsupported(BATTERY_POWER, "microwatts", "absent", "no_power"),
    };
    vec![capacity, power]
}

fn rapl_reading(previous: &mut Option<u64>) -> Reading {
    let Some(path) = find_rapl() else {
        return Reading::unsupported(RAPL_ENERGY, "uj", "absent", "no_rapl");
    };
    let Some(value) = read_u64(&path) else {
        return Reading::failed(RAPL_ENERGY, "uj", "powercap", "rapl_read");
    };
    match previous.take() {
        None => {
            *previous = Some(value);
            Reading::baseline(RAPL_ENERGY, "uj", "powercap")
        }
        Some(old) if value < old => {
            *previous = Some(value);
            Reading::failed(RAPL_ENERGY, "uj", "powercap", "counter_reset")
        }
        Some(old) => {
            *previous = Some(value);
            match i64::try_from(value - old) {
                Ok(delta) => Reading::measured(RAPL_ENERGY, delta, "uj", "powercap"),
                Err(_) => Reading::failed(RAPL_ENERGY, "uj", "powercap", "counter_inconsistent"),
            }
        }
    }
}

fn find_rapl() -> Option<PathBuf> {
    let mut fallback = None;
    for entry in fs::read_dir("/sys/class/powercap").ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("intel-rapl:") || name.matches(':').count() != 1 {
            continue;
        }
        let energy = entry.path().join("energy_uj");
        if !energy.is_file() {
            continue;
        }
        let label = fs::read_to_string(entry.path().join("name")).unwrap_or_default();
        if label.to_ascii_lowercase().contains("package") {
            return Some(energy);
        }
        fallback = Some(energy);
    }
    fallback
}

fn gpu_readings() -> Vec<Reading> {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return gpu_unsupported();
    };
    let mut busy = Vec::new();
    let mut total = Vec::new();
    let mut used = Vec::new();
    let mut saw_card = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(suffix) = name.strip_prefix("card") else {
            continue;
        };
        if suffix.is_empty()
            || name.contains('-')
            || !suffix.chars().all(|character| character.is_ascii_digit())
        {
            continue;
        }
        saw_card = true;
        let device = entry.path().join("device");
        if let Some(value) = read_u64(&device.join("gpu_busy_percent")) {
            busy.push(value);
        }
        if let Some(value) = read_u64(&device.join("mem_info_vram_total")) {
            total.push(value);
        }
        if let Some(value) = read_u64(&device.join("mem_info_vram_used")) {
            used.push(value);
        }
    }
    if !saw_card {
        return gpu_unsupported();
    }
    let mut readings = Vec::new();
    if busy.is_empty() {
        readings.push(Reading::unsupported(
            GPU_USAGE,
            "ppm",
            "absent",
            "no_counter",
        ));
    } else if busy.iter().any(|value| *value > 100) {
        readings.push(Reading::failed(
            GPU_USAGE,
            "ppm",
            "sysfs.drm",
            "counter_inconsistent",
        ));
    } else {
        let max = busy.iter().copied().max().unwrap_or(0);
        let Some(max) = i64::try_from(max).ok() else {
            readings.push(Reading::failed(
                GPU_USAGE,
                "ppm",
                "sysfs.drm",
                "counter_inconsistent",
            ));
            readings.push(sum_bytes(GPU_VRAM_TOTAL, &total));
            readings.push(sum_bytes(GPU_VRAM_USED, &used));
            return readings;
        };
        let source = if busy.len() > 1 {
            "sysfs.drm.max"
        } else {
            "sysfs.drm"
        };
        readings.push(Reading::measured(GPU_USAGE, max * 10_000, "ppm", source));
    }
    readings.push(sum_bytes(GPU_VRAM_TOTAL, &total));
    readings.push(sum_bytes(GPU_VRAM_USED, &used));
    readings
}

fn gpu_unsupported() -> Vec<Reading> {
    vec![
        Reading::unsupported(GPU_USAGE, "ppm", "absent", "no_counter"),
        Reading::unsupported(GPU_VRAM_TOTAL, "bytes", "absent", "no_counter"),
        Reading::unsupported(GPU_VRAM_USED, "bytes", "absent", "no_counter"),
    ]
}

fn sum_bytes(id: &'static str, values: &[u64]) -> Reading {
    if values.is_empty() {
        return Reading::unsupported(id, "bytes", "absent", "no_counter");
    }
    match values
        .iter()
        .try_fold(0u64, |sum, value| sum.checked_add(*value))
        .and_then(|sum| i64::try_from(sum).ok())
    {
        Some(value) => Reading::measured(id, value, "bytes", "sysfs.drm"),
        None => Reading::failed(id, "bytes", "sysfs.drm", "counter_inconsistent"),
    }
}

fn process_readings(previous: &mut BTreeMap<u32, ProcBase>) -> Processes {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Processes::failed();
    };
    let mut rows = Vec::new();
    let mut count = 0u64;
    let mut truncated = false;
    let mut limited = false;
    let mut seen = BTreeMap::new();
    for entry in entries {
        let Ok(entry) = entry else {
            limited = true;
            continue;
        };
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|text| text.parse::<u32>().ok()) else {
            continue;
        };
        count = count.saturating_add(1);
        if rows.len() >= MAX_PROCESSES {
            truncated = true;
            continue;
        }
        match read_process(pid, previous) {
            Some(row) => {
                if row.usage.rss_bytes.is_none() {
                    limited = true;
                }
                if let Some(start) = row.start {
                    seen.insert(pid, start);
                }
                rows.push(row.usage);
            }
            None => limited = true,
        }
    }
    previous.retain(|pid, base| seen.get(pid) == Some(&base.start));
    let (by_rss, by_cpu, cpu_state) = rank_processes(&mut rows);
    Processes {
        count: Some(count),
        truncated,
        limited,
        by_rss,
        by_cpu,
        cpu_state,
    }
}

struct ProcRow {
    usage: ProcUse,
    start: Option<u64>,
}

fn read_process(pid: u32, previous: &mut BTreeMap<u32, ProcBase>) -> Option<ProcRow> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (start, ticks) = parse_proc_stat(&stat)?;
    let name = process_name(pid, &stat);
    let rss = read_rss(pid);
    let cpu_ticks = match previous.get(&pid) {
        Some(old) if old.start == start && ticks >= old.ticks => Some(ticks - old.ticks),
        Some(old) if old.start == start => None,
        _ => None,
    };
    previous.insert(pid, ProcBase { start, ticks });
    Some(ProcRow {
        usage: ProcUse {
            pid,
            name,
            rss_bytes: rss,
            cpu_ticks,
        },
        start: Some(start),
    })
}

fn process_name(pid: u32, stat: &str) -> String {
    if let Ok(text) = fs::read_to_string(format!("/proc/{pid}/status")) {
        for line in text.lines() {
            if let Some(name) = line.strip_prefix("Name:") {
                return clean_name(name.trim());
            }
        }
    }
    if let Some(open) = stat.find('(')
        && let Some(close) = stat.rfind(')')
        && close > open
    {
        return clean_name(&stat[open + 1..close]);
    }
    clean_name("")
}

fn read_rss(pid: u32) -> Option<i64> {
    let text = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kib: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return bytes_from_kib(kib);
        }
    }
    None
}

fn parse_proc_stat(text: &str) -> Option<(u64, u64)> {
    let close = text.rfind(')')?;
    let fields: Vec<&str> = text[close + 1..].split_whitespace().collect();
    if fields.len() < 20 {
        return None;
    }
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    let start: u64 = fields[19].parse().ok()?;
    Some((start, utime.saturating_add(stime)))
}

fn read_u64(path: &Path) -> Option<u64> {
    let text = fs::read_to_string(path).ok()?;
    text.trim().parse().ok()
}

fn read_i64(path: &Path) -> Option<i64> {
    let text = fs::read_to_string(path).ok()?;
    text.trim().parse().ok()
}

impl PerfSource {
    fn readings(&mut self) -> Vec<Reading> {
        if let Some(reason) = self.absent {
            return vec![
                Reading::unsupported(PERF_CYCLES, "count", "perf", reason),
                Reading::unsupported(PERF_INSTRUCTIONS, "count", "perf", reason),
            ];
        }
        let cycles = counter_delta(&mut self.cycles);
        let instructions = counter_delta(&mut self.instructions);
        let mut readings = vec![
            delta_reading(PERF_CYCLES, cycles, self.source),
            delta_reading(PERF_INSTRUCTIONS, instructions, self.source),
        ];
        if self.source != "perf.self" {
            readings.push(Reading::measured(
                PERF_CPUS,
                i64::try_from(self.cycles.len()).unwrap_or(i64::MAX),
                "count",
                self.source,
            ));
        }
        readings
    }
}

fn delta_reading(id: &'static str, delta: CounterDelta, source: &'static str) -> Reading {
    match delta {
        CounterDelta::Baseline => Reading::baseline(id, "count", source),
        CounterDelta::Measured(value) => Reading::measured(id, value, "count", source),
        CounterDelta::Failed(reason) => Reading::failed(id, "count", source, reason),
    }
}

enum CounterDelta {
    Baseline,
    Measured(i64),
    Failed(&'static str),
}

fn counter_delta(counters: &mut [Counter]) -> CounterDelta {
    if counters.is_empty() {
        return CounterDelta::Failed("perf_read");
    }
    let mut any_previous = false;
    let mut sum = 0u64;
    let mut failed = false;
    for counter in counters.iter_mut() {
        let Some(value) = read_counter(counter.fd) else {
            failed = true;
            counter.previous = None;
            continue;
        };
        match counter.previous {
            None => counter.previous = Some(value),
            Some(old) if value < old => {
                counter.previous = Some(value);
                failed = true;
            }
            Some(old) => {
                any_previous = true;
                sum = sum.saturating_add(value - old);
                counter.previous = Some(value);
            }
        }
    }
    if !any_previous && !failed {
        CounterDelta::Baseline
    } else if !any_previous {
        CounterDelta::Failed("perf_read")
    } else {
        match i64::try_from(sum) {
            Ok(value) => CounterDelta::Measured(value),
            Err(_) => CounterDelta::Failed("counter_inconsistent"),
        }
    }
}

fn read_counter(fd: i32) -> Option<u64> {
    let mut value = 0u64;
    let read = unsafe {
        libc::read(
            fd,
            &mut value as *mut u64 as *mut libc::c_void,
            std::mem::size_of::<u64>(),
        )
    };
    if read == std::mem::size_of::<u64>() as isize {
        Some(value)
    } else {
        None
    }
}

fn open_perf() -> PerfSource {
    let cpus = online_cpus();
    let mut cycles = Vec::new();
    let mut instructions = Vec::new();
    let mut missed = 0u64;
    for cpu in cpus {
        let Ok(cycle) = open_counter(PERF_COUNT_HW_CPU_CYCLES, -1, cpu) else {
            missed = missed.saturating_add(1);
            continue;
        };
        let Ok(instruction) = open_counter(PERF_COUNT_HW_INSTRUCTIONS, -1, cpu) else {
            missed = missed.saturating_add(1);
            continue;
        };
        cycles.push(cycle);
        instructions.push(instruction);
    }
    if !cycles.is_empty() {
        let source = if missed == 0 {
            "perf.cpu"
        } else {
            "perf.cpu.partial"
        };
        return PerfSource {
            cycles,
            instructions,
            source,
            absent: None,
        };
    }
    match (
        open_counter(PERF_COUNT_HW_CPU_CYCLES, 0, -1),
        open_counter(PERF_COUNT_HW_INSTRUCTIONS, 0, -1),
    ) {
        (Ok(cycle), Ok(instruction)) => PerfSource {
            cycles: vec![cycle],
            instructions: vec![instruction],
            source: "perf.self",
            absent: None,
        },
        (Err(error), _) | (_, Err(error)) => PerfSource {
            cycles: Vec::new(),
            instructions: Vec::new(),
            source: "perf",
            absent: Some(perf_reason(error)),
        },
    }
}

fn online_cpus() -> Vec<i32> {
    let Ok(text) = fs::read_to_string("/sys/devices/system/cpu/online") else {
        return vec![0];
    };
    let mut cpus = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let Ok(start) = start.parse::<i32>() else {
                continue;
            };
            let Ok(end) = end.parse::<i32>() else {
                continue;
            };
            if (0..=256).contains(&start) && end >= start && end - start < 256 {
                cpus.extend(start..=end);
            }
        } else if let Ok(cpu) = part.parse::<i32>()
            && (0..=256).contains(&cpu)
        {
            cpus.push(cpu);
        }
    }
    cpus.truncate(256);
    if cpus.is_empty() { vec![0] } else { cpus }
}

fn open_counter(config: u64, pid: i32, cpu: i32) -> Result<Counter, i32> {
    let mut attr = PerfEventAttr {
        type_: PERF_TYPE_HARDWARE,
        size: PERF_ATTR_SIZE_VER0,
        config,
        sample_period: 0,
        sample_type: 0,
        read_format: 0,
        flags: EXCLUDE_KERNEL | EXCLUDE_HV,
        wakeup: 0,
        bp_type: 0,
        bp_addr: 0,
    };
    let fd = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &mut attr as *mut PerfEventAttr,
            pid,
            cpu,
            -1i32,
            0u64,
        )
    };
    if fd < 0 {
        Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(0))
    } else {
        Ok(Counter {
            fd: fd as i32,
            previous: None,
        })
    }
}

/// `perf_event_attr` 的前 64 字节，对应内核 `PERF_ATTR_SIZE_VER0`。
#[repr(C)]
struct PerfEventAttr {
    type_: u32,
    size: u32,
    config: u64,
    sample_period: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup: u32,
    bp_type: u32,
    bp_addr: u64,
}

const PERF_ATTR_SIZE_VER0: u32 = 64;
const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
const EXCLUDE_KERNEL: u64 = 1 << 5;
const EXCLUDE_HV: u64 = 1 << 6;

const _: () = assert!(std::mem::size_of::<PerfEventAttr>() == 64);

fn perf_reason(error: i32) -> &'static str {
    if error == libc::EACCES || error == libc::EPERM {
        "perf_denied"
    } else {
        "perf_unavailable"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sample::{MEMORY_USED, Status};

    #[test]
    fn cpu_delta_is_parts_per_million_and_the_first_sample_is_baseline() {
        let first = "cpu  100 0 100 800 0 0 0 0 0 0\ncpu0 100 0 100 800 0 0 0 0 0 0\n";
        let second = "cpu  200 0 200 1600 0 0 0 0 0 0\ncpu0 200 0 200 1600 0 0 0 0 0 0\n";
        let mut previous = None;
        let baseline = cpu_readings(&mut previous, first);
        assert_eq!(baseline[0].status, Status::Baseline);
        assert_eq!(baseline[0].value, None);
        assert_eq!(baseline[1].value, Some(1));
        let measured = cpu_readings(&mut previous, second);
        assert_eq!(measured[0].status, Status::Measured);
        assert_eq!(measured[0].value, Some(200_000));
    }

    #[test]
    fn missing_available_memory_does_not_invent_used() {
        let text = "MemTotal: 1024 kB\nMemFree: 100 kB\n";
        let readings = memory_readings(text);
        let used = readings
            .iter()
            .find(|reading| reading.id == MEMORY_USED)
            .unwrap();
        assert_eq!(used.status, Status::Unsupported);
        assert_eq!(used.value, None);
        let total = readings
            .iter()
            .find(|reading| reading.id == MEMORY_TOTAL)
            .unwrap();
        assert_eq!(total.value, Some(1024 * 1024));
    }

    #[test]
    fn proc_stat_keeps_ticks_after_a_name_with_spaces() {
        let text = "12 (a name) S 1 1 1 0 -1 0 0 0 0 0 5 7 0 0 20 0 1 0 99 0 0\n";
        assert_eq!(parse_proc_stat(text), Some((99, 12)));
    }
}

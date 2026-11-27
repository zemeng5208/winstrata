//! 一次脉冲采样的读数。
//!
//! 测到的值才带数字。基线、不支持和失败都不写数值，也不把 0 当成当前测量。

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

pub const CPU_USAGE: &str = "cpu.usage.ppm";
pub const CPU_LOGICAL: &str = "cpu.logical";
pub const CPU_FREQUENCY_MIN: &str = "cpu.frequency.min.khz";
pub const CPU_FREQUENCY_MAX: &str = "cpu.frequency.max.khz";
pub const CPU_TEMPERATURE: &str = "cpu.temperature.millic";
pub const MEMORY_TOTAL: &str = "memory.total.bytes";
pub const MEMORY_AVAILABLE: &str = "memory.available.bytes";
pub const MEMORY_USED: &str = "memory.used.bytes";
pub const MEMORY_SWAP_TOTAL: &str = "memory.swap.total.bytes";
pub const LOAD: &str = "load.1m.milli";
pub const GPU_USAGE: &str = "gpu.usage.ppm";
pub const GPU_VRAM_TOTAL: &str = "gpu.vram.total.bytes";
pub const GPU_VRAM_USED: &str = "gpu.vram.used.bytes";
pub const BATTERY_CAPACITY: &str = "battery.capacity.percent";
pub const BATTERY_POWER: &str = "battery.power.microwatts";
pub const SENSOR_TEMPERATURE: &str = "sensor.temperature.max.millic";
pub const RAPL_ENERGY: &str = "rapl.energy.uj";
pub const PERF_CYCLES: &str = "perf.cycles";
pub const PERF_INSTRUCTIONS: &str = "perf.instructions";
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub const PERF_CPUS: &str = "perf.cpus";
pub const PROCESS_COUNT: &str = "process.count";

pub const TOP_PROCESSES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Measured,
    Baseline,
    Unsupported,
    Failed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Baseline => "baseline",
            Self::Unsupported => "unsupported",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub id: &'static str,
    pub status: Status,
    pub value: Option<i64>,
    pub unit: &'static str,
    pub source: &'static str,
    pub reason: Option<&'static str>,
}

impl Reading {
    pub fn measured(
        id: &'static str,
        value: i64,
        unit: &'static str,
        source: &'static str,
    ) -> Self {
        Self {
            id,
            status: Status::Measured,
            value: Some(value),
            unit,
            source,
            reason: None,
        }
    }

    pub fn baseline(id: &'static str, unit: &'static str, source: &'static str) -> Self {
        Self {
            id,
            status: Status::Baseline,
            value: None,
            unit,
            source,
            reason: Some("baseline"),
        }
    }

    pub fn unsupported(
        id: &'static str,
        unit: &'static str,
        source: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            id,
            status: Status::Unsupported,
            value: None,
            unit,
            source,
            reason: Some(reason),
        }
    }

    pub fn failed(
        id: &'static str,
        unit: &'static str,
        source: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            id,
            status: Status::Failed,
            value: None,
            unit,
            source,
            reason: Some(reason),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcUse {
    pub pid: u32,
    pub name: String,
    pub rss_bytes: Option<i64>,
    pub cpu_ticks: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Processes {
    pub count: Option<u64>,
    pub truncated: bool,
    pub limited: bool,
    pub by_rss: Vec<ProcUse>,
    pub by_cpu: Vec<ProcUse>,
    pub cpu_state: Status,
}

impl Processes {
    pub fn failed() -> Self {
        Self {
            count: None,
            truncated: false,
            limited: true,
            by_rss: Vec::new(),
            by_cpu: Vec::new(),
            cpu_state: Status::Failed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub unix_ns: i64,
    pub readings: Vec<Reading>,
    pub processes: Processes,
}

pub fn clock_unix_ns() -> Option<i64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(elapsed.as_nanos()).ok()
}

pub fn ratio_ppm(busy: u64, total: u64) -> Result<i64, &'static str> {
    if total == 0 {
        return Err("no_tick");
    }
    if busy > total {
        return Err("counter_inconsistent");
    }
    let ppm = busy.saturating_mul(1_000_000) / total;
    i64::try_from(ppm).map_err(|_| "counter_inconsistent")
}

pub fn frequency_extent(values: &[u64]) -> Option<(i64, i64)> {
    let mut present = values.iter().copied().filter(|value| *value > 0);
    let first = present.next()?;
    let mut min = first;
    let mut max = first;
    for value in present {
        min = min.min(value);
        max = max.max(value);
    }
    let min = i64::try_from(min).ok()?;
    let max = i64::try_from(max).ok()?;
    Some((min, max))
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn battery_capacity(flag: u8, percent: u8) -> Reading {
    if flag & 128 != 0 {
        return Reading::unsupported(BATTERY_CAPACITY, "percent", "power", "no_battery");
    }
    if percent == 255 {
        return Reading::unsupported(BATTERY_CAPACITY, "percent", "power", "unknown");
    }
    if percent > 100 {
        return Reading::failed(BATTERY_CAPACITY, "percent", "power", "counter_inconsistent");
    }
    Reading::measured(BATTERY_CAPACITY, i64::from(percent), "percent", "power")
}

pub fn memory_used(total: u64, available: u64) -> Result<i64, &'static str> {
    if available > total {
        return Err("counter_inconsistent");
    }
    i64::try_from(total - available).map_err(|_| "counter_inconsistent")
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn bytes_from_kib(kib: u64) -> Option<i64> {
    kib.checked_mul(1024)
        .and_then(|bytes| i64::try_from(bytes).ok())
}

pub fn clean_name(value: &str) -> String {
    let mut name: String = value
        .chars()
        .filter(|character| !character.is_control())
        .take(64)
        .collect();
    if name.is_empty() {
        name.push('?');
    }
    name
}

pub fn rank_processes(rows: &mut Vec<ProcUse>) -> (Vec<ProcUse>, Vec<ProcUse>, Status) {
    let mut by_rss: Vec<ProcUse> = rows
        .iter()
        .filter(|row| row.rss_bytes.is_some())
        .cloned()
        .collect();
    by_rss.sort_by(|left, right| {
        right
            .rss_bytes
            .cmp(&left.rss_bytes)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    by_rss.truncate(TOP_PROCESSES);
    let any_cpu = rows.iter().any(|row| row.cpu_ticks.is_some());
    let mut by_cpu: Vec<ProcUse> = rows
        .iter()
        .filter(|row| row.cpu_ticks.is_some())
        .cloned()
        .collect();
    by_cpu.sort_by(|left, right| {
        right
            .cpu_ticks
            .cmp(&left.cpu_ticks)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    by_cpu.truncate(TOP_PROCESSES);
    let cpu_state = if any_cpu {
        Status::Measured
    } else {
        Status::Baseline
    };
    (by_rss, by_cpu, cpu_state)
}

pub fn sample_document(
    sample: &Sample,
    peaks: &BTreeMap<String, i64>,
    store: &str,
    ordinal: u64,
) -> String {
    let readings: Vec<Value> = sample.readings.iter().map(reading_value).collect();
    let peaks: Map<String, Value> = peaks
        .iter()
        .map(|(id, value)| (id.clone(), json!(value)))
        .collect();
    let body = json!({
        "unix_ns": sample.unix_ns.to_string(),
        "sample": ordinal,
        "store": store,
        "readings": readings,
        "processes": {
            "count": sample.processes.count,
            "truncated": sample.processes.truncated,
            "limited": sample.processes.limited,
            "cpu": sample.processes.cpu_state.as_str(),
            "by_rss": sample.processes.by_rss.iter().map(process_value).collect::<Vec<_>>(),
            "by_cpu": sample.processes.by_cpu.iter().map(process_value).collect::<Vec<_>>(),
        },
        "peaks": peaks,
    });
    body.to_string()
}

pub fn update_peaks(peaks: &mut BTreeMap<String, i64>, readings: &[Reading]) {
    for reading in readings {
        if reading.status != Status::Measured {
            continue;
        }
        let Some(value) = reading.value else {
            continue;
        };
        let key = format!("pulse.{}", reading.id);
        match peaks.get(&key) {
            Some(current) if *current >= value => {}
            _ => {
                peaks.insert(key, value);
            }
        }
    }
}

fn reading_value(reading: &Reading) -> Value {
    let mut body = Map::new();
    body.insert("id".to_owned(), json!(reading.id));
    body.insert("status".to_owned(), json!(reading.status.as_str()));
    body.insert("unit".to_owned(), json!(reading.unit));
    body.insert("source".to_owned(), json!(reading.source));
    if let Some(value) = reading.value {
        body.insert("value".to_owned(), json!(value));
    }
    if let Some(reason) = reading.reason {
        body.insert("reason".to_owned(), json!(reason));
    }
    Value::Object(body)
}

fn process_value(row: &ProcUse) -> Value {
    let mut body = Map::new();
    body.insert("pid".to_owned(), json!(row.pid));
    body.insert("name".to_owned(), json!(row.name));
    if let Some(rss) = row.rss_bytes {
        body.insert("rss_bytes".to_owned(), json!(rss));
    }
    if let Some(ticks) = row.cpu_ticks {
        body.insert("cpu_ticks".to_owned(), json!(ticks));
    }
    Value::Object(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_uses_the_delta_and_rejects_a_zero_span() {
        assert_eq!(ratio_ppm(200, 1_000), Ok(200_000));
        assert_eq!(ratio_ppm(0, 1_000), Ok(0));
        assert_eq!(ratio_ppm(1, 0), Err("no_tick"));
        assert_eq!(ratio_ppm(5, 4), Err("counter_inconsistent"));
    }

    #[test]
    fn frequency_ignores_a_zero_placeholder() {
        assert_eq!(
            frequency_extent(&[0, 2_400_000, 800_000]),
            Some((800_000, 2_400_000))
        );
        assert_eq!(frequency_extent(&[0, 0]), None);
    }

    #[test]
    fn battery_does_not_turn_an_unknown_percent_into_zero() {
        assert_eq!(battery_capacity(128, 0).status, Status::Unsupported);
        assert_eq!(battery_capacity(128, 0).value, None);
        assert_eq!(battery_capacity(0, 255).status, Status::Unsupported);
        assert_eq!(battery_capacity(0, 255).value, None);
        assert_eq!(battery_capacity(0, 40).value, Some(40));
        assert_eq!(battery_capacity(1, 0).value, Some(0));
    }

    #[test]
    fn used_memory_is_not_invented_when_available_exceeds_total() {
        assert_eq!(memory_used(1_000, 400), Ok(600));
        assert_eq!(memory_used(400, 1_000), Err("counter_inconsistent"));
    }
}

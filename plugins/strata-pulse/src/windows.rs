//! Windows 上的 CPU、内存、频率、电池和进程资源。
//!
//! GPU 利用率、显存和温度在这个采集器里没有对应的 Win32 读取器，因此记为不支持，
//! 原因是 `reader_absent`，而不是把设备说成没有计数器，也不是写成 0。

use std::collections::BTreeMap;
use std::mem::{self, size_of};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::Foundation::{FILETIME, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Power::{
    CallNtPowerInformation, GetSystemPowerStatus, PROCESSOR_POWER_INFORMATION,
    ProcessorInformation, SYSTEM_POWER_STATUS,
};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::SystemInformation::{
    GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, GetSystemTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::sample::{
    BATTERY_CAPACITY, BATTERY_POWER, CPU_FREQUENCY_MAX, CPU_FREQUENCY_MIN, CPU_LOGICAL,
    CPU_TEMPERATURE, CPU_USAGE, GPU_USAGE, GPU_VRAM_TOTAL, GPU_VRAM_USED, LOAD, MEMORY_AVAILABLE,
    MEMORY_SWAP_TOTAL, MEMORY_TOTAL, MEMORY_USED, PERF_CYCLES, PERF_INSTRUCTIONS, PROCESS_COUNT,
    ProcUse, Processes, RAPL_ENERGY, Reading, SENSOR_TEMPERATURE, Sample, battery_capacity,
    clean_name, clock_unix_ns, frequency_extent, memory_used, rank_processes, ratio_ppm,
};

const MAX_PROCESSES: usize = 4_096;

struct CpuTicks {
    busy: u64,
    total: u64,
}

struct ProcBase {
    created: u64,
    ticks: u64,
}

pub struct Sampler {
    cpu: Option<CpuTicks>,
    processes: BTreeMap<u32, ProcBase>,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            cpu: None,
            processes: BTreeMap::new(),
        }
    }

    pub fn sample(&mut self) -> Sample {
        let unix_ns = clock_unix_ns().unwrap_or(-1);
        let mut readings = Vec::new();
        readings.extend(cpu_readings(&mut self.cpu));
        readings.extend(memory_readings());
        readings.push(Reading::unsupported(LOAD, "milli", "absent", "no_loadavg"));
        readings.extend(frequency_readings());
        readings.push(Reading::unsupported(
            CPU_TEMPERATURE,
            "millic",
            "absent",
            "reader_absent",
        ));
        readings.push(sensor_absent());
        readings.extend(battery_readings());
        readings.push(Reading::unsupported(
            RAPL_ENERGY,
            "uj",
            "absent",
            "reader_absent",
        ));
        readings.extend(gpu_absent());
        readings.push(Reading::unsupported(
            PERF_CYCLES,
            "count",
            "absent",
            "reader_absent",
        ));
        readings.push(Reading::unsupported(
            PERF_INSTRUCTIONS,
            "count",
            "absent",
            "reader_absent",
        ));
        let processes = process_readings(&mut self.processes);
        if let Some(count) = processes.count {
            readings.push(Reading::measured(
                PROCESS_COUNT,
                i64::try_from(count).unwrap_or(i64::MAX),
                "count",
                "toolhelp",
            ));
        } else {
            readings.push(Reading::failed(
                PROCESS_COUNT,
                "count",
                "toolhelp",
                "process_snapshot",
            ));
        }
        Sample {
            unix_ns,
            readings,
            processes,
        }
    }
}

fn sensor_absent() -> Reading {
    Reading::unsupported(SENSOR_TEMPERATURE, "millic", "absent", "reader_absent")
}

fn gpu_absent() -> Vec<Reading> {
    vec![
        Reading::unsupported(GPU_USAGE, "ppm", "absent", "reader_absent"),
        Reading::unsupported(GPU_VRAM_TOTAL, "bytes", "absent", "reader_absent"),
        Reading::unsupported(GPU_VRAM_USED, "bytes", "absent", "reader_absent"),
    ]
}

fn cpu_readings(previous: &mut Option<CpuTicks>) -> Vec<Reading> {
    let logical = logical_processors();
    let Some(now) = system_times() else {
        return vec![
            Reading::failed(CPU_USAGE, "ppm", "kernel32", "system_times"),
            logical,
        ];
    };
    let usage = match previous.take() {
        None => {
            *previous = Some(now);
            Reading::baseline(CPU_USAGE, "ppm", "kernel32")
        }
        Some(old) if now.busy < old.busy || now.total < old.total => {
            *previous = Some(now);
            Reading::failed(CPU_USAGE, "ppm", "kernel32", "counter_reset")
        }
        Some(old) => {
            let busy = now.busy - old.busy;
            let total = now.total - old.total;
            *previous = Some(now);
            match ratio_ppm(busy, total) {
                Ok(ppm) => Reading::measured(CPU_USAGE, ppm, "ppm", "kernel32"),
                Err(reason) => Reading::failed(CPU_USAGE, "ppm", "kernel32", reason),
            }
        }
    };
    vec![usage, logical]
}

fn logical_processors() -> Reading {
    let mut info: SYSTEM_INFO = unsafe { mem::zeroed() };
    unsafe { GetSystemInfo(&mut info) };
    let count = info.dwNumberOfProcessors;
    if count == 0 {
        Reading::failed(CPU_LOGICAL, "count", "kernel32", "system_info")
    } else {
        Reading::measured(CPU_LOGICAL, i64::from(count), "count", "kernel32")
    }
}

fn system_times() -> Option<CpuTicks> {
    let mut idle = zero_filetime();
    let mut kernel = zero_filetime();
    let mut user = zero_filetime();
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return None;
    }
    let idle = filetime_u64(idle);
    let kernel = filetime_u64(kernel);
    let user = filetime_u64(user);
    if kernel < idle {
        return None;
    }
    let busy = (kernel - idle).saturating_add(user);
    let total = kernel.saturating_add(user);
    Some(CpuTicks { busy, total })
}

fn memory_readings() -> Vec<Reading> {
    let mut status: MEMORYSTATUSEX = unsafe { mem::zeroed() };
    status.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    if ok == 0 || status.ullTotalPhys == 0 {
        return vec![
            Reading::failed(MEMORY_TOTAL, "bytes", "kernel32", "memory_status"),
            Reading::failed(MEMORY_AVAILABLE, "bytes", "kernel32", "memory_status"),
            Reading::failed(MEMORY_USED, "bytes", "kernel32", "memory_status"),
            Reading::unsupported(MEMORY_SWAP_TOTAL, "bytes", "absent", "no_swap"),
        ];
    }
    let total = i64::try_from(status.ullTotalPhys).ok();
    let available = i64::try_from(status.ullAvailPhys).ok();
    let used = match (total, available) {
        (Some(total), Some(available)) => match memory_used(total as u64, available as u64) {
            Ok(used) => Reading::measured(MEMORY_USED, used, "bytes", "kernel32"),
            Err(reason) => Reading::failed(MEMORY_USED, "bytes", "kernel32", reason),
        },
        _ => Reading::failed(MEMORY_USED, "bytes", "kernel32", "counter_inconsistent"),
    };
    vec![
        match total {
            Some(value) => Reading::measured(MEMORY_TOTAL, value, "bytes", "kernel32"),
            None => Reading::failed(MEMORY_TOTAL, "bytes", "kernel32", "counter_inconsistent"),
        },
        match available {
            Some(value) => Reading::measured(MEMORY_AVAILABLE, value, "bytes", "kernel32"),
            None => Reading::failed(
                MEMORY_AVAILABLE,
                "bytes",
                "kernel32",
                "counter_inconsistent",
            ),
        },
        used,
        Reading::unsupported(MEMORY_SWAP_TOTAL, "bytes", "absent", "no_swap"),
    ]
}

fn frequency_readings() -> Vec<Reading> {
    let mut info: SYSTEM_INFO = unsafe { mem::zeroed() };
    unsafe { GetSystemInfo(&mut info) };
    let count = info.dwNumberOfProcessors as usize;
    if count == 0 || count > 4096 {
        return frequency_absent("system_info");
    }
    let mut buffer = vec![unsafe { mem::zeroed::<PROCESSOR_POWER_INFORMATION>() }; count];
    let bytes = (buffer.len() * size_of::<PROCESSOR_POWER_INFORMATION>()) as u32;
    let status = unsafe {
        CallNtPowerInformation(
            ProcessorInformation,
            std::ptr::null(),
            0,
            buffer.as_mut_ptr().cast(),
            bytes,
        )
    };
    if status < 0 {
        return frequency_absent("api_unavailable");
    }
    let values: Vec<u64> = buffer
        .iter()
        .map(|item| u64::from(item.CurrentMhz).saturating_mul(1000))
        .collect();
    match frequency_extent(&values) {
        Some((min, max)) => vec![
            Reading::measured(CPU_FREQUENCY_MIN, min, "khz", "powrprof"),
            Reading::measured(CPU_FREQUENCY_MAX, max, "khz", "powrprof"),
        ],
        None => frequency_absent("zero_placeholder"),
    }
}

fn frequency_absent(reason: &'static str) -> Vec<Reading> {
    vec![
        Reading::unsupported(CPU_FREQUENCY_MIN, "khz", "absent", reason),
        Reading::unsupported(CPU_FREQUENCY_MAX, "khz", "absent", reason),
    ]
}

fn battery_readings() -> Vec<Reading> {
    let mut status: SYSTEM_POWER_STATUS = unsafe { mem::zeroed() };
    let ok = unsafe { GetSystemPowerStatus(&mut status) };
    if ok == 0 {
        return vec![
            Reading::failed(BATTERY_CAPACITY, "percent", "kernel32", "power_status"),
            Reading::unsupported(BATTERY_POWER, "microwatts", "absent", "reader_absent"),
        ];
    }
    vec![
        battery_capacity(status.BatteryFlag, status.BatteryLifePercent),
        Reading::unsupported(BATTERY_POWER, "microwatts", "absent", "reader_absent"),
    ]
}

fn process_readings(previous: &mut BTreeMap<u32, ProcBase>) -> Processes {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return Processes::failed();
    }
    let _snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    if unsafe { Process32FirstW(_snapshot.as_raw_handle(), &mut entry) } == 0 {
        return Processes::failed();
    }
    let mut rows = Vec::new();
    let mut count = 0u64;
    let mut truncated = false;
    let mut limited = false;
    let mut seen = BTreeMap::new();
    loop {
        let pid = entry.th32ProcessID;
        if pid != 0 {
            count = count.saturating_add(1);
            if rows.len() >= MAX_PROCESSES {
                truncated = true;
            } else {
                let row = read_process(pid, &entry, previous);
                if row.rss_bytes.is_none()
                    || row.cpu_ticks.is_none() && previous_missing(previous, pid)
                {
                    limited = true;
                }
                if let Some(base) = previous.get(&pid) {
                    seen.insert(pid, base.created);
                }
                rows.push(row);
            }
        }
        if unsafe { Process32NextW(_snapshot.as_raw_handle(), &mut entry) } == 0 {
            break;
        }
    }
    previous.retain(|pid, base| seen.get(pid) == Some(&base.created));
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

fn previous_missing(previous: &BTreeMap<u32, ProcBase>, pid: u32) -> bool {
    !previous.contains_key(&pid)
}

fn read_process(
    pid: u32,
    entry: &PROCESSENTRY32W,
    previous: &mut BTreeMap<u32, ProcBase>,
) -> ProcUse {
    let name = clean_name(&wide_slice(&entry.szExeFile));
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return ProcUse {
            pid,
            name,
            rss_bytes: None,
            cpu_ticks: None,
        };
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    let times = process_times(handle.as_raw_handle());
    let rss = working_set(handle.as_raw_handle());
    let cpu_ticks = match times {
        Some((created, ticks)) => {
            let delta = match previous.get(&pid) {
                Some(old) if old.created == created && ticks >= old.ticks => {
                    Some(ticks - old.ticks)
                }
                _ => None,
            };
            previous.insert(pid, ProcBase { created, ticks });
            delta
        }
        None => None,
    };
    ProcUse {
        pid,
        name,
        rss_bytes: rss,
        cpu_ticks,
    }
}

fn process_times(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<(u64, u64)> {
    let mut created = zero_filetime();
    let mut exited = zero_filetime();
    let mut kernel = zero_filetime();
    let mut user = zero_filetime();
    let ok = unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
    if ok == 0 {
        return None;
    }
    let created = filetime_u64(created);
    let ticks = filetime_u64(kernel).saturating_add(filetime_u64(user));
    Some((created, ticks))
}

fn working_set(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<i64> {
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { mem::zeroed() };
    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let ok = unsafe { GetProcessMemoryInfo(handle, &mut counters, counters.cb) };
    if ok == 0 {
        return None;
    }
    i64::try_from(counters.WorkingSetSize).ok()
}

fn wide_slice(value: &[u16]) -> String {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

fn zero_filetime() -> FILETIME {
    FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    }
}

fn filetime_u64(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

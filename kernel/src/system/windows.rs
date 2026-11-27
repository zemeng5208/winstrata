//! Windows 通过 Toolhelp、进程查询和服务控制管理器读取一次采样。
//!
//! 服务关联来自 `EnumServicesStatusExW` 的进程号。采样不创建、不启动、不停止服务，
//! 也不加载驱动。

use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    ERROR_MORE_DATA, ERROR_SUCCESS, FILETIME, GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, PROCESSENTRY32W,
    Process32FirstW, Process32NextW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
    TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, SC_ENUM_PROCESS_INFO, SC_HANDLE,
    SC_MANAGER_ENUMERATE_SERVICE, SERVICE_STATE_ALL, SERVICE_WIN32,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

use super::{
    HostSample, MAX_THREADS, ModuleRow, ProcessRow, ServiceRow, image_text, process_name,
    service_name,
};

const WINDOWS_EPOCH_100NS: u64 = 11_644_473_600_000_0000;
const SERVICE_BUFFER_CAP: usize = 4 * 1024 * 1024;

pub(super) fn sample() -> io::Result<HostSample> {
    let device = registry_sz("SOFTWARE\\Microsoft\\Cryptography\0", "MachineGuid\0")?;
    let boot = registry_dword(
        "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Memory Management\\PrefetchParameters\0",
        "BootId\0",
    )?;
    let processes = read_processes()?;
    let (services, services_unavailable) = read_services();
    Ok(HostSample {
        device,
        boot: format!("boot:{boot}"),
        precision_ns: None,
        processes,
        services,
        services_unavailable,
        evidence: "toolhelp",
    })
}

fn registry_dword(subkey: &str, value: &str) -> io::Result<u32> {
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide_literal(subkey).as_ptr(),
            wide_literal(value).as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut data as *mut u32 as *mut _,
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(data)
}

fn registry_sz(subkey: &str, value: &str) -> io::Result<String> {
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide_literal(subkey).as_ptr(),
            wide_literal(value).as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            null_mut(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if size < 2 || size > 1024 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "machine id has an unexpected size",
        ));
    }
    let mut buffer = vec![0u16; (size as usize) / 2];
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide_literal(subkey).as_ptr(),
            wide_literal(value).as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr() as *mut _,
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let chars = (size as usize / 2).min(buffer.len());
    Ok(wide_slice(&buffer[..chars]))
}

fn wide_literal(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

fn read_processes() -> io::Result<Vec<ProcessRow>> {
    let snap = toolhelp(TH32CS_SNAPPROCESS, 0)?;
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    if unsafe { Process32FirstW(snap.as_raw_handle(), &mut entry) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let threads_by_pid = read_threads();
    let mut processes = Vec::new();
    loop {
        let pid = entry.th32ProcessID;
        if pid != 0 {
            let name = wide_slice(&entry.szExeFile);
            let (threads, threads_truncated) =
                limit_threads(threads_by_pid.get(&pid).cloned().unwrap_or_default());
            let (image, start_unix_ns) = query_process(pid);
            let (modules, module_failed, modules_truncated) = read_modules(pid, image.as_deref());
            let limited =
                image.is_none() || start_unix_ns.is_none() || module_failed || threads.is_empty();
            processes.push(ProcessRow {
                pid,
                parent_pid: entry.th32ParentProcessID,
                start_unix_ns,
                name: process_name(&name, pid),
                image,
                threads,
                modules,
                modules_truncated,
                threads_truncated,
                limited,
            });
        }
        if unsafe { Process32NextW(snap.as_raw_handle(), &mut entry) } == 0 {
            break;
        }
    }
    Ok(processes)
}

fn limit_threads(mut threads: Vec<u32>) -> (Vec<u32>, bool) {
    let truncated = threads.len() > MAX_THREADS;
    threads.truncate(MAX_THREADS);
    (threads, truncated)
}

fn read_threads() -> BTreeMap<u32, Vec<u32>> {
    let Ok(snap) = toolhelp(TH32CS_SNAPTHREAD, 0) else {
        return BTreeMap::new();
    };
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
    if unsafe { Thread32First(snap.as_raw_handle(), &mut entry) } == 0 {
        return BTreeMap::new();
    }
    let mut threads = BTreeMap::<u32, Vec<u32>>::new();
    loop {
        threads
            .entry(entry.th32OwnerProcessID)
            .or_default()
            .push(entry.th32ThreadID);
        if unsafe { Thread32Next(snap.as_raw_handle(), &mut entry) } == 0 {
            break;
        }
    }
    threads
}

fn query_process(pid: u32) -> (Option<String>, Option<i64>) {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return (None, None);
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    let image = query_image(&handle);
    let start = query_start(&handle);
    (image, start)
}

fn query_image(handle: &OwnedHandle) -> Option<String> {
    let mut buffer = vec![0u16; 1024];
    let mut chars = buffer.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(handle.as_raw_handle(), 0, buffer.as_mut_ptr(), &mut chars)
    };
    if ok == 0 {
        return None;
    }
    image_text(&wide_slice(&buffer[..chars as usize]))
}

fn query_start(handle: &OwnedHandle) -> Option<i64> {
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut unused = created;
    let ok = unsafe {
        GetProcessTimes(
            handle.as_raw_handle(),
            &mut created,
            &mut unused,
            &mut unused,
            &mut unused,
        )
    };
    if ok == 0 {
        return None;
    }
    let ticks = ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64;
    filetime_to_unix_ns(ticks)
}

fn filetime_to_unix_ns(ticks: u64) -> Option<i64> {
    let since_unix = ticks.checked_sub(WINDOWS_EPOCH_100NS)?;
    since_unix
        .checked_mul(100)
        .and_then(|ns| i64::try_from(ns).ok())
}

fn read_modules(pid: u32, preferred: Option<&str>) -> (Vec<ModuleRow>, bool, bool) {
    let snap = match toolhelp(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) {
        Ok(snap) => snap,
        Err(_) => match toolhelp(TH32CS_SNAPMODULE, pid) {
            Ok(snap) => snap,
            Err(_) => return (Vec::new(), true, false),
        },
    };
    let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
    if unsafe { Module32FirstW(snap.as_raw_handle(), &mut entry) } == 0 {
        return (Vec::new(), true, false);
    }
    let mut modules = BTreeMap::<String, u64>::new();
    loop {
        let path = wide_slice(&entry.szExePath);
        let name = if path.is_empty() {
            wide_slice(&entry.szModule)
        } else {
            path
        };
        if !name.is_empty() {
            let base = entry.modBaseAddr as u64;
            modules
                .entry(name)
                .and_modify(|current| *current = (*current).min(base))
                .or_insert(base);
        }
        if unsafe { Module32NextW(snap.as_raw_handle(), &mut entry) } == 0 {
            break;
        }
    }
    let (modules, truncated) = super::limit_modules(modules, preferred);
    (modules, false, truncated)
}

fn read_services() -> (Vec<ServiceRow>, bool) {
    let manager = unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_ENUMERATE_SERVICE) };
    if manager.is_null() {
        return (Vec::new(), true);
    }
    let manager = ServiceHandle(manager);
    let mut buffer = vec![0u64; 32 * 1024 / 8];
    let mut needed = 0u32;
    let mut returned = 0u32;
    loop {
        // 缓冲区扩大后从头枚举。沿用上次的 resume 会跳过尚未消费的条目。
        let mut resume = 0u32;
        let ok = unsafe {
            EnumServicesStatusExW(
                manager.0,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                buffer.as_mut_ptr() as *mut u8,
                (buffer.len() * 8) as u32,
                &mut needed,
                &mut returned,
                &mut resume,
                null(),
            )
        };
        if ok != 0 {
            break;
        }
        let error = unsafe { GetLastError() };
        if error != ERROR_MORE_DATA || needed as usize > SERVICE_BUFFER_CAP || needed < 8 {
            return (Vec::new(), true);
        }
        buffer.resize((needed as usize / 8) + 1, 0);
    }
    let mut services = Vec::new();
    let count = returned as usize;
    let entry_size =
        std::mem::size_of::<windows_sys::Win32::System::Services::ENUM_SERVICE_STATUS_PROCESSW>();
    if count > 0 && count.saturating_mul(entry_size) <= buffer.len() * 8 {
        let entries = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr()
                    as *const windows_sys::Win32::System::Services::ENUM_SERVICE_STATUS_PROCESSW,
                count,
            )
        };
        for entry in entries {
            let pid = entry.ServiceStatusProcess.dwProcessId;
            if pid == 0 {
                continue;
            }
            let Some(name) = wide_ptr(entry.lpServiceName).and_then(|name| service_name(&name))
            else {
                continue;
            };
            services.push(ServiceRow { name, pid });
        }
    }
    (services, false)
}

struct ServiceHandle(SC_HANDLE);

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseServiceHandle(self.0) };
        }
    }
}

fn toolhelp(flags: u32, pid: u32) -> io::Result<OwnedHandle> {
    let handle = unsafe { CreateToolhelp32Snapshot(flags, pid) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }
}

fn wide_slice(value: &[u16]) -> String {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

fn wide_ptr(value: *const u16) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *value.add(len) != 0 {
            len += 1;
            if len > 32_768 {
                return None;
            }
        }
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(
            value, len,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_epoch_is_zero_unix_time() {
        assert_eq!(filetime_to_unix_ns(WINDOWS_EPOCH_100NS), Some(0));
        assert_eq!(filetime_to_unix_ns(WINDOWS_EPOCH_100NS + 1), Some(100));
        assert!(filetime_to_unix_ns(WINDOWS_EPOCH_100NS - 1).is_none());
    }
}

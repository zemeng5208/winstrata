//! Windows 通过服务控制管理器读取服务和驱动的当前状态。
//!
//! 采样只打开管理器并查询配置。它不创建、不启动、不停止、不删除服务，也不加载驱动。

use std::io::{self, ErrorKind};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_MORE_DATA, ERROR_SUCCESS, GetLastError,
};
use windows_sys::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW,
    QUERY_SERVICE_CONFIGW, QueryServiceConfigW, SC_ENUM_PROCESS_INFO, SC_HANDLE,
    SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_DRIVER, SERVICE_QUERY_CONFIG,
    SERVICE_STATE_ALL, SERVICE_WIN32,
};

use super::{
    Coverage, Fact, Kind, RunState, ServiceSnapshot, StartType, clean_name, clean_path,
};

const MAX_EACH: usize = 1024;
const BUFFER_CAP: usize = 1024 * 1024;

struct Manager(SC_HANDLE);

impl Drop for Manager {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseServiceHandle(self.0) };
        }
    }
}

struct Service(SC_HANDLE);

impl Drop for Service {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseServiceHandle(self.0) };
        }
    }
}

pub(super) fn capture() -> io::Result<ServiceSnapshot> {
    let device = registry_sz(
        "SOFTWARE\\Microsoft\\Cryptography\0",
        "MachineGuid\0",
    )?;
    let boot = registry_dword(
        "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Memory Management\\PrefetchParameters\0",
        "BootId\0",
    )?;
    let mut snapshot = ServiceSnapshot {
        device,
        boot: format!("boot:{boot}"),
        sampled_unix_ns: super::unix_now()?,
        facts: Vec::new(),
        coverage: Coverage::Complete,
        truncated: false,
        permission_denied: false,
        services_unreadable: false,
        driver_permission_denied: false,
        source_failure: None,
        evidence: "scm",
    };
    let handle = unsafe {
        OpenSCManagerW(
            null(),
            null(),
            SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE,
        )
    };
    if handle.is_null() {
        let error = unsafe { GetLastError() };
        if error == ERROR_ACCESS_DENIED {
            snapshot.permission_denied = true;
        } else {
            snapshot.source_failure = Some(format!("scm:{error}"));
        }
        return Ok(snapshot);
    }
    let manager = Manager(handle);
    match read_kind(manager.0, SERVICE_WIN32, Kind::Service) {
        Ok(facts) => push_all(&mut snapshot, facts),
        Err(EnumError::Buffer) => snapshot.source_failure = Some("scm:buffer".to_owned()),
        Err(EnumError::Code(code)) => snapshot.source_failure = Some(format!("scm:{code}")),
    }
    if snapshot.source_failure.is_none() {
        match read_kind(manager.0, SERVICE_DRIVER, Kind::Driver) {
            Ok(facts) => push_all(&mut snapshot, facts),
            Err(EnumError::Buffer) => snapshot.source_failure = Some("scm:buffer".to_owned()),
            Err(EnumError::Code(code)) => snapshot.source_failure = Some(format!("scm:{code}")),
        }
    }
    if snapshot.source_failure.is_some() {
        snapshot.facts.clear();
    }
    Ok(snapshot)
}

enum EnumError {
    Code(u32),
    Buffer,
}

fn push_all(snapshot: &mut ServiceSnapshot, facts: Vec<Fact>) {
    for fact in facts {
        let count = snapshot
            .facts
            .iter()
            .filter(|item| item.kind == fact.kind)
            .count();
        if count >= MAX_EACH {
            snapshot.truncated = true;
            continue;
        }
        snapshot.facts.push(fact);
    }
}

fn read_kind(manager: SC_HANDLE, service_type: u32, kind: Kind) -> Result<Vec<Fact>, EnumError> {
    let mut buffer = vec![0u64; 64 * 1024 / 8];
    let entries = loop {
        let mut needed = 0u32;
        let mut returned = 0u32;
        let mut resume = 0u32;
        let ok = unsafe {
            EnumServicesStatusExW(
                manager,
                SC_ENUM_PROCESS_INFO,
                service_type,
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
            break parse_entries(&buffer, returned)?;
        }
        let error = unsafe { GetLastError() };
        if error != ERROR_MORE_DATA || needed < 8 || needed as usize > BUFFER_CAP {
            return Err(if error == ERROR_MORE_DATA {
                EnumError::Buffer
            } else {
                EnumError::Code(error)
            });
        }
        buffer.resize(needed as usize / 8 + 1, 0);
    };
    let mut facts = Vec::with_capacity(entries.len());
    for entry in entries {
        facts.push(with_config(manager, kind, entry));
    }
    Ok(facts)
}

struct RawEntry {
    name: String,
    state: u32,
    pid: u32,
    win32_exit: u32,
    service_exit: u32,
}

fn parse_entries(buffer: &[u64], returned: u32) -> Result<Vec<RawEntry>, EnumError> {
    let count = returned as usize;
    let entry_size = std::mem::size_of::<
        windows_sys::Win32::System::Services::ENUM_SERVICE_STATUS_PROCESSW,
    >();
    if count > 0 && count.saturating_mul(entry_size) > buffer.len() * 8 {
        return Err(EnumError::Buffer);
    }
    let mut entries = Vec::with_capacity(count);
    if count == 0 {
        return Ok(entries);
    }
    let raw = unsafe {
        std::slice::from_raw_parts(
            buffer.as_ptr() as *const windows_sys::Win32::System::Services::ENUM_SERVICE_STATUS_PROCESSW,
            count,
        )
    };
    for entry in raw {
        let Some(name) = wide_ptr(entry.lpServiceName).and_then(|name| clean_name(&name)) else {
            continue;
        };
        let status = entry.ServiceStatusProcess;
        entries.push(RawEntry {
            name,
            state: status.dwCurrentState,
            pid: status.dwProcessId,
            win32_exit: status.dwWin32ExitCode,
            service_exit: status.dwServiceSpecificExitCode,
        });
    }
    Ok(entries)
}

fn with_config(manager: SC_HANDLE, kind: Kind, entry: RawEntry) -> Fact {
    let mut fact = Fact {
        name: entry.name,
        kind,
        state: run_state(entry.state),
        start_type: StartType::Unknown,
        binary: None,
        pid: (entry.pid != 0).then_some(entry.pid),
        win32_exit: entry.win32_exit,
        service_exit: entry.service_exit,
        config_error: None,
    };
    let wide_name = wide(&fact.name);
    let opened = unsafe { OpenServiceW(manager, wide_name.as_ptr(), SERVICE_QUERY_CONFIG) };
    if opened.is_null() {
        fact.config_error = Some(unsafe { GetLastError() } as i32);
        return fact;
    }
    let service = Service(opened);
    match query_config(service.0) {
        Ok((start, binary)) => {
            fact.start_type = start;
            fact.binary = binary;
        }
        Err(code) => fact.config_error = Some(code),
    }
    fact
}

fn query_config(service: SC_HANDLE) -> Result<(StartType, Option<String>), i32> {
    let mut needed = 0u32;
    unsafe { QueryServiceConfigW(service, null_mut(), 0, &mut needed) };
    if needed == 0 || needed as usize > BUFFER_CAP {
        return Err(unsafe { GetLastError() } as i32);
    }
    let mut buffer = vec![0u8; needed as usize];
    let ok = unsafe {
        QueryServiceConfigW(
            service,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return Err(unsafe { GetLastError() } as i32);
    }
    let configured = unsafe { &*buffer.as_ptr().cast::<QUERY_SERVICE_CONFIGW>() };
    let path = wide_ptr(configured.lpBinaryPathName).and_then(|text| clean_path(&text));
    Ok((start_type(configured.dwStartType), path))
}

fn run_state(value: u32) -> RunState {
    match value {
        1 => RunState::Stopped,
        2 => RunState::StartPending,
        3 => RunState::StopPending,
        4 => RunState::Running,
        5 => RunState::ContinuePending,
        6 => RunState::PausePending,
        7 => RunState::Paused,
        _ => RunState::Unknown,
    }
}

fn start_type(value: u32) -> StartType {
    match value {
        0 => StartType::Boot,
        1 => StartType::System,
        2 => StartType::Auto,
        3 => StartType::Demand,
        4 => StartType::Disabled,
        _ => StartType::Unknown,
    }
}

fn registry_dword(subkey: &str, value: &str) -> io::Result<u32> {
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(subkey).as_ptr(),
            wide(value).as_ptr(),
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
            wide(subkey).as_ptr(),
            wide(value).as_ptr(),
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
    let mut buffer = vec![0u16; size as usize / 2];
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(subkey).as_ptr(),
            wide(value).as_ptr(),
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
    wide_slice(&buffer[..chars])
        .filter(|text| !text.is_empty())
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "machine id is empty"))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

fn wide_ptr(value: *const u16) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *value.add(len) != 0 {
            len += 1;
            if len > 1024 {
                return None;
            }
        }
        wide_slice(std::slice::from_raw_parts(value, len))
    }
}

fn wide_slice(value: &[u16]) -> Option<String> {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    Some(String::from_utf16_lossy(&value[..end])).filter(|text| !text.is_empty())
}

//! Windows Service Control Manager checks for plan 022.
//! This file is empty on Linux. It does not mark those checks passed there.

#![cfg(windows)]

use std::ffi::c_void;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::{Mutex, MutexGuard};
use std::thread;

use winstrata_protocols::Record;
use winstrata_strata_services::{
    Kind, RunState, ServiceSnapshot, ServiceTracker, StartType, capture,
};

use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_EXISTS,
    ERROR_SERVICE_MARKED_FOR_DELETE, GetLastError, LocalFree,
};
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Security::{
    CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, ImpersonateLoggedOnUser, RevertToSelf,
    SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows_sys::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, CreateServiceW, DeleteService, OpenSCManagerW,
    OpenServiceW, QueryServiceConfigW, SC_MANAGER_CONNECT, SC_MANAGER_CREATE_SERVICE,
    SERVICE_AUTO_START, SERVICE_CHANGE_CONFIG, SERVICE_DEMAND_START, SERVICE_ERROR_IGNORE,
    SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_START,
    SERVICE_WIN32_OWN_PROCESS, StartServiceW,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const SERVICE_DELETE: u32 = 0x0001_0000;
const PROBE: &str = "WinStrata022Probe";
const IMAGE: &str = "C:\\Windows\\Temp\\WinStrata022-missing.exe";

struct Service(windows_sys::Win32::System::Services::SC_HANDLE);
impl Drop for Service {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseServiceHandle(self.0) };
        }
    }
}

fn scm_tests_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn service_control_manager_observes_a_driver_and_denies_a_restricted_token() {
    let _lock = scm_tests_lock();
    let live = capture().expect("unrestricted SCM capture");
    assert_eq!(live.evidence, "scm");
    assert!(
        live.facts.iter().any(|fact| fact.kind == Kind::Driver),
        "SCM reported no driver"
    );
    assert!(!live.permission_denied);

    let sid = restricting_sid();
    let wide_sid = wide(&sid);
    let mut raw_sid = null_mut();
    unsafe { check(ConvertStringSidToSidW(wide_sid.as_ptr(), &mut raw_sid)).unwrap() };
    let _sid = LocalBlock(raw_sid);
    let mut parent = null_mut();
    unsafe {
        check(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut parent,
        ))
        .unwrap();
    }
    let parent = unsafe { OwnedHandle::from_raw_handle(parent) };
    let limiting = SID_AND_ATTRIBUTES {
        Sid: raw_sid,
        Attributes: 0,
    };
    let mut restricted = null_mut();
    unsafe {
        check(CreateRestrictedToken(
            parent.as_raw_handle(),
            DISABLE_MAX_PRIVILEGE,
            0,
            null(),
            0,
            null(),
            1,
            &limiting,
            &mut restricted,
        ))
        .unwrap();
    }
    let token = unsafe { OwnedHandle::from_raw_handle(restricted) };
    let denied = thread::spawn(move || {
        unsafe { check(ImpersonateLoggedOnUser(token.as_raw_handle())).unwrap() };
        let result = capture();
        unsafe { check(RevertToSelf()).unwrap() };
        result
    })
    .join()
    .unwrap()
    .expect("restricted capture");
    assert!(denied.permission_denied);
    assert!(denied.facts.is_empty());
    let again = capture().expect("capture after the restricted token");
    assert!(!again.permission_denied);
    assert!(again.facts.iter().any(|fact| fact.kind == Kind::Driver));
}

#[test]
fn missing_image_probe_records_creation_start_type_and_deletion() {
    let _lock = scm_tests_lock();
    assert!(!Path::new(IMAGE).exists(), "probe image must not exist");
    let manager = open_manager(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE);
    let name = wide(PROBE);
    let _guard = ProbeGuard {
        manager: manager.0,
        name: name.clone(),
    };
    let mut tracker = ServiceTracker::new();
    let before = capture().expect("baseline");
    let _ = tracker.observe(&before).expect("baseline events");
    assert!(
        before.facts.iter().all(|fact| fact.name.as_str() != PROBE),
        "{PROBE} already exists"
    );

    let service = create_probe(manager.0, &name);
    let created = capture().expect("capture after create");
    let events = tracker.observe(&created).expect("create events");
    assert_eq!(named(&events, "service.created", PROBE), 1);
    assert_eq!(named(&events, "service.started", PROBE), 0);
    let probe = created
        .facts
        .iter()
        .find(|fact| fact.name.as_str() == PROBE)
        .expect("created probe is missing from the inventory");
    if probe.state == RunState::Stopped {
        assert_eq!(named(&events, "service.stopped", PROBE), 0);
    }
    assert_ne!(
        probe.start_type,
        StartType::Auto,
        "created probe start type is already auto; config_error={:?}",
        probe.config_error
    );

    let changed = unsafe {
        ChangeServiceConfigW(
            service.0,
            SERVICE_NO_CHANGE,
            SERVICE_AUTO_START,
            SERVICE_NO_CHANGE,
            null(),
            null(),
            null_mut(),
            null(),
            null(),
            null(),
            null(),
        )
    };
    assert_ne!(changed, 0, "ChangeServiceConfigW failed: {}", unsafe {
        GetLastError()
    });
    let configured = capture_until_probe_start(StartType::Auto);
    let events = tracker.observe(&configured).expect("start-type events");
    assert_eq!(named(&events, "service.start-type", PROBE), 1);

    let started = unsafe { StartServiceW(service.0, 0, null()) };
    let start_error = unsafe { GetLastError() };
    assert_eq!(started, 0, "a missing image must not start");
    assert!(
        start_error == ERROR_FILE_NOT_FOUND || start_error == ERROR_PATH_NOT_FOUND,
        "missing image returned Win32 error {start_error}"
    );
    let after_start = capture().expect("capture after the failed start");
    let events = tracker.observe(&after_start).expect("failure events");
    let probe = after_start
        .facts
        .iter()
        .find(|fact| fact.name.as_str() == PROBE)
        .expect("probe disappeared after the failed start");
    if probe.win32_exit != 0 || probe.service_exit != 0 {
        assert_eq!(named(&events, "service.failure", PROBE), 1);
    }
    if probe.state == RunState::Stopped {
        assert_eq!(named(&events, "service.stopped", PROBE), 0);
    }

    let deleted = unsafe { DeleteService(service.0) };
    assert_ne!(deleted, 0, "DeleteService failed: {}", unsafe {
        GetLastError()
    });
    drop(service);
    assert_probe_removed(manager.0, &name);
    let gone = capture().expect("capture after deletion");
    let events = tracker.observe(&gone).expect("delete events");
    assert_eq!(named(&events, "service.deleted", PROBE), 1);
    if probe.state == RunState::Stopped {
        assert_eq!(named(&events, "service.stopped", PROBE), 0);
    }
}

struct ProbeGuard {
    manager: windows_sys::Win32::System::Services::SC_HANDLE,
    name: Vec<u16>,
}

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        let existing = unsafe { OpenServiceW(self.manager, self.name.as_ptr(), SERVICE_DELETE) };
        if existing.is_null() {
            return;
        }
        unsafe { DeleteService(existing) };
        unsafe { CloseServiceHandle(existing) };
    }
}

fn create_probe(manager: windows_sys::Win32::System::Services::SC_HANDLE, name: &[u16]) -> Service {
    let path = wide(IMAGE);
    let access = SERVICE_QUERY_CONFIG
        | SERVICE_CHANGE_CONFIG
        | SERVICE_START
        | SERVICE_QUERY_STATUS
        | SERVICE_DELETE;
    let created = unsafe {
        CreateServiceW(
            manager,
            name.as_ptr(),
            name.as_ptr(),
            access,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_IGNORE,
            path.as_ptr(),
            null(),
            null_mut(),
            null(),
            null(),
            null(),
        )
    };
    if !created.is_null() {
        return Service(created);
    }
    let error = unsafe { GetLastError() };
    assert_eq!(
        error, ERROR_SERVICE_EXISTS,
        "CreateServiceW failed: {error}"
    );
    let existing = unsafe {
        OpenServiceW(
            manager,
            name.as_ptr(),
            SERVICE_QUERY_CONFIG | SERVICE_DELETE,
        )
    };
    assert!(!existing.is_null(), "probe exists but cannot be opened");
    let configured = service_binary_path(existing);
    if configured.eq_ignore_ascii_case(IMAGE) {
        let deleted = unsafe { DeleteService(existing) };
        unsafe { CloseServiceHandle(existing) };
        assert_ne!(deleted, 0, "could not remove the previous probe");
    } else {
        unsafe { CloseServiceHandle(existing) };
        panic!("{PROBE} exists with a different image: {configured}");
    }
    let created = unsafe {
        CreateServiceW(
            manager,
            name.as_ptr(),
            name.as_ptr(),
            access,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_IGNORE,
            path.as_ptr(),
            null(),
            null_mut(),
            null(),
            null(),
            null(),
        )
    };
    assert!(!created.is_null(), "CreateServiceW failed after cleanup");
    Service(created)
}

fn capture_until_probe_start(expected: StartType) -> ServiceSnapshot {
    let mut last = String::new();
    for attempt in 0..40 {
        let snapshot = capture().expect("capture while waiting for the probe start type");
        if snapshot.permission_denied {
            last = "permission_denied".to_owned();
        } else if let Some(reason) = &snapshot.source_failure {
            last = format!("source_failure={reason}");
        } else if let Some(fact) = snapshot
            .facts
            .iter()
            .find(|fact| fact.name.as_str() == PROBE)
        {
            if fact.start_type == expected && fact.config_error.is_none() {
                return snapshot;
            }
            last = format!(
                "start={:?} config_error={:?}",
                fact.start_type, fact.config_error
            );
        } else {
            last = format!(
                "probe missing; facts={} truncated={}",
                snapshot.facts.len(),
                snapshot.truncated
            );
        }
        if attempt == 39 {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{PROBE} did not report {expected:?} within 2s; last sample: {last}");
}

fn service_binary_path(service: windows_sys::Win32::System::Services::SC_HANDLE) -> String {
    let mut needed = 0u32;
    unsafe { QueryServiceConfigW(service, null_mut(), 0, &mut needed) };
    assert!(needed > 0, "QueryServiceConfigW returned no size");
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    let ok = unsafe {
        QueryServiceConfigW(
            service,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * 8) as u32,
            &mut needed,
        )
    };
    assert_ne!(ok, 0, "QueryServiceConfigW failed: {}", unsafe {
        GetLastError()
    });
    let config = unsafe {
        &*buffer
            .as_ptr()
            .cast::<windows_sys::Win32::System::Services::QUERY_SERVICE_CONFIGW>()
    };
    wide_string(config.lpBinaryPathName)
}

fn assert_probe_removed(manager: windows_sys::Win32::System::Services::SC_HANDLE, name: &[u16]) {
    let mut gone_error = 0;
    for attempt in 0..40 {
        let gone = unsafe { OpenServiceW(manager, name.as_ptr(), SERVICE_START) };
        if gone.is_null() {
            gone_error = unsafe { GetLastError() };
            if gone_error == ERROR_SERVICE_DOES_NOT_EXIST {
                break;
            }
            if gone_error != ERROR_SERVICE_MARKED_FOR_DELETE {
                panic!("OpenServiceW failed with Win32 error {gone_error}");
            }
        } else {
            unsafe { CloseServiceHandle(gone) };
        }
        if attempt == 39 {
            panic!("probe service still exists");
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(gone_error, ERROR_SERVICE_DOES_NOT_EXIST);
}

fn named(events: &[winstrata_protocols::Event], kind: &str, name: &str) -> usize {
    events
        .iter()
        .filter(|event| match &event.record {
            Record::Observation {
                event_type,
                subject: Some(subject),
                ..
            } => event_type.as_str() == kind && subject.id.as_str() == name,
            _ => false,
        })
        .count()
}

fn open_manager(access: u32) -> Service {
    let handle = unsafe { OpenSCManagerW(null(), null(), access) };
    assert!(!handle.is_null(), "OpenSCManagerW failed: {}", unsafe {
        GetLastError()
    });
    Service(handle)
}

fn restricting_sid() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "S-1-9-{}-{}-{}",
        std::process::id(),
        nanos as u32,
        (nanos >> 32) as u32
    )
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn wide_string(value: *const u16) -> String {
    if value.is_null() {
        return String::new();
    }
    let mut length = 0;
    unsafe {
        while *value.add(length) != 0 {
            length += 1;
        }
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(value, length) })
}

fn check(result: i32) -> Result<(), u32> {
    if result == 0 {
        Err(unsafe { GetLastError() })
    } else {
        Ok(())
    }
}

struct LocalBlock(*mut c_void);
impl Drop for LocalBlock {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { LocalFree(self.0) };
        }
    }
}

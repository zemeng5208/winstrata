use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::thread;

use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SERVICE_DOES_NOT_EXIST,
    ERROR_SERVICE_EXISTS, ERROR_SERVICE_MARKED_FOR_DELETE, GetLastError, LocalFree,
};
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Security::{
    CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, ImpersonateLoggedOnUser, RevertToSelf,
    SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, CreateServiceW, DeleteService, EnumServicesStatusExW, OpenSCManagerW,
    OpenServiceW, QUERY_SERVICE_CONFIGW, QueryServiceConfigW, SC_ENUM_PROCESS_INFO,
    SC_MANAGER_CONNECT, SC_MANAGER_CREATE_SERVICE, SC_MANAGER_ENUMERATE_SERVICE,
    SERVICE_DEMAND_START, SERVICE_DRIVER, SERVICE_ERROR_IGNORE, SERVICE_KERNEL_DRIVER,
    SERVICE_QUERY_CONFIG, SERVICE_START, SERVICE_STATE_ALL, StartServiceW,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const SERVICE_DELETE: u32 = 0x0001_0000;
const PROBE_NAME: &str = "WinStrata008MissingImage";
const PROBE_IMAGE: &str = r"C:\Windows\Temp\WinStrata008-missing.sys";

fn check(ok: i32) -> io::Result<()> {
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
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

struct Manager(windows_sys::Win32::System::Services::SC_HANDLE);

impl Drop for Manager {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseServiceHandle(self.0) };
        }
    }
}

fn open_manager(access: u32) -> io::Result<Manager> {
    let handle = unsafe { OpenSCManagerW(null(), null(), access) };
    if handle.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(Manager(handle))
    }
}

fn create_missing_image_probe(
    manager: windows_sys::Win32::System::Services::SC_HANDLE,
    name: &[u16],
) -> windows_sys::Win32::System::Services::SC_HANDLE {
    let path = wide(PROBE_IMAGE);
    let created = unsafe { create_probe(manager, name, &path) };
    if !created.is_null() {
        return created;
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
    assert!(
        !existing.is_null(),
        "probe name exists but cannot be opened: {}",
        unsafe { GetLastError() }
    );
    let configured = service_binary_path(existing);
    let matches = is_probe_path(&configured);
    if matches {
        let deleted = unsafe { DeleteService(existing) };
        unsafe { CloseServiceHandle(existing) };
        assert_ne!(deleted, 0, "could not remove the previous probe service");
    } else {
        unsafe { CloseServiceHandle(existing) };
        panic!("service {PROBE_NAME} exists with a different image: {configured}");
    }
    let created = unsafe { create_probe(manager, name, &path) };
    assert!(
        !created.is_null(),
        "CreateServiceW failed after removing the previous probe: {}",
        unsafe { GetLastError() }
    );
    created
}

unsafe fn create_probe(
    manager: windows_sys::Win32::System::Services::SC_HANDLE,
    name: &[u16],
    path: &[u16],
) -> windows_sys::Win32::System::Services::SC_HANDLE {
    unsafe {
        CreateServiceW(
            manager,
            name.as_ptr(),
            name.as_ptr(),
            SERVICE_START | SERVICE_DELETE,
            SERVICE_KERNEL_DRIVER,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_IGNORE,
            path.as_ptr(),
            null(),
            null_mut(),
            null(),
            null(),
            null(),
        )
    }
}

fn service_binary_path(service: windows_sys::Win32::System::Services::SC_HANDLE) -> String {
    let mut needed = 0u32;
    unsafe { QueryServiceConfigW(service, null_mut(), 0, &mut needed) };
    assert!(needed > 0, "QueryServiceConfigW returned no size");
    let mut buffer = vec![0u8; needed as usize];
    let ok = unsafe {
        QueryServiceConfigW(
            service,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut needed,
        )
    };
    assert_ne!(ok, 0, "QueryServiceConfigW failed: {}", unsafe {
        GetLastError()
    });
    let configured = unsafe { &*buffer.as_ptr().cast::<QUERY_SERVICE_CONFIGW>() };
    wide_text(configured.lpBinaryPathName)
}

fn is_probe_path(path: &str) -> bool {
    path.eq_ignore_ascii_case(PROBE_IMAGE)
        || path.eq_ignore_ascii_case(r"\??\C:\Windows\Temp\WinStrata008-missing.sys")
}

fn wide_text(value: *const u16) -> String {
    assert!(!value.is_null(), "service configuration string is null");
    let mut len = 0usize;
    unsafe {
        while *value.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(value, len))
    }
}

#[test]
fn service_control_manager_lists_existing_driver_services() {
    let manager = open_manager(SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE)
        .expect("opening SCM for enumeration");
    let mut needed = 0u32;
    let mut returned = 0u32;
    let mut resume = 0u32;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let ok = unsafe {
            EnumServicesStatusExW(
                manager.0,
                SC_ENUM_PROCESS_INFO,
                SERVICE_DRIVER,
                SERVICE_STATE_ALL,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
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
        if error != windows_sys::Win32::Foundation::ERROR_MORE_DATA {
            panic!("EnumServicesStatusExW failed: {error}");
        }
        if needed as usize > 1024 * 1024 {
            panic!("driver service list exceeds 1 MiB");
        }
        buffer.resize(needed as usize, 0);
    }
    assert!(returned > 0, "Windows reported no driver services");
}

#[test]
fn service_control_manager_refuses_to_start_a_missing_kernel_image_and_the_entry_is_removed() {
    let image = Path::new(PROBE_IMAGE);
    assert!(!image.exists(), "probe image must not exist");
    let manager = open_manager(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE)
        .expect("opening SCM to create a probe service");
    let name = wide(PROBE_NAME);
    let service = create_missing_image_probe(manager.0, &name);
    let started = unsafe { StartServiceW(service, 0, null()) };
    let start_error = unsafe { GetLastError() };
    let deleted = unsafe { DeleteService(service) };
    unsafe { CloseServiceHandle(service) };
    assert_eq!(started, 0, "a missing kernel image must not start");
    assert!(
        start_error == ERROR_FILE_NOT_FOUND || start_error == ERROR_PATH_NOT_FOUND,
        "missing image returned Win32 error {start_error}"
    );
    assert_ne!(deleted, 0, "DeleteService failed: {}", unsafe {
        GetLastError()
    });
    // 并发的驱动枚举会占着服务控制管理器。DeleteService 成功后，条目要等那些调用结束才消失。
    assert_probe_removed(manager.0, &name);
}

fn assert_probe_removed(manager: windows_sys::Win32::System::Services::SC_HANDLE, name: &[u16]) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let gone = unsafe { OpenServiceW(manager, name.as_ptr(), SERVICE_START) };
        if gone.is_null() {
            let gone_error = unsafe { GetLastError() };
            if gone_error == ERROR_SERVICE_DOES_NOT_EXIST {
                return;
            }
            if gone_error != ERROR_SERVICE_MARKED_FOR_DELETE {
                panic!("OpenServiceW failed with Win32 error {gone_error}");
            }
        } else {
            unsafe { CloseServiceHandle(gone) };
        }
        if std::time::Instant::now() >= deadline {
            panic!("probe service still exists");
        }
        thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[test]
fn restricted_token_cannot_create_a_driver_service() {
    let unrestricted = open_manager(SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE)
        .expect("the test process cannot open SCM; restricted-token denial was not compared");
    drop(unrestricted);
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
    let error = thread::spawn(move || {
        unsafe { check(ImpersonateLoggedOnUser(token.as_raw_handle())).unwrap() };
        let handle = unsafe {
            OpenSCManagerW(
                null(),
                null(),
                SC_MANAGER_CONNECT | SC_MANAGER_CREATE_SERVICE,
            )
        };
        let error = unsafe { GetLastError() };
        unsafe { check(RevertToSelf()).unwrap() };
        if !handle.is_null() {
            unsafe { CloseServiceHandle(handle) };
            None
        } else {
            Some(error)
        }
    })
    .join()
    .unwrap();
    assert_eq!(error, Some(ERROR_ACCESS_DENIED));
}

fn restricting_sid() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "S-1-9-{}-{}-{}",
        std::process::id(),
        (nanos as u32),
        ((nanos >> 32) as u32)
    )
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

use std::io::Write;
use std::time::Instant;

use winstrata_kernel::{CatalogHost, Driver, DriverHost, HostError};
use winstrata_protocols::ProtocolError;

use crate::protocol::{self, AUDIT_CAP, MAX_FRAME, write_frame};
use crate::session::{self, CoreClient, GuiClient, Service, Session};
use crate::transport::{self, Endpoint, Privilege};

fn code(error: &ProtocolError) -> &str {
    error.code.as_str()
}

fn value(response: &str) -> serde_json::Value {
    serde_json::from_str(response).unwrap()
}

fn send(endpoint: &Endpoint, payload: &str) -> String {
    let mut connection = transport::connect(endpoint).unwrap();
    write_frame(&mut connection, payload.as_bytes()).unwrap();
    let bytes = protocol::read_frame(&mut connection).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn request(user: &str, role: &str, operation: &str, driver: &str) -> String {
    format!(
        r#"{{"version":{{"major":1,"minor":0}},"id":"9","role":"{role}","user":"{user}","operation":"{operation}","driver":{driver}}}"#
    )
}

#[test]
fn gui_client_reaches_an_existing_endpoint_without_replacing_it() {
    let service = Service::listen_catalog(Endpoint::unique().unwrap()).unwrap();
    let client = Endpoint::client(service.address()).unwrap();
    let status = GuiClient::connect(&client).unwrap().status().unwrap();
    assert_eq!(status.drivers.len(), 4);
    assert!(Endpoint::client("").is_err());
}

#[test]
fn live_socket_rejects_unprivileged_roles_and_does_not_call_the_driver_host() {
    let service = Service::listen_catalog(Endpoint::unique().unwrap()).unwrap();
    let endpoint = service.client_endpoint();
    let user = transport::current_user().unwrap();
    let privilege = transport::current_privilege().unwrap();

    let mut gui = GuiClient::connect(&endpoint).unwrap();
    let status = gui.status().unwrap();
    assert!(status.drivers.iter().all(|driver| {
        driver.phase == "idle" && driver.subscriptions == 0 && driver.failure.is_none()
    }));
    drop(gui);

    for role in ["gui", "plugin", "script"] {
        let response = value(&send(
            &endpoint,
            &request(&user, role, "subscribe", r#""strata-fs""#),
        ));
        assert_eq!(response["error"]["code"], "permission_denied");
    }
    let mut core = CoreClient::connect(&endpoint).unwrap();
    let denied = core.subscribe(Driver::Fs).unwrap_err();
    let after = core.status().unwrap();
    match privilege {
        Privilege::NotWindows | Privilege::WindowsOrdinary => {
            assert_eq!(code(&denied), "privilege_unavailable");
            assert!(after.drivers.iter().all(|driver| driver.phase == "idle"));
        }
        Privilege::WindowsElevated => {
            assert_eq!(code(&denied), "image_absent");
            assert_eq!(
                after
                    .drivers
                    .iter()
                    .find(|driver| driver.driver == "strata-fs")
                    .unwrap()
                    .phase,
                "failed"
            );
            assert_eq!(
                after
                    .drivers
                    .iter()
                    .find(|driver| driver.driver == "strata-reg")
                    .unwrap()
                    .phase,
                "idle"
            );
        }
    }
    assert_eq!(after.faults, 0);
}

#[test]
fn live_socket_checks_identity_version_shape_and_audit_loss() {
    let service = Service::listen_catalog(Endpoint::unique().unwrap()).unwrap();
    let endpoint = service.client_endpoint();
    let user = transport::current_user().unwrap();
    let other = if user == "uid:0" { "uid:1" } else { "uid:0" };

    let mismatch = value(&send(&endpoint, &request(other, "core", "status", "null")));
    assert_eq!(mismatch["error"]["code"], "identity_mismatch");

    let version = value(&send(
        &endpoint,
        &format!(
            r#"{{"version":{{"major":2,"minor":0}},"id":"9","role":"core","user":"{user}","operation":"status","driver":null}}"#
        ),
    ));
    assert_eq!(version["error"]["code"], "unsupported_version");

    let duplicate = value(&send(
        &endpoint,
        &format!(
            r#"{{"version":{{"major":1,"minor":0}},"id":"9","role":"core","user":"{user}","operation":"status","driver":null,"driver":null}}"#
        ),
    ));
    assert_eq!(duplicate["error"]["code"], "invalid_record");

    let unknown = value(&send(
        &endpoint,
        &request(&user, "core", "subscribe", r#""strata-other""#),
    ));
    assert_eq!(unknown["error"]["code"], "invalid_record");

    let extra = value(&send(
        &endpoint,
        &format!(
            r#"{{"version":{{"major":1,"minor":0}},"id":"9","role":"core","user":"{user}","operation":"status","driver":null,"extra":1}}"#
        ),
    ));
    assert_eq!(extra["error"]["code"], "invalid_record");

    let mut raw = transport::connect(&endpoint).unwrap();
    write_frame(&mut raw, b"{{{{").unwrap();
    let malformed = String::from_utf8(protocol::read_frame(&mut raw).unwrap()).unwrap();
    assert_eq!(value(&malformed)["error"]["code"], "invalid_json");
    drop(raw);

    let mut oversized = transport::connect(&endpoint).unwrap();
    oversized
        .write_all(&(MAX_FRAME as u32 + 1).to_le_bytes())
        .unwrap();
    assert!(protocol::read_frame(&mut oversized).is_err());
}

#[test]
fn audit_loss_is_visible_and_status_round_trips_stay_bounded() {
    let service = Service::listen_catalog(Endpoint::unique().unwrap()).unwrap();
    let mut core = CoreClient::connect(&service.client_endpoint()).unwrap();
    let started = Instant::now();
    for _ in 0..AUDIT_CAP {
        core.status().unwrap();
    }
    assert!(started.elapsed().as_secs() < 5);
    let page = core.audit().unwrap();
    assert_eq!(page.audit_dropped, 1);
    assert_eq!(page.records.len(), AUDIT_CAP);
    assert!(page.records.iter().all(|record| record.id != "1"));
    assert_eq!(
        page.records.last().unwrap().operation.as_deref(),
        Some("audit")
    );
    assert_eq!(page.records.last().unwrap().code, "allowed");
}

#[cfg(target_os = "linux")]
#[test]
fn private_socket_is_removed_with_its_directory() {
    use std::os::unix::fs::PermissionsExt;
    let service = Service::listen_catalog(Endpoint::unique().unwrap()).unwrap();
    let path = std::path::Path::new(service.address()).to_owned();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    let directory = path.parent().unwrap();
    let directory_mode = std::fs::metadata(directory).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "socket mode {mode:o}");
    assert_eq!(
        directory_mode & 0o077,
        0,
        "directory mode {directory_mode:o}"
    );
    let directory = directory.to_owned();
    drop(service);
    assert!(!directory.exists());
}

#[test]
fn elevated_catalog_decision_reports_image_absent_per_module() {
    // 这里的 WindowsElevated 是决策参数，不是本进程的令牌，也不调用 SCM。
    let owner = "uid:7";
    let mut session = Session::new(Privilege::WindowsElevated, owner, CatalogHost);
    let denied = dispatch(&mut session, owner, &subscribe(owner, "strata-fs"));
    assert_eq!(value(&denied)["error"]["code"], "image_absent");
    let other = dispatch(&mut session, owner, &subscribe(owner, "strata-reg"));
    assert_eq!(value(&other)["error"]["code"], "image_absent");
    let status = dispatch(&mut session, owner, &status_request(owner));
    let drivers = status_drivers(&status);
    assert_eq!(drivers["strata-fs"]["phase"].as_str(), Some("failed"));
    assert_eq!(drivers["strata-reg"]["phase"].as_str(), Some("failed"));
    assert_eq!(drivers["strata-proc"]["phase"].as_str(), Some("idle"));
    assert_eq!(drivers["strata-fs"]["subscriptions"].as_u64(), Some(0));
}

#[test]
fn elevated_decision_contains_one_driver_fault_and_a_host_panic() {
    let owner = "uid:7";
    let mut session = Session::new(Privilege::WindowsElevated, owner, SelectiveHost::default());
    let failed = dispatch(&mut session, owner, &subscribe(owner, "strata-fs"));
    assert_eq!(value(&failed)["error"]["code"], "driver_fault");
    let ready = dispatch(&mut session, owner, &subscribe(owner, "strata-net"));
    assert!(value(&ready).get("error").is_none());
    let drivers = status_drivers(&dispatch(&mut session, owner, &status_request(owner)));
    assert_eq!(drivers["strata-fs"]["phase"].as_str(), Some("failed"));
    assert_eq!(drivers["strata-net"]["phase"].as_str(), Some("ready"));
    assert_eq!(drivers["strata-net"]["subscriptions"].as_u64(), Some(1));

    let mut panicking = Session::new(Privilege::WindowsElevated, owner, PanicHost);
    let panicked = dispatch(&mut panicking, owner, &subscribe(owner, "strata-proc"));
    assert_eq!(value(&panicked)["error"]["code"], "driver_fault");
    assert_eq!(value(&panicked)["faults"].as_str(), Some("1"));
    let after = dispatch(&mut panicking, owner, &status_request(owner));
    assert!(value(&after).get("error").is_none());
    assert_eq!(value(&after)["faults"].as_str(), Some("1"));
    assert_eq!(
        status_drivers(&after)["strata-proc"]["phase"].as_str(),
        Some("idle")
    );
}

#[test]
fn ordinary_and_unmatched_identity_do_not_touch_the_host() {
    let owner = "uid:7";
    let mut session = Session::new(Privilege::WindowsOrdinary, owner, CountingHost::default());
    let denied = dispatch(&mut session, owner, &subscribe(owner, "strata-fs"));
    assert_eq!(value(&denied)["error"]["code"], "privilege_unavailable");
    assert_eq!(session_loads(&session), 0);
    let mut session = Session::new(Privilege::NotWindows, owner, CountingHost::default());
    let mismatch = dispatch(&mut session, "uid:8", &subscribe(owner, "strata-fs"));
    assert_eq!(value(&mismatch)["error"]["code"], "identity_mismatch");
    assert_eq!(session_loads(&session), 0);
}

#[cfg(windows)]
#[test]
fn restricted_token_cannot_open_the_service_pipe() {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::{null, null_mut};
    use std::thread;
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, GetLastError, LocalFree};
    use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows_sys::Win32::Security::{
        CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, ImpersonateLoggedOnUser, RevertToSelf,
        SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_QUERY,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_OVERLAPPED, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let service = Service::listen_catalog(Endpoint::unique().unwrap()).unwrap();
    let address = service.address().to_owned();
    let denied = thread::spawn(move || {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sid = format!(
            "S-1-9-{}-{}-{}",
            std::process::id(),
            nanos as u32,
            (nanos >> 32) as u32
        );
        let wide_sid: Vec<u16> = sid.encode_utf16().chain([0]).collect();
        let mut raw_sid = null_mut();
        let converted = unsafe { ConvertStringSidToSidW(wide_sid.as_ptr(), &mut raw_sid) };
        assert_ne!(converted, 0);
        let mut parent = null_mut();
        let opened = unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE,
                &mut parent,
            )
        };
        assert_ne!(opened, 0);
        let parent = unsafe { OwnedHandle::from_raw_handle(parent) };
        let limiting = SID_AND_ATTRIBUTES {
            Sid: raw_sid,
            Attributes: 0,
        };
        let mut restricted = null_mut();
        let created = unsafe {
            CreateRestrictedToken(
                parent.as_raw_handle(),
                DISABLE_MAX_PRIVILEGE,
                0,
                null(),
                0,
                null(),
                1,
                &limiting,
                &mut restricted,
            )
        };
        assert_ne!(created, 0);
        unsafe { LocalFree(raw_sid) };
        let token = unsafe { OwnedHandle::from_raw_handle(restricted) };
        let impersonated = unsafe { ImpersonateLoggedOnUser(token.as_raw_handle()) };
        assert_ne!(impersonated, 0);
        let name: Vec<u16> = address.encode_utf16().chain([0]).collect();
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                windows_sys::Win32::Foundation::GENERIC_READ
                    | windows_sys::Win32::Foundation::GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                null_mut(),
            )
        };
        let error = unsafe { GetLastError() };
        unsafe { RevertToSelf() };
        if !handle.is_null() && handle != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            None
        } else {
            Some(error)
        }
    })
    .join()
    .unwrap();
    assert_eq!(denied, Some(ERROR_ACCESS_DENIED));
    GuiClient::connect(&service.client_endpoint())
        .unwrap()
        .status()
        .unwrap();
}

fn subscribe(user: &str, driver: &str) -> Vec<u8> {
    request(user, "core", "subscribe", &format!(r#""{driver}""#)).into_bytes()
}

fn status_request(user: &str) -> Vec<u8> {
    request(user, "core", "status", "null").into_bytes()
}

fn dispatch<H: DriverHost>(session: &mut Session<H>, peer: &str, frame: &[u8]) -> String {
    String::from_utf8(session::dispatch(session, peer, frame)).unwrap()
}

fn status_drivers(response: &str) -> serde_json::Map<String, serde_json::Value> {
    value(response)["drivers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|driver| {
            (
                driver["driver"].as_str().unwrap().to_owned(),
                driver.clone(),
            )
        })
        .collect()
}

fn session_loads(session: &Session<CountingHost>) -> usize {
    session.host().loads
}

#[derive(Default)]
struct CountingHost {
    loads: usize,
}

impl DriverHost for CountingHost {
    fn load(&mut self, _driver: Driver) -> Result<(), HostError> {
        self.loads += 1;
        Err(HostError::ImageAbsent)
    }

    fn unload(&mut self, _driver: Driver) -> Result<(), HostError> {
        Ok(())
    }
}

#[derive(Default)]
struct SelectiveHost {
    loads: Vec<Driver>,
}

impl DriverHost for SelectiveHost {
    fn load(&mut self, driver: Driver) -> Result<(), HostError> {
        self.loads.push(driver);
        if driver == Driver::Fs {
            Err(HostError::fault("callback stopped"))
        } else {
            Ok(())
        }
    }

    fn unload(&mut self, _driver: Driver) -> Result<(), HostError> {
        Ok(())
    }
}

struct PanicHost;

impl DriverHost for PanicHost {
    fn load(&mut self, _driver: Driver) -> Result<(), HostError> {
        panic!("host fault");
    }

    fn unload(&mut self, _driver: Driver) -> Result<(), HostError> {
        Ok(())
    }
}

//! Windows 启动项、启动文件夹和计划任务的真实变更。
//! 这个文件在 Linux 上是空的。Linux 通过不能代替这里的验收。

#![cfg(windows)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::ptr::{null, null_mut};
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
    RegOpenKeyExW, RegSetValueExW,
};
use winstrata_protocols::{Attribution, Record};
use winstrata_strata_startup::{Origin, StartupTracker, capture, user_startup_folder};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APPROVED_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";

struct RegValue {
    key: &'static str,
    name: String,
}

impl Drop for RegValue {
    fn drop(&mut self) {
        let _ = delete_value(self.key, &self.name);
    }
}

struct FileGuard(PathBuf);

impl Drop for FileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct TaskGuard(String);

impl Drop for TaskGuard {
    fn drop(&mut self) {
        let _ = Command::new("schtasks")
            .args(["/Delete", "/TN", &self.0, "/F"])
            .output();
    }
}

#[test]
fn windows_run_folder_link_and_task_changes_are_attributed() {
    let pid = std::process::id();
    let run_name = format!("WinStrata023Probe{pid}");
    let _run = RegValue {
        key: RUN_KEY,
        name: run_name.clone(),
    };
    let _approved = RegValue {
        key: APPROVED_KEY,
        name: run_name.clone(),
    };
    delete_value(RUN_KEY, &run_name).ok();
    delete_value(APPROVED_KEY, &run_name).ok();

    let mut tracker = StartupTracker::new();
    let baseline = capture().expect("baseline startup capture");
    assert_eq!(baseline.evidence, "windows");
    tracker.observe(&baseline).expect("baseline");

    set_sz(RUN_KEY, &run_name, "cmd.exe /c exit 0").expect("create run value");
    let added = wait_fact(
        &mut tracker,
        &format!("run:hkcu-run:{run_name}"),
        "startup.added",
    );
    assert!(
        evidence_of(&added).contains("command=cmd.exe /c exit 0"),
        "{}",
        evidence_of(&added)
    );
    assert!(
        evidence_of(&added).contains("principal=hkcu"),
        "{}",
        evidence_of(&added)
    );

    set_sz(RUN_KEY, &run_name, "cmd.exe /c exit 1").expect("change run value");
    let changed = wait_fact(
        &mut tracker,
        &format!("run:hkcu-run:{run_name}"),
        "startup.changed",
    );
    let changed_evidence = evidence_of(&changed);
    assert!(
        changed_evidence.contains("before=cmd.exe /c exit 0"),
        "{changed_evidence}"
    );
    assert!(
        changed_evidence.contains("command=cmd.exe /c exit 1"),
        "{changed_evidence}"
    );

    set_binary(
        APPROVED_KEY,
        &run_name,
        &[0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    )
    .expect("disable startup approval");
    let disabled = wait_fact(
        &mut tracker,
        &format!("run:hkcu-run:{run_name}"),
        "startup.disabled",
    );
    assert!(
        evidence_of(&disabled).contains("enabled=0"),
        "{}",
        evidence_of(&disabled)
    );

    delete_value(RUN_KEY, &run_name).expect("delete run value");
    wait_fact(
        &mut tracker,
        &format!("run:hkcu-run:{run_name}"),
        "startup.removed",
    );

    let folder = user_startup_folder().expect("user startup folder");
    fs::create_dir_all(&folder).unwrap();
    let file_name = format!("WinStrata023Probe{pid}.lnk");
    let file = folder.join(&file_name);
    let _file = FileGuard(file.clone());
    fs::write(&file, probe_link()).unwrap();
    let folder_event = wait_fact(
        &mut tracker,
        &format!("folder:user-startup:{file_name}"),
        "startup.added",
    );
    assert_eq!(folder_event_origin(&folder_event), "folder");
    assert!(
        evidence_of(&folder_event).contains("command=C:\\Probe.exe"),
        "{}",
        evidence_of(&folder_event)
    );
    fs::remove_file(&file).unwrap();
    wait_fact(
        &mut tracker,
        &format!("folder:user-startup:{file_name}"),
        "startup.removed",
    );

    let task = format!("WinStrata023Probe{pid}");
    let _task = TaskGuard(task.clone());
    let created = Command::new("schtasks")
        .args([
            "/Create",
            "/TN",
            &task,
            "/TR",
            "cmd.exe /c exit 0",
            "/SC",
            "ONLOGON",
            "/F",
        ])
        .output()
        .expect("schtasks");
    assert!(
        created.status.success(),
        "schtasks create failed: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task_added = wait_fact(&mut tracker, &format!("task:{task}"), "task.added");
    assert!(
        evidence_of(&task_added).contains("cmd.exe /c exit 0"),
        "{}",
        evidence_of(&task_added)
    );
    let updated = Command::new("schtasks")
        .args(["/Change", "/TN", &task, "/TR", "cmd.exe /c exit 1"])
        .output()
        .expect("schtasks change");
    assert!(
        updated.status.success(),
        "schtasks change failed: {}",
        String::from_utf8_lossy(&updated.stderr)
    );
    let task_changed = wait_fact(&mut tracker, &format!("task:{task}"), "task.changed");
    assert!(
        evidence_of(&task_changed).contains("before=cmd.exe /c exit 0")
            || evidence_of(&task_changed).contains("cmd.exe /c exit 1"),
        "{}",
        evidence_of(&task_changed)
    );
    let deleted = Command::new("schtasks")
        .args(["/Delete", "/TN", &task, "/F"])
        .output()
        .expect("schtasks delete");
    assert!(
        deleted.status.success(),
        "schtasks delete failed: {}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    wait_fact(&mut tracker, &format!("task:{task}"), "task.removed");
}

fn wait_fact(
    tracker: &mut StartupTracker,
    id: &str,
    event_type: &str,
) -> winstrata_protocols::Event {
    let start = Instant::now();
    loop {
        let snapshot = capture().expect("startup capture");
        let events = tracker.observe(&snapshot).expect("observe");
        if let Some(event) = events.into_iter().find(|event| {
            matches!(&event.record, Record::Observation { event_type: kind, subject: Some(subject), .. }
                if kind.as_str() == event_type && subject.id.as_str() == id)
        }) {
            return event;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "timed out waiting for {event_type} {id}"
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn evidence_of(event: &winstrata_protocols::Event) -> &str {
    match &event.record {
        Record::Observation {
            attribution: Attribution::Observed { evidence },
            ..
        } => evidence.as_str(),
        other => panic!("expected observed evidence, got {other:?}"),
    }
}

fn folder_event_origin(event: &winstrata_protocols::Event) -> &str {
    let evidence = evidence_of(event);
    assert!(evidence.contains("origin=folder"), "{evidence}");
    let _ = Origin::Folder;
    "folder"
}

fn probe_link() -> Vec<u8> {
    let mut link = vec![0u8; 76];
    link[0] = 0x4c;
    link[20] = 0x02;
    let path = b"C:\\Probe.exe\0";
    let info_size = (0x1c + path.len()) as u32;
    let mut info = vec![0u8; 0x1c];
    info[0..4].copy_from_slice(&info_size.to_le_bytes());
    info[4..8].copy_from_slice(&0x1cu32.to_le_bytes());
    info[8..12].copy_from_slice(&1u32.to_le_bytes());
    info[16..20].copy_from_slice(&0x1cu32.to_le_bytes());
    link.extend(info);
    link.extend(path);
    link
}

fn set_sz(path: &str, name: &str, value: &str) -> std::io::Result<()> {
    let key = open_key(path, KEY_SET_VALUE)?;
    let mut data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let status = unsafe {
        RegSetValueExW(
            key,
            wide(name).as_ptr(),
            0,
            1,
            data.as_mut_ptr().cast(),
            (data.len() * 2) as u32,
        )
    };
    unsafe { RegCloseKey(key) };
    if status != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

fn set_binary(path: &str, name: &str, value: &[u8]) -> std::io::Result<()> {
    let key = create_key(path, KEY_SET_VALUE)?;
    let status = unsafe {
        RegSetValueExW(
            key,
            wide(name).as_ptr(),
            0,
            3,
            value.as_ptr(),
            value.len() as u32,
        )
    };
    unsafe { RegCloseKey(key) };
    if status != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

fn delete_value(path: &str, name: &str) -> std::io::Result<()> {
    let key = open_key(path, KEY_SET_VALUE)?;
    let status = unsafe { RegDeleteValueW(key, wide(name).as_ptr()) };
    unsafe { RegCloseKey(key) };
    if status != ERROR_SUCCESS && status != 2 {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

fn create_key(path: &str, access: u32) -> std::io::Result<HKEY> {
    let mut key = null_mut();
    let mut disposition = 0u32;
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            null(),
            0,
            access,
            null(),
            &mut key,
            &mut disposition,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    Ok(key)
}

fn open_key(path: &str, access: u32) -> std::io::Result<HKEY> {
    let mut key = null_mut();
    let status =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(path).as_ptr(), 0, access, &mut key) };
    if status != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    Ok(key)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

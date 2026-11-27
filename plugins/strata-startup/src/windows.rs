//! Windows 读取启动文件夹、Run/RunOnce 注册表值和任务计划程序的任务文件。
//!
//! 采样只打开键、枚举目录和读取 XML。它不创建、不修改、不删除这些项。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
    RegCloseKey, RegEnumValueW, RegGetValueW, RegOpenKeyExW,
};
use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
use windows_sys::core::GUID;
use winstrata_protocols::Id;

use super::{
    Coverage, Fact, Kind, Origin, StartupSnapshot, limit_facts, parse, startup_enabled, unix_now,
};

const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const KF_FLAG_DONT_VERIFY: u32 = 0x0000_4000;
const FOLDERID_STARTUP: GUID = GUID {
    data1: 0xB97D_20BB,
    data2: 0xF46A,
    data3: 0x4C97,
    data4: [0xBA, 0x10, 0x5E, 0x36, 0x08, 0x43, 0x08, 0x54],
};
const FOLDERID_COMMON_STARTUP: GUID = GUID {
    data1: 0x82A5_EA35,
    data2: 0xD9CD,
    data3: 0x47C5,
    data4: [0x96, 0x29, 0xE1, 0x5D, 0x2F, 0x71, 0x4E, 0x6E],
};

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { RegCloseKey(self.0) };
        }
    }
}

struct RunKey {
    hive: HKEY,
    path: &'static str,
    scope: &'static str,
    origin: Origin,
    approved: Option<&'static str>,
}

pub(super) fn capture() -> io::Result<StartupSnapshot> {
    let (device, boot, identity_failure) = machine_identity();
    let mut facts = Vec::new();
    let mut unreadable = BTreeSet::new();
    let mut truncated = false;
    let mut source_failure = identity_failure;
    for spec in run_keys() {
        match read_run_key(&spec) {
            Scope::Rows(rows, hit) => {
                facts.extend(rows);
                truncated |= hit;
            }
            Scope::Denied => {
                unreadable.insert(spec.scope.to_owned());
            }
            Scope::Failed(code) => {
                unreadable.insert(spec.scope.to_owned());
                source_failure.get_or_insert_with(|| format!("registry:{code}"));
            }
        }
    }
    match read_folder(&FOLDERID_STARTUP, "user-startup", "hkcu", HKEY_CURRENT_USER) {
        Scope::Rows(rows, hit) => {
            facts.extend(rows);
            truncated |= hit;
        }
        Scope::Denied => {
            unreadable.insert("user-startup".to_owned());
        }
        Scope::Failed(code) => {
            unreadable.insert("user-startup".to_owned());
            source_failure.get_or_insert_with(|| format!("folder:{code}"));
        }
    }
    match read_folder(
        &FOLDERID_COMMON_STARTUP,
        "common-startup",
        "public",
        HKEY_LOCAL_MACHINE,
    ) {
        Scope::Rows(rows, hit) => {
            facts.extend(rows);
            truncated |= hit;
        }
        Scope::Denied => {
            unreadable.insert("common-startup".to_owned());
        }
        Scope::Failed(code) => {
            unreadable.insert("common-startup".to_owned());
            source_failure.get_or_insert_with(|| format!("folder:{code}"));
        }
    }
    match read_tasks() {
        Scope::Rows(rows, hit) => {
            facts.extend(rows);
            truncated |= hit;
        }
        Scope::Denied => {
            unreadable.insert("tasks".to_owned());
        }
        Scope::Failed(code) => {
            unreadable.insert("tasks".to_owned());
            source_failure.get_or_insert_with(|| format!("tasks:{code}"));
        }
    }
    let (facts, hit_cap) = limit_facts(facts);
    Ok(StartupSnapshot {
        device,
        boot,
        sampled_unix_ns: unix_now()?,
        facts,
        coverage: Coverage::Windows,
        truncated: truncated || hit_cap,
        unreadable_scopes: unreadable,
        source_failure,
        evidence: "windows",
    })
}

pub(super) fn user_startup_folder() -> io::Result<PathBuf> {
    known_folder(&FOLDERID_STARTUP)
}

fn run_keys() -> [RunKey; 8] {
    [
        RunKey {
            hive: HKEY_CURRENT_USER,
            path: "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            scope: "hkcu-run",
            origin: Origin::Run,
            approved: Some(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run",
            ),
        },
        RunKey {
            hive: HKEY_LOCAL_MACHINE,
            path: "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            scope: "hklm-run",
            origin: Origin::Run,
            approved: Some(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run",
            ),
        },
        RunKey {
            hive: HKEY_LOCAL_MACHINE,
            path: "Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Run",
            scope: "hklm-wow-run",
            origin: Origin::Run,
            approved: Some(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run32",
            ),
        },
        RunKey {
            hive: HKEY_CURRENT_USER,
            path: "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
            scope: "hkcu-runonce",
            origin: Origin::RunOnce,
            approved: None,
        },
        RunKey {
            hive: HKEY_LOCAL_MACHINE,
            path: "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
            scope: "hklm-runonce",
            origin: Origin::RunOnce,
            approved: None,
        },
        RunKey {
            hive: HKEY_LOCAL_MACHINE,
            path: "Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
            scope: "hklm-wow-runonce",
            origin: Origin::RunOnce,
            approved: None,
        },
        RunKey {
            hive: HKEY_CURRENT_USER,
            path: "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer\\Run",
            scope: "hkcu-policy-run",
            origin: Origin::PolicyRun,
            approved: None,
        },
        RunKey {
            hive: HKEY_LOCAL_MACHINE,
            path: "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer\\Run",
            scope: "hklm-policy-run",
            origin: Origin::PolicyRun,
            approved: None,
        },
    ]
}

enum Scope {
    Rows(Vec<Fact>, bool),
    Denied,
    Failed(u32),
}

fn read_run_key(spec: &RunKey) -> Scope {
    let values = match enum_strings(spec.hive, spec.path) {
        Ok(rows) => rows,
        Err(scope) => return scope,
    };
    let (approved, approved_denied) = match spec.approved.map(|path| enum_binary(spec.hive, path)) {
        None | Some(Enum::Missing) => (BTreeMap::new(), false),
        Some(Enum::Denied) | Some(Enum::Failed(_)) => (BTreeMap::new(), true),
        Some(Enum::Rows(rows, _)) => (
            rows.into_iter()
                .map(|(name, data, _)| (name, data))
                .collect::<BTreeMap<_, _>>(),
            false,
        ),
    };
    let mut facts = Vec::new();
    for (name, command) in values.0 {
        let id = format!("run:{}:{name}", spec.scope);
        if Id::new(&id).is_err() {
            continue;
        }
        let bytes = approved.get(&name).map(Vec::as_slice);
        let mut partial = None;
        let enabled = if approved_denied {
            partial = Some("approved-unreadable".to_owned());
            true
        } else {
            startup_enabled(bytes)
        };
        let principal = if spec.hive == HKEY_CURRENT_USER {
            "hkcu"
        } else {
            "hklm"
        };
        facts.push(Fact {
            id,
            kind: Kind::Startup,
            origin: spec.origin,
            scope: spec.scope.to_owned(),
            location: format!("{}\\{name}", spec.path),
            command,
            schedule: String::new(),
            enabled,
            principal: principal.to_owned(),
            partial,
        });
    }
    Scope::Rows(facts, values.1)
}

fn read_folder(guid: &GUID, scope: &str, principal: &str, hive: HKEY) -> Scope {
    let path = match known_folder(guid) {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return Scope::Denied,
        Err(error) => return Scope::Failed(error.raw_os_error().unwrap_or(1) as u32),
    };
    let entries = match fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Scope::Rows(Vec::new(), false),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return Scope::Denied,
        Err(error) => return Scope::Failed(error.raw_os_error().unwrap_or(1) as u32),
    };
    let approved_path =
        "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\StartupFolder";
    let approved = match enum_binary(hive, approved_path) {
        Enum::Rows(rows, _) => rows
            .into_iter()
            .map(|(name, data, _)| (name, data))
            .collect::<BTreeMap<_, _>>(),
        Enum::Missing => BTreeMap::new(),
        Enum::Denied | Enum::Failed(_) => {
            let facts = folder_facts(entries, scope, principal, None, true);
            return match facts {
                Ok((facts, hit)) => Scope::Rows(facts, hit),
                Err(error) if error.kind() == ErrorKind::PermissionDenied => Scope::Denied,
                Err(error) => Scope::Failed(error.raw_os_error().unwrap_or(1) as u32),
            };
        }
    };
    match folder_facts(entries, scope, principal, Some(&approved), false) {
        Ok((facts, hit)) => Scope::Rows(facts, hit),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => Scope::Denied,
        Err(error) => Scope::Failed(error.raw_os_error().unwrap_or(1) as u32),
    }
}

fn folder_facts(
    entries: fs::ReadDir,
    scope: &str,
    principal: &str,
    approved: Option<&BTreeMap<String, Vec<u8>>>,
    approved_denied: bool,
) -> io::Result<(Vec<Fact>, bool)> {
    let mut facts = Vec::new();
    let mut truncated = false;
    for entry in entries {
        let entry = entry?;
        if facts.len() >= 4096 {
            truncated = true;
            break;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path())?;
        let file = if meta.is_file() {
            true
        } else if meta.file_type().is_symlink() {
            entry.metadata().is_ok_and(|item| item.is_file())
        } else {
            false
        };
        if !file {
            continue;
        }
        let id = format!("folder:{scope}:{name}");
        if Id::new(&id).is_err() {
            continue;
        }
        let path = entry.path();
        let mut partial = None;
        let command = if name.to_ascii_lowercase().ends_with(".lnk") {
            match fs::read(&path) {
                Ok(bytes) => parse::link_local_path(&bytes).unwrap_or_else(|| {
                    partial = Some("link-unresolved".to_owned());
                    path.display().to_string()
                }),
                Err(_) => {
                    partial = Some("link-unresolved".to_owned());
                    path.display().to_string()
                }
            }
        } else {
            path.display().to_string()
        };
        let enabled = if approved_denied {
            partial.get_or_insert_with(|| "approved-unreadable".to_owned());
            true
        } else {
            startup_enabled(approved.and_then(|map| map.get(name)).map(Vec::as_slice))
        };
        facts.push(Fact {
            id,
            kind: Kind::Startup,
            origin: Origin::Folder,
            scope: scope.to_owned(),
            location: path.display().to_string(),
            command,
            schedule: String::new(),
            enabled,
            principal: principal.to_owned(),
            partial,
        });
    }
    Ok((facts, truncated))
}

fn read_tasks() -> Scope {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
    let root = PathBuf::from(root).join("System32").join("Tasks");
    let mut facts = Vec::new();
    let mut truncated = false;
    match walk_tasks(&root, &root, &mut facts, &mut truncated, 0) {
        Ok(()) => Scope::Rows(facts, truncated),
        Err(error) if error.kind() == ErrorKind::NotFound => Scope::Rows(Vec::new(), false),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => Scope::Denied,
        Err(error) => Scope::Failed(error.raw_os_error().unwrap_or(1) as u32),
    }
}

fn walk_tasks(
    root: &Path,
    dir: &Path,
    facts: &mut Vec<Fact>,
    truncated: &mut bool,
    depth: usize,
) -> io::Result<()> {
    if depth > 16 || facts.len() >= 4096 {
        *truncated = true;
        return Ok(());
    }
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        if facts.len() >= 4096 {
            *truncated = true;
            break;
        }
        let meta = fs::symlink_metadata(entry.path())?;
        if meta.is_dir() {
            walk_tasks(root, &entry.path(), facts, truncated, depth + 1)?;
            continue;
        }
        let file = meta.is_file()
            || (meta.file_type().is_symlink() && entry.metadata().is_ok_and(|item| item.is_file()));
        if !file {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let id = format!("task:{relative}");
        if Id::new(&id).is_err() {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) if bytes.len() as u64 <= 1024 * 1024 => bytes,
            Ok(_) => {
                *truncated = true;
                continue;
            }
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                facts.push(task_fact(
                    id,
                    path.display().to_string(),
                    String::new(),
                    true,
                    None,
                    Some("task-unreadable".to_owned()),
                ));
                continue;
            }
            Err(_) => {
                facts.push(task_fact(
                    id,
                    path.display().to_string(),
                    String::new(),
                    true,
                    None,
                    Some("task-unreadable".to_owned()),
                ));
                continue;
            }
        };
        let xml = decode_xml(&bytes);
        let actions = section_loose(&xml, "Actions");
        let commands = tag_all(actions, "Command");
        let arguments = tag_all(actions, "Arguments");
        let mut command = String::new();
        for (index, item) in commands.iter().enumerate() {
            if index > 0 {
                command.push_str(" | ");
            }
            command.push_str(item);
            if let Some(argument) = arguments.get(index) {
                if !argument.is_empty() {
                    command.push(' ');
                    command.push_str(argument);
                }
            }
        }
        let partial = if commands.len() > 1 {
            Some("extra-actions".to_owned())
        } else {
            None
        };
        let user = tag_text(&xml, "UserId").unwrap_or("");
        facts.push(task_fact(
            id,
            path.display().to_string(),
            command,
            settings_enabled(&xml),
            if user.is_empty() {
                None
            } else {
                Some(user.to_owned())
            },
            partial,
        ));
    }
    Ok(())
}

fn task_fact(
    id: String,
    location: String,
    command: String,
    enabled: bool,
    principal: Option<String>,
    partial: Option<String>,
) -> Fact {
    Fact {
        id,
        kind: Kind::Task,
        origin: Origin::ScheduledTask,
        scope: "tasks".to_owned(),
        location,
        command,
        schedule: String::new(),
        enabled,
        principal: principal.unwrap_or_default(),
        partial,
    }
}

enum Enum {
    Rows(Vec<(String, Vec<u8>, u32)>, bool),
    Missing,
    Denied,
    Failed(u32),
}

fn enum_strings(hive: HKEY, path: &str) -> Result<(Vec<(String, String)>, bool), Scope> {
    match enum_values(hive, path) {
        Enum::Rows(rows, hit) => Ok((
            rows.into_iter()
                .filter_map(|(name, data, kind)| {
                    if kind != REG_SZ && kind != REG_EXPAND_SZ {
                        return None;
                    }
                    Some((name, utf16_bytes(&data)))
                })
                .collect(),
            hit,
        )),
        Enum::Missing => Ok((Vec::new(), false)),
        Enum::Denied => Err(Scope::Denied),
        Enum::Failed(code) => Err(Scope::Failed(code)),
    }
}

fn enum_binary(hive: HKEY, path: &str) -> Enum {
    match enum_values(hive, path) {
        Enum::Rows(rows, hit) => Enum::Rows(
            rows.into_iter()
                .filter_map(|(name, data, kind)| (kind == 3).then_some((name, data, kind)))
                .collect(),
            hit,
        ),
        other => other,
    }
}

fn enum_values(hive: HKEY, path: &str) -> Enum {
    let wide_path = wide(path);
    let mut raw = null_mut();
    let status = unsafe { RegOpenKeyExW(hive, wide_path.as_ptr(), 0, KEY_READ, &mut raw) };
    if status == ERROR_FILE_NOT_FOUND {
        return Enum::Missing;
    }
    if status == ERROR_ACCESS_DENIED {
        return Enum::Denied;
    }
    if status != ERROR_SUCCESS {
        return Enum::Failed(status);
    }
    let key = Key(raw);
    let mut rows = Vec::new();
    let mut truncated = false;
    let mut index = 0u32;
    loop {
        let mut name = vec![0u16; 16 * 1024];
        let mut name_len = (name.len() - 1) as u32;
        let mut kind = 0u32;
        let mut data = vec![0u8; 64 * 1024];
        let mut data_len = data.len() as u32;
        let status = unsafe {
            RegEnumValueW(
                key.0,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                null_mut(),
                &mut kind,
                data.as_mut_ptr(),
                &mut data_len,
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        if status == ERROR_MORE_DATA {
            truncated = true;
            index = index.saturating_add(1);
            continue;
        }
        if status != ERROR_SUCCESS {
            return Enum::Failed(status);
        }
        index = index.saturating_add(1);
        let Some(name) = wide_slice(&name[..name_len as usize]) else {
            continue;
        };
        data.truncate(data_len as usize);
        rows.push((name, data, kind));
        if rows.len() >= 4096 {
            truncated = true;
            break;
        }
    }
    Enum::Rows(rows, truncated)
}

fn known_folder(guid: &GUID) -> io::Result<PathBuf> {
    let mut raw = null_mut();
    let status = unsafe { SHGetKnownFolderPath(guid, KF_FLAG_DONT_VERIFY, null_mut(), &mut raw) };
    if status < 0 {
        let code = status & 0xFFFF;
        let kind = if code == ERROR_ACCESS_DENIED as i32 {
            ErrorKind::PermissionDenied
        } else {
            ErrorKind::Other
        };
        return Err(io::Error::new(kind, format!("known folder {status:#x}")));
    }
    let path = wide_ptr(raw);
    unsafe { CoTaskMemFree(raw.cast()) };
    path.ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "known folder path is empty"))
}

fn machine_identity() -> (String, String, Option<String>) {
    match registry_sz(
        HKEY_LOCAL_MACHINE,
        "SOFTWARE\\Microsoft\\Cryptography",
        "MachineGuid",
    ) {
        Ok(device) => {
            let boot = registry_dword(
                HKEY_LOCAL_MACHINE,
                "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Memory Management\\PrefetchParameters",
                "BootId",
            );
            match boot {
                Ok(value) => (device, format!("boot:{value}"), None),
                Err(error) if access_denied(&error) => (
                    device,
                    "boot:unavailable".to_owned(),
                    Some("identity:5".to_owned()),
                ),
                Err(_) => (device, "boot:unavailable".to_owned(), None),
            }
        }
        Err(error) if access_denied(&error) => (
            String::new(),
            "boot:unavailable".to_owned(),
            Some("identity:5".to_owned()),
        ),
        Err(error) => (
            String::new(),
            "boot:unavailable".to_owned(),
            Some(format!("identity:{}", error.raw_os_error().unwrap_or(1))),
        ),
    }
}

fn access_denied(error: &io::Error) -> bool {
    error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32)
}

fn registry_dword(hive: HKEY, subkey: &str, value: &str) -> io::Result<u32> {
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            hive,
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

fn registry_sz(hive: HKEY, subkey: &str, value: &str) -> io::Result<String> {
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            hive,
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
            hive,
            wide(subkey).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
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
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_ptr(value: *const u16) -> Option<PathBuf> {
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
        wide_slice(std::slice::from_raw_parts(value, len)).map(PathBuf::from)
    }
}

fn wide_slice(value: &[u16]) -> Option<String> {
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    let text = String::from_utf16_lossy(&value[..end]);
    if text.is_empty() { None } else { Some(text) }
}

fn utf16_bytes(bytes: &[u8]) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    String::from_utf16_lossy(&units)
}

fn decode_xml(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return utf16_bytes(&bytes[2..]);
    }
    if bytes.len() >= 2 && bytes[0] == b'<' && bytes[1] == 0 {
        return utf16_bytes(bytes);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn settings_enabled(xml: &str) -> bool {
    let settings = section_loose(xml, "Settings");
    match tag_text(settings, "Enabled") {
        Some(value) => !value.eq_ignore_ascii_case("false"),
        None => true,
    }
}

fn section_loose<'a>(xml: &'a str, tag: &str) -> &'a str {
    let open = format!("<{tag}");
    let Some(start) = xml.find(&open) else {
        return "";
    };
    let Some(relative) = xml[start..].find('>') else {
        return "";
    };
    let after = start + relative + 1;
    let close = format!("</{tag}>");
    let end = xml[after..]
        .find(&close)
        .map(|index| after + index)
        .unwrap_or(xml.len());
    &xml[after..end]
}

fn tag_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    tag_all(xml, tag).into_iter().next()
}

fn tag_all<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut values = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let body = &rest[start + open.len()..];
        let Some(end) = body.find(&close) else {
            break;
        };
        values.push(body[..end].trim());
        rest = &body[end + close.len()..];
        if values.len() >= 8 {
            break;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    #[test]
    fn registry_names_are_null_terminated() {
        let encoded = super::wide("WinStrata023Probe");
        assert_eq!(encoded.last().copied(), Some(0));
        assert!(encoded[..encoded.len() - 1].iter().all(|unit| *unit != 0));
    }
}

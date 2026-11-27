//! Windows 用户态文件与注册表观察。
//!
//! `ReadDirectoryChangesW` 和 `RegNotifyChangeKeyValue` 不提供变更时刻，也不读取文件字节。
//! 缓冲溢出记为丢失缺口。这里不调用服务控制管理器，也不加载驱动。

use std::fs::symlink_metadata;
use std::io;
use std::mem::zeroed;
use std::os::windows::ffi::OsStrExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
    FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SECURITY, FILE_NOTIFY_CHANGE_SIZE,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_QUERY_VALUE, REG_NOTIFY_CHANGE_LAST_SET,
    REG_NOTIFY_CHANGE_NAME, RegCloseKey, RegEnumValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
};
use winstrata_kernel::Channel;
use winstrata_protocols::{GapReason, Id};

use super::decode::{self, NotifyRead, ValueSample, classify_notify, decode_file_notifications};
use super::encode::{ObservedPath, PathKind};
use super::{FILE_PROVIDER, Publisher, REGISTRY_PROVIDER, emit_files, emit_registry};
use crate::CollectError;

const VALUE_LIMIT: usize = 64;
const NAME_CHARS: usize = 256;
const DATA_BYTES: usize = 1024;

pub(super) struct StopFlag {
    event: HANDLE,
}

unsafe impl Send for StopFlag {}
unsafe impl Sync for StopFlag {}

impl StopFlag {
    fn new() -> io::Result<Self> {
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { event })
    }

    fn signal(&self) {
        unsafe {
            SetEvent(self.event);
        }
    }
}

impl Drop for StopFlag {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.event);
        }
    }
}

struct WinHandle(HANDLE);

impl Drop for WinHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

struct RegKey(HKEY);

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

impl super::Platform {
    pub(super) fn start_channel(&mut self, channel: Channel) -> Result<(), CollectError> {
        if self.slot(channel).worker.is_some() {
            return Err(CollectError::fault("collector already running"));
        }
        let epoch_number = self.slot(channel).epoch.saturating_add(1);
        let health = Arc::clone(&self.slot(channel).health);
        *health.lock().expect("collector health") = None;
        let (prefix, provider) = match channel {
            Channel::Files => ("fs", FILE_PROVIDER),
            Channel::Registry => ("reg", REGISTRY_PROVIDER),
        };
        let publisher = Publisher {
            bus: self.bus.clone(),
            stream: super::encode::Stream {
                device: self.device.clone(),
                provider: Id::new(provider).map_err(|_| CollectError::fault("invalid provider"))?,
                epoch: Id::new(format!("{prefix}-{epoch_number}"))
                    .map_err(|_| CollectError::fault("invalid epoch"))?,
            },
            sequence: 1,
            user: self.user.clone(),
            health: Arc::clone(&health),
        };
        let directory = self.directory.clone();
        let subkey = self.subkey.clone();
        let stop = Arc::new(StopFlag::new().map_err(|error| os_code(&error))?);
        let stop_for_thread = Arc::clone(&stop);
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = thread::Builder::new()
            .name(if channel == Channel::Files {
                "ws-fs".into()
            } else {
                "ws-reg".into()
            })
            .spawn(move || {
                let ran = catch_unwind(AssertUnwindSafe(|| match channel {
                    Channel::Files => watch_directory(&directory, publisher, &stop_for_thread, tx),
                    Channel::Registry => watch_registry(&subkey, publisher, &stop_for_thread, tx),
                }));
                if ran.is_err() {
                    let mut health = health.lock().expect("collector health");
                    if health.is_none() {
                        *health = Some(CollectError::fault("collector thread panicked"));
                    }
                }
            })
            .map_err(|_| CollectError::fault("collector thread did not start"))?;
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {
                let slot = self.slot_mut(channel);
                slot.epoch = epoch_number;
                slot.worker = Some(worker);
                slot.stop = Some(stop);
                Ok(())
            }
            Ok(Err(error)) => {
                stop.signal();
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                stop.signal();
                let _ = worker.join();
                Err(CollectError::fault("collector did not start"))
            }
        }
    }

    pub(super) fn stop_channel(&mut self, channel: Channel) -> Result<(), CollectError> {
        if let Some(stop) = self.slot_mut(channel).stop.take() {
            stop.signal();
        }
        let Some(worker) = self.slot_mut(channel).worker.take() else {
            return Ok(());
        };
        match worker.join() {
            Ok(()) => Ok(()),
            Err(_) => Err(CollectError::fault("collector thread panicked")),
        }
    }
}

fn os_code(error: &io::Error) -> CollectError {
    CollectError::Os(error.raw_os_error().unwrap_or(-1))
}

struct DirectoryIo {
    overlapped: OVERLAPPED,
    // 系统在重叠完成时写入这个地址。完成字节数以 GetOverlappedResult 为准。
    #[allow(dead_code)]
    transferred: u32,
    buffer: Vec<u32>,
}

fn watch_directory(
    root: &Path,
    mut publisher: Publisher,
    stop: &StopFlag,
    ready: Sender<Result<(), CollectError>>,
) {
    // 重叠结构、字节数和缓冲区必须留在第一次 ReadDirectoryChangesW 之后的地址上。
    // 系统把完成状态写回这个地址；移动它们会让随后的 GetOverlappedResult 读到空副本。
    let directory = match open_directory(root) {
        Ok(directory) => directory,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let event = match event_handle(true) {
        Ok(event) => WinHandle(event),
        Err(error) => {
            let _ = ready.send(Err(os_code(&error)));
            return;
        }
    };
    let mut io = DirectoryIo {
        overlapped: overlapped(event.0),
        transferred: 0,
        buffer: vec![0u32; 4096],
    };
    if let Err(error) = issue_read(&directory, &mut io) {
        let _ = ready.send(Err(error));
        return;
    }
    if ready.send(Ok(())).is_err() {
        cancel(&directory, &io.overlapped);
        return;
    }
    loop {
        match wait_pair(event.0, stop.event) {
            Wait::Stop => {
                cancel(&directory, &io.overlapped);
                break;
            }
            Wait::Failed(code) => {
                cancel(&directory, &io.overlapped);
                publisher.fail(CollectError::Os(code));
                break;
            }
            Wait::Ready => match read_result(&directory, &io) {
                NotifyRead::Aborted => break,
                NotifyRead::Lost => {
                    if !publisher.send_gap(GapReason::Lost) {
                        break;
                    }
                }
                NotifyRead::Failed(code) => {
                    let _ = publisher.send_gap(GapReason::SourceFailure);
                    publisher.fail(CollectError::Os(code));
                    break;
                }
                NotifyRead::Ready(size) => {
                    let bytes = buffer_bytes(&io.buffer, size);
                    let continued = match decode_file_notifications(bytes) {
                        Ok(notices) => emit_files(&mut publisher, &notices, root),
                        Err(decode::DecodeError::Truncated) => {
                            publisher
                                .fail(CollectError::fault("directory notification was truncated"));
                            publisher.send_gap(GapReason::SourceFailure)
                        }
                    };
                    if !continued {
                        break;
                    }
                }
            },
        }
        if issue_read(&directory, &mut io).is_err() {
            publisher.fail(CollectError::fault("directory read was not rearmed"));
            break;
        }
    }
}

fn watch_registry(
    subkey: &str,
    mut publisher: Publisher,
    stop: &StopFlag,
    ready: Sender<Result<(), CollectError>>,
) {
    let opened = (|| {
        let key = open_key(subkey)?;
        let event = WinHandle(event_handle(true).map_err(|error| os_code(&error))?);
        let baseline = snapshot(key.0)?;
        arm(key.0, event.0)?;
        Ok((key, event, baseline))
    })();
    let (key, event, mut baseline) = match opened {
        Ok(ready_state) => ready_state,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    if unsafe { WaitForSingleObject(event.0, 0) } == WAIT_OBJECT_0 {
        // 武装和基线之间的变化无法逐值归因，只记未知丢失。
        if !publisher.send_gap(GapReason::Lost) {
            return;
        }
        match snapshot(key.0).and_then(|next| {
            baseline = next;
            arm(key.0, event.0)
        }) {
            Ok(()) => {}
            Err(error) => {
                publisher.fail(error);
                return;
            }
        }
    }
    loop {
        match wait_pair(event.0, stop.event) {
            Wait::Stop => break,
            Wait::Failed(code) => {
                publisher.fail(CollectError::Os(code));
                break;
            }
            Wait::Ready => {}
        }
        if arm(key.0, event.0).is_err() {
            publisher.fail(CollectError::fault("registry watch was not rearmed"));
            break;
        }
        let now = match snapshot(key.0) {
            Ok(now) => now,
            Err(error) => {
                publisher.fail(error);
                break;
            }
        };
        let changes = if baseline.complete && now.complete {
            decode::diff_values(&baseline.values, &now.values).map_err(|_| ())
        } else {
            Err(())
        };
        let continued = match &changes {
            Ok(changes) if changes.is_empty() => true,
            Ok(changes) => emit_registry(&mut publisher, subkey, Ok(changes)),
            Err(()) => emit_registry(&mut publisher, subkey, Err(())),
        };
        baseline = now;
        if !continued {
            break;
        }
    }
}

pub(super) fn observe_path(root: &Path, name: &str) -> Option<ObservedPath> {
    if name_escapes(name) {
        return Some(ObservedPath {
            full_path: None,
            kind: PathKind::Missing,
            owner: None,
            sddl: None,
            escaped: true,
        });
    }
    let path = root.join(name);
    let full_path = path.to_str().map(str::to_owned);
    let Ok(metadata) = symlink_metadata(&path) else {
        return Some(ObservedPath {
            full_path,
            kind: PathKind::Missing,
            owner: None,
            sddl: None,
            escaped: false,
        });
    };
    if metadata.file_type().is_symlink() {
        return Some(ObservedPath {
            full_path,
            kind: PathKind::Reparse,
            owner: None,
            sddl: None,
            escaped: false,
        });
    }
    let kind = if metadata.is_dir() {
        PathKind::Directory
    } else {
        PathKind::File
    };
    match crate::platform::path_security(&path) {
        Ok(security) => Some(ObservedPath {
            full_path,
            kind,
            owner: Some(security.owner),
            sddl: security.sddl,
            escaped: false,
        }),
        Err(_) => Some(ObservedPath {
            full_path,
            kind,
            owner: None,
            sddl: None,
            escaped: false,
        }),
    }
}

fn name_escapes(name: &str) -> bool {
    name.is_empty()
        || Path::new(name).is_absolute()
        || name
            .split(['\\', '/'])
            .any(|part| part.is_empty() || part == "." || part == "..")
}

fn open_directory(root: &Path) -> Result<WinHandle, CollectError> {
    let mut name = wide_path(root).map_err(|_| CollectError::fault("invalid directory"))?;
    let handle = unsafe {
        windows_sys::Win32::Storage::FileSystem::CreateFileW(
            name.as_mut_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(os_code(&io::Error::last_os_error()));
    }
    Ok(WinHandle(handle))
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL",
        ));
    }
    name.push(0);
    Ok(name)
}

fn event_handle(manual: bool) -> io::Result<HANDLE> {
    let event = unsafe { CreateEventW(null(), i32::from(manual), 0, null()) };
    if event.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(event)
    }
}

fn overlapped(event: HANDLE) -> OVERLAPPED {
    let mut overlapped = unsafe { zeroed::<OVERLAPPED>() };
    overlapped.hEvent = event;
    overlapped
}

fn issue_read(directory: &WinHandle, io: &mut DirectoryIo) -> Result<(), CollectError> {
    let event = io.overlapped.hEvent;
    io.overlapped = unsafe { zeroed() };
    io.overlapped.hEvent = event;
    io.transferred = 0;
    unsafe { ResetEvent(event) };
    let filter = FILE_NOTIFY_CHANGE_FILE_NAME
        | FILE_NOTIFY_CHANGE_DIR_NAME
        | FILE_NOTIFY_CHANGE_ATTRIBUTES
        | FILE_NOTIFY_CHANGE_SIZE
        | FILE_NOTIFY_CHANGE_LAST_WRITE
        | FILE_NOTIFY_CHANGE_SECURITY;
    let byte_len = u32::try_from(io.buffer.len() * 4).unwrap_or(u32::MAX);
    let ok = unsafe {
        ReadDirectoryChangesW(
            directory.0,
            io.buffer.as_mut_ptr().cast(),
            byte_len,
            0,
            filter,
            &mut io.transferred,
            &mut io.overlapped,
            None,
        )
    };
    if ok == 0 {
        let code = unsafe { GetLastError() };
        if code != ERROR_IO_PENDING {
            return Err(CollectError::Os(code as i32));
        }
    }
    Ok(())
}

fn read_result(directory: &WinHandle, io: &DirectoryIo) -> NotifyRead {
    let mut transferred = 0u32;
    let ok = unsafe { GetOverlappedResult(directory.0, &io.overlapped, &mut transferred, 0) };
    if ok == 0 {
        let code = unsafe { GetLastError() } as i32;
        return classify_notify(code, 0);
    }
    let limit = u32::try_from(io.buffer.len() * 4).unwrap_or(u32::MAX);
    if transferred > limit {
        return NotifyRead::Failed(transferred as i32);
    }
    classify_notify(0, transferred)
}

fn buffer_bytes(buffer: &[u32], size: u32) -> &[u8] {
    let bytes =
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), buffer.len() * 4) };
    &bytes[..size as usize]
}

fn cancel(directory: &WinHandle, overlapped: &OVERLAPPED) {
    unsafe {
        CancelIoEx(directory.0, overlapped);
        let mut transferred = 0;
        GetOverlappedResult(directory.0, overlapped, &mut transferred, 1);
    }
}

enum Wait {
    Ready,
    Stop,
    Failed(i32),
}

fn wait_pair(first: HANDLE, stop: HANDLE) -> Wait {
    let handles = [first, stop];
    let status = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
    if status == WAIT_OBJECT_0 {
        Wait::Ready
    } else if status == WAIT_OBJECT_0 + 1 {
        Wait::Stop
    } else {
        Wait::Failed(unsafe { GetLastError() } as i32)
    }
}

fn open_key(subkey: &str) -> Result<RegKey, CollectError> {
    let name = wide_text(subkey)?;
    let mut key = null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            name.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_NOTIFY,
            &mut key,
        )
    };
    if status != 0 {
        return Err(CollectError::Os(status as i32));
    }
    Ok(RegKey(key))
}

fn arm(key: HKEY, event: HANDLE) -> Result<(), CollectError> {
    unsafe { ResetEvent(event) };
    let status = unsafe {
        RegNotifyChangeKeyValue(
            key,
            0,
            REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
            event,
            1,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(CollectError::Os(status as i32))
    }
}

struct Snapshot {
    values: Vec<ValueSample>,
    complete: bool,
}

fn snapshot(key: HKEY) -> Result<Snapshot, CollectError> {
    let mut values = Vec::new();
    let mut complete = true;
    let mut index = 0u32;
    loop {
        if values.len() == VALUE_LIMIT {
            complete = false;
            break;
        }
        let mut name = vec![0u16; NAME_CHARS];
        let mut name_chars = name.len() as u32;
        let mut kind = 0u32;
        let mut data = vec![0u8; DATA_BYTES];
        let mut data_len = data.len() as u32;
        let status = unsafe {
            RegEnumValueW(
                key,
                index,
                name.as_mut_ptr(),
                &mut name_chars,
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
            complete = false;
            break;
        }
        if status != 0 {
            return Err(CollectError::Os(status as i32));
        }
        let name = match String::from_utf16(&name[..name_chars as usize]) {
            Ok(name) => name,
            Err(_) => {
                complete = false;
                break;
            }
        };
        if values
            .iter()
            .any(|sample: &ValueSample| sample.name == name)
        {
            complete = false;
            break;
        }
        values.push(ValueSample {
            name,
            kind,
            data: data[..data_len as usize].to_vec(),
        });
        index += 1;
    }
    Ok(Snapshot { values, complete })
}

fn wide_text(text: &str) -> Result<Vec<u16>, CollectError> {
    if text.chars().any(|symbol| symbol == '\0') {
        return Err(CollectError::fault("invalid registry key"));
    }
    Ok(text.encode_utf16().chain(std::iter::once(0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::System::Registry::{
        REG_DWORD, REG_OPTION_VOLATILE, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW,
        RegSetValueExW,
    };
    use winstrata_protocols::{Attribution, Completeness, Event, Id, Record};

    use crate::platform::{current_id, fresh_id, path_security};
    use crate::{Channel, EventBus, FileRegistry, Filter, Limits};
    use winstrata_kernel::Phase;

    const SECRET: &str = "winstrata-secret-009-content";

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(name);
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct KeyCleanup(Vec<u16>);

    impl Drop for KeyCleanup {
        fn drop(&mut self) {
            unsafe {
                RegDeleteTreeW(HKEY_CURRENT_USER, self.0.as_ptr());
            }
        }
    }

    struct WriteKey(HKEY);

    impl WriteKey {
        fn create(subkey: &str) -> Self {
            let name = wide_text(subkey).unwrap();
            let mut key = null_mut();
            let status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    name.as_ptr(),
                    0,
                    null(),
                    REG_OPTION_VOLATILE,
                    KEY_QUERY_VALUE | KEY_NOTIFY | 0x0002 | 0x00010000,
                    null(),
                    &mut key,
                    null_mut(),
                )
            };
            assert_eq!(status, 0, "RegCreateKeyExW failed with {status}");
            Self(key)
        }

        fn set_dword(&self, name: &str, value: u32) {
            let name = wide_text(name).unwrap();
            let bytes = value.to_le_bytes();
            let status =
                unsafe { RegSetValueExW(self.0, name.as_ptr(), 0, REG_DWORD, bytes.as_ptr(), 4) };
            assert_eq!(status, 0, "RegSetValueExW failed with {status}");
        }

        fn delete_value(&self, name: &str) {
            let name = wide_text(name).unwrap();
            let status = unsafe { RegDeleteValueW(self.0, name.as_ptr()) };
            assert_eq!(status, 0, "RegDeleteValueW failed with {status}");
        }
    }

    impl Drop for WriteKey {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }

    struct Harness {
        watch: FileRegistry,
        events: crate::Subscription,
        _bus: EventBus,
    }

    impl Harness {
        fn open(directory: &Path, subkey: &str) -> Self {
            let bus = EventBus::new(Limits::standard()).unwrap();
            let events = bus.subscribe(0, Filter::default()).unwrap();
            let watch = FileRegistry::open(
                bus.clone(),
                Id::new("winstrata-test-device").unwrap(),
                directory,
                subkey,
            )
            .unwrap();
            Self {
                watch,
                events,
                _bus: bus,
            }
        }

        fn until(&self, mut found: impl FnMut(&Event) -> bool) -> Vec<Event> {
            let deadline = Instant::now() + Duration::from_secs(8);
            let mut seen = Vec::new();
            while Instant::now() < deadline {
                let batch = self
                    .events
                    .recv_batch(64, Duration::from_millis(200))
                    .unwrap();
                if batch.is_empty() {
                    continue;
                }
                let mut done = false;
                for event in batch {
                    let event = Arc::unwrap_or_clone(event);
                    if found(&event) {
                        done = true;
                    }
                    seen.push(event);
                }
                if done {
                    break;
                }
            }
            seen
        }
    }

    fn text(event: &Event) -> String {
        event.to_json().unwrap()
    }

    fn event_type(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation { event_type, .. } => Some(event_type.as_str()),
            Record::Gap { .. } => None,
        }
    }

    fn evidence(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation {
                attribution: Attribution::Observed { evidence },
                ..
            } => Some(evidence.as_str()),
            _ => None,
        }
    }

    fn collector_error(watch: &mut FileRegistry) -> String {
        match &watch.view()[Channel::Files.index()].failure {
            Some(error) => format!("{} ({})", error.code(), error.detail()),
            None => "no collector error".into(),
        }
    }

    fn summarize(events: &[Event]) -> String {
        if events.is_empty() {
            return "no events".into();
        }
        events
            .iter()
            .map(|event| match &event.record {
                Record::Observation {
                    event_type,
                    subject,
                    ..
                } => format!(
                    "{}:{}",
                    event_type.as_str(),
                    subject.as_ref().map(|item| item.id.as_str()).unwrap_or("-")
                ),
                Record::Gap { reason, .. } => format!("gap:{reason:?}"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn has_subject(event: &Event, fragment: &str) -> bool {
        match &event.record {
            Record::Observation {
                subject: Some(subject),
                ..
            } => subject.id.as_str().contains(fragment),
            _ => false,
        }
    }

    #[test]
    fn windows_file_watch_reports_real_changes_without_content() {
        let id = fresh_id().unwrap();
        let dir = TempDir::new(&format!("winstrata-009-{id}"));
        let subkey = format!(r"Software\WinStrata009{id}");
        fs::write(dir.0.join("before.txt"), SECRET).unwrap();
        let mut harness = Harness::open(&dir.0, &subkey);
        harness.watch.subscribe(Channel::Files).unwrap();
        let again = harness.watch.subscribe(Channel::Files).unwrap();
        assert_eq!(again.subscriptions, 2);
        let quiet = harness
            .events
            .recv_batch(64, Duration::from_millis(250))
            .unwrap();
        assert!(
            quiet
                .iter()
                .all(|event| !text(event).contains("before.txt")),
            "a file created before subscribe was reported"
        );
        fs::write(dir.0.join("observed.txt"), SECRET).unwrap();
        let added = harness.until(|event| {
            has_subject(event, "observed.txt") && event_type(event) == Some("file.added")
        });
        assert!(
            added
                .iter()
                .any(|event| event_type(event) == Some("file.added")),
            "create was not observed; saw {}; {}",
            summarize(&added),
            collector_error(&mut harness.watch)
        );
        let created = added
            .iter()
            .find(|event| text(event).contains("file.added"))
            .unwrap();
        assert!(created.time.precision_ns.is_none());
        assert_eq!(created.source.provider.as_str(), FILE_PROVIDER);
        assert!(!text(created).contains(SECRET));
        match &created.record {
            Record::Observation {
                user,
                attribution: Attribution::Observed { evidence },
                completeness,
                process,
                ..
            } => {
                assert!(process.is_none());
                let security = path_security(&dir.0.join("observed.txt")).unwrap();
                assert_eq!(user.as_ref().map(Id::as_str), Some(security.owner.as_str()));
                match completeness {
                    Completeness::Complete => {
                        assert_eq!(evidence.as_str(), security.sddl.as_deref().unwrap());
                    }
                    Completeness::Partial { reason } => {
                        assert!(
                            matches!(
                                reason.as_str(),
                                "permissions_exceed_identifier" | "permissions_unavailable"
                            ),
                            "unexpected security reason {}",
                            reason.as_str()
                        );
                        assert_eq!(evidence.as_str(), FILE_PROVIDER);
                    }
                }
            }
            _ => panic!("added event was not an observation"),
        }
        fs::rename(dir.0.join("observed.txt"), dir.0.join("renamed.txt")).unwrap();
        let renamed = harness.until(|event| has_subject(event, "renamed.txt"));
        assert!(
            renamed
                .iter()
                .any(|event| text(event).contains("renamed_from")
                    && text(event).contains("observed.txt"))
        );
        assert!(
            renamed.iter().any(
                |event| text(event).contains("renamed_to") && has_subject(event, "renamed.txt")
            )
        );
        fs::remove_file(dir.0.join("renamed.txt")).unwrap();
        let removed = harness.until(|event| text(event).contains("path.removed"));
        assert!(removed.iter().any(|event| {
            text(event).contains("path.removed")
                && text(event).contains("security_unavailable")
                && !text(event).contains(SECRET)
        }));
        fs::create_dir(dir.0.join("nested")).unwrap();
        fs::write(dir.0.join("nested").join("inside.txt"), SECRET).unwrap();
        let nested = harness.until(|event| has_subject(event, "nested"));
        assert!(
            nested
                .iter()
                .all(|event| !text(event).contains("inside.txt"))
        );
        assert!(nested.iter().all(|event| !text(event).contains(SECRET)));
        harness.watch.release(Channel::Files).unwrap();
        fs::write(dir.0.join("still.txt"), "still").unwrap();
        let still = harness.until(|event| has_subject(event, "still.txt"));
        assert!(still.iter().any(|event| has_subject(event, "still.txt")));
        harness.watch.release(Channel::Files).unwrap();
        fs::write(dir.0.join("later.txt"), "later").unwrap();
        let later = harness
            .events
            .recv_batch(64, Duration::from_millis(400))
            .unwrap();
        assert!(later.iter().all(|event| !text(event).contains("later.txt")));
    }

    #[test]
    fn windows_registry_watch_reports_exact_values() {
        let id = fresh_id().unwrap();
        let dir = TempDir::new(&format!("winstrata-009-reg-{id}"));
        let subkey = format!(r"Software\WinStrata009{id}");
        let _cleanup = KeyCleanup(wide_text(&subkey).unwrap());
        let write = WriteKey::create(&subkey);
        let mut harness = Harness::open(&dir.0, &subkey);
        harness.watch.subscribe(Channel::Registry).unwrap();
        let quiet = harness
            .events
            .recv_batch(8, Duration::from_millis(200))
            .unwrap();
        assert!(quiet.is_empty(), "baseline values were reported as changes");
        write.set_dword("Probe", 0x11);
        let probe = format!(r"HKCU\{subkey}\Probe");
        let added = harness.until(|event| {
            event_type(event) == Some("registry.value_added") && has_subject(event, &probe)
        });
        assert!(
            added.iter().any(|event| {
                event_type(event) == Some("registry.value_added")
                    && has_subject(event, &probe)
                    && evidence(event) == Some("t=4;a=11000000")
                    && event.time.precision_ns.is_none()
                    && event.source.provider.as_str() == REGISTRY_PROVIDER
            }),
            "DWORD 0x11 was not reported exactly; saw {}",
            summarize(&added)
        );
        if let Ok(user) = current_id() {
            assert!(added.iter().any(|event| match &event.record {
                Record::Observation {
                    user: Some(seen), ..
                } => seen.as_str() == user,
                _ => false,
            }));
        }
        write.set_dword("Probe", 0x22);
        let changed = harness.until(|event| text(event).contains("t=4;b=11000000;a=22000000"));
        assert!(
            changed
                .iter()
                .any(|event| text(event).contains("registry.value_changed"))
        );
        let child = format!("{subkey}\\child");
        let child_key = WriteKey::create(&child);
        child_key.set_dword("hidden", 7);
        let pause = harness
            .events
            .recv_batch(8, Duration::from_millis(400))
            .unwrap();
        assert!(pause.iter().all(|event| !text(event).contains("hidden")));
        write.delete_value("Probe");
        let removed = harness.until(|event| text(event).contains("registry.value_removed"));
        assert!(removed.iter().any(|event| {
            text(event).contains("t=4;b=22000000") && !text(event).contains("hidden")
        }));
        harness.watch.release(Channel::Registry).unwrap();
        write.set_dword("After", 1);
        let after = harness
            .events
            .recv_batch(8, Duration::from_millis(400))
            .unwrap();
        assert!(after.iter().all(|event| !text(event).contains("After")));
    }

    #[test]
    fn windows_registry_failure_leaves_the_file_watch_running() {
        let id = fresh_id().unwrap();
        let dir = TempDir::new(&format!("winstrata-009-iso-{id}"));
        let subkey = format!(r"Software\WinStrata009Missing{id}");
        let mut harness = Harness::open(&dir.0, &subkey);
        harness.watch.subscribe(Channel::Files).unwrap();
        let error = harness.watch.subscribe(Channel::Registry).unwrap_err();
        assert_eq!(error.code(), "collector_os");
        assert_eq!(
            harness.watch.view()[Channel::Files.index()].phase,
            Phase::Ready
        );
        assert_eq!(
            harness.watch.view()[Channel::Registry.index()].phase,
            Phase::Failed
        );
        fs::write(dir.0.join("alive.txt"), "alive").unwrap();
        let seen = harness.until(|event| has_subject(event, "alive.txt"));
        assert!(
            seen.iter().any(|event| has_subject(event, "alive.txt")),
            "file watch stopped when the registry key was missing; saw {}; {}",
            summarize(&seen),
            collector_error(&mut harness.watch)
        );
        let burst: Vec<_> = (0..40).map(|index| format!("n{index}.txt")).collect();
        for name in &burst {
            fs::write(dir.0.join(name), "x").unwrap();
        }
        fs::write(dir.0.join("tail.txt"), "tail").unwrap();
        let tail = harness.until(|event| has_subject(event, "tail.txt"));
        assert!(
            tail.iter().any(|event| has_subject(event, "tail.txt")),
            "the watch did not recover after a burst"
        );
    }
}

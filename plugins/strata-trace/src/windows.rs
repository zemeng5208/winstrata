//! Windows 用户态文件监视。
//!
//! `ReadDirectoryChangesW` 不提供变更时刻，也不提供执行进程，也不读取文件字节。
//! 缓冲溢出记为丢失缺口，不为这些变更编造逐文件归因。这里不加载驱动。

use std::collections::BTreeMap;
use std::fs::{self, symlink_metadata};
use std::io;
use std::mem::zeroed;
use std::os::windows::ffi::OsStrExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
    WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
    FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SECURITY, FILE_NOTIFY_CHANGE_SIZE,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects,
};
use winstrata_protocols::GapReason;

use crate::{
    FileMeta, Kind, Memory, Notice, NotifyRead, SNAPSHOT_LIMIT, Scope, Session, TraceError,
    classify_notify, decode_file_notifications, file_event, gap_event, incomplete_event,
    name_escapes, next_sequence, paused_generation, publish, record_fault, stream_ids,
};

const BUFFER_WORDS: usize = 4096;

pub(crate) struct StopFlag {
    event: HANDLE,
}

unsafe impl Send for StopFlag {}
unsafe impl Sync for StopFlag {}

impl StopFlag {
    fn new() -> Result<Self, TraceError> {
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(TraceError::Os(unsafe { GetLastError() } as i32));
        }
        Ok(Self { event })
    }

    pub(crate) fn signal(&self) {
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

struct LocalBlock(*mut core::ffi::c_void);

impl Drop for LocalBlock {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}

struct DirectoryIo {
    overlapped: OVERLAPPED,
    // 系统在重叠完成时可能写回这个地址。完成字节数以 GetOverlappedResult 为准。
    transferred: u32,
    buffer: Vec<u32>,
}

struct Watched {
    root: std::path::PathBuf,
    root_text: String,
    directory: WinHandle,
    event: WinHandle,
    io: DirectoryIo,
    baseline: BTreeMap<String, FileMeta>,
    complete: bool,
    pause_seen: Option<u64>,
}

pub(crate) fn spawn(scope: &Scope, memory: Arc<Mutex<Memory>>) -> Result<Session, TraceError> {
    let paths = scope.paths().to_vec();
    let stop = Arc::new(StopFlag::new()?);
    let stop_for_thread = Arc::clone(&stop);
    let memory_for_thread = Arc::clone(&memory);
    let (ready_tx, ready_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("ws-trace".to_owned())
        .spawn(move || {
            let ran = catch_unwind(AssertUnwindSafe(|| {
                watch(&paths, &memory_for_thread, &stop_for_thread, ready_tx)
            }));
            if ran.is_err() {
                record_fault(&memory_for_thread, TraceError::fault("file watch panicked"));
            }
        })
        .map_err(|_| TraceError::fault("file watch did not start"))?;
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(Session {
            memory,
            worker: Some(worker),
            stop: Some(stop),
        }),
        Ok(Err(error)) => {
            stop.signal();
            let _ = worker.join();
            Err(error)
        }
        Err(_) => {
            stop.signal();
            let _ = worker.join();
            Err(TraceError::fault("file watch did not start"))
        }
    }
}

fn watch(
    paths: &[std::path::PathBuf],
    memory: &Mutex<Memory>,
    stop: &StopFlag,
    ready: Sender<Result<(), TraceError>>,
) {
    let mut watched = Vec::with_capacity(paths.len());
    for path in paths {
        match prepare_directory(path) {
            Ok(item) => watched.push(item),
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        }
    }
    // 重叠结构和完成字节数必须停在这个向量里。先发出读取再移动结构，
    // GetOverlappedResult 会读到空副本，并把真实变更报成 source_failure。
    for item in &mut watched {
        if let Err(error) = issue_read(&item.directory, &mut item.io) {
            for armed in &watched {
                cancel(&armed.directory, &armed.io.overlapped);
            }
            let _ = ready.send(Err(error));
            return;
        }
    }
    if ready.send(Ok(())).is_err() {
        for item in &watched {
            cancel(&item.directory, &item.io.overlapped);
        }
        return;
    }
    for item in &mut watched {
        if wait_for_single_object_signaled(item.event.0) {
            // 基线和第一次武装之间的变更无法逐个归因，只记丢失。
            let _ = read_result(&item.directory, &item.io);
            if !publish_gap(memory, GapReason::Lost) {
                return;
            }
            if replace_baseline(item).is_err() {
                record_fault(memory, TraceError::fault("directory snapshot failed"));
                return;
            }
            if issue_read(&item.directory, &mut item.io).is_err() {
                record_fault(memory, TraceError::fault("directory read was not rearmed"));
                return;
            }
        }
    }
    loop {
        let mut handles = Vec::with_capacity(watched.len() + 1);
        handles.push(stop.event);
        for item in &watched {
            handles.push(item.event.0);
        }
        let status =
            unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, INFINITE) };
        if status == WAIT_OBJECT_0 {
            for item in &watched {
                cancel(&item.directory, &item.io.overlapped);
            }
            break;
        }
        if status < WAIT_OBJECT_0 || status >= WAIT_OBJECT_0 + handles.len() as u32 {
            record_fault(memory, TraceError::Os(unsafe { GetLastError() } as i32));
            for item in &watched {
                cancel(&item.directory, &item.io.overlapped);
            }
            break;
        }
        let index = (status - WAIT_OBJECT_0 - 1) as usize;
        let item = &mut watched[index];
        if !handle_ready(memory, item) {
            break;
        }
        if issue_read(&item.directory, &mut item.io).is_err() {
            record_fault(memory, TraceError::fault("directory read was not rearmed"));
            break;
        }
    }
}

fn wait_for_single_object_signaled(event: HANDLE) -> bool {
    unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(event, 0) == WAIT_OBJECT_0 }
}

fn prepare_directory(path: &Path) -> Result<Watched, TraceError> {
    let directory = open_directory(path)?;
    let event = WinHandle(event_handle()?);
    let io = DirectoryIo {
        overlapped: overlapped(event.0),
        transferred: 0,
        buffer: vec![0u32; BUFFER_WORDS],
    };
    let (baseline, complete) = snapshot(path)?;
    let root_text = path
        .to_str()
        .ok_or_else(|| TraceError::fault("invalid trace scope"))?
        .to_owned();
    Ok(Watched {
        root: path.to_path_buf(),
        root_text,
        directory,
        event,
        io,
        baseline,
        complete,
        pause_seen: None,
    })
}

fn handle_ready(memory: &Mutex<Memory>, item: &mut Watched) -> bool {
    let generation = paused_generation(memory);
    let outcome = read_result(&item.directory, &item.io);
    if let Some(generation) = generation
        && item.pause_seen != Some(generation)
    {
        item.pause_seen = Some(generation);
        if !publish_gap(memory, GapReason::Paused) {
            return false;
        }
        if replace_baseline(item).is_err() {
            record_fault(memory, TraceError::fault("directory snapshot failed"));
            return false;
        }
        return match outcome {
            NotifyRead::Aborted => false,
            NotifyRead::Failed(code) => {
                record_fault(memory, TraceError::Os(code));
                false
            }
            _ => true,
        };
    }
    match outcome {
        NotifyRead::Aborted => false,
        NotifyRead::Lost => {
            if !publish_gap(memory, GapReason::Lost) {
                return false;
            }
            if replace_baseline(item).is_err() {
                record_fault(memory, TraceError::fault("directory snapshot failed"));
                return false;
            }
            true
        }
        NotifyRead::Failed(code) => {
            let _ = publish_gap(memory, GapReason::SourceFailure);
            record_fault(memory, TraceError::Os(code));
            false
        }
        NotifyRead::Ready(size) => {
            if !item.complete {
                if !publish_incomplete(memory) {
                    return false;
                }
                if replace_baseline(item).is_err() {
                    record_fault(memory, TraceError::fault("directory snapshot failed"));
                    return false;
                }
                return true;
            }
            let bytes = buffer_bytes(&item.io.buffer, size);
            match decode_file_notifications(bytes) {
                Ok(notices) => apply_notices(memory, item, &notices),
                Err(_) => {
                    record_fault(
                        memory,
                        TraceError::fault("directory notification was truncated"),
                    );
                    publish_gap(memory, GapReason::SourceFailure)
                }
            }
        }
    }
}

fn apply_notices(memory: &Mutex<Memory>, item: &mut Watched, notices: &[Notice]) -> bool {
    for notice in notices {
        let (before, after) = metas_for(item, notice);
        let events = file_event(
            &item.root_text,
            notice,
            before.as_ref(),
            after.as_ref(),
            &stream_ids(memory),
            1,
        );
        remember(item, notice, after);
        for mut event in events {
            let sequence = next_sequence(memory);
            if let Ok(counter) = winstrata_protocols::Counter::new(sequence) {
                event.sequence = counter;
            }
            if !publish(memory, event) {
                return false;
            }
        }
    }
    true
}

fn metas_for(item: &Watched, notice: &Notice) -> (Option<FileMeta>, Option<FileMeta>) {
    match notice {
        Notice::Added(name) | Notice::Modified(name) => {
            (item.baseline.get(name).cloned(), observe(&item.root, name))
        }
        Notice::Removed(name) => (item.baseline.get(name).cloned(), None),
        Notice::Renamed { from, to } => (item.baseline.get(from).cloned(), observe(&item.root, to)),
        Notice::Unpaired { name, old } => {
            if *old {
                (item.baseline.get(name).cloned(), None)
            } else {
                (None, observe(&item.root, name))
            }
        }
        Notice::Undecodable { .. } => (None, None),
    }
}

fn remember(item: &mut Watched, notice: &Notice, after: Option<FileMeta>) {
    match notice {
        Notice::Added(name) | Notice::Modified(name) => {
            if let Some(meta) = after {
                item.baseline.insert(name.clone(), meta);
            }
        }
        Notice::Removed(name) => {
            item.baseline.remove(name);
        }
        Notice::Renamed { from, to } => {
            item.baseline.remove(from);
            if let Some(meta) = after {
                item.baseline.insert(to.clone(), meta);
            }
        }
        Notice::Unpaired { name, old } if *old => {
            item.baseline.remove(name);
        }
        Notice::Unpaired { name, .. } => {
            if let Some(meta) = after {
                item.baseline.insert(name.clone(), meta);
            }
        }
        Notice::Undecodable { .. } => {}
    }
}

fn replace_baseline(item: &mut Watched) -> Result<(), TraceError> {
    let (baseline, complete) = snapshot(&item.root)?;
    item.baseline = baseline;
    item.complete = complete;
    Ok(())
}

fn publish_gap(memory: &Mutex<Memory>, reason: GapReason) -> bool {
    let sequence = next_sequence(memory);
    match gap_event(&stream_ids(memory), sequence, reason) {
        Some(event) => publish(memory, event),
        None => false,
    }
}

fn publish_incomplete(memory: &Mutex<Memory>) -> bool {
    let sequence = next_sequence(memory);
    match incomplete_event(&stream_ids(memory), sequence) {
        Some(event) => publish(memory, event),
        None => false,
    }
}

fn observe(root: &Path, name: &str) -> Option<FileMeta> {
    if name_escapes(name) {
        return None;
    }
    let path = root.join(name);
    let metadata = symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() {
        return Some(FileMeta {
            name: name.to_owned(),
            kind: Kind::Reparse,
            size: 0,
            owner: None,
            sddl: None,
        });
    }
    let kind = if metadata.is_dir() {
        Kind::Directory
    } else {
        Kind::File
    };
    let size = if kind == Kind::File {
        metadata.len()
    } else {
        0
    };
    let security = path_security(&path).ok();
    Some(FileMeta {
        name: name.to_owned(),
        kind,
        size,
        owner: security.as_ref().map(|item| item.0.clone()),
        sddl: security.and_then(|item| item.1),
    })
}

fn snapshot(root: &Path) -> Result<(BTreeMap<String, FileMeta>, bool), TraceError> {
    let mut baseline = BTreeMap::new();
    let mut complete = true;
    let listing =
        fs::read_dir(root).map_err(|error| TraceError::Os(error.raw_os_error().unwrap_or(-1)))?;
    for entry in listing {
        let entry = entry.map_err(|error| TraceError::Os(error.raw_os_error().unwrap_or(-1)))?;
        if baseline.len() == SNAPSHOT_LIMIT {
            complete = false;
            break;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            complete = false;
            break;
        };
        if name.len() > 200 {
            complete = false;
            break;
        }
        if let Some(meta) = observe(root, &name) {
            if baseline.insert(name, meta).is_some() {
                complete = false;
                break;
            }
        }
    }
    Ok((baseline, complete))
}

fn path_security(path: &Path) -> io::Result<(String, Option<String>)> {
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL",
        ));
    }
    name.push(0);
    let mut owner = null_mut();
    let mut descriptor = null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _allocation = LocalBlock(descriptor);
    if owner.is_null() || descriptor.is_null() {
        return Err(io::Error::other("security lookup omitted the owner"));
    }
    let owner = sid_text(owner)?;
    let sddl = sddl_text(descriptor).ok();
    Ok((owner, sddl))
}

fn sid_text(sid: PSID) -> io::Result<String> {
    let mut raw = null_mut();
    let ok = unsafe { ConvertSidToStringSidW(sid, &mut raw) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let _allocation = LocalBlock(raw.cast());
    wide_to_string(raw)
}

fn sddl_text(descriptor: *mut core::ffi::c_void) -> io::Result<String> {
    let mut raw = null_mut();
    let ok = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut raw,
            null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let _allocation = LocalBlock(raw.cast());
    wide_to_string(raw)
}

fn wide_to_string(raw: *const u16) -> io::Result<String> {
    if raw.is_null() {
        return Err(io::Error::other("empty wide string"));
    }
    let mut length = 0usize;
    unsafe {
        while *raw.add(length) != 0 {
            length += 1;
            if length > 1024 {
                return Err(io::Error::other("wide string is too long"));
            }
        }
        String::from_utf16(std::slice::from_raw_parts(raw, length))
            .map_err(|_| io::Error::other("wide string is not unicode"))
    }
}

fn open_directory(root: &Path) -> Result<WinHandle, TraceError> {
    let mut name = wide_path(root).map_err(|_| TraceError::fault("invalid trace scope"))?;
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
        return Err(TraceError::Os(unsafe { GetLastError() } as i32));
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

fn event_handle() -> Result<HANDLE, TraceError> {
    let event = unsafe { CreateEventW(null(), 1, 0, null()) };
    if event.is_null() {
        Err(TraceError::Os(unsafe { GetLastError() } as i32))
    } else {
        Ok(event)
    }
}

fn overlapped(event: HANDLE) -> OVERLAPPED {
    let mut overlapped = unsafe { zeroed::<OVERLAPPED>() };
    overlapped.hEvent = event;
    overlapped
}

fn issue_read(directory: &WinHandle, io: &mut DirectoryIo) -> Result<(), TraceError> {
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
            return Err(TraceError::Os(code as i32));
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
    let size = size as usize;
    if size > bytes.len() {
        return &[];
    }
    &bytes[..size]
}

fn cancel(directory: &WinHandle, overlapped: &OVERLAPPED) {
    unsafe {
        CancelIoEx(directory.0, overlapped);
        let mut transferred = 0;
        GetOverlappedResult(directory.0, overlapped, &mut transferred, 1);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use winstrata_protocols::{Attribution, Completeness, Record};

    use super::*;
    use crate::PLUGIN_ID;
    use crate::PROVIDER;

    #[test]
    fn windows_trace_records_metadata_without_content() {
        let dir = std::env::temp_dir().join(format!("winstrata-021-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let scope = Scope::from_paths(std::slice::from_ref(&dir)).unwrap();
        let mut session = Session::start(&scope, 1, None).unwrap();
        let secret = b"SECRET-CONTENT-021-DO-NOT-STORE";
        let file = dir.join("observed.txt");
        fs::write(&file, secret).unwrap();
        let added = wait_type(&session, "file.added");
        assert!(
            added
                .iter()
                .any(|event| event_type(event) == Some("file.added")),
            "create was not observed: {}; {}",
            summarize(&added),
            session
                .fault_text()
                .unwrap_or_else(|| "no collector fault".to_owned())
        );
        let created = added
            .iter()
            .find(|event| event_type(event) == Some("file.added"))
            .unwrap();
        assert!(!created.to_json().unwrap().contains("SECRET-CONTENT-021"));
        assert!(created.time.precision_ns.is_none());
        assert_eq!(created.source.provider.as_str(), PROVIDER);
        assert_eq!(created.source.device.as_str(), PLUGIN_ID);
        assert_provenance(created);
        fs::write(&file, b"SECRET-CONTENT-021-DO-NOT-STORE-MORE").unwrap();
        let modified = wait_type(&session, "file.modified");
        let changed = modified
            .iter()
            .find(|event| event_type(event) == Some("file.modified"))
            .unwrap_or_else(|| panic!("size change was not observed: {}", summarize(&modified)));
        let text = evidence(changed).unwrap_or("");
        assert!(text.contains("sb="), "{text}");
        assert!(text.contains("sa="), "{text}");
        assert!(!changed.to_json().unwrap().contains("SECRET-CONTENT-021"));
        assert_provenance(changed);
        fs::remove_file(&file).unwrap();
        let removed = wait_type(&session, "path.removed");
        assert!(
            removed
                .iter()
                .any(|event| event_type(event) == Some("path.removed")),
            "removal was not observed: {}",
            summarize(&removed)
        );
        session.stop();
        let _ = fs::remove_dir_all(&dir);
    }

    fn wait_type(session: &Session, kind: &str) -> Vec<crate::Event> {
        let started = std::time::Instant::now();
        loop {
            let events = session.query();
            if events.iter().any(|event| event_type(event) == Some(kind))
                || started.elapsed() > Duration::from_secs(5)
            {
                return events;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    fn event_type(event: &crate::Event) -> Option<&str> {
        match &event.record {
            Record::Observation { event_type, .. } => Some(event_type.as_str()),
            Record::Gap { .. } => None,
        }
    }

    fn evidence(event: &crate::Event) -> Option<&str> {
        match &event.record {
            Record::Observation {
                attribution: Attribution::Observed { evidence },
                ..
            } => Some(evidence.as_str()),
            _ => None,
        }
    }

    fn assert_provenance(event: &crate::Event) {
        match &event.record {
            Record::Observation {
                process,
                completeness,
                ..
            } => {
                assert!(process.is_none());
                assert!(
                    matches!(
                        completeness,
                        Completeness::Complete | Completeness::Partial { .. }
                    ),
                    "{completeness:?}"
                );
            }
            other => panic!("unexpected record {other:?}"),
        }
    }

    fn summarize(events: &[crate::Event]) -> String {
        if events.is_empty() {
            return "no events".into();
        }
        events
            .iter()
            .map(|event| match &event.record {
                Record::Observation { event_type, .. } => event_type.as_str().to_owned(),
                Record::Gap { reason, .. } => format!("gap:{reason:?}"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

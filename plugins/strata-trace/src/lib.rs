#![cfg_attr(not(windows), allow(dead_code))]

//! StrataTrace 观察目录中的文件变更、路径和权限元数据。
//!
//! 监视留在普通用户进程中。不加载 `strata-fs.sys`，也不读取文件字节。
//! 这些 API 不报告变更时刻的精度，也不报告执行进程，因此省略 `precision_ns` 和进程字段。
//! 大小和权限的前后值是用户态元数据，不是内核归因。
//! Linux 不能观察 Windows 文件变更，启动返回 `unsupported`，不生成事件。

use std::collections::{BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, ObjectRef, Record,
    SignedNanos, Source, Version,
};

#[cfg(windows)]
mod windows;

pub const PLUGIN_ID: &str = "strata.trace";
pub const PROVIDER: &str = "windows.read_directory_changes";
pub const LINK_PROVIDER: &str = "winstrata.trace";

const MAX_DIRECTORIES: usize = 4;
const MAX_PATH_BYTES: usize = 200;
const QUERY_LIMIT: usize = 128;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const SNAPSHOT_LIMIT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceError {
    Unsupported,
    Fault(String),
    Os(i32),
}

impl TraceError {
    pub fn fault(detail: &str) -> Self {
        Self::Fault(safe_detail(detail))
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Fault(_) => "collector_fault",
            Self::Os(_) => "collector_os",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Unsupported => "This host cannot observe file changes".to_owned(),
            Self::Fault(detail) => detail.clone(),
            Self::Os(code) => format!("Collector reported OS code {code}"),
        }
    }
}

fn safe_detail(detail: &str) -> String {
    if detail.is_empty()
        || detail.len() > 160
        || detail.trim() != detail
        || detail.chars().any(char::is_control)
    {
        "unspecified collector fault".to_owned()
    } else {
        detail.to_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    paths: Vec<PathBuf>,
}

impl Scope {
    pub fn from_paths(paths: &[PathBuf]) -> Result<Self, TraceError> {
        if paths.is_empty() || paths.len() > MAX_DIRECTORIES {
            return Err(TraceError::fault("invalid trace scope"));
        }
        let mut stored: Vec<PathBuf> = Vec::with_capacity(paths.len());
        for path in paths {
            if !valid_directory(path) {
                return Err(TraceError::fault("invalid trace scope"));
            }
            if stored.iter().any(|existing| same_directory(existing, path)) {
                return Err(TraceError::fault("invalid trace scope"));
            }
            stored.push(path.clone());
        }
        Ok(Self { paths: stored })
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

fn valid_directory(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if text.is_empty()
        || text.len() > MAX_PATH_BYTES
        || !path.is_absolute()
        || text.chars().any(char::is_control)
    {
        return false;
    }
    path.components()
        .all(|part| !matches!(part, Component::ParentDir | Component::CurDir))
}

fn same_directory(left: &Path, right: &Path) -> bool {
    match (left.to_str(), right.to_str()) {
        (Some(left), Some(right)) => {
            #[cfg(windows)]
            {
                left.eq_ignore_ascii_case(right)
            }
            #[cfg(not(windows))]
            {
                left == right
            }
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    File,
    Directory,
    Reparse,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileMeta {
    pub name: String,
    pub kind: Kind,
    pub size: u64,
    pub owner: Option<String>,
    pub sddl: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    Added(String),
    Removed(String),
    Modified(String),
    Renamed { from: String, to: String },
    Unpaired { name: String, old: bool },
    Undecodable { action: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignObservation {
    pub device: String,
    pub provider: String,
    pub subject: Option<String>,
    pub evidence: Option<String>,
}

pub(crate) struct StreamIds {
    device: Id,
    provider: Id,
    epoch: Id,
}

struct Memory {
    sequence: u64,
    ring: VecDeque<Event>,
    paused: bool,
    pause_generation: u64,
    #[cfg_attr(not(windows), allow(dead_code))]
    fault: Option<TraceError>,
    on_event: Option<Arc<dyn Fn(&Event) -> bool + Send + Sync>>,
    stream: StreamIds,
}

fn lock(memory: &Mutex<Memory>) -> MutexGuard<'_, Memory> {
    memory.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub(crate) fn unix_ns() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX),
        Err(error) => -i64::try_from(error.duration().as_nanos()).unwrap_or(i64::MAX),
    }
}

pub(crate) const ERROR_NOTIFY_ENUM_DIR: i32 = 1022;
pub(crate) const ERROR_OPERATION_ABORTED: i32 = 995;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotifyRead {
    Ready(u32),
    Lost,
    Aborted,
    Failed(i32),
}

pub(crate) fn classify_notify(code: i32, transferred: u32) -> NotifyRead {
    if code == ERROR_OPERATION_ABORTED {
        NotifyRead::Aborted
    } else if code == ERROR_NOTIFY_ENUM_DIR || (code == 0 && transferred == 0) {
        NotifyRead::Lost
    } else if code == 0 {
        NotifyRead::Ready(transferred)
    } else {
        NotifyRead::Failed(code)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DecodeError;

pub(crate) fn decode_file_notifications(bytes: &[u8]) -> Result<Vec<Notice>, DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError);
    }
    let mut notices = Vec::new();
    let mut pending_old: Option<Result<String, ()>> = None;
    let mut offset = 0usize;
    loop {
        let rest = bytes.len().checked_sub(offset).ok_or(DecodeError)?;
        if rest < 12 {
            return Err(DecodeError);
        }
        let next = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let action = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let name_bytes =
            u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()) as usize;
        let name_end = offset
            .checked_add(12)
            .and_then(|start| start.checked_add(name_bytes))
            .ok_or(DecodeError)?;
        if name_bytes % 2 != 0 || name_end > bytes.len() {
            return Err(DecodeError);
        }
        let name = utf16_name(&bytes[offset + 12..name_end]);
        if action != 5 {
            flush_pending(&mut notices, &mut pending_old);
        }
        match action {
            4 => pending_old = Some(name),
            5 => {
                if let Some(old) = pending_old.take() {
                    match (old, name) {
                        (Ok(from), Ok(to)) => notices.push(Notice::Renamed { from, to }),
                        _ => notices.push(Notice::Undecodable { action }),
                    }
                } else {
                    push_named(&mut notices, action, name, true);
                }
            }
            _ => push_named(&mut notices, action, name, false),
        }
        if next == 0 {
            break;
        }
        let step = next as usize;
        if step < 12 || offset.checked_add(step).is_none_or(|end| end > bytes.len()) {
            return Err(DecodeError);
        }
        offset += step;
        if offset == bytes.len() {
            break;
        }
    }
    flush_pending(&mut notices, &mut pending_old);
    Ok(notices)
}

fn utf16_name(bytes: &[u8]) -> Result<String, ()> {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect();
    String::from_utf16(&units).map_err(|_| ())
}

fn push_named(
    notices: &mut Vec<Notice>,
    action: u32,
    name: Result<String, ()>,
    unpaired_new: bool,
) {
    let Ok(name) = name else {
        notices.push(Notice::Undecodable { action });
        return;
    };
    notices.push(match action {
        1 => Notice::Added(name),
        2 => Notice::Removed(name),
        3 => Notice::Modified(name),
        5 if unpaired_new => Notice::Unpaired { name, old: false },
        _ => Notice::Undecodable { action },
    });
}

fn flush_pending(notices: &mut Vec<Notice>, pending: &mut Option<Result<String, ()>>) {
    if let Some(old) = pending.take() {
        match old {
            Ok(name) => notices.push(Notice::Unpaired { name, old: true }),
            Err(()) => notices.push(Notice::Undecodable { action: 4 }),
        }
    }
}

pub(crate) fn name_escapes(name: &str) -> bool {
    name.is_empty()
        || Path::new(name).is_absolute()
        || name
            .split(['\\', '/'])
            .any(|part| part.is_empty() || part == "." || part == "..")
}

pub(crate) fn metadata_evidence(
    before: Option<&FileMeta>,
    after: Option<&FileMeta>,
) -> (String, Option<&'static str>) {
    let sb = size_token(before);
    let sa = size_token(after);
    let changed = permission_changed(before, after);
    let mut text = format!("sb={sb};sa={sa};pd={}", u8::from(changed));
    if let (Some(before), Some(after)) = (before, after)
        && before.owner != after.owner
        && let (Some(ob), Some(oa)) = (before.owner.as_deref(), after.owner.as_deref())
    {
        let with_owners = format!("{text};ob={ob};oa={oa}");
        if Id::new(&with_owners).is_ok() {
            text = with_owners;
        }
    }
    let sddl = after
        .and_then(|item| item.sddl.clone())
        .or_else(|| before.and_then(|item| item.sddl.clone()));
    if let Some(sddl) = sddl {
        let with_sd = format!("{text};sd={sddl}");
        if Id::new(&with_sd).is_ok() {
            return (with_sd, None);
        }
        if Id::new(&text).is_ok() {
            return (text, Some("permissions_exceed_identifier"));
        }
    } else if Id::new(&text).is_ok() {
        let partial = if before.is_none() && after.is_some() {
            Some("permissions_unavailable")
        } else if before.is_some() && after.is_none() {
            Some("security_unavailable")
        } else {
            Some("permissions_unavailable")
        };
        return (text, partial);
    }
    (PROVIDER.to_owned(), Some("metadata_exceeds_identifier"))
}

fn size_token(meta: Option<&FileMeta>) -> String {
    meta.map(|item| item.size.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn permission_changed(before: Option<&FileMeta>, after: Option<&FileMeta>) -> bool {
    match (before, after) {
        (Some(before), Some(after)) => before.owner != after.owner || before.sddl != after.sddl,
        _ => true,
    }
}

pub(crate) fn events_for(
    root: &str,
    notice: &Notice,
    before: Option<&FileMeta>,
    after: Option<&FileMeta>,
    stream: &StreamIds,
    mut sequence: u64,
) -> Vec<Event> {
    let mut events = Vec::new();
    match notice {
        Notice::Added(name) | Notice::Modified(name) => {
            let action = if matches!(notice, Notice::Added(_)) {
                "added"
            } else {
                "modified"
            };
            if let Some(event) = live_event(root, name, action, before, after, stream, sequence) {
                events.push(event);
            }
        }
        Notice::Removed(name) => {
            if let Some(event) = removal_event(root, name, before, stream, sequence) {
                events.push(event);
            }
        }
        Notice::Renamed { from, to } => {
            if let Some(event) = removal_event(root, from, before, stream, sequence) {
                events.push(rename_type(event, "path.renamed_from"));
            }
            sequence = sequence.saturating_add(1);
            if let Some(event) = live_event(root, to, "renamed_to", None, after, stream, sequence) {
                events.push(event);
            }
        }
        Notice::Unpaired { name, old } => {
            let event_type = if *old {
                "path.renamed_from"
            } else {
                "path.renamed_to"
            };
            if let Some(event) =
                partial_path(root, name, event_type, "rename_unpaired", stream, sequence)
            {
                events.push(event);
            }
        }
        Notice::Undecodable { action } => {
            if let Some(event) = bare_event(
                "path.unreadable",
                None,
                None,
                PROVIDER,
                Some(&format!("name_not_unicode_{action}")),
                stream,
                sequence,
            ) {
                events.push(event);
            }
        }
    }
    events
}

fn rename_type(mut event: Event, event_type: &str) -> Event {
    if let Record::Observation {
        event_type: slot, ..
    } = &mut event.record
        && let Ok(next) = Id::new(event_type)
    {
        *slot = next;
    }
    event
}

fn live_event(
    root: &str,
    name: &str,
    action: &str,
    before: Option<&FileMeta>,
    after: Option<&FileMeta>,
    stream: &StreamIds,
    sequence: u64,
) -> Option<Event> {
    if name_escapes(name) {
        return partial_path(
            root,
            name,
            &format!("path.{action}"),
            "path_escapes_root",
            stream,
            sequence,
        );
    }
    let Some(after) = after else {
        return partial_path(
            root,
            name,
            &format!("path.{action}"),
            "metadata_unavailable",
            stream,
            sequence,
        );
    };
    let (event_type, mut partial) = match after.kind {
        Kind::File => (format!("file.{action}"), None),
        Kind::Directory => (format!("directory.{action}"), None),
        Kind::Reparse => (format!("path.{action}"), Some("reparse_unresolved")),
    };
    if after.kind == Kind::Reparse {
        return partial_path(
            root,
            name,
            &event_type,
            partial.unwrap_or("reparse_unresolved"),
            stream,
            sequence,
        );
    }
    let (evidence, evidence_partial) = metadata_evidence(before, Some(after));
    if partial.is_none() {
        partial = evidence_partial;
    }
    let owner = after
        .owner
        .as_deref()
        .filter(|owner| Id::new(*owner).is_ok());
    if partial.is_none() && after.owner.is_some() && owner.is_none() {
        partial = Some("owner_unavailable");
    }
    path_event(
        root,
        name,
        &event_type,
        owner,
        &evidence,
        partial,
        stream,
        sequence,
    )
}

fn removal_event(
    root: &str,
    name: &str,
    before: Option<&FileMeta>,
    stream: &StreamIds,
    sequence: u64,
) -> Option<Event> {
    if name_escapes(name) {
        return partial_path(
            root,
            name,
            "path.removed",
            "path_escapes_root",
            stream,
            sequence,
        );
    }
    let (evidence, mut partial) = metadata_evidence(before, None);
    if partial.is_none() {
        partial = Some("security_unavailable");
    }
    // 删除之后没有当前所有者。之前的 SDDL 只在放得进标识符时留在证据里。
    path_event(
        root,
        name,
        "path.removed",
        None,
        &evidence,
        partial,
        stream,
        sequence,
    )
}

fn partial_path(
    root: &str,
    name: &str,
    event_type: &str,
    partial: &str,
    stream: &StreamIds,
    sequence: u64,
) -> Option<Event> {
    path_event(
        root,
        name,
        event_type,
        None,
        PROVIDER,
        Some(partial),
        stream,
        sequence,
    )
}

fn path_event(
    root: &str,
    name: &str,
    event_type: &str,
    user: Option<&str>,
    evidence: &str,
    mut partial: Option<&str>,
    stream: &StreamIds,
    sequence: u64,
) -> Option<Event> {
    let full = join_root(root, name);
    let (subject, path_partial) = match Id::new(&full) {
        Ok(id) => (Some(id.as_str().to_owned()), None),
        Err(_) => match Id::new(name) {
            Ok(id) => (
                Some(id.as_str().to_owned()),
                Some("absolute_path_exceeds_identifier"),
            ),
            Err(_) => (None, Some("path_exceeds_identifier")),
        },
    };
    if partial.is_none() {
        partial = path_partial;
    }
    let subject = subject.map(|id| (subject_kind(event_type), id));
    bare_event(
        event_type,
        subject.as_ref().map(|(kind, id)| (*kind, id.as_str())),
        user,
        evidence,
        partial,
        stream,
        sequence,
    )
}

fn join_root(root: &str, name: &str) -> String {
    if root.ends_with('\\') || root.ends_with('/') {
        format!("{root}{name}")
    } else if root.contains('\\') {
        format!("{root}\\{name}")
    } else {
        format!("{root}/{name}")
    }
}

fn subject_kind(event_type: &str) -> &'static str {
    if event_type.starts_with("file.") {
        "file"
    } else if event_type.starts_with("directory.") {
        "directory"
    } else {
        "path"
    }
}

fn bare_event(
    event_type: &str,
    subject: Option<(&str, &str)>,
    user: Option<&str>,
    evidence: &str,
    partial: Option<&str>,
    stream: &StreamIds,
    sequence: u64,
) -> Option<Event> {
    let evidence = Id::new(evidence).ok()?;
    let user = user.and_then(|text| Id::new(text).ok());
    let partial = match partial {
        Some(reason) => Some(Id::new(reason).ok()?),
        None => None,
    };
    let subject = match subject {
        Some((kind, id)) => Some(ObjectRef {
            kind: Id::new(kind).ok()?,
            id: Id::new(id).ok()?,
        }),
        None => None,
    };
    Some(Event {
        version: Version::CURRENT,
        source: Source {
            device: stream.device.clone(),
            provider: stream.provider.clone(),
            epoch: stream.epoch.clone(),
        },
        sequence: Counter::new(sequence).ok()?,
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns()),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new(event_type).ok()?,
            subject,
            user,
            process: None,
            attribution: Attribution::Observed { evidence },
            completeness: match partial {
                Some(reason) => Completeness::Partial { reason },
                None => Completeness::Complete,
            },
        },
    })
}

pub(crate) fn file_event(
    root: &str,
    notice: &Notice,
    before: Option<&FileMeta>,
    after: Option<&FileMeta>,
    stream: &StreamIds,
    sequence: u64,
) -> Vec<Event> {
    events_for(root, notice, before, after, stream, sequence)
}

#[cfg_attr(not(test), allow(dead_code))]
fn event_type_is(event: &Event, expected: &str) -> bool {
    match &event.record {
        Record::Observation { event_type, .. } => event_type.as_str() == expected,
        Record::Gap { .. } => false,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn gap_event(stream: &StreamIds, sequence: u64, reason: GapReason) -> Option<Event> {
    Some(Event {
        version: Version::CURRENT,
        source: Source {
            device: stream.device.clone(),
            provider: stream.provider.clone(),
            epoch: stream.epoch.clone(),
        },
        sequence: Counter::new(sequence).ok()?,
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns()),
            precision_ns: None,
        },
        record: Record::Gap {
            reason,
            range: None,
        },
    })
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn incomplete_event(stream: &StreamIds, sequence: u64) -> Option<Event> {
    bare_event(
        "directory.changed",
        None,
        None,
        PROVIDER,
        Some("snapshot_incomplete"),
        stream,
        sequence,
    )
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn emit(memory: &Mutex<Memory>, event: Event) -> bool {
    let callback = {
        let mut guard = lock(memory);
        if guard.ring.len() == QUERY_LIMIT {
            guard.ring.pop_front();
        }
        guard.ring.push_back(event.clone());
        guard.on_event.clone()
    };
    match callback {
        Some(callback) => callback(&event),
        None => true,
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn publish(memory: &Mutex<Memory>, event: Event) -> bool {
    emit(memory, event)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn next_sequence(memory: &Mutex<Memory>) -> u64 {
    let mut guard = lock(memory);
    guard.sequence = guard.sequence.saturating_add(1);
    guard.sequence
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn stream_ids(memory: &Mutex<Memory>) -> StreamIds {
    let guard = lock(memory);
    StreamIds {
        device: guard.stream.device.clone(),
        provider: guard.stream.provider.clone(),
        epoch: guard.stream.epoch.clone(),
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn paused_generation(memory: &Mutex<Memory>) -> Option<u64> {
    let guard = lock(memory);
    guard.paused.then_some(guard.pause_generation)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn record_fault(memory: &Mutex<Memory>, error: TraceError) {
    lock(memory).fault = Some(error);
}

/// 按精确路径投影历史。时间重叠不会生成关联。
///
/// `trace.path_shared` 只表示另一来源的主体或证据与该路径相同。
/// 它不写入进程字段，也不表示该进程造成了这次文件变更。
pub fn project_history(
    path: &str,
    live: &[Event],
    baseline_paths: &[String],
    foreign: &[ForeignObservation],
) -> Vec<Event> {
    let mut out = Vec::new();
    for event in live {
        if subject_equals(event, path) {
            out.push(event.clone());
        }
    }
    let Ok(path_id) = Id::new(path) else {
        return out;
    };
    let stream = match projection_stream() {
        Some(stream) => stream,
        None => return out,
    };
    let mut sequence = 1u64;
    if baseline_paths.iter().any(|item| item == path)
        && let Some(event) = bare_event(
            "trace.reconstructed",
            Some(("path", path_id.as_str())),
            None,
            "state=reconstructed",
            None,
            &stream,
            sequence,
        )
    {
        out.push(event);
        sequence = sequence.saturating_add(1);
    }
    let mut seen = BTreeSet::new();
    for item in foreign {
        let hit = item.subject.as_deref() == Some(path) || item.evidence.as_deref() == Some(path);
        if !hit || item.device.is_empty() || !seen.insert(item.device.clone()) {
            continue;
        }
        let evidence = format!("with={}", item.device);
        if Id::new(&evidence).is_err() {
            continue;
        }
        if let Some(event) = bare_event(
            "trace.path_shared",
            Some(("path", path_id.as_str())),
            None,
            &evidence,
            None,
            &stream,
            sequence,
        ) {
            out.push(event);
            sequence = sequence.saturating_add(1);
        }
    }
    out
}

fn subject_equals(event: &Event, path: &str) -> bool {
    match &event.record {
        Record::Observation {
            subject: Some(subject),
            ..
        } => subject.id.as_str() == path,
        _ => false,
    }
}

fn projection_stream() -> Option<StreamIds> {
    Some(StreamIds {
        device: Id::new(PLUGIN_ID).ok()?,
        provider: Id::new(LINK_PROVIDER).ok()?,
        epoch: Id::new("trace-q").ok()?,
    })
}

pub struct Session {
    memory: Arc<Mutex<Memory>>,
    worker: Option<JoinHandle<()>>,
    #[cfg(windows)]
    stop: Option<Arc<windows::StopFlag>>,
}

impl Session {
    pub fn start(
        scope: &Scope,
        epoch: u64,
        on_event: Option<Arc<dyn Fn(&Event) -> bool + Send + Sync>>,
    ) -> Result<Self, TraceError> {
        if epoch == 0 || scope.paths.is_empty() {
            return Err(TraceError::fault("invalid trace scope"));
        }
        let memory = Arc::new(Mutex::new(Memory {
            sequence: 0,
            ring: VecDeque::new(),
            paused: false,
            pause_generation: 0,
            fault: None,
            on_event,
            stream: StreamIds {
                device: Id::new(PLUGIN_ID)
                    .map_err(|_| TraceError::fault("invalid trace device"))?,
                provider: Id::new(PROVIDER)
                    .map_err(|_| TraceError::fault("invalid trace provider"))?,
                epoch: Id::new(format!("fs-{epoch}"))
                    .map_err(|_| TraceError::fault("invalid trace epoch"))?,
            },
        }));
        #[cfg(windows)]
        {
            windows::spawn(scope, memory)
        }
        #[cfg(not(windows))]
        {
            let _ = memory;
            Err(TraceError::Unsupported)
        }
    }

    pub fn query(&self) -> Vec<Event> {
        lock(&self.memory).ring.iter().cloned().collect()
    }

    pub fn pause(&self) -> Result<(), TraceError> {
        let mut memory = lock(&self.memory);
        memory.paused = true;
        memory.pause_generation = memory.pause_generation.saturating_add(1);
        Ok(())
    }

    pub fn resume(&self) -> Result<(), TraceError> {
        lock(&self.memory).paused = false;
        Ok(())
    }

    pub fn stop(&mut self) {
        #[cfg(windows)]
        if let Some(stop) = self.stop.take() {
            stop.signal();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream() -> StreamIds {
        StreamIds {
            device: Id::new(PLUGIN_ID).unwrap(),
            provider: Id::new(PROVIDER).unwrap(),
            epoch: Id::new("fs-1").unwrap(),
        }
    }

    fn meta(name: &str, size: u64, owner: &str, sddl: &str) -> FileMeta {
        FileMeta {
            name: name.to_owned(),
            kind: Kind::File,
            size,
            owner: Some(owner.to_owned()),
            sddl: Some(sddl.to_owned()),
        }
    }

    fn evidence(event: &Event) -> &str {
        match &event.record {
            Record::Observation {
                attribution: Attribution::Observed { evidence },
                ..
            } => evidence.as_str(),
            other => panic!("expected observation, got {other:?}"),
        }
    }

    fn abs(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn overflow_and_rename_decode_without_reading_bytes() {
        assert_eq!(classify_notify(ERROR_NOTIFY_ENUM_DIR, 0), NotifyRead::Lost);
        assert_eq!(classify_notify(0, 0), NotifyRead::Lost);
        assert_eq!(
            classify_notify(ERROR_OPERATION_ABORTED, 0),
            NotifyRead::Aborted
        );
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&14u32.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&u16::from(b'a').to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&u16::from(b'b').to_le_bytes());
        let notices = decode_file_notifications(&bytes).unwrap();
        assert_eq!(
            notices,
            vec![Notice::Renamed {
                from: "a".into(),
                to: "b".into()
            }]
        );
    }

    #[test]
    fn scope_rejects_relative_parent_duplicate_and_overflow() {
        assert!(Scope::from_paths(&[]).is_err());
        assert!(Scope::from_paths(&[PathBuf::from("relative")]).is_err());
        let parent = {
            let mut path = abs("winstrata-trace-parent");
            path.push("..");
            path.push("hidden");
            path
        };
        let error = Scope::from_paths(&[parent]).unwrap_err();
        assert_eq!(error.detail(), "invalid trace scope");
        assert!(!error.detail().contains("hidden"));
        let first = abs("winstrata-trace-a");
        assert!(Scope::from_paths(&[first.clone(), first]).is_err());
        let many: Vec<_> = (0..5)
            .map(|index| abs(&format!("winstrata-trace-{index}")))
            .collect();
        assert!(Scope::from_paths(&many).is_err());
        let control = abs("winstrata-trace-ok");
        let text = format!("{}\nbad", control.display());
        assert!(Scope::from_paths(&[PathBuf::from(text)]).is_err());
        let accepted = Scope::from_paths(&[abs("winstrata-trace-ok")]).unwrap();
        assert_eq!(accepted.paths().len(), 1);
    }

    #[test]
    fn modify_records_size_and_permission_delta_without_content() {
        let before = meta("notes.txt", 4, "S-1-5-18", "D:P");
        let after = meta("notes.txt", 9, "S-1-5-18", "D:PA");
        let secret = "SECRET-CONTENT-021";
        let events = file_event(
            "/tmp/scope",
            &Notice::Modified("notes.txt".into()),
            Some(&before),
            Some(&after),
            &stream(),
            1,
        );
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(evidence(event), "sb=4;sa=9;pd=1;sd=D:PA");
        assert!(!event.to_json().unwrap().contains(secret));
        assert!(event.time.precision_ns.is_none());
        match &event.record {
            Record::Observation {
                event_type,
                subject,
                user,
                process,
                completeness: Completeness::Complete,
                ..
            } => {
                assert_eq!(event_type.as_str(), "file.modified");
                assert_eq!(
                    subject.as_ref().unwrap().id.as_str(),
                    "/tmp/scope/notes.txt"
                );
                assert_eq!(user.as_ref().unwrap().as_str(), "S-1-5-18");
                assert!(process.is_none());
            }
            other => panic!("unexpected record {other:?}"),
        }
        assert_eq!(event.source.provider.as_str(), PROVIDER);
        assert_eq!(event.source.device.as_str(), PLUGIN_ID);
    }

    #[test]
    fn long_sddl_is_not_truncated() {
        let sddl = "D:".to_owned() + &"A".repeat(300);
        let before = meta("a.txt", 1, "S-1-5-18", "D:P");
        let mut after = meta("a.txt", 2, "S-1-5-18", &sddl);
        after.sddl = Some(sddl.clone());
        let (text, partial) = metadata_evidence(Some(&before), Some(&after));
        assert_eq!(partial, Some("permissions_exceed_identifier"));
        assert!(!text.contains(&sddl));
        assert!(text.starts_with("sb=1;sa=2;pd=1"));
        assert!(Id::new(&text).is_ok());
    }

    #[test]
    fn removal_does_not_invent_an_owner() {
        let before = meta("gone.txt", 3, "S-1-5-18", "D:P");
        let events = file_event(
            "/tmp/scope",
            &Notice::Removed("gone.txt".into()),
            Some(&before),
            None,
            &stream(),
            2,
        );
        let event = &events[0];
        assert_eq!(evidence(event), "sb=3;sa=-;pd=1;sd=D:P");
        match &event.record {
            Record::Observation {
                event_type,
                user,
                process,
                completeness: Completeness::Partial { reason },
                ..
            } => {
                assert_eq!(event_type.as_str(), "path.removed");
                assert!(user.is_none());
                assert!(process.is_none());
                assert_eq!(reason.as_str(), "security_unavailable");
            }
            other => panic!("unexpected record {other:?}"),
        }
    }

    #[test]
    fn escaped_name_is_not_joined() {
        let events = file_event(
            "/tmp/scope",
            &Notice::Added("../outside.txt".into()),
            None,
            None,
            &stream(),
            3,
        );
        let event = &events[0];
        match &event.record {
            Record::Observation {
                completeness: Completeness::Partial { reason },
                ..
            } => assert_eq!(reason.as_str(), "path_escapes_root"),
            other => panic!("unexpected record {other:?}"),
        }
        assert!(!evidence(event).contains("outside"));
    }

    #[test]
    fn history_links_exact_paths_only() {
        let before = meta("notes.txt", 1, "S-1-5-18", "D:P");
        let after = meta("notes.txt", 2, "S-1-5-18", "D:P");
        let live = file_event(
            "/tmp/scope",
            &Notice::Modified("notes.txt".into()),
            Some(&before),
            Some(&after),
            &stream(),
            4,
        );
        let foreign = vec![
            ForeignObservation {
                device: "strata.process".into(),
                provider: "winstrata.process".into(),
                subject: Some("/tmp/scope/notes.txt".into()),
                evidence: Some("toolhelp".into()),
            },
            ForeignObservation {
                device: "strata.registry".into(),
                provider: "windows.reg_notify".into(),
                subject: Some(r"HKCU\Software\Example".into()),
                evidence: Some("t=4;a=01".into()),
            },
            ForeignObservation {
                device: "strata.process".into(),
                provider: "winstrata.process".into(),
                subject: Some("/tmp/scope/other.txt".into()),
                evidence: None,
            },
        ];
        let history = project_history(
            "/tmp/scope/notes.txt",
            &live,
            &["/tmp/scope/notes.txt".to_owned()],
            &foreign,
        );
        assert!(
            history
                .iter()
                .any(|event| event_type_is(event, "file.modified"))
        );
        assert!(history.iter().any(|event| {
            event_type_is(event, "trace.reconstructed") && evidence(event) == "state=reconstructed"
        }));
        let shared: Vec<_> = history
            .iter()
            .filter(|event| event_type_is(event, "trace.path_shared"))
            .collect();
        assert_eq!(shared.len(), 1);
        assert_eq!(evidence(shared[0]), "with=strata.process");
        assert_eq!(shared[0].source.provider.as_str(), LINK_PROVIDER);
        match &shared[0].record {
            Record::Observation { process, .. } => assert!(process.is_none()),
            other => panic!("unexpected record {other:?}"),
        }
        assert!(shared[0].time.precision_ns.is_none());
        assert!(!history.iter().any(|event| evidence(event).contains("HKCU")));
    }

    #[cfg(not(windows))]
    #[test]
    fn linux_start_is_unsupported() {
        let scope = Scope::from_paths(&[abs("winstrata-trace-unused")]).unwrap();
        match Session::start(&scope, 1, None) {
            Err(error) => assert_eq!(error.code(), "unsupported"),
            Ok(_) => panic!("linux start must be unsupported"),
        }
    }
}

//! StrataEnv 观察当前用户和系统环境变量，包括 PATH。
//!
//! 这不是第二个注册表监视器。它只读取两个环境存储，主体是 `user\{name}` 或
//! `system\{name}`，不是注册表路径。监视留在普通用户进程中，使用已经链接的
//! Windows API，不加载驱动。Linux 不能观察这些存储，启动返回 `unsupported`，
//! 不生成事件。这些 API 不报告变更时刻的精度，也不报告执行进程，因此省略
//! `precision_ns` 和进程字段。

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, ObjectRef, Record,
    SignedNanos, Source, Version,
};

#[cfg(windows)]
mod windows;

pub const PLUGIN_ID: &str = "strata.env";
pub const PROVIDER: &str = "windows.env_notify";

const QUERY_LIMIT: usize = 128;
const SUBJECT_KIND: &str = "environment";
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const VALUE_LIMIT: usize = 256;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const NAME_CHARS: usize = 256;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const DATA_BYTES: usize = 32768;

const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvError {
    Unsupported,
    Fault(String),
    Os(i32),
}

impl EnvError {
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
            Self::Unsupported => "This host cannot observe the environment".to_owned(),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

impl Hive {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurrentUser => "HKCU",
            Self::LocalMachine => "HKLM",
        }
    }

    fn parse(value: &str) -> Result<Self, EnvError> {
        match value {
            "HKCU" => Ok(Self::CurrentUser),
            "HKLM" => Ok(Self::LocalMachine),
            _ => Err(EnvError::fault("invalid env scope")),
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn store_prefix(hive: Hive) -> &'static str {
    match hive {
        Hive::CurrentUser => "user",
        Hive::LocalMachine => "system",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvKey {
    pub hive: Hive,
    pub subkey: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    keys: Vec<EnvKey>,
}

impl Scope {
    pub fn product() -> Self {
        Self::from_pairs(&[
            ("HKCU", "Environment"),
            (
                "HKLM",
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            ),
        ])
        .expect("product env scope")
    }

    pub fn from_pairs(pairs: &[(&str, &str)]) -> Result<Self, EnvError> {
        if pairs.len() != 2 {
            return Err(EnvError::fault("invalid env scope"));
        }
        let mut keys = Vec::with_capacity(pairs.len());
        for (hive, subkey) in pairs {
            let hive = Hive::parse(hive)?;
            if !valid_subkey(subkey) {
                return Err(EnvError::fault("invalid env scope"));
            }
            let key = EnvKey {
                hive,
                subkey: (*subkey).to_owned(),
            };
            if keys
                .iter()
                .any(|existing: &EnvKey| same_key(existing, &key))
            {
                return Err(EnvError::fault("invalid env scope"));
            }
            keys.push(key);
        }
        if !keys.iter().any(|key| key.hive == Hive::CurrentUser)
            || !keys.iter().any(|key| key.hive == Hive::LocalMachine)
        {
            return Err(EnvError::fault("invalid env scope"));
        }
        Ok(Self { keys })
    }

    pub fn keys(&self) -> &[EnvKey] {
        &self.keys
    }
}

fn same_key(left: &EnvKey, right: &EnvKey) -> bool {
    left.hive == right.hive && left.subkey.eq_ignore_ascii_case(&right.subkey)
}

fn valid_subkey(subkey: &str) -> bool {
    if subkey.len() > 200 || subkey.starts_with('\\') || subkey.ends_with('\\') {
        return false;
    }
    let mut count = 0;
    for part in subkey.split('\\') {
        count += 1;
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.len() > 255
            || part
                .chars()
                .any(|symbol| symbol.is_control() || symbol == '/' || symbol == ':')
        {
            return false;
        }
    }
    (1..=8).contains(&count)
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValueSample {
    pub name: String,
    pub kind: u32,
    pub data: Vec<u8>,
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ValueChange {
    Added(ValueSample),
    Removed(ValueSample),
    Changed {
        before: ValueSample,
        after: ValueSample,
    },
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn diff_values(
    before: &[ValueSample],
    after: &[ValueSample],
) -> Result<Vec<ValueChange>, ()> {
    let mut prior = index(before)?;
    let mut current = index(after)?;
    let mut names: Vec<String> = prior.keys().chain(current.keys()).cloned().collect();
    names.sort_unstable();
    names.dedup();
    let mut changes = Vec::new();
    for name in names {
        match (prior.remove(&name), current.remove(&name)) {
            (None, Some(sample)) => changes.push(ValueChange::Added(sample.clone())),
            (Some(sample), None) => changes.push(ValueChange::Removed(sample.clone())),
            (Some(old), Some(new)) if !same_content(old, new) => {
                changes.push(ValueChange::Changed {
                    before: old.clone(),
                    after: new.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(changes)
}

fn index(samples: &[ValueSample]) -> Result<BTreeMap<String, &ValueSample>, ()> {
    let mut map = BTreeMap::new();
    for sample in samples {
        let key = sample.name.to_ascii_lowercase();
        if map.insert(key, sample).is_some() {
            return Err(());
        }
    }
    Ok(map)
}

fn same_content(old: &ValueSample, new: &ValueSample) -> bool {
    if old.kind != new.kind {
        return false;
    }
    match (decode_text(old), decode_text(new)) {
        (Some(before), Some(after)) => before == after,
        (None, None) => old.data == new.data,
        _ => false,
    }
}

struct StreamIds {
    device: Id,
    provider: Id,
    epoch: Id,
    user: Option<Id>,
}

#[cfg_attr(not(windows), allow(dead_code))]
struct Memory {
    sequence: u64,
    ring: VecDeque<Event>,
    paused: bool,
    pause_generation: u64,
    fault: Option<EnvError>,
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

struct Planned {
    event_type: &'static str,
    subject: Option<String>,
    evidence: String,
    partial: Option<&'static str>,
}

#[allow(dead_code)]
pub(crate) fn events_for_change(
    stream: &StreamIds,
    sequence: u64,
    prefix: &str,
    change: &ValueChange,
) -> Vec<Event> {
    let mut sequence = sequence;
    let mut events = Vec::new();
    for planned in plan_change(prefix, change) {
        if let Some(event) = value_observation(
            stream,
            sequence,
            planned.event_type,
            planned.subject.as_deref(),
            &planned.evidence,
            planned.partial,
        ) {
            events.push(event);
        }
        sequence = sequence.saturating_add(1);
    }
    events
}

fn plan_change(prefix: &str, change: &ValueChange) -> Vec<Planned> {
    let (action, name, before, after) = match change {
        ValueChange::Added(sample) => ("added", sample.name.as_str(), None, Some(sample)),
        ValueChange::Removed(sample) => ("removed", sample.name.as_str(), Some(sample), None),
        ValueChange::Changed { before, after } => {
            ("changed", after.name.as_str(), Some(before), Some(after))
        }
    };
    let path = name.eq_ignore_ascii_case("path");
    let event_type = event_type_for(path, action);
    let subject = format!("{prefix}\\{name}");
    let before_text = before.and_then(decode_text);
    let after_text = after.and_then(decode_text);
    let text_ready = match (before, after) {
        (None, Some(_)) => after_text.is_some(),
        (Some(_), None) => before_text.is_some(),
        (Some(_), Some(_)) => before_text.is_some() && after_text.is_some(),
        (None, None) => false,
    };
    if !text_ready {
        return vec![withheld(&subject, event_type, "value_not_text")];
    }
    let evidence = text_evidence(before, before_text.as_deref(), after, after_text.as_deref());
    if Id::new(&evidence).is_ok() {
        return vec![Planned {
            event_type,
            subject: Some(subject),
            evidence,
            partial: None,
        }];
    }
    if !path {
        return vec![withheld(&subject, event_type, "value_exceeds_identifier")];
    }
    // 完整 PATH 放不进标识符时不截断，也不发出那条完整值。按分号拆段比较。
    plan_segments(
        &subject,
        before_text.as_deref().unwrap_or(""),
        after_text.as_deref().unwrap_or(""),
    )
}

fn event_type_for(path: bool, action: &str) -> &'static str {
    match (path, action) {
        (false, "added") => "env.variable_added",
        (false, "removed") => "env.variable_removed",
        (false, "changed") => "env.variable_changed",
        (true, "added") => "env.path_added",
        (true, "removed") => "env.path_removed",
        _ => "env.path_changed",
    }
}

fn withheld(subject: &str, event_type: &'static str, reason: &'static str) -> Planned {
    Planned {
        event_type,
        subject: Some(subject.to_owned()),
        evidence: PROVIDER.to_owned(),
        partial: Some(reason),
    }
}

fn plan_segments(subject: &str, before: &str, after: &str) -> Vec<Planned> {
    let (removed, added) = diff_segments(before, after);
    if removed.is_empty() && added.is_empty() {
        return vec![withheld(
            subject,
            "env.path_changed",
            "value_exceeds_identifier",
        )];
    }
    let mut planned = Vec::new();
    for segment in removed {
        push_segment(&mut planned, subject, "env.path_removed", "b", &segment);
    }
    for segment in added {
        push_segment(&mut planned, subject, "env.path_added", "a", &segment);
    }
    planned
}

fn push_segment(
    planned: &mut Vec<Planned>,
    subject: &str,
    event_type: &'static str,
    side: &str,
    segment: &str,
) {
    let evidence = format!("{side}={}", hex(segment.as_bytes()));
    if Id::new(&evidence).is_ok() {
        planned.push(Planned {
            event_type,
            subject: Some(subject.to_owned()),
            evidence,
            partial: None,
        });
    } else {
        planned.push(withheld(
            subject,
            "env.path_changed",
            "value_exceeds_identifier",
        ));
    }
}

fn split_segments(text: &str) -> Vec<&str> {
    text.split(';').filter(|part| !part.is_empty()).collect()
}

fn diff_segments(before: &str, after: &str) -> (Vec<String>, Vec<String>) {
    let before_parts = split_segments(before);
    let after_parts = split_segments(after);
    let mut after_left: BTreeMap<&str, usize> = BTreeMap::new();
    for part in &after_parts {
        *after_left.entry(*part).or_insert(0) += 1;
    }
    let mut removed = Vec::new();
    for part in &before_parts {
        match after_left.get_mut(*part) {
            Some(count) if *count > 0 => *count -= 1,
            _ => removed.push((*part).to_owned()),
        }
    }
    let mut before_left: BTreeMap<&str, usize> = BTreeMap::new();
    for part in &before_parts {
        *before_left.entry(*part).or_insert(0) += 1;
    }
    let mut added = Vec::new();
    for part in &after_parts {
        match before_left.get_mut(*part) {
            Some(count) if *count > 0 => *count -= 1,
            _ => added.push((*part).to_owned()),
        }
    }
    (removed, added)
}

fn decode_text(sample: &ValueSample) -> Option<String> {
    if sample.kind != REG_SZ && sample.kind != REG_EXPAND_SZ {
        return None;
    }
    if sample.data.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = sample
        .data
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let end = if units.last() == Some(&0) {
        units.len() - 1
    } else {
        units.len()
    };
    String::from_utf16(&units[..end]).ok()
}

fn type_token(kind: u32) -> &'static str {
    if kind == REG_EXPAND_SZ {
        "expand"
    } else {
        "sz"
    }
}

fn text_evidence(
    before: Option<&ValueSample>,
    before_text: Option<&str>,
    after: Option<&ValueSample>,
    after_text: Option<&str>,
) -> String {
    match (before, before_text, after, after_text) {
        (None, _, Some(after), Some(text)) => {
            format!("t={};a={}", type_token(after.kind), hex(text.as_bytes()))
        }
        (Some(before), Some(text), None, _) => {
            format!("t={};b={}", type_token(before.kind), hex(text.as_bytes()))
        }
        (Some(before), Some(old), Some(after), Some(new)) if before.kind == after.kind => {
            format!(
                "t={};b={};a={}",
                type_token(before.kind),
                hex(old.as_bytes()),
                hex(new.as_bytes())
            )
        }
        (Some(before), Some(old), Some(after), Some(new)) => format!(
            "tb={};ta={};b={};a={}",
            type_token(before.kind),
            type_token(after.kind),
            hex(old.as_bytes()),
            hex(new.as_bytes())
        ),
        _ => String::new(),
    }
}

fn hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(data.len() * 2);
    for byte in data {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
}

fn value_observation(
    stream: &StreamIds,
    sequence: u64,
    event_type: &str,
    subject: Option<&str>,
    evidence: &str,
    partial: Option<&str>,
) -> Option<Event> {
    let subject_id = subject.and_then(|text| Id::new(text).ok());
    let mut partial = partial;
    if subject.is_some() && subject_id.is_none() {
        partial = Some("identifier_limit");
    }
    observation(
        stream,
        sequence,
        event_type,
        subject_id.as_ref().map(|id| (SUBJECT_KIND, id.as_str())),
        evidence,
        partial,
    )
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn incomplete_event(stream: &StreamIds, sequence: u64) -> Option<Event> {
    observation(
        stream,
        sequence,
        "env.block_changed",
        None,
        PROVIDER,
        Some("snapshot_incomplete"),
    )
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn gap_event(stream: &StreamIds, sequence: u64, reason: GapReason) -> Option<Event> {
    Some(Event {
        version: Version::CURRENT,
        source: source(stream),
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

fn observation(
    stream: &StreamIds,
    sequence: u64,
    event_type: &str,
    subject: Option<(&str, &str)>,
    evidence: &str,
    partial: Option<&str>,
) -> Option<Event> {
    let evidence = Id::new(evidence).ok()?;
    let partial = partial.map(Id::new).transpose().ok()?;
    let subject = subject.map(|(kind, id)| {
        Some(ObjectRef {
            kind: Id::new(kind).ok()?,
            id: Id::new(id).ok()?,
        })
    });
    let subject = match subject {
        Some(Some(subject)) => Some(subject),
        Some(None) => return None,
        None => None,
    };
    Some(Event {
        version: Version::CURRENT,
        source: source(stream),
        sequence: Counter::new(sequence).ok()?,
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns()),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new(event_type).ok()?,
            subject,
            user: stream.user.clone(),
            process: None,
            attribution: Attribution::Observed { evidence },
            completeness: match partial {
                Some(reason) => Completeness::Partial { reason },
                None => Completeness::Complete,
            },
        },
    })
}

fn source(stream: &StreamIds) -> Source {
    Source {
        device: stream.device.clone(),
        provider: stream.provider.clone(),
        epoch: stream.epoch.clone(),
    }
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
pub(crate) fn next_event(
    memory: &Mutex<Memory>,
    build: impl FnOnce(&StreamIds, u64) -> Option<Event>,
) -> bool {
    let event = {
        let mut guard = lock(memory);
        guard.sequence = guard.sequence.saturating_add(1);
        let sequence = guard.sequence;
        let stream_device = guard.stream.device.clone();
        let stream_provider = guard.stream.provider.clone();
        let stream_epoch = guard.stream.epoch.clone();
        let stream_user = guard.stream.user.clone();
        drop(guard);
        let stream = StreamIds {
            device: stream_device,
            provider: stream_provider,
            epoch: stream_epoch,
            user: stream_user,
        };
        match build(&stream, sequence) {
            Some(event) => event,
            None => return false,
        }
    };
    emit(memory, event)
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
        user: Option<&str>,
        epoch: u64,
        on_event: Option<Arc<dyn Fn(&Event) -> bool + Send + Sync>>,
    ) -> Result<Self, EnvError> {
        if epoch == 0 {
            return Err(EnvError::fault("invalid env epoch"));
        }
        let user = match user {
            Some(text) => match Id::new(text) {
                Ok(id) => Some(id),
                Err(_) => None,
            },
            None => None,
        };
        let memory = Arc::new(Mutex::new(Memory {
            sequence: 0,
            ring: VecDeque::new(),
            paused: false,
            pause_generation: 0,
            fault: None,
            on_event,
            stream: StreamIds {
                device: Id::new(PLUGIN_ID).map_err(|_| EnvError::fault("invalid env device"))?,
                provider: Id::new(PROVIDER).map_err(|_| EnvError::fault("invalid env provider"))?,
                epoch: Id::new(format!("env-{epoch}"))
                    .map_err(|_| EnvError::fault("invalid env epoch"))?,
                user,
            },
        }));
        #[cfg(windows)]
        {
            windows::spawn(scope, memory)
        }
        #[cfg(not(windows))]
        {
            let _ = (scope, memory);
            Err(EnvError::Unsupported)
        }
    }

    pub fn query(&self) -> Vec<Event> {
        lock(&self.memory).ring.iter().cloned().collect()
    }

    pub fn pause(&self) -> Result<(), EnvError> {
        let mut memory = lock(&self.memory);
        memory.paused = true;
        memory.pause_generation = memory.pause_generation.saturating_add(1);
        Ok(())
    }

    pub fn resume(&self) -> Result<(), EnvError> {
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
            epoch: Id::new("env-1").unwrap(),
            user: Some(Id::new("uid-7").unwrap()),
        }
    }

    fn text_sample(name: &str, kind: u32, text: &str) -> ValueSample {
        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);
        ValueSample {
            name: name.to_owned(),
            kind,
            data: units.iter().flat_map(|unit| unit.to_le_bytes()).collect(),
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

    fn event_type(event: &Event) -> &str {
        match &event.record {
            Record::Observation { event_type, .. } => event_type.as_str(),
            other => panic!("expected observation, got {other:?}"),
        }
    }

    fn subject(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation {
                subject: Some(subject),
                ..
            } => Some(subject.id.as_str()),
            Record::Observation { subject: None, .. } => None,
            other => panic!("expected observation, got {other:?}"),
        }
    }

    fn partial(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation {
                completeness: Completeness::Partial { reason },
                ..
            } => Some(reason.as_str()),
            Record::Observation {
                completeness: Completeness::Complete,
                ..
            } => None,
            other => panic!("expected observation, got {other:?}"),
        }
    }

    fn rendered(event: &Event) -> String {
        format!("{event:?}")
    }

    fn rejected(pairs: &[(&str, &str)]) {
        let error = match Scope::from_pairs(pairs) {
            Err(error) => error,
            Ok(scope) => panic!("scope accepted: {:?}", scope.keys()),
        };
        assert_eq!(error.code(), "collector_fault");
        assert_eq!(error.detail(), "invalid env scope");
        let rendered = format!("{pairs:?}");
        for (_, path) in pairs {
            if path.len() > 12 {
                assert!(
                    !error.detail().contains(path),
                    "rejected path leaked into {error:?} from {rendered}"
                );
            }
        }
    }

    #[test]
    fn scope_requires_user_and_system_stores() {
        rejected(&[]);
        rejected(&[("HKCU", "Environment")]);
        rejected(&[
            ("HKCU", "Environment"),
            (
                "HKLM",
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            ),
            ("HKCU", r"Software\WinStrataEnvExtra"),
        ]);
        rejected(&[("HKU", "Environment"), ("HKLM", "Environment")]);
        rejected(&[("HKCU", ".."), ("HKLM", "Environment")]);
        rejected(&[
            ("HKCU", r"Software\..\SecretEnvPath"),
            ("HKLM", "Environment"),
        ]);
        rejected(&[
            ("HKCU", r"Software\WinStrataEnv"),
            ("HKCU", r"software\winstrataenv"),
        ]);
        let error = Scope::from_pairs(&[
            ("HKCU", r"Software\..\SecretEnvPath"),
            ("HKLM", "Environment"),
        ])
        .expect_err("dotdot scope");
        assert_eq!(error.detail(), "invalid env scope");
        assert!(!error.detail().contains("SecretEnvPath"));

        let scope =
            Scope::from_pairs(&[("HKCU", r"Software\WinStrataEnv"), ("HKLM", "SOFTWARE")]).unwrap();
        assert_eq!(scope.keys().len(), 2);
        assert_eq!(scope.keys()[0].hive.as_str(), "HKCU");
        assert_eq!(scope.keys()[1].hive.as_str(), "HKLM");

        let product = Scope::product();
        let user: Vec<_> = product
            .keys()
            .iter()
            .filter(|key| key.hive == Hive::CurrentUser && key.subkey == "Environment")
            .collect();
        let system: Vec<_> = product
            .keys()
            .iter()
            .filter(|key| {
                key.hive == Hive::LocalMachine
                    && key.subkey == r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment"
            })
            .collect();
        assert_eq!(user.len(), 1);
        assert_eq!(system.len(), 1);
        assert_eq!(product.keys().len(), 2);
        assert_eq!(user[0].hive.as_str(), "HKCU");
        assert_eq!(system[0].hive.as_str(), "HKLM");
    }

    #[test]
    fn before_and_after_text_stays_exact_or_is_withheld() {
        let added = ValueChange::Added(text_sample("Probe", REG_SZ, "alpha"));
        let events = events_for_change(&stream(), 1, "user", &added);
        assert_eq!(events.len(), 1);
        assert_eq!(evidence(&events[0]), "t=sz;a=616c706861");
        assert_eq!(event_type(&events[0]), "env.variable_added");
        assert_eq!(subject(&events[0]), Some("user\\Probe"));
        assert!(partial(&events[0]).is_none());
        match &events[0].record {
            Record::Observation {
                process,
                user,
                completeness: Completeness::Complete,
                ..
            } => {
                assert!(process.is_none());
                assert_eq!(user.as_ref().map(Id::as_str), Some("uid-7"));
                assert!(events[0].time.precision_ns.is_none());
                assert_eq!(events[0].source.device.as_str(), PLUGIN_ID);
                assert_eq!(events[0].source.provider.as_str(), PROVIDER);
                assert_eq!(events[0].source.epoch.as_str(), "env-1");
                assert_eq!(events[0].sequence.get(), 1);
            }
            other => panic!("unexpected record {other:?}"),
        }

        let removed = ValueChange::Removed(text_sample("Probe", REG_SZ, "alpha"));
        let events = events_for_change(&stream(), 2, "system", &removed);
        assert_eq!(events.len(), 1);
        assert_eq!(evidence(&events[0]), "t=sz;b=616c706861");
        assert_eq!(event_type(&events[0]), "env.variable_removed");
        assert_eq!(subject(&events[0]), Some("system\\Probe"));

        let changed = ValueChange::Changed {
            before: text_sample("Probe", REG_SZ, "alpha"),
            after: text_sample("Probe", REG_SZ, "beta"),
        };
        let events = events_for_change(&stream(), 3, "user", &changed);
        assert_eq!(events.len(), 1);
        assert_eq!(evidence(&events[0]), "t=sz;b=616c706861;a=62657461");
        assert_eq!(event_type(&events[0]), "env.variable_changed");

        let typed = ValueChange::Changed {
            before: text_sample("Probe", REG_SZ, "alpha"),
            after: text_sample("Probe", REG_EXPAND_SZ, "alpha"),
        };
        let events = events_for_change(&stream(), 4, "user", &typed);
        assert_eq!(events.len(), 1);
        assert_eq!(
            evidence(&events[0]),
            "tb=sz;ta=expand;b=616c706861;a=616c706861"
        );
        assert_eq!(event_type(&events[0]), "env.variable_changed");
        assert!(partial(&events[0]).is_none());

        let kept = text_sample("Probe", REG_SZ, "a\u{0}b");
        let events = events_for_change(&stream(), 5, "user", &ValueChange::Added(kept));
        assert_eq!(evidence(&events[0]), "t=sz;a=610062");

        let payload = "q".repeat(200);
        let huge = ValueChange::Added(text_sample("Probe", REG_SZ, &payload));
        let events = events_for_change(&stream(), 6, "user", &huge);
        assert_eq!(events.len(), 1);
        assert_eq!(evidence(&events[0]), PROVIDER);
        assert_eq!(partial(&events[0]), Some("value_exceeds_identifier"));
        assert_eq!(event_type(&events[0]), "env.variable_added");
        let text = rendered(&events[0]);
        assert!(!text.contains(&payload));
        assert!(!text.contains(&hex(payload.as_bytes())));

        let secret = ValueChange::Added(ValueSample {
            name: "Probe".into(),
            kind: 3,
            data: b"SECRET".to_vec(),
        });
        let events = events_for_change(&stream(), 7, "user", &secret);
        assert_eq!(events.len(), 1);
        assert_eq!(evidence(&events[0]), PROVIDER);
        assert_eq!(partial(&events[0]), Some("value_not_text"));
        assert_eq!(event_type(&events[0]), "env.variable_added");
        let text = rendered(&events[0]);
        assert!(!text.contains("SECRET"));
        assert!(!text.contains("534543524554"));

        let odd = ValueChange::Changed {
            before: text_sample("Probe", REG_SZ, "alpha"),
            after: ValueSample {
                name: "Probe".into(),
                kind: REG_SZ,
                data: vec![0x61],
            },
        };
        let events = events_for_change(&stream(), 8, "user", &odd);
        assert_eq!(evidence(&events[0]), PROVIDER);
        assert_eq!(partial(&events[0]), Some("value_not_text"));
        assert!(!rendered(&events[0]).contains("t=sz"));

        let path = ValueChange::Changed {
            before: text_sample("Path", REG_SZ, "alpha"),
            after: text_sample("Path", REG_SZ, "beta"),
        };
        let events = events_for_change(&stream(), 9, "user", &path);
        assert_eq!(events.len(), 1);
        assert_eq!(event_type(&events[0]), "env.path_changed");
        assert_ne!(event_type(&events[0]), "env.variable_changed");
        assert_eq!(evidence(&events[0]), "t=sz;b=616c706861;a=62657461");
        assert_eq!(subject(&events[0]), Some("user\\Path"));
        assert!(partial(&events[0]).is_none());

        let filler = "Z".repeat(80);
        let long = ValueChange::Changed {
            before: text_sample("PATH", REG_SZ, &format!("OLD;{filler}")),
            after: text_sample("PATH", REG_SZ, &format!("NEW;{filler}")),
        };
        let events = events_for_change(&stream(), 10, "system", &long);
        assert_eq!(events.len(), 2);
        assert_eq!(event_type(&events[0]), "env.path_removed");
        assert_eq!(evidence(&events[0]), "b=4f4c44");
        assert_eq!(event_type(&events[1]), "env.path_added");
        assert_eq!(evidence(&events[1]), "a=4e4557");
        assert!(partial(&events[0]).is_none());
        assert!(partial(&events[1]).is_none());
        for event in &events {
            assert_eq!(subject(event), Some("system\\PATH"));
            let text = rendered(event);
            assert!(!text.contains(&filler));
            assert!(!text.contains("t=sz"));
            assert!(!text.contains("tb="));
            assert!(!text.contains(&"5a".repeat(80)));
            assert!(!event_type(event).contains("variable"));
        }

        let ordered = ValueChange::Changed {
            before: text_sample("PATH", REG_SZ, &format!("B;A;{filler}")),
            after: text_sample("PATH", REG_SZ, &format!("A;B;{filler}")),
        };
        let events = events_for_change(&stream(), 12, "user", &ordered);
        assert_eq!(events.len(), 1);
        assert_eq!(event_type(&events[0]), "env.path_changed");
        assert_eq!(evidence(&events[0]), PROVIDER);
        assert_eq!(partial(&events[0]), Some("value_exceeds_identifier"));
        let text = rendered(&events[0]);
        assert!(!text.contains(&filler));
        assert!(!text.contains(&"5a".repeat(80)));
        assert!(!text.contains("t=sz"));

        let huge_segment = "Q".repeat(128);
        let split = ValueChange::Changed {
            before: text_sample("PATH", REG_SZ, &huge_segment),
            after: text_sample("PATH", REG_SZ, "NEW"),
        };
        let events = events_for_change(&stream(), 13, "user", &split);
        assert_eq!(events.len(), 2);
        assert_eq!(event_type(&events[0]), "env.path_changed");
        assert_eq!(evidence(&events[0]), PROVIDER);
        assert_eq!(partial(&events[0]), Some("value_exceeds_identifier"));
        assert_eq!(event_type(&events[1]), "env.path_added");
        assert_eq!(evidence(&events[1]), "a=4e4557");
        assert_eq!(subject(&events[0]), Some("user\\PATH"));
        assert_eq!(subject(&events[1]), Some("user\\PATH"));
        let text = rendered(&events[0]);
        assert!(!text.contains(&huge_segment));
        assert!(!text.contains(&"51".repeat(128)));

        assert!(
            diff_values(
                &[text_sample("Path", REG_SZ, "alpha")],
                &[text_sample("PATH", REG_SZ, "alpha")],
            )
            .unwrap()
            .is_empty()
        );
        let renamed = diff_values(
            &[text_sample("Probe", REG_SZ, "alpha")],
            &[text_sample("probe", REG_SZ, "beta")],
        )
        .unwrap();
        assert_eq!(renamed.len(), 1);
        let events = events_for_change(&stream(), 15, "system", &renamed[0]);
        assert_eq!(subject(&events[0]), Some("system\\probe"));
        assert_eq!(evidence(&events[0]), "t=sz;b=616c706861;a=62657461");
        assert!(
            diff_values(
                &[
                    text_sample("Path", REG_SZ, "A"),
                    text_sample("PATH", REG_SZ, "B")
                ],
                &[text_sample("Path", REG_SZ, "A")],
            )
            .is_err()
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn linux_start_publishes_nothing() {
        let published = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&published);
        match Session::start(
            &Scope::product(),
            Some("uid-7"),
            1,
            Some(Arc::new(move |_event| {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                true
            })),
        ) {
            Err(error) => assert_eq!(error.code(), "unsupported"),
            Ok(_) => panic!("linux started an environment watch"),
        }
        assert!(!published.load(std::sync::atomic::Ordering::SeqCst));
        let error = match Session::start(&Scope::product(), Some("uid-7"), 0, None) {
            Err(error) => error,
            Ok(_) => panic!("epoch 0 started an environment watch"),
        };
        assert_eq!(error.detail(), "invalid env epoch");
        assert_eq!(error.code(), "collector_fault");
    }
}

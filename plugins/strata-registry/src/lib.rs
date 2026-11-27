//! StrataRegistry 观察当前用户和相关系统注册表键值。
//!
//! 监视留在普通用户进程中，使用已经链接的 Windows API。不加载 `strata-reg.sys`。
//! Linux 不能观察 Windows 注册表，启动返回 `unsupported`，不生成事件。
//! 这些 API 不报告变更时刻的精度，也不报告执行进程，因此省略 `precision_ns` 和进程字段。

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

pub const PLUGIN_ID: &str = "strata.registry";
pub const PROVIDER: &str = "windows.reg_notify";

const MAX_KEYS: usize = 4;
const QUERY_LIMIT: usize = 128;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const VALUE_LIMIT: usize = 64;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const NAME_CHARS: usize = 256;
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const DATA_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    Unsupported,
    Fault(String),
    Os(i32),
}

impl RegistryError {
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
            Self::Unsupported => "This host cannot observe the registry".to_owned(),
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

    fn parse(value: &str) -> Result<Self, RegistryError> {
        match value {
            "HKCU" => Ok(Self::CurrentUser),
            "HKLM" => Ok(Self::LocalMachine),
            _ => Err(RegistryError::fault("invalid registry scope")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryKey {
    pub hive: Hive,
    pub subkey: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    keys: Vec<RegistryKey>,
}

impl Scope {
    pub fn product() -> Self {
        Self::from_pairs(&[
            (
                "HKCU",
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\Shell Folders",
            ),
            ("HKLM", r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
        ])
        .expect("product registry scope")
    }

    pub fn from_pairs(pairs: &[(&str, &str)]) -> Result<Self, RegistryError> {
        if pairs.is_empty() || pairs.len() > MAX_KEYS {
            return Err(RegistryError::fault("invalid registry scope"));
        }
        let mut keys = Vec::with_capacity(pairs.len());
        for (hive, subkey) in pairs {
            let hive = Hive::parse(hive)?;
            if !valid_subkey(subkey) {
                return Err(RegistryError::fault("invalid registry scope"));
            }
            let key = RegistryKey {
                hive,
                subkey: (*subkey).to_owned(),
            };
            if keys
                .iter()
                .any(|existing: &RegistryKey| same_key(existing, &key))
            {
                return Err(RegistryError::fault("invalid registry scope"));
            }
            keys.push(key);
        }
        if !keys.iter().any(|key| key.hive == Hive::CurrentUser)
            || !keys.iter().any(|key| key.hive == Hive::LocalMachine)
        {
            return Err(RegistryError::fault("invalid registry scope"));
        }
        Ok(Self { keys })
    }

    pub fn keys(&self) -> &[RegistryKey] {
        &self.keys
    }
}

fn same_key(left: &RegistryKey, right: &RegistryKey) -> bool {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValueSample {
    pub name: String,
    pub kind: u32,
    pub data: Vec<u8>,
}

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
    let mut names: Vec<&str> = prior.keys().chain(current.keys()).copied().collect();
    names.sort_unstable();
    names.dedup();
    let mut changes = Vec::new();
    for name in names {
        match (prior.remove(name), current.remove(name)) {
            (None, Some(sample)) => changes.push(ValueChange::Added(sample.clone())),
            (Some(sample), None) => changes.push(ValueChange::Removed(sample.clone())),
            (Some(old), Some(new)) if old.kind != new.kind || old.data != new.data => {
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

fn index(samples: &[ValueSample]) -> Result<BTreeMap<&str, &ValueSample>, ()> {
    let mut map = BTreeMap::new();
    for sample in samples {
        if map.insert(sample.name.as_str(), sample).is_some() {
            return Err(());
        }
    }
    Ok(map)
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
    fault: Option<RegistryError>,
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

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn value_event(
    stream: &StreamIds,
    sequence: u64,
    hive: &str,
    subkey: &str,
    change: &ValueChange,
) -> Option<Event> {
    let (event_type, sample, before, after) = match change {
        ValueChange::Added(sample) => ("registry.value_added", sample, None, Some(sample)),
        ValueChange::Removed(sample) => ("registry.value_removed", sample, Some(sample), None),
        ValueChange::Changed { before, after } => {
            ("registry.value_changed", after, Some(before), Some(after))
        }
    };
    let subject = format!("{hive}\\{subkey}\\{}", sample.name);
    let subject_id = Id::new(&subject).ok();
    let detailed = registry_evidence(before, after);
    let (evidence, mut partial) = match detailed {
        Some(text) => (text, None),
        None => (PROVIDER.to_owned(), Some("value_exceeds_identifier")),
    };
    if subject_id.is_none() {
        partial = Some("identifier_limit");
    }
    observation(
        stream,
        sequence,
        event_type,
        subject_id
            .as_ref()
            .map(|id| ("registry_value", id.as_str())),
        &evidence,
        partial,
    )
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn incomplete_event(stream: &StreamIds, sequence: u64) -> Option<Event> {
    observation(
        stream,
        sequence,
        "registry.key_changed",
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
fn registry_evidence(before: Option<&ValueSample>, after: Option<&ValueSample>) -> Option<String> {
    let text = match (before, after) {
        (None, Some(after)) => format!("t={};a={}", after.kind, hex(&after.data)),
        (Some(before), None) => format!("t={};b={}", before.kind, hex(&before.data)),
        (Some(before), Some(after)) if before.kind == after.kind => {
            format!(
                "t={};b={};a={}",
                before.kind,
                hex(&before.data),
                hex(&after.data)
            )
        }
        (Some(before), Some(after)) => format!(
            "tb={};ta={};b={};a={}",
            before.kind,
            after.kind,
            hex(&before.data),
            hex(&after.data)
        ),
        (None, None) => return None,
    };
    Id::new(&text).ok().map(|id| id.as_str().to_owned())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(data.len() * 2);
    for byte in data {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
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
    ) -> Result<Self, RegistryError> {
        if epoch == 0 {
            return Err(RegistryError::fault("invalid registry epoch"));
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
                device: Id::new(PLUGIN_ID)
                    .map_err(|_| RegistryError::fault("invalid registry device"))?,
                provider: Id::new(PROVIDER)
                    .map_err(|_| RegistryError::fault("invalid registry provider"))?,
                epoch: Id::new(format!("reg-{epoch}"))
                    .map_err(|_| RegistryError::fault("invalid registry epoch"))?,
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
            Err(RegistryError::Unsupported)
        }
    }

    pub fn query(&self) -> Vec<Event> {
        lock(&self.memory).ring.iter().cloned().collect()
    }

    pub fn pause(&self) -> Result<(), RegistryError> {
        let mut memory = lock(&self.memory);
        memory.paused = true;
        memory.pause_generation = memory.pause_generation.saturating_add(1);
        Ok(())
    }

    pub fn resume(&self) -> Result<(), RegistryError> {
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
            epoch: Id::new("reg-1").unwrap(),
            user: Some(Id::new("uid-7").unwrap()),
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

    #[test]
    fn scope_requires_the_current_user_and_one_system_key() {
        assert!(Scope::from_pairs(&[]).is_err());
        assert!(Scope::from_pairs(&[("HKCU", r"Software\WinStrata")]).is_err());
        assert!(
            Scope::from_pairs(&[("HKU", r"Software\WinStrata"), ("HKLM", "SOFTWARE")]).is_err()
        );
        assert!(Scope::from_pairs(&[("HKCU", r"..\Software"), ("HKLM", "SOFTWARE")]).is_err());
        assert!(
            Scope::from_pairs(&[
                ("HKCU", r"Software\WinStrata"),
                ("HKLM", "SOFTWARE"),
                ("HKCU", r"software\winstrata"),
            ])
            .is_err()
        );
        let scope =
            Scope::from_pairs(&[("HKCU", r"Software\WinStrata"), ("HKLM", "SOFTWARE")]).unwrap();
        assert_eq!(scope.keys().len(), 2);
        let product = Scope::product();
        assert!(
            product
                .keys()
                .iter()
                .any(|key| key.hive == Hive::CurrentUser)
        );
        assert!(
            product
                .keys()
                .iter()
                .any(|key| key.hive == Hive::LocalMachine)
        );
    }

    #[test]
    fn before_and_after_bytes_stay_exact_or_are_withheld() {
        let added = ValueChange::Added(ValueSample {
            name: "Probe".into(),
            kind: 4,
            data: vec![0x11, 0, 0, 0],
        });
        let event = value_event(&stream(), 1, "HKLM", r"SOFTWARE\WinStrata", &added).unwrap();
        assert_eq!(evidence(&event), "t=4;a=11000000");
        match &event.record {
            Record::Observation {
                subject: Some(subject),
                process,
                user,
                completeness: Completeness::Complete,
                ..
            } => {
                assert_eq!(subject.id.as_str(), r"HKLM\SOFTWARE\WinStrata\Probe");
                assert!(process.is_none());
                assert_eq!(user.as_ref().map(Id::as_str), Some("uid-7"));
                assert!(event.time.precision_ns.is_none());
            }
            other => panic!("unexpected record {other:?}"),
        }

        let changed = ValueChange::Changed {
            before: ValueSample {
                name: "Probe".into(),
                kind: 4,
                data: vec![0x11, 0, 0, 0],
            },
            after: ValueSample {
                name: "Probe".into(),
                kind: 1,
                data: vec![0x22],
            },
        };
        let event = value_event(&stream(), 2, "HKCU", r"Software\WinStrata", &changed).unwrap();
        assert_eq!(evidence(&event), "tb=4;ta=1;b=11000000;a=22");

        let huge = ValueChange::Added(ValueSample {
            name: "Probe".into(),
            kind: 3,
            data: vec![0xab; 200],
        });
        let event = value_event(&stream(), 3, "HKCU", r"Software\WinStrata", &huge).unwrap();
        assert_eq!(evidence(&event), PROVIDER);
        match &event.record {
            Record::Observation {
                completeness: Completeness::Partial { reason },
                ..
            } => assert_eq!(reason.as_str(), "value_exceeds_identifier"),
            other => panic!("unexpected record {other:?}"),
        }
        assert!(!evidence(&event).contains("ab"));
    }

    #[cfg(not(windows))]
    #[test]
    fn linux_start_publishes_nothing() {
        match Session::start(&Scope::product(), Some("uid-7"), 1, None) {
            Err(error) => assert_eq!(error.code(), "unsupported"),
            Ok(_) => panic!("linux started a registry watch"),
        }
    }
}

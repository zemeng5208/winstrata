//! 服务与驱动的只读采样和差异。
//!
//! 采集程序不创建、不启动、不停止、不删除服务，也不加载驱动。
//! Windows 服务控制管理器模块使用 unsafe；跟踪器本身不使用。

use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::time::{SystemTime, UNIX_EPOCH};

use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, ObjectRef, Record,
    SignedNanos, Source, Version,
};

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod host;

#[cfg(windows)]
#[path = "windows.rs"]
mod host;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("service observation supports Windows and the Linux development host");

const DEVICE: &str = "strata.services";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Service,
    Driver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Stopped,
    StartPending,
    StopPending,
    Running,
    ContinuePending,
    PausePending,
    Paused,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartType {
    Boot,
    System,
    Auto,
    Demand,
    Disabled,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Complete,
    ActiveOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub name: String,
    pub kind: Kind,
    pub state: RunState,
    pub start_type: StartType,
    pub binary: Option<String>,
    pub pid: Option<u32>,
    pub win32_exit: u32,
    pub service_exit: u32,
    pub config_error: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceSnapshot {
    pub device: String,
    pub boot: String,
    pub sampled_unix_ns: i64,
    pub facts: Vec<Fact>,
    pub coverage: Coverage,
    pub truncated: bool,
    pub permission_denied: bool,
    pub services_unreadable: bool,
    pub driver_permission_denied: bool,
    pub source_failure: Option<String>,
    pub evidence: &'static str,
}

pub struct ServiceTracker {
    next: u64,
    device: Option<String>,
    boot: Option<String>,
    facts: BTreeMap<(Kind, String), Fact>,
    opened: bool,
}

impl ServiceTracker {
    pub fn new() -> Self {
        Self {
            next: 0,
            device: None,
            boot: None,
            facts: BTreeMap::new(),
            opened: false,
        }
    }

    pub fn observe(&mut self, snapshot: &ServiceSnapshot) -> io::Result<Vec<Event>> {
        let identity_gap = snapshot
            .source_failure
            .as_deref()
            .is_some_and(|reason| reason.starts_with("identity:"));
        let keep_epoch = identity_gap && self.device.is_some();
        if !keep_epoch
            && (self.device.as_deref() != Some(snapshot.device.as_str())
                || self.boot.as_deref() != Some(snapshot.boot.as_str()))
        {
            self.device = Some(snapshot.device.clone());
            self.boot = Some(snapshot.boot.clone());
            self.facts.clear();
            self.opened = false;
            self.next = 0;
        }
        if snapshot.permission_denied {
            let gap = self.gap(snapshot)?;
            let denied = self.permission(snapshot)?;
            return Ok(vec![gap, denied]);
        }
        if snapshot.source_failure.is_some() && !identity_gap {
            return Ok(vec![self.gap(snapshot)?]);
        }
        let mut events = Vec::new();
        if identity_gap {
            events.push(self.gap(snapshot)?);
        }
        if !self.opened {
            self.opened = true;
            events.extend(self.baseline(snapshot)?);
        } else {
            events.extend(self.diff(snapshot)?);
        }
        Ok(events)
    }
}

impl Default for ServiceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceTracker {
    fn baseline(&mut self, snapshot: &ServiceSnapshot) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        for fact in visible(snapshot) {
            if !valid_name(&fact.name) {
                continue;
            }
            events.push(self.event(snapshot, fact, state_type(fact.kind), fact)?);
            if failed(fact) {
                events.push(self.event(snapshot, fact, "service.failure", fact)?);
            }
            self.facts
                .insert((fact.kind, fact.name.clone()), fact.clone());
        }
        Ok(events)
    }

    fn diff(&mut self, snapshot: &ServiceSnapshot) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        let mut seen = Vec::new();
        for fact in visible(snapshot) {
            if !valid_name(&fact.name) {
                continue;
            }
            let key = (fact.kind, fact.name.clone());
            seen.push(key.clone());
            if let Some(previous) = self.facts.get(&key).cloned() {
                events.extend(self.changes(snapshot, &previous, fact)?);
            } else {
                events.extend(self.added(snapshot, fact)?);
            }
            self.facts.insert(key, fact.clone());
        }
        if snapshot.truncated {
            return Ok(events);
        }
        let missing = self
            .facts
            .keys()
            .filter(|(kind, name)| {
                kind_visible(snapshot, *kind) && !seen.iter().any(|key| key.0 == *kind && key.1 == *name)
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in missing {
            let Some(previous) = self.facts.remove(&key) else {
                continue;
            };
            events.extend(self.removed(snapshot, &previous)?);
        }
        Ok(events)
    }

    fn added(&mut self, snapshot: &ServiceSnapshot, fact: &Fact) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        let event_type = match (snapshot.coverage, fact.kind, fact.state) {
            (Coverage::ActiveOnly, Kind::Service, _) => "service.started",
            (Coverage::Complete, Kind::Service, _) => "service.created",
            (Coverage::ActiveOnly, Kind::Driver, _) => "driver.loaded",
            (Coverage::Complete, Kind::Driver, RunState::Running) => "driver.loaded",
            (Coverage::Complete, Kind::Driver, _) => "driver.state",
        };
        events.push(self.event(snapshot, fact, event_type, fact)?);
        if failed(fact) {
            events.push(self.event(snapshot, fact, "service.failure", fact)?);
        }
        Ok(events)
    }

    fn removed(&mut self, snapshot: &ServiceSnapshot, previous: &Fact) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        match (snapshot.coverage, previous.kind, previous.state) {
            (Coverage::ActiveOnly, Kind::Service, _) => {
                events.push(self.event(snapshot, previous, "service.stopped", &stopped(previous))?);
            }
            (Coverage::Complete, Kind::Service, RunState::Stopped) => {
                events.push(self.event(snapshot, previous, "service.deleted", previous)?);
            }
            (Coverage::Complete, Kind::Service, _) => {
                events.push(self.event(snapshot, previous, "service.stopped", &stopped(previous))?);
                events.push(self.event(snapshot, previous, "service.deleted", previous)?);
            }
            (Coverage::ActiveOnly, Kind::Driver, _) => {
                events.push(self.event(snapshot, previous, "driver.unloaded", &stopped(previous))?);
            }
            (Coverage::Complete, Kind::Driver, RunState::Running) => {
                events.push(self.event(snapshot, previous, "driver.unloaded", &stopped(previous))?);
                events.push(self.event(snapshot, previous, "driver.removed", previous)?);
            }
            (Coverage::Complete, Kind::Driver, _) => {
                events.push(self.event(snapshot, previous, "driver.removed", previous)?);
            }
        }
        Ok(events)
    }

    fn changes(
        &mut self,
        snapshot: &ServiceSnapshot,
        previous: &Fact,
        fact: &Fact,
    ) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        if previous.state != fact.state {
            if fact.kind == Kind::Service && fact.state == RunState::Stopped {
                events.push(self.event(snapshot, fact, "service.stopped", fact)?);
            } else if fact.kind == Kind::Service && fact.state == RunState::Running {
                events.push(self.event(snapshot, fact, "service.started", fact)?);
            } else if fact.kind == Kind::Driver && fact.state == RunState::Running {
                events.push(self.event(snapshot, fact, "driver.loaded", fact)?);
            } else if fact.kind == Kind::Driver
                && previous.state == RunState::Running
                && fact.state != RunState::Running
            {
                events.push(self.event(snapshot, fact, "driver.unloaded", fact)?);
            }
        }
        if previous.start_type != fact.start_type {
            events.push(self.event(snapshot, fact, "service.start-type", fact)?);
        }
        if previous.binary != fact.binary {
            events.push(self.event(snapshot, fact, "service.config", fact)?);
        }
        let previous_failed = (previous.win32_exit, previous.service_exit);
        let current_failed = (fact.win32_exit, fact.service_exit);
        if failed(fact) && previous_failed != current_failed {
            events.push(self.event(snapshot, fact, "service.failure", fact)?);
        }
        Ok(events)
    }

    fn permission(&mut self, snapshot: &ServiceSnapshot) -> io::Result<Event> {
        self.observation(snapshot, "service.permission", None, "access-denied", None)
    }

    fn gap(&mut self, snapshot: &ServiceSnapshot) -> io::Result<Event> {
        let sequence = self.bump()?;
        Ok(Event {
            version: Version::CURRENT,
            source: source(snapshot)?,
            sequence: Counter::new(sequence).expect("sequence is positive"),
            time: EventTime {
                unix_ns: SignedNanos::new(snapshot.sampled_unix_ns),
                precision_ns: None,
            },
            record: Record::Gap {
                reason: GapReason::SourceFailure,
                range: None,
            },
        })
    }

    fn event(
        &mut self,
        snapshot: &ServiceSnapshot,
        subject: &Fact,
        event_type: &str,
        evidence_from: &Fact,
    ) -> io::Result<Event> {
        let (evidence, partial) = evidence_text(evidence_from);
        self.observation(
            snapshot,
            event_type,
            Some((kind_token(subject.kind), subject.name.as_str())),
            &evidence,
            partial,
        )
    }

    fn observation(
        &mut self,
        snapshot: &ServiceSnapshot,
        event_type: &str,
        subject: Option<(&str, &str)>,
        evidence: &str,
        partial: Option<&str>,
    ) -> io::Result<Event> {
        let sequence = self.bump()?;
        let evidence = Id::new(evidence).map_err(|error| invalid(error.to_string()))?;
        let partial = match partial {
            Some(reason) => {
                Some(Id::new(reason).map_err(|error| invalid(error.to_string()))?)
            }
            None => None,
        };
        let subject = match subject {
            Some((kind, id)) => Some(ObjectRef {
                kind: Id::new(kind).map_err(|error| invalid(error.to_string()))?,
                id: Id::new(id).map_err(|error| invalid(error.to_string()))?,
            }),
            None => None,
        };
        Ok(Event {
            version: Version::CURRENT,
            source: source(snapshot)?,
            sequence: Counter::new(sequence).expect("sequence is positive"),
            time: EventTime {
                unix_ns: SignedNanos::new(snapshot.sampled_unix_ns),
                precision_ns: None,
            },
            record: Record::Observation {
                event_type: Id::new(event_type).map_err(|error| invalid(error.to_string()))?,
                subject,
                user: None,
                process: None,
                attribution: Attribution::Observed { evidence },
                completeness: match partial {
                    Some(reason) => Completeness::Partial { reason },
                    None => Completeness::Complete,
                },
            },
        })
    }

    fn bump(&mut self) -> io::Result<u64> {
        self.next = self.next.checked_add(1).ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "service sequence space is exhausted")
        })?;
        Ok(self.next)
    }
}

pub fn capture() -> io::Result<ServiceSnapshot> {
    host::capture()
}

fn source(snapshot: &ServiceSnapshot) -> io::Result<Source> {
    let provider = if snapshot.evidence == "scm" {
        "windows.scm"
    } else {
        "procfs"
    };
    Ok(Source {
        device: Id::new(DEVICE).map_err(|error| invalid(error.to_string()))?,
        provider: Id::new(provider).map_err(|error| invalid(error.to_string()))?,
        epoch: Id::new(&snapshot.boot).map_err(|error| invalid(error.to_string()))?,
    })
}

fn visible(snapshot: &ServiceSnapshot) -> impl Iterator<Item = &Fact> {
    snapshot.facts.iter().filter(|fact| kind_visible(snapshot, fact.kind))
}

fn kind_visible(snapshot: &ServiceSnapshot, kind: Kind) -> bool {
    match kind {
        Kind::Service => !snapshot.services_unreadable,
        Kind::Driver => !snapshot.driver_permission_denied,
    }
}

fn valid_name(name: &str) -> bool {
    Id::new(name).is_ok()
}

fn failed(fact: &Fact) -> bool {
    fact.win32_exit != 0 || fact.service_exit != 0
}

fn stopped(fact: &Fact) -> Fact {
    let mut copy = fact.clone();
    copy.state = RunState::Stopped;
    copy
}

fn state_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Service => "service.state",
        Kind::Driver => "driver.state",
    }
}

fn kind_token(kind: Kind) -> &'static str {
    match kind {
        Kind::Service => "service",
        Kind::Driver => "driver",
    }
}

fn evidence_text(fact: &Fact) -> (String, Option<&'static str>) {
    let mut text = format!(
        "state={};start={};win32={};svc={}",
        state_token(fact.state),
        start_token(fact.start_type),
        fact.win32_exit,
        fact.service_exit
    );
    if let Some(pid) = fact.pid {
        text.push_str(&format!(";pid={pid}"));
    }
    let mut partial = fact.config_error.map(|_| "config-unavailable");
    if let Some(path) = &fact.binary {
        let candidate = format!("{text};path={path}");
        if path.contains(';')
            || path.chars().any(char::is_control)
            || candidate.len() > 256
        {
            partial = partial.or(Some("config-exceeds-identifier"));
        } else {
            text = candidate;
        }
    }
    (text, partial)
}

fn state_token(state: RunState) -> &'static str {
    match state {
        RunState::Stopped => "stopped",
        RunState::StartPending => "start-pending",
        RunState::StopPending => "stop-pending",
        RunState::Running => "running",
        RunState::ContinuePending => "continue-pending",
        RunState::PausePending => "pause-pending",
        RunState::Paused => "paused",
        RunState::Unknown => "unknown",
    }
}

fn start_token(start: StartType) -> &'static str {
    match start {
        StartType::Boot => "boot",
        StartType::System => "system",
        StartType::Auto => "auto",
        StartType::Demand => "demand",
        StartType::Disabled => "disabled",
        StartType::Unknown => "unknown",
    }
}

pub(crate) fn clean_name(value: &str) -> Option<String> {
    bounded(value, 240).filter(|text| Id::new(text).is_ok())
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn clean_path(value: &str) -> Option<String> {
    bounded(value, 1024)
}

fn bounded(value: &str, max: usize) -> Option<String> {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_control() {
            continue;
        }
        let mut encoded = [0u8; 4];
        let bytes = ch.encode_utf8(&mut encoded);
        if out.len() + bytes.len() > max {
            break;
        }
        out.push(ch);
    }
    let out = out.trim().to_owned();
    if out.is_empty() { None } else { Some(out) }
}

pub(crate) fn unix_now() -> io::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("system clock is before the Unix epoch"))?;
    i64::try_from(elapsed.as_nanos()).map_err(|_| invalid("system clock does not fit in signed nanoseconds"))
}

fn invalid(detail: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(name: &str, kind: Kind, state: RunState) -> Fact {
        Fact {
            name: name.to_owned(),
            kind,
            state,
            start_type: StartType::Demand,
            binary: Some("C:/svc.exe".to_owned()),
            pid: Some(40),
            win32_exit: 0,
            service_exit: 0,
            config_error: None,
        }
    }

    fn snapshot(facts: Vec<Fact>, coverage: Coverage) -> ServiceSnapshot {
        ServiceSnapshot {
            device: "device-1".to_owned(),
            boot: "boot-1".to_owned(),
            sampled_unix_ns: 10,
            facts,
            coverage,
            truncated: false,
            permission_denied: false,
            services_unreadable: false,
            driver_permission_denied: false,
            source_failure: None,
            evidence: "scm",
        }
    }

    fn types(events: &[Event]) -> Vec<&str> {
        events
            .iter()
            .map(|event| match &event.record {
                Record::Observation { event_type, .. } => event_type.as_str(),
                Record::Gap { .. } => "gap",
            })
            .collect()
    }

    fn named<'a>(events: &'a [Event], event_type: &str, name: &str) -> Vec<&'a Event> {
        events
            .iter()
            .filter(|event| match &event.record {
                Record::Observation {
                    event_type: kind,
                    subject: Some(subject),
                    ..
                } => kind.as_str() == event_type && subject.id.as_str() == name,
                _ => false,
            })
            .collect()
    }

    fn evidence(event: &Event) -> &str {
        match &event.record {
            Record::Observation {
                attribution: Attribution::Observed { evidence },
                ..
            } => evidence.as_str(),
            other => panic!("expected observed evidence, got {other:?}"),
        }
    }

    #[test]
    fn first_sample_records_state_and_a_repeat_is_empty() {
        let mut tracker = ServiceTracker::new();
        let running = fact("Spooler", Kind::Service, RunState::Running);
        let first = tracker
            .observe(&snapshot(vec![running.clone()], Coverage::Complete))
            .unwrap();
        assert_eq!(types(&first), vec!["service.state"]);
        assert_eq!(first[0].source.provider.as_str(), "windows.scm");
        assert!(first[0].time.precision_ns.is_none());
        assert!(matches!(
            first[0].record,
            Record::Observation { process: None, user: None, .. }
        ));
        let second = tracker
            .observe(&snapshot(vec![running], Coverage::Complete))
            .unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn an_already_stopped_service_is_not_a_stop_record() {
        let mut tracker = ServiceTracker::new();
        let events = tracker
            .observe(&snapshot(
                vec![fact("StoppedOne", Kind::Service, RunState::Stopped)],
                Coverage::Complete,
            ))
            .unwrap();
        assert_eq!(types(&events), vec!["service.state"]);
    }

    #[test]
    fn running_to_stopped_emits_one_stop_and_does_not_repeat() {
        let mut tracker = ServiceTracker::new();
        let mut running = fact("Worker", Kind::Service, RunState::Running);
        tracker
            .observe(&snapshot(vec![running.clone()], Coverage::Complete))
            .unwrap();
        running.state = RunState::Stopped;
        let stopped = tracker
            .observe(&snapshot(vec![running.clone()], Coverage::Complete))
            .unwrap();
        assert_eq!(types(&stopped), vec!["service.stopped"]);
        assert!(evidence(&stopped[0]).contains("state=stopped"));
        let again = tracker
            .observe(&snapshot(vec![running], Coverage::Complete))
            .unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn permission_and_source_failure_keep_the_baseline() {
        let mut tracker = ServiceTracker::new();
        let service = fact("Keep", Kind::Service, RunState::Running);
        tracker
            .observe(&snapshot(vec![service.clone()], Coverage::Complete))
            .unwrap();
        let mut denied = snapshot(Vec::new(), Coverage::Complete);
        denied.permission_denied = true;
        let events = tracker.observe(&denied).unwrap();
        assert_eq!(types(&events), vec!["gap", "service.permission"]);
        assert_eq!(evidence(&events[1]), "access-denied");
        let mut failed = snapshot(Vec::new(), Coverage::Complete);
        failed.source_failure = Some("scm:5".to_owned());
        assert_eq!(types(&tracker.observe(&failed).unwrap()), vec!["gap"]);
        let mut truncated = snapshot(Vec::new(), Coverage::Complete);
        truncated.truncated = true;
        assert!(tracker.observe(&truncated).unwrap().is_empty());
        let quiet = tracker
            .observe(&snapshot(vec![service], Coverage::Complete))
            .unwrap();
        assert!(quiet.is_empty(), "{:?}", types(&quiet));
    }

    #[test]
    fn an_identity_gap_keeps_the_service_and_driver_rows() {
        let mut tracker = ServiceTracker::new();
        let service = fact("Live", Kind::Service, RunState::Running);
        let mut denied = snapshot(vec![service], Coverage::Complete);
        denied.device.clear();
        denied.boot = "boot:unavailable".to_owned();
        denied.source_failure = Some("identity:5".to_owned());
        let events = tracker.observe(&denied).unwrap();
        assert_eq!(types(&events), vec!["gap", "service.state"]);
        let again = tracker.observe(&denied).unwrap();
        assert_eq!(types(&again), vec!["gap"]);

        let mut known = ServiceTracker::new();
        let driver = fact("disk", Kind::Driver, RunState::Running);
        known
            .observe(&snapshot(vec![driver.clone()], Coverage::Complete))
            .unwrap();
        let mut unreadable = snapshot(vec![driver], Coverage::Complete);
        unreadable.device.clear();
        unreadable.boot = "boot:unavailable".to_owned();
        unreadable.source_failure = Some("identity:5".to_owned());
        let kept = known.observe(&unreadable).unwrap();
        assert_eq!(types(&kept), vec!["gap"]);
        let gone = known
            .observe(&snapshot(Vec::new(), Coverage::Complete))
            .unwrap();
        assert_eq!(types(&gone), vec!["driver.unloaded", "driver.removed"]);
    }

    #[test]
    fn unreadable_services_and_driver_permission_do_not_invent_removals() {
        let mut tracker = ServiceTracker::new();
        let service = fact("Unit", Kind::Service, RunState::Running);
        let driver = fact("disk", Kind::Driver, RunState::Running);
        tracker
            .observe(&snapshot(vec![service, driver], Coverage::Complete))
            .unwrap();
        let mut blocked = snapshot(Vec::new(), Coverage::Complete);
        blocked.services_unreadable = true;
        blocked.driver_permission_denied = true;
        assert!(tracker.observe(&blocked).unwrap().is_empty());
    }

    #[test]
    fn creation_deletion_and_active_only_stop_stay_distinct() {
        let mut tracker = ServiceTracker::new();
        tracker
            .observe(&snapshot(Vec::new(), Coverage::Complete))
            .unwrap();
        let created = fact("Probe", Kind::Service, RunState::Stopped);
        let events = tracker
            .observe(&snapshot(vec![created.clone()], Coverage::Complete))
            .unwrap();
        assert_eq!(types(&events), vec!["service.created"]);
        let removed = tracker
            .observe(&snapshot(Vec::new(), Coverage::Complete))
            .unwrap();
        assert_eq!(types(&removed), vec!["service.deleted"]);

        let mut active = ServiceTracker::new();
        let running = fact("editor.service", Kind::Service, RunState::Running);
        active
            .observe(&snapshot(vec![running], Coverage::ActiveOnly))
            .unwrap();
        let stopped = active
            .observe(&snapshot(Vec::new(), Coverage::ActiveOnly))
            .unwrap();
        assert_eq!(types(&stopped), vec!["service.stopped"]);
    }

    #[test]
    fn a_running_service_that_disappears_records_stop_then_deletion() {
        let mut tracker = ServiceTracker::new();
        let running = fact("Live", Kind::Service, RunState::Running);
        tracker
            .observe(&snapshot(vec![running], Coverage::Complete))
            .unwrap();
        let events = tracker
            .observe(&snapshot(Vec::new(), Coverage::Complete))
            .unwrap();
        assert_eq!(types(&events), vec!["service.stopped", "service.deleted"]);
    }

    #[test]
    fn start_type_config_and_failure_are_deduplicated() {
        let mut tracker = ServiceTracker::new();
        let mut service = fact("Cfg", Kind::Service, RunState::Running);
        tracker
            .observe(&snapshot(vec![service.clone()], Coverage::Complete))
            .unwrap();
        service.start_type = StartType::Auto;
        service.binary = Some("C:/other.exe".to_owned());
        service.win32_exit = 2;
        let events = tracker
            .observe(&snapshot(vec![service.clone()], Coverage::Complete))
            .unwrap();
        assert_eq!(
            types(&events),
            vec!["service.start-type", "service.config", "service.failure"]
        );
        let same = tracker
            .observe(&snapshot(vec![service.clone()], Coverage::Complete))
            .unwrap();
        assert!(same.is_empty());
        service.win32_exit = 3;
        let changed = tracker
            .observe(&snapshot(vec![service], Coverage::Complete))
            .unwrap();
        assert_eq!(types(&changed), vec!["service.failure"]);
    }

    #[test]
    fn drivers_load_unload_and_removal_follow_coverage() {
        let mut tracker = ServiceTracker::new();
        tracker
            .observe(&snapshot(Vec::new(), Coverage::Complete))
            .unwrap();
        let running = fact("cdrom", Kind::Driver, RunState::Running);
        let loaded = tracker
            .observe(&snapshot(vec![running.clone()], Coverage::Complete))
            .unwrap();
        assert_eq!(types(&loaded), vec!["driver.loaded"]);
        let gone = tracker
            .observe(&snapshot(Vec::new(), Coverage::Complete))
            .unwrap();
        assert_eq!(types(&gone), vec!["driver.unloaded", "driver.removed"]);

        let mut installed = ServiceTracker::new();
        installed
            .observe(&snapshot(Vec::new(), Coverage::Complete))
            .unwrap();
        let stopped = fact("unused", Kind::Driver, RunState::Stopped);
        assert_eq!(
            types(
                &installed
                    .observe(&snapshot(vec![stopped.clone()], Coverage::Complete))
                    .unwrap()
            ),
            vec!["driver.state"]
        );
        assert_eq!(
            types(
                &installed
                    .observe(&snapshot(Vec::new(), Coverage::Complete))
                    .unwrap()
            ),
            vec!["driver.removed"]
        );

        let mut active = ServiceTracker::new();
        active
            .observe(&snapshot(vec![running], Coverage::ActiveOnly))
            .unwrap();
        assert_eq!(
            types(&active.observe(&snapshot(Vec::new(), Coverage::ActiveOnly)).unwrap()),
            vec!["driver.unloaded"]
        );
    }

    #[test]
    fn a_path_that_does_not_fit_is_omitted() {
        let mut tracker = ServiceTracker::new();
        let mut service = fact("Wide", Kind::Service, RunState::Running);
        service.binary = Some("P".repeat(300));
        let events = tracker
            .observe(&snapshot(vec![service], Coverage::Complete))
            .unwrap();
        let event = &events[0];
        assert!(!evidence(event).contains("path="));
        match &event.record {
            Record::Observation {
                completeness: Completeness::Partial { reason },
                ..
            } => assert_eq!(reason.as_str(), "config-exceeds-identifier"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn config_failure_stays_partial_and_a_boot_change_does_not_delete() {
        let mut tracker = ServiceTracker::new();
        let mut service = fact("Partial", Kind::Service, RunState::Running);
        service.config_error = Some(5);
        service.binary = None;
        let events = tracker
            .observe(&snapshot(vec![service], Coverage::Complete))
            .unwrap();
        match &events[0].record {
            Record::Observation {
                completeness: Completeness::Partial { reason },
                ..
            } => assert_eq!(reason.as_str(), "config-unavailable"),
            other => panic!("{other:?}"),
        }
        let mut restarted = snapshot(Vec::new(), Coverage::Complete);
        restarted.boot = "boot-2".to_owned();
        assert!(tracker.observe(&restarted).unwrap().is_empty());
        assert!(named(&events, "service.deleted", "Partial").is_empty());
    }

    #[test]
    fn live_capture_keeps_a_stable_identity_and_does_not_invent_rows() {
        let first = capture().expect("first service capture");
        let second = capture().expect("second service capture");
        assert_eq!(first.device, second.device);
        assert_eq!(first.boot, second.boot);
        assert!(matches!(first.evidence, "procfs" | "scm"));
        if first.facts.is_empty() {
            let mut tracker = ServiceTracker::new();
            assert!(tracker.observe(&first).unwrap().is_empty());
        }
    }
}

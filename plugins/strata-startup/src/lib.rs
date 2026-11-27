//! 启动项和计划任务的只读采样、归因和历史。
//!
//! 采集程序不创建、不修改、不删除启动项或计划任务。
//! Windows 读取模块使用 unsafe。跟踪器和解析器不使用。

use std::collections::{BTreeMap, BTreeSet};
#[cfg(any(target_os = "linux", test))]
use std::fs;
use std::io::{self, ErrorKind};
#[cfg(any(target_os = "linux", test))]
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, ObjectRef, Record,
    SignedNanos, Source, Version,
};

mod parse;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod host;

#[cfg(windows)]
#[path = "windows.rs"]
mod host;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("startup observation supports Windows and the Linux development host");

const DEVICE: &str = "strata.startup";
const MAX_EACH: usize = 1024;
#[cfg(any(target_os = "linux", test))]
const MAX_TEXT: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Startup,
    Task,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Folder,
    Run,
    RunOnce,
    PolicyRun,
    Autostart,
    Cron,
    Timer,
    ScheduledTask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Windows,
    HostFiles,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub id: String,
    pub kind: Kind,
    pub origin: Origin,
    pub scope: String,
    pub location: String,
    pub command: String,
    pub schedule: String,
    pub enabled: bool,
    pub principal: String,
    pub partial: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupSnapshot {
    pub device: String,
    pub boot: String,
    pub sampled_unix_ns: i64,
    pub facts: Vec<Fact>,
    pub coverage: Coverage,
    pub truncated: bool,
    pub unreadable_scopes: BTreeSet<String>,
    pub source_failure: Option<String>,
    pub evidence: &'static str,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, Default)]
pub(crate) struct Layout {
    pub autostart_system: Vec<PathBuf>,
    pub autostart_user: Vec<PathBuf>,
    pub autostart_unreadable: bool,
    pub system_crontabs: Vec<PathBuf>,
    pub cron_directories: Vec<PathBuf>,
    pub user_crontabs: Vec<PathBuf>,
    pub periodic: Vec<(String, PathBuf)>,
    pub cron_unreadable: bool,
    pub system_timer_dirs: Vec<PathBuf>,
    pub system_timer_wants: Vec<PathBuf>,
    pub system_timers_unreadable: bool,
    pub user_timer_dirs: Vec<PathBuf>,
    pub user_timer_wants: Vec<PathBuf>,
    pub user_timers_unreadable: bool,
}

pub struct StartupTracker {
    next: u64,
    device: Option<String>,
    boot: Option<String>,
    facts: BTreeMap<String, Fact>,
    opened: bool,
    history_gap: bool,
}

impl StartupTracker {
    pub fn new() -> Self {
        Self {
            next: 0,
            device: None,
            boot: None,
            facts: BTreeMap::new(),
            opened: false,
            history_gap: false,
        }
    }

    pub fn resume(&mut self, device: &str, facts: Vec<Fact>) {
        self.device = Some(device.to_owned());
        self.facts = facts
            .into_iter()
            .filter(|fact| valid_id(&fact.id))
            .map(|fact| (fact.id.clone(), fact))
            .collect();
        self.opened = true;
        self.boot = None;
        self.next = 0;
    }

    pub fn note_unreadable_history(&mut self) {
        self.history_gap = true;
    }

    pub fn export(&self) -> Option<(String, Vec<Fact>)> {
        let device = self.device.as_ref()?;
        if device.is_empty() || !self.opened {
            return None;
        }
        let mut facts: Vec<Fact> = self.facts.values().cloned().collect();
        facts.sort_by(|left, right| left.id.cmp(&right.id));
        Some((device.clone(), facts))
    }

    pub fn observe(&mut self, snapshot: &StartupSnapshot) -> io::Result<Vec<Event>> {
        let identity_gap = snapshot
            .source_failure
            .as_deref()
            .is_some_and(|reason| reason.starts_with("identity:"));
        if identity_gap && self.device.is_some() {
            if !snapshot.boot.is_empty() {
                self.boot = Some(snapshot.boot.clone());
            }
        } else if self.device.as_deref() != Some(snapshot.device.as_str()) {
            self.device = Some(snapshot.device.clone());
            self.boot = Some(snapshot.boot.clone());
            self.facts.clear();
            self.opened = false;
            self.next = 0;
        } else {
            self.boot = Some(snapshot.boot.clone());
        }
        let mut events = Vec::new();
        if snapshot.source_failure.is_some()
            || !snapshot.unreadable_scopes.is_empty()
            || self.history_gap
        {
            events.push(self.gap(snapshot)?);
        }
        self.history_gap = false;
        if snapshot
            .unreadable_scopes
            .iter()
            .any(|scope| scope_kind(scope) == Kind::Startup)
        {
            events.push(self.permission(snapshot, "startup.permission")?);
        }
        if snapshot
            .unreadable_scopes
            .iter()
            .any(|scope| scope_kind(scope) == Kind::Task)
        {
            events.push(self.permission(snapshot, "task.permission")?);
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

impl Default for StartupTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupTracker {
    fn baseline(&mut self, snapshot: &StartupSnapshot) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        for fact in visible(snapshot) {
            events.push(self.event(snapshot, fact, state_type(fact.kind), None)?);
            self.facts.insert(fact.id.clone(), fact.clone());
        }
        Ok(events)
    }

    fn diff(&mut self, snapshot: &StartupSnapshot) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        let mut seen = BTreeSet::new();
        for fact in visible(snapshot) {
            seen.insert(fact.id.clone());
            if let Some(previous) = self.facts.get(&fact.id).cloned() {
                events.extend(self.changes(snapshot, &previous, fact)?);
            } else {
                let event_type = match fact.kind {
                    Kind::Startup => "startup.added",
                    Kind::Task => "task.added",
                };
                events.push(self.event(snapshot, fact, event_type, None)?);
            }
            self.facts.insert(fact.id.clone(), fact.clone());
        }
        if snapshot.truncated {
            return Ok(events);
        }
        let missing = self
            .facts
            .values()
            .filter(|fact| {
                !snapshot.unreadable_scopes.contains(&fact.scope) && !seen.contains(&fact.id)
            })
            .map(|fact| fact.id.clone())
            .collect::<Vec<_>>();
        for id in missing {
            let Some(previous) = self.facts.remove(&id) else {
                continue;
            };
            let event_type = match previous.kind {
                Kind::Startup => "startup.removed",
                Kind::Task => "task.removed",
            };
            events.push(self.event(snapshot, &previous, event_type, None)?);
        }
        Ok(events)
    }

    fn changes(
        &mut self,
        snapshot: &StartupSnapshot,
        previous: &Fact,
        fact: &Fact,
    ) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        if previous.enabled != fact.enabled {
            let event_type = match (fact.kind, fact.enabled) {
                (Kind::Startup, true) => "startup.enabled",
                (Kind::Startup, false) => "startup.disabled",
                (Kind::Task, true) => "task.enabled",
                (Kind::Task, false) => "task.disabled",
            };
            events.push(self.event(snapshot, fact, event_type, None)?);
        }
        if previous.command != fact.command
            || previous.schedule != fact.schedule
            || previous.location != fact.location
            || previous.principal != fact.principal
        {
            let event_type = match fact.kind {
                Kind::Startup => "startup.changed",
                Kind::Task => "task.changed",
            };
            events.push(self.event(snapshot, fact, event_type, Some(previous))?);
        }
        Ok(events)
    }

    fn permission(&mut self, snapshot: &StartupSnapshot, event_type: &str) -> io::Result<Event> {
        self.observation(snapshot, event_type, None, "access-denied", None)
    }

    fn gap(&mut self, snapshot: &StartupSnapshot) -> io::Result<Event> {
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
        snapshot: &StartupSnapshot,
        fact: &Fact,
        event_type: &str,
        before: Option<&Fact>,
    ) -> io::Result<Event> {
        let (evidence, partial) = evidence_text(fact, before);
        self.observation(
            snapshot,
            event_type,
            Some((kind_token(fact.kind), fact.id.as_str())),
            &evidence,
            partial,
        )
    }

    fn observation(
        &mut self,
        snapshot: &StartupSnapshot,
        event_type: &str,
        subject: Option<(&str, &str)>,
        evidence: &str,
        partial: Option<&str>,
    ) -> io::Result<Event> {
        let sequence = self.bump()?;
        let evidence = Id::new(evidence).map_err(|error| invalid(error.to_string()))?;
        let partial = match partial {
            Some(reason) => Some(Id::new(reason).map_err(|error| invalid(error.to_string()))?),
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
            io::Error::new(
                ErrorKind::InvalidData,
                "startup sequence space is exhausted",
            )
        })?;
        Ok(self.next)
    }
}

pub fn capture() -> io::Result<StartupSnapshot> {
    host::capture()
}

#[cfg(target_os = "linux")]
pub fn user_config_dir() -> io::Result<PathBuf> {
    host::user_config_dir()
}

#[cfg(windows)]
pub fn user_startup_folder() -> io::Result<PathBuf> {
    host::user_startup_folder()
}

pub fn encode_inventory(device: &str, facts: &[Fact]) -> io::Result<String> {
    let facts = facts
        .iter()
        .map(|fact| {
            serde_json::json!({
                "id": fact.id,
                "kind": kind_token(fact.kind),
                "origin": origin_token(fact.origin),
                "scope": fact.scope,
                "location": fact.location,
                "command": fact.command,
                "schedule": fact.schedule,
                "enabled": fact.enabled,
                "principal": fact.principal,
                "partial": fact.partial,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&serde_json::json!({
        "version": 1,
        "device": device,
        "facts": facts,
    }))
    .map_err(|error| invalid(error.to_string()))
}

pub fn decode_inventory(text: &str) -> io::Result<(String, Vec<Fact>)> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| invalid(error.to_string()))?;
    if value.get("version").and_then(|item| item.as_u64()) != Some(1) {
        return Err(invalid("startup history version is not 1"));
    }
    let device = value
        .get("device")
        .and_then(|item| item.as_str())
        .filter(|text| valid_id(text))
        .ok_or_else(|| invalid("startup history device is invalid"))?
        .to_owned();
    let rows = value
        .get("facts")
        .and_then(|item| item.as_array())
        .ok_or_else(|| invalid("startup history facts are missing"))?;
    let mut facts = Vec::new();
    for row in rows {
        facts.push(fact_from_json(row)?);
    }
    Ok((device, facts))
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn from_layout(
    device: &str,
    boot: &str,
    layout: &Layout,
    sampled_unix_ns: i64,
) -> io::Result<StartupSnapshot> {
    let mut facts = Vec::new();
    let mut unreadable = BTreeSet::new();
    let mut truncated = false;
    if layout.autostart_unreadable {
        unreadable.insert("autostart".to_owned());
    } else {
        match read_autostart(layout) {
            Read::Rows(rows) => facts.extend(rows),
            Read::Denied => {
                unreadable.insert("autostart".to_owned());
            }
            Read::TooBig => truncated = true,
        }
    }
    if layout.cron_unreadable {
        unreadable.insert("cron".to_owned());
    } else {
        match read_cron(layout) {
            Read::Rows(rows) => facts.extend(rows),
            Read::Denied => {
                unreadable.insert("cron".to_owned());
            }
            Read::TooBig => truncated = true,
        }
    }
    if layout.system_timers_unreadable {
        unreadable.insert("timer-system".to_owned());
    } else {
        match read_timers(
            &layout.system_timer_dirs,
            &layout.system_timer_wants,
            "timer-system",
            "system",
        ) {
            Read::Rows(rows) => facts.extend(rows),
            Read::Denied => {
                unreadable.insert("timer-system".to_owned());
            }
            Read::TooBig => truncated = true,
        }
    }
    if layout.user_timers_unreadable {
        unreadable.insert("timer-user".to_owned());
    } else {
        match read_timers(
            &layout.user_timer_dirs,
            &layout.user_timer_wants,
            "timer-user",
            "user",
        ) {
            Read::Rows(rows) => facts.extend(rows),
            Read::Denied => {
                unreadable.insert("timer-user".to_owned());
            }
            Read::TooBig => truncated = true,
        }
    }
    let (facts, hit_cap) = limit_facts(facts);
    Ok(StartupSnapshot {
        device: device.to_owned(),
        boot: boot.to_owned(),
        sampled_unix_ns,
        facts,
        coverage: Coverage::HostFiles,
        truncated: truncated || hit_cap,
        unreadable_scopes: unreadable,
        source_failure: None,
        evidence: "host-files",
    })
}

#[cfg(any(target_os = "linux", test))]
enum Read {
    Rows(Vec<Fact>),
    Denied,
    TooBig,
}

#[cfg(any(target_os = "linux", test))]
fn read_autostart(layout: &Layout) -> Read {
    let mut chosen: BTreeMap<String, Fact> = BTreeMap::new();
    for (dirs, user) in [
        (&layout.autostart_system, false),
        (&layout.autostart_user, true),
    ] {
        for dir in dirs {
            match read_dir_files(dir) {
                Dir::Missing => continue,
                Dir::Denied => return Read::Denied,
                Dir::TooBig => return Read::TooBig,
                Dir::Files(files) => {
                    for path in files {
                        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                            continue;
                        };
                        if !name.ends_with(".desktop") {
                            continue;
                        }
                        let text = match read_text(&path) {
                            Text::Missing => continue,
                            Text::Denied => return Read::Denied,
                            Text::TooBig => return Read::TooBig,
                            Text::Body(text) => text,
                        };
                        let desktop = parse::parse_desktop(&text);
                        let id = format!("autostart:{name}");
                        if !valid_id(&id) {
                            continue;
                        }
                        let mut partial = None;
                        if desktop.exec.is_empty() {
                            partial = Some("command-absent".to_owned());
                        }
                        chosen.insert(
                            name.to_owned(),
                            Fact {
                                id,
                                kind: Kind::Startup,
                                origin: Origin::Autostart,
                                scope: "autostart".to_owned(),
                                location: path.display().to_string(),
                                command: desktop.exec,
                                schedule: String::new(),
                                enabled: desktop.enabled,
                                principal: if user {
                                    "user".to_owned()
                                } else {
                                    "system".to_owned()
                                },
                                partial,
                            },
                        );
                    }
                }
            }
        }
    }
    Read::Rows(chosen.into_values().collect())
}

#[cfg(any(target_os = "linux", test))]
fn read_cron(layout: &Layout) -> Read {
    let mut facts = Vec::new();
    for path in &layout.system_crontabs {
        if let Some(read) = push_crontab(&mut facts, path, "crontab", true, "") {
            return read;
        }
    }
    for dir in &layout.cron_directories {
        match read_dir_files(dir) {
            Dir::Missing => continue,
            Dir::Denied => return Read::Denied,
            Dir::TooBig => return Read::TooBig,
            Dir::Files(files) => {
                for path in files {
                    let stem = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("cron");
                    if let Some(read) = push_crontab(&mut facts, &path, "d", true, stem) {
                        return read;
                    }
                }
            }
        }
    }
    for path in &layout.user_crontabs {
        let stem = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("user");
        if let Some(read) = push_crontab(&mut facts, path, "user", false, stem) {
            return read;
        }
    }
    for (label, dir) in &layout.periodic {
        match read_dir_files(dir) {
            Dir::Missing => continue,
            Dir::Denied => return Read::Denied,
            Dir::TooBig => return Read::TooBig,
            Dir::Files(files) => {
                for path in files {
                    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    let id = format!("cron:{label}:{name}");
                    if !valid_id(&id) {
                        continue;
                    }
                    facts.push(Fact {
                        id,
                        kind: Kind::Task,
                        origin: Origin::Cron,
                        scope: "cron".to_owned(),
                        location: path.display().to_string(),
                        command: path.display().to_string(),
                        schedule: label.clone(),
                        enabled: true,
                        principal: String::new(),
                        partial: None,
                    });
                }
            }
        }
    }
    Read::Rows(facts)
}

#[cfg(any(target_os = "linux", test))]
fn push_crontab(
    facts: &mut Vec<Fact>,
    path: &Path,
    bucket: &str,
    system: bool,
    stem: &str,
) -> Option<Read> {
    let text = match read_text(path) {
        Text::Missing => return None,
        Text::Denied => return Some(Read::Denied),
        Text::TooBig => return Some(Read::TooBig),
        Text::Body(text) => text,
    };
    for entry in parse::parse_crontab(&text, system) {
        let id = if stem.is_empty() {
            format!("cron:{bucket}:{}", entry.line)
        } else {
            format!("cron:{bucket}:{stem}:{}", entry.line)
        };
        if !valid_id(&id) {
            continue;
        }
        facts.push(Fact {
            id,
            kind: Kind::Task,
            origin: Origin::Cron,
            scope: "cron".to_owned(),
            location: path.display().to_string(),
            command: entry.command,
            schedule: entry.schedule,
            enabled: true,
            principal: entry.user,
            partial: None,
        });
    }
    None
}

#[cfg(any(target_os = "linux", test))]
fn read_timers(dirs: &[PathBuf], wants: &[PathBuf], scope: &str, principal: &str) -> Read {
    let mut chosen: BTreeMap<String, Fact> = BTreeMap::new();
    for dir in dirs {
        match read_dir_files(dir) {
            Dir::Missing => continue,
            Dir::Denied => return Read::Denied,
            Dir::TooBig => return Read::TooBig,
            Dir::Files(files) => {
                for path in files {
                    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    if !name.ends_with(".timer") {
                        continue;
                    }
                    let text = match read_text(&path) {
                        Text::Missing => continue,
                        Text::Denied => return Read::Denied,
                        Text::TooBig => return Read::TooBig,
                        Text::Body(text) => text,
                    };
                    let (schedule, unit) = parse::parse_timer(&text);
                    let service_name = if unit.is_empty() {
                        format!("{}.service", name.trim_end_matches(".timer"))
                    } else {
                        unit
                    };
                    let command = exec_from_dirs(dirs, &service_name);
                    let id = format!("timer:{principal}:{name}");
                    if !valid_id(&id) {
                        continue;
                    }
                    let enabled = wants.iter().any(|dir| marker_exists(dir, name));
                    chosen.insert(
                        name.to_owned(),
                        Fact {
                            id,
                            kind: Kind::Task,
                            origin: Origin::Timer,
                            scope: scope.to_owned(),
                            location: path.display().to_string(),
                            command,
                            schedule,
                            enabled,
                            principal: principal.to_owned(),
                            partial: None,
                        },
                    );
                }
            }
        }
    }
    Read::Rows(chosen.into_values().collect())
}

#[cfg(any(target_os = "linux", test))]
fn exec_from_dirs(dirs: &[PathBuf], service_name: &str) -> String {
    if service_name.contains('/') || service_name.contains('\\') || service_name.is_empty() {
        return String::new();
    }
    for dir in dirs.iter().rev() {
        let path = dir.join(service_name);
        if let Text::Body(text) = read_text(&path) {
            let exec = parse::parse_exec_start(&text);
            if !exec.is_empty() {
                return exec;
            }
        }
    }
    String::new()
}

#[cfg(any(target_os = "linux", test))]
fn marker_exists(dir: &Path, name: &str) -> bool {
    fs::symlink_metadata(dir.join(name)).is_ok()
}

#[cfg(any(target_os = "linux", test))]
enum Dir {
    Missing,
    Denied,
    TooBig,
    Files(Vec<PathBuf>),
}

#[cfg(any(target_os = "linux", test))]
fn read_dir_files(dir: &Path) -> Dir {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Dir::Missing,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return Dir::Denied,
        Err(_) => return Dir::Denied,
    };
    let mut files = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        if files.len() >= 4096 {
            return Dir::TooBig;
        }
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            match entry.metadata() {
                Ok(target) if target.is_file() => files.push(entry.path()),
                _ => continue,
            }
        } else if meta.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Dir::Files(files)
}

#[cfg(any(target_os = "linux", test))]
enum Text {
    Missing,
    Denied,
    TooBig,
    Body(String),
}

#[cfg(any(target_os = "linux", test))]
fn read_text(path: &Path) -> Text {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == ErrorKind::NotFound => return Text::Missing,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => return Text::Denied,
        Err(_) => return Text::Denied,
    };
    if meta.is_dir() {
        return Text::Missing;
    }
    if meta.len() > MAX_TEXT {
        return Text::TooBig;
    }
    match fs::read(path) {
        Ok(bytes) => Text::Body(String::from_utf8_lossy(&bytes).into_owned()),
        Err(error) if error.kind() == ErrorKind::NotFound => Text::Missing,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => Text::Denied,
        Err(_) => Text::Denied,
    }
}

pub(crate) fn limit_facts(mut facts: Vec<Fact>) -> (Vec<Fact>, bool) {
    facts.retain(|fact| valid_id(&fact.id));
    facts.sort_by(|left, right| left.id.cmp(&right.id));
    facts.dedup_by(|left, right| left.id == right.id);
    let mut startup = 0usize;
    let mut tasks = 0usize;
    let mut kept = Vec::new();
    let mut truncated = false;
    for fact in facts {
        let count = match fact.kind {
            Kind::Startup => &mut startup,
            Kind::Task => &mut tasks,
        };
        if *count >= MAX_EACH {
            truncated = true;
            continue;
        }
        *count += 1;
        kept.push(fact);
    }
    (kept, truncated)
}

#[cfg_attr(all(not(windows), not(test)), allow(dead_code))]
pub(crate) fn startup_enabled(bytes: Option<&[u8]>) -> bool {
    match bytes {
        None | Some([]) => true,
        Some(bytes) => bytes[0] & 0x1 == 0,
    }
}

fn fact_from_json(value: &serde_json::Value) -> io::Result<Fact> {
    let id = json_str(value, "id")?;
    if !valid_id(&id) {
        return Err(invalid("startup history id is invalid"));
    }
    let fact = Fact {
        id,
        kind: kind_from(json_str(value, "kind")?)?,
        origin: origin_from(json_str(value, "origin")?)?,
        scope: json_str(value, "scope")?,
        location: json_str(value, "location")?,
        command: json_str(value, "command")?,
        schedule: json_str(value, "schedule")?,
        enabled: value
            .get("enabled")
            .and_then(|item| item.as_bool())
            .ok_or_else(|| invalid("startup history enabled flag is missing"))?,
        principal: json_str(value, "principal")?,
        partial: match value.get("partial") {
            None | Some(serde_json::Value::Null) => None,
            Some(item) => Some(
                item.as_str()
                    .ok_or_else(|| invalid("startup history partial is invalid"))?
                    .to_owned(),
            ),
        },
    };
    if fact.scope.is_empty() || scope_kind(&fact.scope) != fact.kind {
        return Err(invalid("startup history scope does not match its kind"));
    }
    Ok(fact)
}

fn json_str(value: &serde_json::Value, key: &str) -> io::Result<String> {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("startup history {key} is missing")))
}

fn source(snapshot: &StartupSnapshot) -> io::Result<Source> {
    let provider = if snapshot.evidence == "windows" {
        "windows.startup"
    } else {
        "posix.startup"
    };
    Ok(Source {
        device: Id::new(DEVICE).map_err(|error| invalid(error.to_string()))?,
        provider: Id::new(provider).map_err(|error| invalid(error.to_string()))?,
        epoch: Id::new(&snapshot.boot).map_err(|error| invalid(error.to_string()))?,
    })
}

fn visible(snapshot: &StartupSnapshot) -> impl Iterator<Item = &Fact> {
    snapshot
        .facts
        .iter()
        .filter(|fact| !snapshot.unreadable_scopes.contains(&fact.scope) && valid_id(&fact.id))
}

fn scope_kind(scope: &str) -> Kind {
    if scope == "cron" || scope == "tasks" || scope.starts_with("timer") {
        Kind::Task
    } else {
        Kind::Startup
    }
}

fn valid_id(value: &str) -> bool {
    Id::new(value).is_ok()
}

fn state_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Startup => "startup.state",
        Kind::Task => "task.state",
    }
}

fn kind_token(kind: Kind) -> &'static str {
    match kind {
        Kind::Startup => "startup",
        Kind::Task => "task",
    }
}

fn kind_from(value: String) -> io::Result<Kind> {
    match value.as_str() {
        "startup" => Ok(Kind::Startup),
        "task" => Ok(Kind::Task),
        _ => Err(invalid("startup history kind is invalid")),
    }
}

fn origin_token(origin: Origin) -> &'static str {
    match origin {
        Origin::Folder => "folder",
        Origin::Run => "run",
        Origin::RunOnce => "runonce",
        Origin::PolicyRun => "policy-run",
        Origin::Autostart => "autostart",
        Origin::Cron => "cron",
        Origin::Timer => "timer",
        Origin::ScheduledTask => "scheduled-task",
    }
}

fn origin_from(value: String) -> io::Result<Origin> {
    match value.as_str() {
        "folder" => Ok(Origin::Folder),
        "run" => Ok(Origin::Run),
        "runonce" => Ok(Origin::RunOnce),
        "policy-run" => Ok(Origin::PolicyRun),
        "autostart" => Ok(Origin::Autostart),
        "cron" => Ok(Origin::Cron),
        "timer" => Ok(Origin::Timer),
        "scheduled-task" => Ok(Origin::ScheduledTask),
        _ => Err(invalid("startup history origin is invalid")),
    }
}

fn evidence_text(fact: &Fact, before: Option<&Fact>) -> (String, Option<&'static str>) {
    let mut text = format!(
        "origin={};scope={};enabled={}",
        origin_token(fact.origin),
        fact.scope,
        if fact.enabled { "1" } else { "0" }
    );
    let mut partial = None;
    push_field(
        &mut text,
        &mut partial,
        "principal",
        &fact.principal,
        "principal-exceeds-identifier",
    );
    push_field(
        &mut text,
        &mut partial,
        "location",
        &fact.location,
        "location-exceeds-identifier",
    );
    push_field(
        &mut text,
        &mut partial,
        "command",
        &fact.command,
        "command-exceeds-identifier",
    );
    push_field(
        &mut text,
        &mut partial,
        "schedule",
        &fact.schedule,
        "schedule-exceeds-identifier",
    );
    if let Some(previous) = before {
        if previous.command != fact.command {
            push_field(
                &mut text,
                &mut partial,
                "before",
                &previous.command,
                "before-exceeds-identifier",
            );
        }
        if previous.schedule != fact.schedule {
            push_field(
                &mut text,
                &mut partial,
                "before-schedule",
                &previous.schedule,
                "before-exceeds-identifier",
            );
        }
        if previous.location != fact.location {
            push_field(
                &mut text,
                &mut partial,
                "before-location",
                &previous.location,
                "before-exceeds-identifier",
            );
        }
        if previous.principal != fact.principal {
            push_field(
                &mut text,
                &mut partial,
                "before-principal",
                &previous.principal,
                "before-exceeds-identifier",
            );
        }
    }
    if partial.is_none() {
        if let Some(reason) = fact.partial.as_deref() {
            partial = Some(match reason {
                "command-absent" => "command-absent",
                "link-unresolved" => "link-unresolved",
                "approved-unreadable" => "approved-unreadable",
                "extra-actions" => "extra-actions",
                "task-unreadable" => "task-unreadable",
                _ => "partial",
            });
        }
    }
    (text, partial)
}

fn push_field(
    text: &mut String,
    partial: &mut Option<&'static str>,
    name: &str,
    value: &str,
    reason: &'static str,
) {
    if value.is_empty() || partial.is_some() {
        return;
    }
    if value.contains(';') || value.chars().any(char::is_control) {
        *partial = Some(reason);
        return;
    }
    let candidate = format!("{text};{name}={value}");
    if candidate.len() > 256 || Id::new(&candidate).is_err() {
        *partial = Some(reason);
        return;
    }
    *text = candidate;
}

pub(crate) fn unix_now() -> io::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid("system clock is before the Unix epoch"))?;
    i64::try_from(elapsed.as_nanos())
        .map_err(|_| invalid("system clock does not fit in signed nanoseconds"))
}

fn invalid(detail: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, detail.into())
}

#[cfg(test)]
pub(crate) fn blank_snapshot(
    facts: Vec<Fact>,
    unreadable: &[&str],
    truncated: bool,
) -> StartupSnapshot {
    StartupSnapshot {
        device: "device-1".to_owned(),
        boot: "boot-1".to_owned(),
        sampled_unix_ns: 1,
        facts,
        coverage: Coverage::HostFiles,
        truncated,
        unreadable_scopes: unreadable.iter().map(|scope| (*scope).to_owned()).collect(),
        source_failure: None,
        evidence: "host-files",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(id: &str, kind: Kind, command: &str, enabled: bool) -> Fact {
        Fact {
            id: id.to_owned(),
            kind,
            origin: if kind == Kind::Startup {
                Origin::Run
            } else {
                Origin::Cron
            },
            scope: if kind == Kind::Startup {
                "hkcu-run".to_owned()
            } else {
                "cron".to_owned()
            },
            location: format!("loc-{id}"),
            command: command.to_owned(),
            schedule: String::new(),
            enabled,
            principal: "user".to_owned(),
            partial: None,
        }
    }

    fn evidence_of(event: &Event) -> &str {
        match &event.record {
            Record::Observation {
                attribution: Attribution::Observed { evidence },
                ..
            } => evidence.as_str(),
            other => panic!("expected observed attribution, got {other:?}"),
        }
    }

    fn event_type(event: &Event) -> &str {
        match &event.record {
            Record::Observation { event_type, .. } => event_type.as_str(),
            Record::Gap { .. } => "gap",
        }
    }

    #[test]
    fn changes_keep_before_and_enabled_and_history_reloads() {
        let mut tracker = StartupTracker::new();
        let first = blank_snapshot(
            vec![fact("run:probe", Kind::Startup, "cmd-a", true)],
            &[],
            false,
        );
        let opened = tracker.observe(&first).unwrap();
        assert_eq!(event_type(&opened[0]), "startup.state");
        assert!(evidence_of(&opened[0]).contains("command=cmd-a"));
        let mut changed = fact("run:probe", Kind::Startup, "cmd-b", false);
        changed.schedule = "daily".to_owned();
        let second = blank_snapshot(vec![changed], &[], false);
        let types: Vec<_> = tracker
            .observe(&second)
            .unwrap()
            .iter()
            .map(event_type)
            .map(str::to_owned)
            .collect();
        assert_eq!(
            types,
            vec!["startup.disabled".to_owned(), "startup.changed".to_owned()]
        );
        let events = tracker.observe(&second).unwrap();
        assert!(events.is_empty());
        let (device, saved) = tracker.export().unwrap();
        let encoded = encode_inventory(&device, &saved).unwrap();
        let (loaded_device, loaded) = decode_inventory(&encoded).unwrap();
        let mut restarted = StartupTracker::new();
        restarted.resume(&loaded_device, loaded);
        let mut next = fact("run:probe", Kind::Startup, "cmd-c", false);
        next.schedule = "daily".to_owned();
        let again = blank_snapshot(vec![next], &[], false);
        let reloaded = restarted.observe(&again).unwrap();
        assert_eq!(event_type(&reloaded[0]), "startup.changed");
        assert!(evidence_of(&reloaded[0]).contains("before=cmd-b"));
        assert!(
            !reloaded
                .iter()
                .any(|event| event_type(event) == "startup.state")
        );
    }

    #[test]
    fn unreadable_truncated_and_other_boot_do_not_invent_removals() {
        let mut tracker = StartupTracker::new();
        let initial = blank_snapshot(
            vec![
                fact("run:keep", Kind::Startup, "a", true),
                fact("cron:keep", Kind::Task, "b", true),
            ],
            &[],
            false,
        );
        tracker.observe(&initial).unwrap();
        let hidden = blank_snapshot(vec![], &["hkcu-run", "cron"], false);
        let events = tracker.observe(&hidden).unwrap();
        assert!(events.iter().any(|event| event_type(event) == "gap"));
        assert!(
            events
                .iter()
                .any(|event| event_type(event) == "startup.permission")
        );
        assert!(
            events
                .iter()
                .any(|event| event_type(event) == "task.permission")
        );
        assert!(
            !events
                .iter()
                .any(|event| event_type(event).ends_with("removed"))
        );
        assert!(tracker.export().unwrap().1.len() == 2);
        let mut truncated = initial.clone();
        truncated.facts.pop();
        truncated.truncated = true;
        let events = tracker.observe(&truncated).unwrap();
        assert!(
            !events
                .iter()
                .any(|event| event_type(event).ends_with("removed"))
        );
        let mut rebooted = initial.clone();
        rebooted.boot = "boot-2".to_owned();
        rebooted.facts.clear();
        let events = tracker.observe(&rebooted).unwrap();
        let types: Vec<_> = events.iter().map(event_type).collect();
        assert_eq!(types, vec!["task.removed", "startup.removed"]);
        let mut other = initial.clone();
        other.device = "device-2".to_owned();
        other.facts.clear();
        let events = tracker.observe(&other).unwrap();
        assert!(
            events
                .iter()
                .all(|event| event_type(event) != "startup.removed")
        );
    }

    #[test]
    fn corrupt_history_baselines_without_deleting_and_identity_gap_keeps_rows() {
        let mut tracker = StartupTracker::new();
        tracker.note_unreadable_history();
        let sample = blank_snapshot(
            vec![fact("run:new", Kind::Startup, "new", true)],
            &[],
            false,
        );
        let events = tracker.observe(&sample).unwrap();
        assert_eq!(event_type(&events[0]), "gap");
        assert!(
            events
                .iter()
                .any(|event| event_type(event) == "startup.state")
        );
        assert!(
            !events
                .iter()
                .any(|event| event_type(event).ends_with("removed"))
        );
        let mut denied = sample.clone();
        denied.device.clear();
        denied.boot = "boot:unavailable".to_owned();
        denied.source_failure = Some("identity:5".to_owned());
        denied.facts.clear();
        denied.unreadable_scopes.insert("hkcu-run".to_owned());
        let events = tracker.observe(&denied).unwrap();
        assert!(events.iter().any(|event| event_type(event) == "gap"));
        assert!(
            !events
                .iter()
                .any(|event| event_type(event).ends_with("removed"))
        );
        assert!(decode_inventory("{\"version\":2}").is_err());
    }

    #[test]
    fn desktop_cron_timer_and_link_parsers_match_the_files() {
        let root = std::env::temp_dir().join(format!(
            "winstrata-startup-layout-{}-{}",
            std::process::id(),
            unix_now().unwrap()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("system")).unwrap();
        fs::create_dir_all(root.join("user")).unwrap();
        fs::create_dir_all(root.join("cron.d")).unwrap();
        fs::create_dir_all(root.join("timers")).unwrap();
        fs::create_dir_all(root.join("wants")).unwrap();
        fs::write(
            root.join("system/probe.desktop"),
            "[Desktop Entry]\nExec=/bin/system\n",
        )
        .unwrap();
        fs::write(
            root.join("user/probe.desktop"),
            "[Desktop Entry]\nHidden=true\nExec=/bin/user\nX-GNOME-Autostart-enabled=false\n",
        )
        .unwrap();
        fs::write(
            root.join("crontab"),
            "# comment\nPATH=/bin\n15 1 * * * root /bin/true --flag\n",
        )
        .unwrap();
        fs::write(
            root.join("timers/probe.timer"),
            "[Timer]\nOnCalendar=daily\nUnit=probe.service\n",
        )
        .unwrap();
        fs::write(
            root.join("timers/probe.service"),
            "[Service]\nExecStart=/bin/probe --now\n",
        )
        .unwrap();
        fs::write(root.join("wants/probe.timer"), "").unwrap();
        let layout = Layout {
            autostart_system: vec![root.join("system")],
            autostart_user: vec![root.join("user")],
            system_crontabs: vec![root.join("crontab")],
            system_timer_dirs: vec![root.join("timers")],
            system_timer_wants: vec![root.join("wants")],
            ..Layout::default()
        };
        let snapshot = from_layout("device-1", "boot-1", &layout, 1).unwrap();
        let desktop = snapshot
            .facts
            .iter()
            .find(|fact| fact.id == "autostart:probe.desktop")
            .unwrap();
        assert_eq!(desktop.command, "/bin/user");
        assert!(!desktop.enabled);
        assert_eq!(desktop.principal, "user");
        let cron = snapshot
            .facts
            .iter()
            .find(|fact| fact.origin == Origin::Cron)
            .unwrap();
        assert_eq!(cron.command, "/bin/true --flag");
        assert_eq!(cron.principal, "root");
        assert_eq!(cron.schedule, "15 1 * * *");
        let timer = snapshot
            .facts
            .iter()
            .find(|fact| fact.origin == Origin::Timer)
            .unwrap();
        assert_eq!(timer.command, "/bin/probe --now");
        assert_eq!(timer.schedule, "OnCalendar=daily");
        assert!(timer.enabled);
        let mut link = vec![0u8; 76];
        link[0] = 0x4c;
        link[20] = 0x02;
        let mut info = vec![0u8; 0x1c];
        let path = b"C:\\Probe.exe\0";
        let info_size = (0x1c + path.len()) as u32;
        info[0..4].copy_from_slice(&info_size.to_le_bytes());
        info[4..8].copy_from_slice(&0x1cu32.to_le_bytes());
        info[8..12].copy_from_slice(&1u32.to_le_bytes());
        info[16..20].copy_from_slice(&0x1cu32.to_le_bytes());
        link.extend(info);
        link.extend(path);
        assert_eq!(
            parse::link_local_path(&link).as_deref(),
            Some("C:\\Probe.exe")
        );
        assert!(parse::link_local_path(&[0, 1, 2]).is_none());
        assert!(startup_enabled(None));
        assert!(startup_enabled(Some(&[0x02])));
        assert!(startup_enabled(Some(&[0x06])));
        assert!(!startup_enabled(Some(&[0x03])));
        assert!(!startup_enabled(Some(&[0x01])));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn oversized_command_stays_partial_and_live_capture_does_not_invent_a_provider() {
        let mut wide = fact("run:wide", Kind::Startup, &"x".repeat(300), true);
        wide.location = "y".repeat(300);
        let (text, partial) = evidence_text(&wide, None);
        assert!(text.len() <= 256);
        assert_eq!(partial, Some("location-exceeds-identifier"));
        assert!(!text.contains("command="));
        let first = capture().expect("live startup capture");
        let second = capture().expect("second live startup capture");
        assert_eq!(first.device, second.device);
        assert_eq!(first.boot, second.boot);
        assert!(matches!(first.evidence, "host-files" | "windows"));
        assert_ne!(first.device, "");
        let mut tracker = StartupTracker::new();
        let events = tracker.observe(&first).unwrap();
        if first.facts.is_empty() {
            assert!(
                events
                    .iter()
                    .all(|event| event_type(event) == "gap" || first.unreadable_scopes.is_empty())
            );
        } else {
            assert!(
                events
                    .iter()
                    .any(|event| { matches!(event_type(event), "startup.state" | "task.state") })
            );
        }
    }
}

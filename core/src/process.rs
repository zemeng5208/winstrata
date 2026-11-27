//! 进程树、PID 生命周期、服务间接关联和采样去重。
//!
//! 进程实例由启动标识、PID 和启动时间组成。同一个 PID 只有在启动时间变化时才成为新实例。
//! 服务控制管理器或 cgroup 单元提供的关联是推断，不会再记一次进程启动。

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, ErrorKind};

use winstrata_kernel::{ModuleRow, ProcessRow, ServiceRow, SystemSnapshot};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, Id, ObjectRef, ProcessRef, Record,
    SignedNanos, Source, Version,
};

pub const PROCESS_PROVIDER: &str = "winstrata.process";
pub const PROCESS_STARTED: &str = "process.started";
pub const PROCESS_EXITED: &str = "process.exited";
pub const THREAD_STARTED: &str = "process.thread.started";
pub const THREAD_EXITED: &str = "process.thread.exited";
pub const MODULE_LOADED: &str = "process.module.loaded";
pub const MODULE_UNLOADED: &str = "process.module.unloaded";
pub const PROCESS_SERVICE: &str = "process.service";
pub const PROCESS_UNRESOLVED: &str = "process.unresolved";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessNode {
    pub pid: u32,
    pub start_unix_ns: i64,
    pub parent_pid: Option<u32>,
    pub parent_start_unix_ns: Option<i64>,
    pub name: String,
    pub image: Option<String>,
    pub services: Vec<String>,
    pub threads: Vec<u32>,
    pub modules: Vec<ModuleRow>,
}

#[derive(Clone, Debug)]
struct Live {
    start: i64,
    parent_pid: Option<u32>,
    parent_start: Option<i64>,
    name: String,
    image: Option<String>,
    threads: BTreeSet<u32>,
    modules: BTreeSet<(String, u64)>,
    services: BTreeSet<String>,
    modules_truncated: bool,
    threads_truncated: bool,
}

struct Epoch {
    source: Source,
    precision_ns: Option<u64>,
    evidence: Id,
    next_sequence: u64,
    live: HashMap<u32, Live>,
    unresolved: BTreeSet<u32>,
    loose_services: BTreeSet<(String, u32)>,
}

pub struct ProcessTracker {
    epoch: Option<Epoch>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        Self { epoch: None }
    }

    pub fn tree(&self) -> Vec<ProcessNode> {
        let Some(epoch) = &self.epoch else {
            return Vec::new();
        };
        let mut nodes = epoch
            .live
            .iter()
            .map(|(pid, live)| ProcessNode {
                pid: *pid,
                start_unix_ns: live.start,
                parent_pid: live.parent_pid,
                parent_start_unix_ns: live.parent_start,
                name: live.name.clone(),
                image: live.image.clone(),
                services: live.services.iter().cloned().collect(),
                threads: live.threads.iter().copied().collect(),
                modules: live
                    .modules
                    .iter()
                    .map(|(name, base)| ModuleRow {
                        name: name.clone(),
                        base: *base,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| (node.start_unix_ns, node.pid));
        nodes
    }

    pub fn observe(&mut self, snapshot: &SystemSnapshot) -> io::Result<Vec<Event>> {
        let mut events = Vec::new();
        if self.epoch.as_ref().is_some_and(|epoch| {
            epoch.source.device.as_str() != snapshot.device
                || epoch.source.epoch.as_str() != snapshot.boot
        }) {
            events.extend(self.close_epoch(snapshot.sampled_unix_ns)?);
        }
        if self.epoch.is_none() {
            self.epoch = Some(Epoch::open(snapshot)?);
        }
        let rows = snapshot
            .processes
            .iter()
            .map(OwnedProcess::from)
            .collect::<Vec<_>>();
        events.extend(self.reconcile(snapshot, &rows)?);
        Ok(events)
    }

    fn close_epoch(&mut self, sampled_unix_ns: i64) -> io::Result<Vec<Event>> {
        let Some(epoch) = self.epoch.as_mut() else {
            return Ok(Vec::new());
        };
        let mut pids = epoch.live.keys().copied().collect::<Vec<_>>();
        pids.sort_unstable();
        let mut events = Vec::new();
        for pid in pids {
            events.push(exit_event(epoch, sampled_unix_ns, pid)?);
            epoch.live.remove(&pid);
        }
        self.epoch = None;
        Ok(events)
    }

    fn reconcile(
        &mut self,
        snapshot: &SystemSnapshot,
        rows: &[OwnedProcess],
    ) -> io::Result<Vec<Event>> {
        let epoch = self.epoch.as_mut().expect("epoch is open");
        let mut events = Vec::new();
        let resolved = resolved_rows(rows);
        let present = resolved.keys().copied().collect::<HashSet<_>>();
        let unresolved_rows = rows
            .iter()
            .filter(|row| row.start.is_none() && !present.contains(&row.pid))
            .map(|row| row.pid)
            .collect::<BTreeSet<_>>();

        if !snapshot.processes_truncated {
            let mut exited = epoch
                .live
                .keys()
                .copied()
                .filter(|pid| match resolved.get(pid) {
                    Some(row) => row.start != Some(epoch.live[pid].start),
                    None => !unresolved_rows.contains(pid),
                })
                .collect::<Vec<_>>();
            exited.sort_unstable();
            for pid in exited {
                let start_changed = resolved.get(&pid).is_some();
                events.push(exit_event(epoch, snapshot.sampled_unix_ns, pid)?);
                epoch.live.remove(&pid);
                if !start_changed {
                    epoch.unresolved.remove(&pid);
                }
            }
        }

        // 截断的采样不能用另一条启动时间覆盖仍在跟踪的 PID，否则会丢掉没有退出事件的实例。
        let mut births = resolved
            .keys()
            .copied()
            .filter(|pid| !epoch.live.contains_key(pid))
            .collect::<Vec<_>>();
        births.sort_unstable();
        for pid in births {
            let row = &resolved[&pid];
            let start = row.start.expect("resolved row has a start time");
            epoch.live.insert(pid, Live::new(start, row));
            epoch.unresolved.remove(&pid);
            events.push(observation(
                epoch,
                snapshot.sampled_unix_ns,
                PROCESS_STARTED,
                "process",
                &format!("pid:{pid}:{start}"),
                Some(process_ref(&epoch.source.epoch, pid, start)?),
                Attribution::Observed {
                    evidence: id(snapshot.evidence)?,
                },
                Completeness::Complete,
            )?);
        }

        relink(epoch, &resolved);
        break_cycles(epoch);

        let mut updates = resolved.keys().copied().collect::<Vec<_>>();
        updates.sort_unstable();
        for pid in updates {
            let row = &resolved[&pid];
            let Some(live) = epoch.live.get(&pid) else {
                continue;
            };
            if live.start != row.start.unwrap_or(live.start) {
                continue;
            }
            events.extend(diff_threads(epoch, snapshot, pid, row)?);
            events.extend(diff_modules(epoch, snapshot, pid, row)?);
            if let Some(live) = epoch.live.get_mut(&pid) {
                live.name.clone_from(&row.name);
                live.image.clone_from(&row.image);
                live.modules_truncated = row.modules_truncated;
                live.threads_truncated = row.threads_truncated;
            }
        }

        events.extend(diff_services(epoch, snapshot)?);
        events.extend(diff_unresolved(epoch, snapshot, &unresolved_rows)?);
        Ok(events)
    }
}

impl Default for ProcessTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct OwnedProcess {
    pid: u32,
    parent_pid: u32,
    start: Option<i64>,
    name: String,
    image: Option<String>,
    threads: BTreeSet<u32>,
    modules: BTreeSet<(String, u64)>,
    modules_truncated: bool,
    threads_truncated: bool,
}

impl OwnedProcess {
    fn from(row: &ProcessRow) -> Self {
        Self {
            pid: row.pid,
            parent_pid: row.parent_pid,
            start: row.start_unix_ns.filter(|start| *start > 0),
            name: row.name.clone(),
            image: row.image.clone(),
            threads: row.threads.iter().copied().collect(),
            modules: row
                .modules
                .iter()
                .map(|module| (module.name.clone(), module.base))
                .collect(),
            modules_truncated: row.modules_truncated,
            threads_truncated: row.threads_truncated,
        }
    }
}

impl Live {
    fn new(start: i64, row: &OwnedProcess) -> Self {
        Self {
            start,
            parent_pid: None,
            parent_start: None,
            name: row.name.clone(),
            image: row.image.clone(),
            threads: BTreeSet::new(),
            modules: BTreeSet::new(),
            services: BTreeSet::new(),
            modules_truncated: row.modules_truncated,
            threads_truncated: row.threads_truncated,
        }
    }
}

impl Epoch {
    fn open(snapshot: &SystemSnapshot) -> io::Result<Self> {
        Ok(Self {
            source: Source {
                device: id(&snapshot.device)?,
                provider: id(PROCESS_PROVIDER)?,
                epoch: id(&snapshot.boot)?,
            },
            precision_ns: snapshot.precision_ns,
            evidence: id(snapshot.evidence)?,
            next_sequence: 0,
            live: HashMap::new(),
            unresolved: BTreeSet::new(),
            loose_services: BTreeSet::new(),
        })
    }
}

fn resolved_rows(rows: &[OwnedProcess]) -> BTreeMap<u32, OwnedProcess> {
    let mut resolved: BTreeMap<u32, OwnedProcess> = BTreeMap::new();
    for row in rows {
        let Some(start) = row.start else {
            continue;
        };
        match resolved.get(&row.pid) {
            Some(previous) if previous.start.unwrap_or(0) >= start => {}
            _ => {
                let mut chosen = row.clone();
                chosen.start = Some(start);
                resolved.insert(row.pid, chosen);
            }
        }
    }
    resolved
}

fn relink(epoch: &mut Epoch, resolved: &BTreeMap<u32, OwnedProcess>) {
    let starts = epoch
        .live
        .iter()
        .map(|(pid, live)| (*pid, live.start))
        .collect::<HashMap<_, _>>();
    for (pid, row) in resolved {
        let Some(live) = epoch.live.get_mut(pid) else {
            continue;
        };
        if live.start != row.start.unwrap_or(live.start) {
            continue;
        }
        live.parent_pid = None;
        live.parent_start = None;
        if row.parent_pid == 0 || row.parent_pid == *pid {
            continue;
        }
        let Some(parent_start) = starts.get(&row.parent_pid).copied() else {
            continue;
        };
        let Some(child_start) = row.start else {
            continue;
        };
        if parent_start > child_start {
            continue;
        }
        live.parent_pid = Some(row.parent_pid);
        live.parent_start = Some(parent_start);
    }
}

fn break_cycles(epoch: &mut Epoch) {
    let mut pids = epoch.live.keys().copied().collect::<Vec<_>>();
    pids.sort_unstable();
    for start in pids {
        let mut seen = HashSet::new();
        let mut cursor = start;
        loop {
            if !seen.insert(cursor) {
                if let Some(live) = epoch.live.get_mut(&start) {
                    live.parent_pid = None;
                    live.parent_start = None;
                }
                break;
            }
            let Some(parent) = epoch.live.get(&cursor).and_then(|live| live.parent_pid) else {
                break;
            };
            cursor = parent;
        }
    }
}

fn diff_threads(
    epoch: &mut Epoch,
    snapshot: &SystemSnapshot,
    pid: u32,
    row: &OwnedProcess,
) -> io::Result<Vec<Event>> {
    let live = epoch.live.get(&pid).expect("live process");
    let start = live.start;
    let truncated = row.threads_truncated || live.threads_truncated;
    let added = row
        .threads
        .difference(&live.threads)
        .copied()
        .collect::<Vec<_>>();
    let removed = if truncated {
        Vec::new()
    } else {
        live.threads
            .difference(&row.threads)
            .copied()
            .collect::<Vec<_>>()
    };
    let process = process_ref(&epoch.source.epoch, pid, start)?;
    let evidence = Attribution::Observed {
        evidence: id(snapshot.evidence)?,
    };
    let mut events = Vec::new();
    for tid in &added {
        events.push(observation(
            epoch,
            snapshot.sampled_unix_ns,
            THREAD_STARTED,
            "thread",
            &format!("pid:{pid}:{start}:tid:{tid}"),
            Some(process.clone()),
            evidence.clone(),
            Completeness::Complete,
        )?);
    }
    for tid in &removed {
        events.push(observation(
            epoch,
            snapshot.sampled_unix_ns,
            THREAD_EXITED,
            "thread",
            &format!("pid:{pid}:{start}:tid:{tid}"),
            Some(process.clone()),
            evidence.clone(),
            Completeness::Partial {
                reason: id("not_in_snapshot")?,
            },
        )?);
    }
    if let Some(live) = epoch.live.get_mut(&pid) {
        for tid in added {
            live.threads.insert(tid);
        }
        for tid in removed {
            live.threads.remove(&tid);
        }
    }
    Ok(events)
}

fn diff_modules(
    epoch: &mut Epoch,
    snapshot: &SystemSnapshot,
    pid: u32,
    row: &OwnedProcess,
) -> io::Result<Vec<Event>> {
    let live = epoch.live.get(&pid).expect("live process");
    let start = live.start;
    let truncated = row.modules_truncated || live.modules_truncated;
    let added = row
        .modules
        .difference(&live.modules)
        .cloned()
        .collect::<Vec<_>>();
    let removed = if truncated {
        Vec::new()
    } else {
        live.modules
            .difference(&row.modules)
            .cloned()
            .collect::<Vec<_>>()
    };
    let process = process_ref(&epoch.source.epoch, pid, start)?;
    let evidence = Attribution::Observed {
        evidence: id(snapshot.evidence)?,
    };
    let mut events = Vec::new();
    for (name, base) in &added {
        events.push(observation(
            epoch,
            snapshot.sampled_unix_ns,
            MODULE_LOADED,
            "module",
            &module_id(pid, start, *base, name)?,
            Some(process.clone()),
            evidence.clone(),
            Completeness::Complete,
        )?);
    }
    for (name, base) in &removed {
        events.push(observation(
            epoch,
            snapshot.sampled_unix_ns,
            MODULE_UNLOADED,
            "module",
            &module_id(pid, start, *base, name)?,
            Some(process.clone()),
            evidence.clone(),
            Completeness::Partial {
                reason: id("not_in_snapshot")?,
            },
        )?);
    }
    if let Some(live) = epoch.live.get_mut(&pid) {
        for module in added {
            live.modules.insert(module);
        }
        for module in removed {
            live.modules.remove(&module);
        }
    }
    Ok(events)
}

fn diff_services(epoch: &mut Epoch, snapshot: &SystemSnapshot) -> io::Result<Vec<Event>> {
    let mut grouped = BTreeMap::<u32, BTreeSet<String>>::new();
    for ServiceRow { name, pid } in &snapshot.services {
        grouped.entry(*pid).or_default().insert(name.clone());
    }
    let mut events = Vec::new();
    let pids = grouped.keys().copied().collect::<Vec<_>>();
    for pid in pids {
        let names = grouped[&pid].iter().cloned().collect::<Vec<_>>();
        let Some(start) = epoch.live.get(&pid).map(|live| live.start) else {
            continue;
        };
        let process = process_ref(&epoch.source.epoch, pid, start)?;
        for name in names {
            let fresh = epoch
                .live
                .get(&pid)
                .is_some_and(|live| !live.services.contains(&name));
            if !fresh {
                continue;
            }
            events.push(observation(
                epoch,
                snapshot.sampled_unix_ns,
                PROCESS_SERVICE,
                "service",
                &name,
                Some(process.clone()),
                Attribution::Inferred {
                    evidence: id(&format!("service:{name}"))?,
                },
                Completeness::Complete,
            )?);
            if let Some(live) = epoch.live.get_mut(&pid) {
                live.services.insert(name);
            }
        }
    }
    if !snapshot.services_truncated && !snapshot.services_unavailable {
        epoch.loose_services.retain(|service| {
            grouped
                .get(&service.1)
                .is_some_and(|names| names.contains(&service.0))
        });
        for (pid, names) in &grouped {
            if epoch.live.contains_key(pid) {
                continue;
            }
            for name in names {
                if !epoch.loose_services.insert((name.clone(), *pid)) {
                    continue;
                }
                events.push(observation(
                    epoch,
                    snapshot.sampled_unix_ns,
                    PROCESS_SERVICE,
                    "service",
                    name,
                    None,
                    Attribution::Inferred {
                        evidence: id(&format!("service:{name}"))?,
                    },
                    Completeness::Partial {
                        reason: id("process_start_unavailable")?,
                    },
                )?);
            }
        }
    }
    Ok(events)
}

fn diff_unresolved(
    epoch: &mut Epoch,
    snapshot: &SystemSnapshot,
    unresolved: &BTreeSet<u32>,
) -> io::Result<Vec<Event>> {
    let mut events = Vec::new();
    for pid in unresolved {
        if epoch.unresolved.insert(*pid) {
            events.push(observation(
                epoch,
                snapshot.sampled_unix_ns,
                PROCESS_UNRESOLVED,
                "process",
                &format!("pid:{pid}"),
                None,
                Attribution::Observed {
                    evidence: id(snapshot.evidence)?,
                },
                Completeness::Partial {
                    reason: id("process_start_unavailable")?,
                },
            )?);
        }
    }
    if !snapshot.processes_truncated {
        epoch.unresolved.retain(|pid| unresolved.contains(pid));
    }
    Ok(events)
}

fn exit_event(epoch: &mut Epoch, sampled_unix_ns: i64, pid: u32) -> io::Result<Event> {
    let start = epoch.live.get(&pid).expect("exiting process").start;
    let evidence = epoch.evidence.clone();
    observation(
        epoch,
        sampled_unix_ns,
        PROCESS_EXITED,
        "process",
        &format!("pid:{pid}:{start}"),
        Some(process_ref(&epoch.source.epoch, pid, start)?),
        Attribution::Observed { evidence },
        Completeness::Partial {
            reason: id("not_in_snapshot")?,
        },
    )
}

fn observation(
    epoch: &mut Epoch,
    sampled_unix_ns: i64,
    event_type: &str,
    kind: &str,
    subject: &str,
    process: Option<ProcessRef>,
    attribution: Attribution,
    completeness: Completeness,
) -> io::Result<Event> {
    let sequence = epoch.next_sequence.checked_add(1).ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            "process sequence space is exhausted",
        )
    })?;
    epoch.next_sequence = sequence;
    Ok(Event {
        version: Version::CURRENT,
        source: epoch.source.clone(),
        sequence: Counter::new(sequence).expect("sequence is positive"),
        time: EventTime {
            unix_ns: SignedNanos::new(sampled_unix_ns),
            precision_ns: epoch
                .precision_ns
                .and_then(|value| Counter::new(value).ok()),
        },
        record: Record::Observation {
            event_type: id(event_type)?,
            subject: Some(ObjectRef {
                kind: id(kind)?,
                id: id(subject)?,
            }),
            user: None,
            process,
            attribution,
            completeness,
        },
    })
}

fn process_ref(boot: &Id, pid: u32, start: i64) -> io::Result<ProcessRef> {
    Ok(ProcessRef {
        boot: boot.clone(),
        pid,
        started_unix_ns: SignedNanos::new(start),
    })
}

fn module_id(pid: u32, start: i64, base: u64, name: &str) -> io::Result<String> {
    let mut subject = format!("pid:{pid}:{start}:base:{base:x}");
    let room = 256usize.saturating_sub(subject.len().saturating_add(1));
    let mut used = 0usize;
    let mut short = String::new();
    for ch in name.chars() {
        let mut encoded = [0u8; 4];
        let bytes = ch.encode_utf8(&mut encoded);
        if used + bytes.len() > room {
            break;
        }
        used += bytes.len();
        short.push(ch);
    }
    if !short.is_empty() {
        subject.push(':');
        subject.push_str(&short);
    }
    if subject.len() > 256 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "module identity is too long",
        ));
    }
    Ok(subject)
}

fn id(value: &str) -> io::Result<Id> {
    Id::new(value).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            "process field is not a valid identifier",
        )
    })
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    use winstrata_kernel::capture;
    use winstrata_protocols::{Attribution, Completeness, Record};

    use super::*;
    use crate::{EventBus, Filter, Limits, MAX_QUEUE, Runtime};

    struct Held(Option<Child>);

    impl Held {
        fn spawn() -> Self {
            let mut command = hold_command();
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            Self(Some(command.spawn().expect("start a real child process")))
        }

        fn id(&self) -> u32 {
            self.0.as_ref().expect("child is running").id()
        }

        fn finish(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            self.finish();
        }
    }

    #[cfg(target_os = "linux")]
    fn hold_command() -> Command {
        let mut command = Command::new("/bin/sleep");
        command.arg("120");
        command
    }

    #[cfg(windows)]
    fn hold_command() -> Command {
        let mut command = Command::new("ping.exe");
        command.args(["-n", "40", "127.0.0.1"]);
        command
    }

    fn event_type(event: &Event) -> Option<&str> {
        match &event.record {
            Record::Observation { event_type, .. } => Some(event_type.as_str()),
            Record::Gap { .. } => None,
        }
    }

    fn identity(event: &Event) -> Option<(u32, i64)> {
        match &event.record {
            Record::Observation {
                process: Some(process),
                ..
            } => Some((process.pid, process.started_unix_ns.get())),
            _ => None,
        }
    }

    fn has_event(events: &[Event], kind: &str, pid: u32, start: i64) -> bool {
        events
            .iter()
            .any(|event| event_type(event) == Some(kind) && identity(event) == Some((pid, start)))
    }

    fn capture_with(pid: u32) -> winstrata_kernel::SystemSnapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = capture().expect("capture");
            if snapshot
                .processes
                .iter()
                .any(|row| row.pid == pid && row.start_unix_ns.is_some_and(|start| start > 0))
            {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "pid {pid} did not appear in a snapshot"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn capture_without(pid: u32, start: i64) -> winstrata_kernel::SystemSnapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = capture().expect("capture");
            let present = snapshot
                .processes
                .iter()
                .any(|row| row.pid == pid && row.start_unix_ns == Some(start));
            if !present {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "pid {pid} stayed in the snapshot"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn chain_len(nodes: &[ProcessNode], pid: u32, start: i64) -> usize {
        let mut cursor = nodes
            .iter()
            .find(|node| node.pid == pid && node.start_unix_ns == start);
        let mut seen = HashSet::new();
        let mut count = 0usize;
        while let Some(node) = cursor {
            if !seen.insert(node.pid) {
                break;
            }
            count += 1;
            cursor = node.parent_pid.and_then(|parent| {
                nodes.iter().find(|candidate| {
                    candidate.pid == parent
                        && Some(candidate.start_unix_ns) == node.parent_start_unix_ns
                })
            });
        }
        count
    }

    #[test]
    fn real_process_tree_pid_lifecycle_and_dedup() {
        let mut child = Held::spawn();
        let pid = child.id();
        let snapshot = capture_with(pid);
        assert!(
            !snapshot.processes_truncated,
            "the process sample was truncated"
        );
        let row = snapshot
            .processes
            .iter()
            .find(|row| row.pid == pid)
            .expect("child row");
        let start = row.start_unix_ns.expect("child start time");
        let self_id = std::process::id();
        assert_eq!(
            row.parent_pid, self_id,
            "child parent is not this test process"
        );

        let mut tracker = ProcessTracker::new();
        let first = tracker.observe(&snapshot).expect("first observation");
        assert!(has_event(&first, PROCESS_STARTED, pid, start));
        assert_eq!(
            first
                .iter()
                .filter(|event| has_event(std::slice::from_ref(event), PROCESS_STARTED, pid, start))
                .count(),
            1
        );
        let tree = tracker.tree();
        let node = tree
            .iter()
            .find(|node| node.pid == pid && node.start_unix_ns == start)
            .expect("child node");
        assert_eq!(node.parent_pid, Some(self_id));
        let parent = tree
            .iter()
            .find(|node| node.pid == self_id)
            .expect("test process node");
        assert_eq!(node.parent_start_unix_ns, Some(parent.start_unix_ns));
        assert!(parent.start_unix_ns <= start);
        assert!(
            chain_len(&tree, pid, start) >= 2,
            "child is not linked to its parent"
        );
        assert!(
            tree.iter()
                .any(|node| chain_len(&tree, node.pid, node.start_unix_ns) >= 3),
            "snapshot has no three-process chain"
        );

        let second = tracker
            .observe(&capture().expect("second capture"))
            .expect("dedup");
        assert!(!has_event(&second, PROCESS_STARTED, pid, start));
        assert!(!has_event(&second, PROCESS_EXITED, pid, start));

        child.finish();
        let gone = capture_without(pid, start);
        let third = tracker.observe(&gone).expect("exit");
        assert!(has_event(&third, PROCESS_EXITED, pid, start));
        assert_eq!(
            third
                .iter()
                .filter(|event| event_type(event) == Some(PROCESS_EXITED)
                    && identity(event) == Some((pid, start)))
                .count(),
            1
        );
        assert!(
            tracker
                .tree()
                .iter()
                .all(|node| node.pid != pid || node.start_unix_ns != start)
        );
        let fourth = tracker
            .observe(&capture().expect("fourth capture"))
            .expect("exit dedup");
        assert!(!has_event(&fourth, PROCESS_EXITED, pid, start));
        assert!(!has_event(&fourth, PROCESS_STARTED, pid, start));
    }

    #[test]
    fn service_associations_are_indirect_and_deduplicated() {
        let snapshot = capture().expect("capture");
        #[cfg(windows)]
        {
            assert!(
                !snapshot.services_unavailable,
                "Service Control Manager enumeration was unavailable"
            );
            assert!(
                !snapshot.services.is_empty(),
                "Windows reported no Win32 service process"
            );
        }
        let mut tracker = ProcessTracker::new();
        let first = tracker.observe(&snapshot).expect("observe");
        let service_events = first
            .iter()
            .filter(|event| event_type(event) == Some(PROCESS_SERVICE))
            .collect::<Vec<_>>();
        eprintln!(
            "winstrata process sample: processes={} services={} service_events={} unavailable={}",
            snapshot.processes.len(),
            snapshot.services.len(),
            service_events.len(),
            snapshot.services_unavailable
        );
        if snapshot.services.is_empty() {
            assert!(service_events.is_empty(), "invented a service event");
            return;
        }
        assert!(
            !service_events.is_empty(),
            "service rows produced no indirect event"
        );
        for event in &service_events {
            match &event.record {
                Record::Observation {
                    attribution: Attribution::Inferred { evidence },
                    process,
                    completeness,
                    ..
                } => {
                    assert!(evidence.as_str().starts_with("service:"));
                    if let Some(process) = process {
                        assert_eq!(
                            first
                                .iter()
                                .filter(|candidate| {
                                    event_type(candidate) == Some(PROCESS_STARTED)
                                        && identity(candidate)
                                            == Some((process.pid, process.started_unix_ns.get()))
                                })
                                .count(),
                            1,
                            "a service association duplicated the process start"
                        );
                    }
                    assert!(matches!(
                        completeness,
                        Completeness::Complete | Completeness::Partial { .. }
                    ));
                }
                _ => panic!("service event has the wrong attribution"),
            }
        }
        let second = tracker
            .observe(&capture().expect("second capture"))
            .expect("dedup");
        for event in &service_events {
            let Some((pid, start)) = identity(event) else {
                continue;
            };
            let Record::Observation {
                attribution: Attribution::Inferred { evidence },
                ..
            } = &event.record
            else {
                continue;
            };
            let repeated = second.iter().any(|candidate| {
                event_type(candidate) == Some(PROCESS_SERVICE)
                    && identity(candidate) == Some((pid, start))
                    && matches!(
                        &candidate.record,
                        Record::Observation { attribution: Attribution::Inferred { evidence: next }, .. }
                            if next == evidence
                    )
            });
            assert!(!repeated, "repeated an indirect service event");
        }
    }

    #[test]
    fn recycled_pid_uses_the_new_start_time() {
        // 两条启动时间都来自真实进程。第二次采样只替换当前 PID 的启动时间，
        // 用来执行复用规则；它不是操作系统已经回收了这个 PID。
        let base = capture().expect("base");
        let mut tracker = ProcessTracker::new();
        tracker.observe(&base).expect("baseline");
        let self_id = std::process::id();
        let original = base
            .processes
            .iter()
            .find(|row| row.pid == self_id)
            .expect("self")
            .start_unix_ns
            .expect("self start");
        let mut child = Held::spawn();
        let child_snapshot = capture_with(child.id());
        let child_start = child_snapshot
            .processes
            .iter()
            .find(|row| row.pid == child.id())
            .expect("child")
            .start_unix_ns
            .expect("child start");
        assert_ne!(original, child_start);
        let mut recycled = base.clone();
        for row in &mut recycled.processes {
            if row.pid == self_id {
                row.start_unix_ns = Some(child_start);
            }
        }
        let events = tracker.observe(&recycled).expect("reuse");
        assert!(has_event(&events, PROCESS_EXITED, self_id, original));
        assert!(has_event(&events, PROCESS_STARTED, self_id, child_start));
        let node = tracker
            .tree()
            .into_iter()
            .find(|node| node.pid == self_id)
            .expect("replacement");
        assert_eq!(node.start_unix_ns, child_start);
        assert!(
            tracker
                .tree()
                .iter()
                .all(|node| node.start_unix_ns != original || node.pid != self_id)
        );
        child.finish();
    }

    #[test]
    fn lifecycle_events_publish_contiguously() {
        let snapshot = capture().expect("capture");
        let events = ProcessTracker::new().observe(&snapshot).expect("observe");
        assert!(!events.is_empty());
        assert!(
            events.len() < MAX_QUEUE,
            "process sample exceeds the bus queue"
        );
        let mut limits = Limits::standard();
        limits.event_queue = MAX_QUEUE;
        limits.global_queue = MAX_QUEUE;
        let runtime = Runtime::start(limits).expect("runtime");
        let subscription = runtime
            .bus()
            .subscribe(1, Filter::default())
            .expect("subscribe");
        for event in &events {
            let outcome = runtime.bus().publish(event.clone()).expect("publish");
            assert_eq!(outcome.lost, 0);
        }
        let mut received = Vec::new();
        while received.len() < events.len() {
            let batch = subscription
                .recv_batch(64, Duration::from_secs(2))
                .expect("receive");
            assert!(!batch.is_empty(), "bus dropped process events");
            received.extend(batch);
        }
        for (index, event) in received.iter().enumerate() {
            assert_eq!(event.sequence.get(), (index as u64) + 1);
        }
        assert_eq!(runtime.bus().stats().lost, 0);
        let _bus: &EventBus = runtime.bus();
    }
}

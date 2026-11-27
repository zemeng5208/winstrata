//! 统一混合存储。
//!
//! 结构化对象、事件、时序、快照和风险抓包各写一份分段载荷。提交日志只保存
//! 位置和 CRC。打开时按最后一个完整且校验通过的提交重建索引；不完整或损坏
//! 的提交会截断回退，查询不会返回未通过校验的字节。

mod format;
mod pcap;
mod series;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
#[cfg(test)]
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use winstrata_protocols::{Event, Id, ObjectRef, Record, Source, Version};

use crate::files::{create_private, open_existing};
use crate::platform::Anchor;
use crate::storage;
use format::{
    HEADER_LEN, JOURNAL_MAGIC, JOURNAL_VERSION, MAX_RECORD, MAX_SEGMENT_BYTES, MIN_SEGMENT_BYTES,
    Reader, commit_frame, crc32, frame, input, invalid, journal_header, put_str, put_u32, put_u64,
    read_frames, segment_header,
};

const STRUCTURED: u8 = 0;
const EVENTS: u8 = 1;
const METRICS: u8 = 2;
const CAPTURES: u8 = 3;
const SNAPSHOTS: u8 = 4;
const INDEXES: u8 = 5;
const DIRS: [&str; 6] = [
    "databases",
    "events",
    "metrics",
    "captures",
    "snapshots",
    "indexes",
];
const JOURNAL: &str = "journal.wsj";
const MAX_POINTS: usize = 8_192;
const MAX_ENTRIES: usize = 8_192;
const MAX_PATH: usize = 4_096;
const MAX_BODY: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataClass {
    Structured,
    Events,
    Metrics,
    Captures,
    Snapshots,
}

impl DataClass {
    fn code(self) -> u8 {
        match self {
            Self::Structured => STRUCTURED,
            Self::Events => EVENTS,
            Self::Metrics => METRICS,
            Self::Captures => CAPTURES,
            Self::Snapshots => SNAPSHOTS,
        }
    }

    fn from_code(code: u8) -> io::Result<Self> {
        Ok(match code {
            STRUCTURED => Self::Structured,
            EVENTS => Self::Events,
            METRICS => Self::Metrics,
            CAPTURES => Self::Captures,
            SNAPSHOTS => Self::Snapshots,
            _ => return Err(invalid("unknown retention class")),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Stated,
    Inferred,
    Unknown,
}

impl Confidence {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stated => "stated",
            Self::Inferred => "inferred",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> io::Result<Self> {
        Ok(match value {
            "stated" => Self::Stated,
            "inferred" => Self::Inferred,
            "unknown" => Self::Unknown,
            _ => return Err(invalid("unknown capture confidence")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectRecord {
    pub id: Id,
    pub kind: Id,
    pub body: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    pub from: Id,
    pub kind: Id,
    pub to: Id,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub unix_ns: i64,
    pub value: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskTag {
    pub source: Id,
    pub reason: Id,
    pub unix_ns: i64,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureWrite {
    pub id: Id,
    pub tag: Option<RiskTag>,
    pub link_type: u16,
    pub timestamp_ns: i64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCapture {
    pub id: Id,
    pub tag: RiskTag,
    pub link_type: u16,
    pub timestamp_ns: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotEntry {
    pub path: String,
    pub state: Id,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub id: Id,
    pub base: Option<Id>,
    pub entries: Vec<SnapshotEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordRef {
    Object { id: Id, kind: Id },
    Relation { from: Id, kind: Id, to: Id },
    Event { source: Source, sequence: u64 },
    Metric { series: Id },
    Capture { id: Id },
    Snapshot { id: Id },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CapacityReport {
    pub structured: u64,
    pub events: u64,
    pub metrics: u64,
    pub captures: u64,
    pub snapshots: u64,
    pub indexes: u64,
}

impl CapacityReport {
    pub fn total(&self) -> u64 {
        [
            self.structured,
            self.events,
            self.metrics,
            self.captures,
            self.snapshots,
            self.indexes,
        ]
        .into_iter()
        .fold(0, u64::saturating_add)
    }

    fn field_mut(&mut self, category: u8) -> &mut u64 {
        match category {
            STRUCTURED => &mut self.structured,
            EVENTS => &mut self.events,
            METRICS => &mut self.metrics,
            CAPTURES => &mut self.captures,
            SNAPSHOTS => &mut self.snapshots,
            _ => &mut self.indexes,
        }
    }
}

/// 打开存储时实际截掉的尾部，以及因载荷校验失败而放弃的已解析提交。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recovery {
    pub commits: u64,
    pub truncated_bytes: u64,
    pub rolled_back_commits: u64,
}

#[derive(Clone, Debug)]
enum Op {
    Object(ObjectRecord),
    Relation(Relation),
    Event(Event),
    Metric { series: Id, points: Vec<Point> },
    Capture(CaptureWrite),
    Snapshot(Snapshot),
    Retention { class: DataClass, days: Option<u32> },
}

#[derive(Clone, Debug, Default)]
pub struct Batch {
    ops: Vec<Op>,
}

impl Batch {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn put_object(&mut self, object: ObjectRecord) -> io::Result<()> {
        if serde_json::to_vec(&object.body)
            .map_err(|_| input("object body is not valid JSON"))?
            .len()
            > MAX_BODY
        {
            return Err(input("object body exceeds 256 KiB"));
        }
        self.ops.push(Op::Object(object));
        Ok(())
    }

    pub fn put_relation(&mut self, relation: Relation) -> io::Result<()> {
        self.ops.push(Op::Relation(relation));
        Ok(())
    }

    pub fn append_event(&mut self, event: Event) -> io::Result<()> {
        if event.version != Version::CURRENT {
            return Err(input("event version cannot be stored"));
        }
        let _ = event.to_json().map_err(|error| input(&error.to_string()))?;
        self.ops.push(Op::Event(event));
        Ok(())
    }

    pub fn append_metrics(&mut self, series: Id, points: Vec<Point>) -> io::Result<()> {
        if points.is_empty() || points.len() > MAX_POINTS {
            return Err(input("metric batch must contain 1 to 8192 points"));
        }
        self.ops.push(Op::Metric { series, points });
        Ok(())
    }

    pub fn append_capture(&mut self, capture: CaptureWrite) -> io::Result<()> {
        if capture.tag.is_none() {
            return Err(input("untagged packets are not stored"));
        }
        if capture.timestamp_ns < 0 {
            return Err(input("capture timestamp must be zero or positive"));
        }
        if capture.bytes.is_empty() || capture.bytes.len() > MAX_RECORD {
            return Err(input("packet length is outside the capture limit"));
        }
        self.ops.push(Op::Capture(capture));
        Ok(())
    }

    pub fn put_snapshot(&mut self, snapshot: Snapshot) -> io::Result<()> {
        if snapshot.entries.len() > MAX_ENTRIES {
            return Err(input("snapshot exceeds 8192 entries"));
        }
        let mut paths = BTreeSet::new();
        for entry in &snapshot.entries {
            snapshot_path(&entry.path)?;
            if !paths.insert(entry.path.as_str()) {
                return Err(input("snapshot repeats a path"));
            }
        }
        self.ops.push(Op::Snapshot(snapshot));
        Ok(())
    }

    /// 保存保留天数。`None` 表示继续永久保留。此调用不删除已有数据。
    pub fn set_retention(&mut self, class: DataClass, days: Option<u32>) -> io::Result<()> {
        if matches!(days, Some(0)) {
            return Err(input("retention days start at 1; use none for indefinite"));
        }
        self.ops.push(Op::Retention { class, days });
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct Location {
    segment: u32,
    offset: u64,
    len: u32,
    crc: u32,
}

#[derive(Clone)]
struct EventLoc {
    time: i64,
    subject: Option<String>,
    loc: Location,
}

#[derive(Clone)]
struct MetricSpan {
    start: i64,
    end: i64,
    loc: Location,
}

#[derive(Clone)]
struct SnapMeta {
    loc: Location,
}

#[derive(Clone)]
struct Tail {
    segment: u32,
    len: u64,
    link_type: Option<u16>,
}

impl Default for Tail {
    fn default() -> Self {
        Self {
            segment: 0,
            len: 0,
            link_type: None,
        }
    }
}

#[derive(Clone)]
struct Index {
    commit: u64,
    water: BTreeMap<(u8, u32), u64>,
    tail: [Tail; 6],
    objects: BTreeMap<String, (String, Location)>,
    relations: BTreeMap<(String, String, String), Location>,
    events: BTreeMap<(String, String, String), BTreeMap<u64, EventLoc>>,
    metrics: BTreeMap<String, Vec<MetricSpan>>,
    captures: BTreeMap<String, Location>,
    snapshots: BTreeMap<String, SnapMeta>,
    retention: [Option<u32>; 5],
}

impl Default for Index {
    fn default() -> Self {
        Self {
            commit: 0,
            water: BTreeMap::new(),
            tail: std::array::from_fn(|_| Tail::default()),
            objects: BTreeMap::new(),
            relations: BTreeMap::new(),
            events: BTreeMap::new(),
            metrics: BTreeMap::new(),
            captures: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            retention: [None; 5],
        }
    }
}

struct RefRec {
    category: u8,
    segment: u32,
    offset: u64,
    len: u32,
    crc: u32,
}

struct CommitRec {
    seq: u64,
    waters: Vec<(u8, u32, u64)>,
    refs: Vec<RefRec>,
}

struct Pending {
    category: u8,
    payload: Vec<u8>,
    capture_link: Option<u16>,
}

struct Buf {
    category: u8,
    segment: u32,
    start: u64,
    bytes: Vec<u8>,
    new_file: bool,
}

pub struct Hybrid {
    dirs: [Anchor; 6],
    segment_bytes: u32,
    index: Index,
    recovery: Recovery,
}

impl Hybrid {
    pub(crate) fn open(user: &Anchor) -> io::Result<Self> {
        Self::open_with(user, None)
    }

    #[cfg(test)]
    pub(crate) fn create(user: &Anchor, segment_bytes: u32) -> io::Result<Self> {
        if journal_exists(user)? {
            return Err(input("hybrid store already exists"));
        }
        Self::open_with(user, Some(segment_bytes))
    }

    pub fn segment_bytes(&self) -> u32 {
        self.segment_bytes
    }

    pub fn recovery(&self) -> Recovery {
        self.recovery
    }

    pub fn commits(&self) -> u64 {
        self.index.commit
    }

    pub fn reserve(&self, bytes: u64) -> io::Result<()> {
        storage::capacity(&self.dirs[0], bytes)
    }

    pub fn retention(&self, class: DataClass) -> Option<u32> {
        self.index.retention[class.code() as usize]
    }

    pub fn commit(&mut self, batch: Batch) -> io::Result<u64> {
        let pending = self.prepare(batch)?;
        if pending.is_empty() {
            return Ok(self.index.commit);
        }
        let mut tails = self.index.tail.clone();
        let mut bufs = Vec::new();
        let mut refs = Vec::new();
        let mut payloads = Vec::new();
        for item in pending {
            let offset = stage(
                &mut bufs,
                &mut tails,
                &self.index.water,
                &item,
                self.segment_bytes as u64,
            )?;
            refs.push(RefRec {
                category: item.category,
                segment: tails[item.category as usize].segment,
                offset,
                len: item.payload.len() as u32,
                crc: crc32(&item.payload),
            });
            payloads.push(item.payload);
        }
        let waters: Vec<_> = bufs
            .iter()
            .map(|buf| {
                let len = if buf.new_file {
                    buf.bytes.len() as u64
                } else {
                    buf.start + buf.bytes.len() as u64
                };
                (buf.category, buf.segment, len)
            })
            .collect();
        let record = CommitRec {
            seq: self.index.commit + 1,
            waters,
            refs,
        };
        let encoded = encode_commit(&record)?;
        let framed = commit_frame(&encoded)?;
        let mut bytes = framed.len() as u64;
        for buf in &bufs {
            bytes = bytes.saturating_add(buf.bytes.len() as u64);
        }
        self.reserve(bytes)?;
        let mut updated = self.index.clone();
        updated.accept(&record, &payloads)?;
        updated.tail = tails;

        let mut written = Vec::new();
        if let Err(error) = (|| {
            for buf in &bufs {
                write_buf(&self.dirs[buf.category as usize], buf)?;
                if buf.new_file {
                    sync_dir(&self.dirs[buf.category as usize])?;
                }
                written.push(buf);
            }
            append_journal(&self.dirs[INDEXES as usize], &framed)?;
            sync_dir(&self.dirs[INDEXES as usize])?;
            Ok(())
        })() {
            rollback(&self.dirs, &written);
            return Err(error);
        }
        self.index = updated;
        self.recovery.commits = self.index.commit;
        Ok(self.index.commit)
    }

    pub fn get_object(&self, id: &Id) -> io::Result<Option<ObjectRecord>> {
        let Some((_, loc)) = self.index.objects.get(id.as_str()) else {
            return Ok(None);
        };
        let payload = self.read_framed(STRUCTURED, *loc)?;
        decode_object(&payload).map(Some)
    }

    pub fn relations_from(&self, id: &Id) -> io::Result<Vec<Relation>> {
        let mut found = Vec::new();
        for ((from, kind, to), loc) in &self.index.relations {
            if from == id.as_str() {
                let payload = self.read_framed(STRUCTURED, *loc)?;
                let relation = decode_relation(&payload)?;
                if relation.from.as_str() != from
                    || relation.kind.as_str() != kind
                    || relation.to.as_str() != to
                {
                    return Err(invalid("relation record does not match the index"));
                }
                found.push(relation);
            }
        }
        Ok(found)
    }

    pub fn event(&self, source: &Source, sequence: u64) -> io::Result<Option<Event>> {
        let Some(loc) = self
            .index
            .events
            .get(&source_key(source))
            .and_then(|map| map.get(&sequence))
        else {
            return Ok(None);
        };
        let payload = self.read_framed(EVENTS, loc.loc)?;
        let text = std::str::from_utf8(&payload).map_err(|_| invalid("event is not UTF-8"))?;
        Event::from_json(text)
            .map(Some)
            .map_err(|_| invalid("stored event is invalid"))
    }

    pub fn events_in_time(
        &self,
        source: &Source,
        start_ns: i64,
        end_ns: i64,
    ) -> io::Result<Vec<Event>> {
        if start_ns > end_ns {
            return Err(input("event time range is reversed"));
        }
        let Some(map) = self.index.events.get(&source_key(source)) else {
            return Ok(Vec::new());
        };
        let mut events = Vec::new();
        for (sequence, loc) in map {
            if (start_ns..=end_ns).contains(&loc.time) {
                events.push(
                    self.event(source, *sequence)?
                        .ok_or_else(|| invalid("event index points nowhere"))?,
                );
            }
        }
        Ok(events)
    }

    pub fn metrics(&self, series: &Id, start_ns: i64, end_ns: i64) -> io::Result<Vec<Point>> {
        if start_ns > end_ns {
            return Err(input("metric time range is reversed"));
        }
        let Some(spans) = self.index.metrics.get(series.as_str()) else {
            return Ok(Vec::new());
        };
        let mut points = Vec::new();
        for span in spans {
            if span.end < start_ns || span.start > end_ns {
                continue;
            }
            let payload = self.read_framed(METRICS, span.loc)?;
            for (time, value) in decode_metric(&payload)?.1 {
                if (start_ns..=end_ns).contains(&time) {
                    points.push(Point {
                        unix_ns: time,
                        value,
                    });
                }
            }
        }
        points.sort_by(|left, right| {
            left.unix_ns
                .cmp(&right.unix_ns)
                .then(left.value.cmp(&right.value))
        });
        Ok(points)
    }

    pub fn capture(&self, id: &Id) -> io::Result<Option<StoredCapture>> {
        let Some(loc) = self.index.captures.get(id.as_str()) else {
            return Ok(None);
        };
        let block = self.read_range(CAPTURES, loc.segment, loc.offset, loc.len, loc.crc)?;
        let (timestamp, bytes, comment) = pcap::parse_packet(&block)?;
        let note = decode_note(&comment)?;
        if note.id != id.as_str() {
            return Err(invalid("capture identifier does not match its block"));
        }
        let link_type = self.capture_link(loc.segment)?;
        Ok(Some(StoredCapture {
            id: id.clone(),
            tag: note.tag()?,
            link_type,
            timestamp_ns: timestamp,
            bytes,
        }))
    }

    pub fn snapshot(&self, id: &Id) -> io::Result<Option<Snapshot>> {
        let Some(meta) = self.index.snapshots.get(id.as_str()) else {
            return Ok(None);
        };
        let payload = self.read_framed(SNAPSHOTS, meta.loc)?;
        let snapshot = decode_snapshot(&payload)?;
        if snapshot.id != *id {
            return Err(invalid("snapshot identifier does not match its record"));
        }
        Ok(Some(snapshot))
    }

    pub fn materialize_snapshot(&self, id: &Id) -> io::Result<BTreeMap<String, Id>> {
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        let mut cursor = Some(id.clone());
        while let Some(current) = cursor {
            if !seen.insert(current.as_str().to_owned()) || chain.len() == 1024 {
                return Err(invalid("snapshot chain is cyclic or too deep"));
            }
            let snapshot = self
                .snapshot(&current)?
                .ok_or_else(|| invalid("snapshot base is missing"))?;
            cursor = snapshot.base.clone();
            chain.push(snapshot);
        }
        if chain.last().is_some_and(|snapshot| snapshot.base.is_some()) {
            return Err(invalid(
                "snapshot chain does not start from a full snapshot",
            ));
        }
        chain.reverse();
        let mut state = BTreeMap::new();
        for snapshot in chain {
            for entry in snapshot.entries {
                state.insert(entry.path, entry.state);
            }
        }
        Ok(state)
    }

    pub fn lookup(&self, id: &Id) -> io::Result<Vec<RecordRef>> {
        let key = id.as_str();
        let mut hits = Vec::new();
        if let Some((kind, _)) = self.index.objects.get(key) {
            hits.push(RecordRef::Object {
                id: id.clone(),
                kind: stored_id(kind)?,
            });
        }
        for (from, kind, to) in self.index.relations.keys() {
            if from == key || to == key {
                hits.push(RecordRef::Relation {
                    from: stored_id(from)?,
                    kind: stored_id(kind)?,
                    to: stored_id(to)?,
                });
            }
        }
        for ((device, provider, epoch), sequences) in &self.index.events {
            for (sequence, loc) in sequences {
                if loc.subject.as_deref() == Some(key) {
                    hits.push(RecordRef::Event {
                        source: Source {
                            device: stored_id(device)?,
                            provider: stored_id(provider)?,
                            epoch: stored_id(epoch)?,
                        },
                        sequence: *sequence,
                    });
                }
            }
        }
        if self.index.metrics.contains_key(key) {
            hits.push(RecordRef::Metric { series: id.clone() });
        }
        if self.index.captures.contains_key(key) {
            hits.push(RecordRef::Capture { id: id.clone() });
        }
        if self.index.snapshots.contains_key(key) {
            hits.push(RecordRef::Snapshot { id: id.clone() });
        }
        Ok(hits)
    }

    pub fn capacity(&self) -> io::Result<CapacityReport> {
        let mut report = CapacityReport::default();
        for (category, dir) in self.dirs.iter().enumerate() {
            *report.field_mut(category as u8) = directory_bytes(dir)?;
        }
        Ok(report)
    }

    #[cfg(test)]
    pub(crate) fn category_path(&self, category: u8) -> &Path {
        self.dirs[category as usize].io_path()
    }

    fn open_with(user: &Anchor, requested: Option<u32>) -> io::Result<Self> {
        let data = user.child(std::ffi::OsStr::new("data"), true, true)?;
        let dirs = [
            data.child(std::ffi::OsStr::new(DIRS[0]), true, true)?,
            data.child(std::ffi::OsStr::new(DIRS[1]), true, true)?,
            data.child(std::ffi::OsStr::new(DIRS[2]), true, true)?,
            data.child(std::ffi::OsStr::new(DIRS[3]), true, true)?,
            data.child(std::ffi::OsStr::new(DIRS[4]), true, true)?,
            data.child(std::ffi::OsStr::new(DIRS[5]), true, true)?,
        ];
        let path = dirs[INDEXES as usize].io_path().join(JOURNAL);
        let (segment_bytes, recovery, index) = match open_existing(&path, true) {
            Ok(file) => recover(&dirs, file, requested)?,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let segment_bytes = requested.unwrap_or(format::DEFAULT_SEGMENT_BYTES);
                check_segment_bytes(segment_bytes)?;
                let mut file = create_private(&path)?;
                file.write_all(&journal_header(segment_bytes))?;
                file.sync_all()?;
                sync_dir(&dirs[INDEXES as usize])?;
                (segment_bytes, Recovery::default(), Index::default())
            }
            Err(error) => return Err(error),
        };
        Ok(Self {
            dirs,
            segment_bytes,
            index,
            recovery,
        })
    }

    fn prepare(&self, batch: Batch) -> io::Result<Vec<Pending>> {
        let mut pending = Vec::new();
        let mut events = BTreeSet::new();
        let mut captures = BTreeSet::new();
        let mut snapshots = BTreeSet::new();
        let mut relations = BTreeSet::new();
        for op in batch.ops {
            match op {
                Op::Object(object) => {
                    if let Some(current) = self.get_object(&object.id)?
                        && current.kind == object.kind
                        && current.body == object.body
                    {
                        continue;
                    }
                    pending.push(Pending {
                        category: STRUCTURED,
                        payload: encode_object(&object)?,
                        capture_link: None,
                    });
                }
                Op::Relation(relation) => {
                    let key = (
                        relation.from.as_str().to_owned(),
                        relation.kind.as_str().to_owned(),
                        relation.to.as_str().to_owned(),
                    );
                    if self.index.relations.contains_key(&key) || !relations.insert(key) {
                        continue;
                    }
                    pending.push(Pending {
                        category: STRUCTURED,
                        payload: encode_relation(&relation),
                        capture_link: None,
                    });
                }
                Op::Event(event) => {
                    let key = (source_key(&event.source), event.sequence.get());
                    if self
                        .index
                        .events
                        .get(&key.0)
                        .is_some_and(|map| map.contains_key(&key.1))
                        || !events.insert((key.0.0, key.0.1, key.0.2, key.1))
                    {
                        return Err(input("event sequence is already stored"));
                    }
                    let json = event.to_json().map_err(|error| input(&error.to_string()))?;
                    if json.len() > MAX_RECORD {
                        return Err(input("event exceeds the 1 MiB storage limit"));
                    }
                    pending.push(Pending {
                        category: EVENTS,
                        payload: json.into_bytes(),
                        capture_link: None,
                    });
                }
                Op::Metric { series, points } => {
                    pending.push(Pending {
                        category: METRICS,
                        payload: encode_metric(&series, &points)?,
                        capture_link: None,
                    });
                }
                Op::Capture(capture) => {
                    let tag = capture
                        .tag
                        .ok_or_else(|| input("untagged packets are not stored"))?;
                    if !captures.insert(capture.id.as_str().to_owned())
                        || self.index.captures.contains_key(capture.id.as_str())
                    {
                        return Err(input("capture identifier is already stored"));
                    }
                    let note = CaptureNote::from_tag(&capture.id, &tag);
                    let comment = serde_json::to_string(&note)
                        .map_err(|_| input("capture tag cannot be encoded"))?;
                    let block =
                        pcap::packet(capture.timestamp_ns as u64, &capture.bytes, &comment)?;
                    if block.len() > MAX_RECORD {
                        return Err(input("capture block exceeds the 1 MiB storage limit"));
                    }
                    pending.push(Pending {
                        category: CAPTURES,
                        payload: block,
                        capture_link: Some(capture.link_type),
                    });
                }
                Op::Snapshot(snapshot) => {
                    if snapshot.base.as_ref() == Some(&snapshot.id) {
                        return Err(input("snapshot cannot be its own base"));
                    }
                    if let Some(base) = &snapshot.base
                        && !self.index.snapshots.contains_key(base.as_str())
                        && !snapshots.contains(base.as_str())
                    {
                        return Err(input("snapshot base does not exist"));
                    }
                    if snapshot.base.is_none() && snapshot.entries.is_empty() {
                        return Err(input("a full snapshot needs at least one entry"));
                    }
                    if !snapshots.insert(snapshot.id.as_str().to_owned())
                        || self.index.snapshots.contains_key(snapshot.id.as_str())
                    {
                        return Err(input("snapshot identifier is already stored"));
                    }
                    pending.push(Pending {
                        category: SNAPSHOTS,
                        payload: encode_snapshot(&snapshot)?,
                        capture_link: None,
                    });
                }
                Op::Retention { class, days } => {
                    pending.push(Pending {
                        category: INDEXES,
                        payload: encode_retention(class, days),
                        capture_link: None,
                    });
                }
            }
        }
        Ok(pending)
    }

    fn read_framed(&self, category: u8, loc: Location) -> io::Result<Vec<u8>> {
        if loc.offset < HEADER_LEN as u64 {
            return Err(invalid("record frame is outside the segment"));
        }
        let header = self.read_exact(
            category,
            loc.segment,
            loc.offset - HEADER_LEN as u64,
            HEADER_LEN as u32,
        )?;
        if header.get(..4) != Some(format::FRAME_MAGIC) {
            return Err(invalid("record frame magic does not match"));
        }
        let len = u32::from_le_bytes(header[4..8].try_into().expect("4 bytes"));
        let crc = u32::from_le_bytes(header[8..12].try_into().expect("4 bytes"));
        if len != loc.len || crc != loc.crc {
            return Err(invalid("record frame does not match the index"));
        }
        let payload = self.read_exact(category, loc.segment, loc.offset, loc.len)?;
        if crc32(&payload) != loc.crc {
            return Err(invalid("record checksum does not match"));
        }
        Ok(payload)
    }

    fn read_range(
        &self,
        category: u8,
        segment: u32,
        offset: u64,
        len: u32,
        crc: u32,
    ) -> io::Result<Vec<u8>> {
        let payload = self.read_exact(category, segment, offset, len)?;
        if crc32(&payload) != crc {
            return Err(invalid("record checksum does not match"));
        }
        Ok(payload)
    }

    fn read_exact(&self, category: u8, segment: u32, offset: u64, len: u32) -> io::Result<Vec<u8>> {
        let mut file = open_existing(&segment_path(&self.dirs[category as usize], segment), false)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0; len as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn capture_link(&self, segment: u32) -> io::Result<u16> {
        let prefix = self.read_exact(CAPTURES, segment, 0, capture_header_len() as u32)?;
        pcap::link_type(&prefix)
    }
}

pub(crate) fn suspends(error: &io::Error) -> bool {
    !matches!(
        error.kind(),
        ErrorKind::InvalidInput | ErrorKind::InvalidData
    )
}

fn check_segment_bytes(value: u32) -> io::Result<()> {
    if (MIN_SEGMENT_BYTES..=MAX_SEGMENT_BYTES).contains(&value) {
        Ok(())
    } else {
        Err(input("segment size must be from 256 bytes through 16 MiB"))
    }
}

fn snapshot_path(path: &str) -> io::Result<()> {
    if path.is_empty()
        || path.len() > MAX_PATH
        || path
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
        || path.split(['/', '\\']).any(|part| part == "..")
    {
        return Err(input(
            "snapshot path is empty, too long, or contains traversal",
        ));
    }
    Ok(())
}

fn stored_id(value: &str) -> io::Result<Id> {
    Id::new(value).map_err(|_| invalid("stored identifier is invalid"))
}

fn source_key(source: &Source) -> (String, String, String) {
    (
        source.device.as_str().to_owned(),
        source.provider.as_str().to_owned(),
        source.epoch.as_str().to_owned(),
    )
}

fn subject_of(event: &Event) -> Option<String> {
    match &event.record {
        Record::Observation {
            subject: Some(ObjectRef { id, .. }),
            ..
        } => Some(id.as_str().to_owned()),
        _ => None,
    }
}

fn capture_header_len() -> usize {
    pcap::preamble(0).len()
}

#[cfg(test)]
fn journal_exists(user: &Anchor) -> io::Result<bool> {
    let data = match user.child(std::ffi::OsStr::new("data"), false, true) {
        Ok(data) => data,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let indexes = match data.child(std::ffi::OsStr::new("indexes"), false, true) {
        Ok(dir) => dir,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    match open_existing(&indexes.io_path().join(JOURNAL), false) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn recover(
    dirs: &[Anchor; 6],
    mut journal: File,
    requested: Option<u32>,
) -> io::Result<(u32, Recovery, Index)> {
    let mut bytes = Vec::new();
    journal.read_to_end(&mut bytes)?;
    if bytes.len() < HEADER_LEN
        || bytes.get(..4) != Some(JOURNAL_MAGIC)
        || u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes")) != JOURNAL_VERSION
    {
        return Err(invalid("hybrid journal header is not supported"));
    }
    let segment_bytes = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
    if !(MIN_SEGMENT_BYTES..=MAX_SEGMENT_BYTES).contains(&segment_bytes) {
        return Err(invalid("hybrid journal header is not supported"));
    }
    if requested.is_some_and(|value| value != segment_bytes) {
        return Err(input("segment size does not match the existing store"));
    }
    let (frames, _) = read_frames(&bytes, HEADER_LEN);
    let mut index = Index::default();
    let mut applied = 0usize;
    let mut rolled_back = 0u64;
    for (_, payload) in &frames {
        let record = match decode_commit(payload) {
            Ok(record) => record,
            Err(_) => {
                rolled_back = (frames.len() - applied) as u64;
                break;
            }
        };
        let mut payloads = Vec::new();
        let mut readable = true;
        for ref_ in &record.refs {
            if ref_.category >= 6 {
                readable = false;
                break;
            }
            match read_ref(&dirs[ref_.category as usize], ref_) {
                Ok(payload) => payloads.push(payload),
                Err(_) => {
                    readable = false;
                    break;
                }
            }
        }
        if !readable || index.accept(&record, &payloads).is_err() {
            rolled_back = (frames.len() - applied) as u64;
            break;
        }
        applied += 1;
    }
    let accepted_end = if applied == 0 {
        HEADER_LEN
    } else {
        frames[applied - 1].0
    };
    let truncated = (bytes.len() - accepted_end) as u64;
    if bytes.len() != accepted_end {
        journal.set_len(accepted_end as u64)?;
        journal.sync_all()?;
    }
    index.refresh_tails(dirs)?;
    let removed = enforce_water(dirs, &index)?;
    Ok((
        segment_bytes,
        Recovery {
            commits: index.commit,
            truncated_bytes: truncated + removed,
            rolled_back_commits: rolled_back,
        },
        index,
    ))
}

fn read_ref(dir: &Anchor, ref_: &RefRec) -> io::Result<Vec<u8>> {
    check_segment_header(dir, ref_)?;
    if ref_.category == CAPTURES {
        let block = read_file_range(dir, ref_.segment, ref_.offset, ref_.len)?;
        if crc32(&block) != ref_.crc {
            return Err(invalid("record checksum does not match"));
        }
        let _ = pcap::parse_packet(&block)?;
        return Ok(block);
    }
    if ref_.offset < HEADER_LEN as u64 {
        return Err(invalid("record frame is outside the segment"));
    }
    let header = read_file_range(
        dir,
        ref_.segment,
        ref_.offset - HEADER_LEN as u64,
        HEADER_LEN as u32,
    )?;
    if header.get(..4) != Some(format::FRAME_MAGIC) {
        return Err(invalid("record frame magic does not match"));
    }
    let payload = read_file_range(dir, ref_.segment, ref_.offset, ref_.len)?;
    if crc32(&payload) != ref_.crc {
        return Err(invalid("record checksum does not match"));
    }
    Ok(payload)
}

fn check_segment_header(dir: &Anchor, ref_: &RefRec) -> io::Result<()> {
    if ref_.category == CAPTURES {
        let prefix = read_file_range(dir, ref_.segment, 0, capture_header_len() as u32)?;
        let _ = pcap::link_type(&prefix)?;
        return Ok(());
    }
    let header = read_file_range(dir, ref_.segment, 0, HEADER_LEN as u32)?;
    if header.get(..4) != Some(format::SEGMENT_MAGIC) || header[4] != ref_.category {
        return Err(invalid("segment header does not match"));
    }
    let id = u32::from_le_bytes(header[5..9].try_into().expect("4 bytes"));
    if id != ref_.segment {
        return Err(invalid("segment header does not match"));
    }
    Ok(())
}

fn read_file_range(dir: &Anchor, segment: u32, offset: u64, len: u32) -> io::Result<Vec<u8>> {
    let mut file = open_existing(&segment_path(dir, segment), false)?;
    if offset.saturating_add(u64::from(len)) > file.metadata()?.len() {
        return Err(invalid("record extends past the segment"));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0; len as usize];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

fn enforce_water(dirs: &[Anchor; 6], index: &Index) -> io::Result<u64> {
    let mut removed = 0u64;
    for (category, dir) in dirs.iter().enumerate() {
        let category = category as u8;
        for entry in fs::read_dir(dir.io_path())? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(invalid("unexpected file in hybrid storage"));
            };
            if category == INDEXES && name == JOURNAL {
                continue;
            }
            let Some(segment) = parse_segment_name(name) else {
                return Err(invalid("unexpected file in hybrid storage"));
            };
            let path = entry.path();
            let len = fs::symlink_metadata(&path)?.len();
            let water = index.water.get(&(category, segment)).copied().unwrap_or(0);
            if water == 0 {
                fs::remove_file(&path)?;
                removed += len;
            } else if len > water {
                let file = open_existing(&path, true)?;
                file.set_len(water)?;
                file.sync_all()?;
                removed += len - water;
            } else if len < water {
                return Err(invalid("segment is shorter than its committed length"));
            }
        }
        sync_dir(dir)?;
    }
    Ok(removed)
}

impl Index {
    fn accept(&mut self, record: &CommitRec, payloads: &[Vec<u8>]) -> io::Result<()> {
        if record.seq != self.commit + 1 || payloads.len() != record.refs.len() {
            return Err(invalid("commit sequence is not contiguous"));
        }
        let mut updated = self.clone();
        for &(category, segment, len) in &record.waters {
            if category >= 6 {
                return Err(invalid("commit names an unknown segment"));
            }
            let previous = updated
                .water
                .get(&(category, segment))
                .copied()
                .unwrap_or(0);
            if len < previous {
                return Err(invalid("commit shortens a segment"));
            }
        }
        for (ref_, payload) in record.refs.iter().zip(payloads) {
            let water = record
                .waters
                .iter()
                .find(|item| item.0 == ref_.category && item.1 == ref_.segment)
                .map(|item| item.2)
                .ok_or_else(|| invalid("commit omits a segment length"))?;
            let end = ref_
                .offset
                .checked_add(u64::from(ref_.len))
                .ok_or_else(|| invalid("record offset overflows"))?;
            if payload.len() != ref_.len as usize || crc32(payload) != ref_.crc || end > water {
                return Err(invalid("commit reference does not match its payload"));
            }
            updated.apply(ref_, payload)?;
        }
        for &(category, segment, len) in &record.waters {
            updated.water.insert((category, segment), len);
        }
        updated.commit = record.seq;
        *self = updated;
        Ok(())
    }

    fn apply(&mut self, ref_: &RefRec, payload: &[u8]) -> io::Result<()> {
        let loc = Location {
            segment: ref_.segment,
            offset: ref_.offset,
            len: ref_.len,
            crc: ref_.crc,
        };
        match ref_.category {
            STRUCTURED => match payload.first().copied() {
                Some(1) => {
                    let object = decode_object(payload)?;
                    self.objects.insert(
                        object.id.as_str().to_owned(),
                        (object.kind.as_str().to_owned(), loc),
                    );
                }
                Some(2) => {
                    let relation = decode_relation(payload)?;
                    let key = (
                        relation.from.as_str().to_owned(),
                        relation.kind.as_str().to_owned(),
                        relation.to.as_str().to_owned(),
                    );
                    if self.relations.contains_key(&key) {
                        return Err(invalid("commit repeats a relation"));
                    }
                    self.relations.insert(key, loc);
                }
                _ => return Err(invalid("structured record type is unknown")),
            },
            EVENTS => {
                let text =
                    std::str::from_utf8(payload).map_err(|_| invalid("event is not UTF-8"))?;
                let event =
                    Event::from_json(text).map_err(|_| invalid("stored event is invalid"))?;
                let key = source_key(&event.source);
                let sequence = event.sequence.get();
                let slot = self.events.entry(key).or_default();
                if slot.contains_key(&sequence) {
                    return Err(invalid("commit repeats an event sequence"));
                }
                slot.insert(
                    sequence,
                    EventLoc {
                        time: event.time.unix_ns.get(),
                        subject: subject_of(&event),
                        loc,
                    },
                );
            }
            METRICS => {
                let (series, points) = decode_metric(payload)?;
                if points.is_empty() {
                    return Err(invalid("metric record is empty"));
                }
                let start = points.iter().map(|point| point.0).min().expect("points");
                let end = points.iter().map(|point| point.0).max().expect("points");
                self.metrics
                    .entry(series)
                    .or_default()
                    .push(MetricSpan { start, end, loc });
            }
            CAPTURES => {
                let (_, _, comment) = pcap::parse_packet(payload)?;
                let note = decode_note(&comment)?;
                if self.captures.contains_key(&note.id) {
                    return Err(invalid("commit repeats a capture"));
                }
                self.captures.insert(note.id, loc);
            }
            SNAPSHOTS => {
                let snapshot = decode_snapshot(payload)?;
                if self.snapshots.contains_key(snapshot.id.as_str()) {
                    return Err(invalid("commit repeats a snapshot"));
                }
                self.snapshots
                    .insert(snapshot.id.as_str().to_owned(), SnapMeta { loc });
            }
            INDEXES => {
                let (class, days) = decode_retention(payload)?;
                self.retention[class.code() as usize] = days;
            }
            _ => return Err(invalid("record category is unknown")),
        }
        Ok(())
    }

    fn refresh_tails(&mut self, dirs: &[Anchor; 6]) -> io::Result<()> {
        for category in 0..6u8 {
            let mut best: Option<(u32, u64)> = None;
            for (&(cat, id), &len) in &self.water {
                if cat == category && len > 0 && best.is_none_or(|(prev, _)| id >= prev) {
                    best = Some((id, len));
                }
            }
            let mut tail = Tail::default();
            if let Some((id, len)) = best {
                tail.segment = id;
                tail.len = len;
                if category == CAPTURES {
                    let prefix = read_file_range(
                        &dirs[category as usize],
                        id,
                        0,
                        capture_header_len() as u32,
                    )?;
                    tail.link_type = Some(pcap::link_type(&prefix)?);
                }
            }
            self.tail[category as usize] = tail;
        }
        Ok(())
    }
}

fn stage(
    bufs: &mut Vec<Buf>,
    tails: &mut [Tail],
    water: &BTreeMap<(u8, u32), u64>,
    item: &Pending,
    limit: u64,
) -> io::Result<u64> {
    let category = item.category as usize;
    let header_len = if item.capture_link.is_some() {
        capture_header_len() as u64
    } else {
        HEADER_LEN as u64
    };
    let addition = if item.capture_link.is_some() {
        item.payload.len() as u64
    } else {
        (HEADER_LEN + item.payload.len()) as u64
    };
    if let Some(link) = item.capture_link
        && tails[category]
            .link_type
            .is_some_and(|current| current != link)
    {
        tails[category] = Tail::default();
    }
    if tails[category].segment != 0
        && tails[category].len > header_len
        && tails[category].len.saturating_add(addition) > limit
    {
        tails[category] = Tail {
            link_type: tails[category].link_type,
            ..Tail::default()
        };
    }
    if tails[category].segment == 0 {
        let id = next_segment(water, bufs, item.category)?;
        let header = if let Some(link) = item.capture_link {
            pcap::preamble(link)
        } else {
            segment_header(item.category, id).to_vec()
        };
        let len = header.len() as u64;
        bufs.push(Buf {
            category: item.category,
            segment: id,
            start: 0,
            bytes: header,
            new_file: true,
        });
        tails[category].segment = id;
        tails[category].len = len;
    } else if !bufs
        .iter()
        .any(|buf| buf.category == item.category && buf.segment == tails[category].segment)
    {
        bufs.push(Buf {
            category: item.category,
            segment: tails[category].segment,
            start: tails[category].len,
            bytes: Vec::new(),
            new_file: false,
        });
    }
    let segment = tails[category].segment;
    let offset = if item.capture_link.is_some() {
        tails[category].len
    } else {
        tails[category].len + HEADER_LEN as u64
    };
    let bytes = if item.capture_link.is_some() {
        item.payload.clone()
    } else {
        frame(&item.payload)?
    };
    let buf = bufs
        .iter_mut()
        .find(|buf| buf.category == item.category && buf.segment == segment)
        .expect("segment buffer");
    buf.bytes.extend_from_slice(&bytes);
    tails[category].len += bytes.len() as u64;
    if let Some(link) = item.capture_link {
        tails[category].link_type = Some(link);
    }
    Ok(offset)
}

fn next_segment(water: &BTreeMap<(u8, u32), u64>, bufs: &[Buf], category: u8) -> io::Result<u32> {
    let max = water
        .keys()
        .filter(|key| key.0 == category)
        .map(|key| key.1)
        .chain(
            bufs.iter()
                .filter(|buf| buf.category == category)
                .map(|buf| buf.segment),
        )
        .max()
        .unwrap_or(0);
    max.checked_add(1)
        .filter(|id| *id != 0)
        .ok_or_else(|| input("segment id space is exhausted"))
}

fn write_buf(dir: &Anchor, buf: &Buf) -> io::Result<()> {
    let path = segment_path(dir, buf.segment);
    let result = (|| {
        let mut file = if buf.new_file {
            create_private(&path)?
        } else {
            let file = open_existing(&path, true)?;
            if file.metadata()?.len() != buf.start {
                return Err(invalid("segment length changed"));
            }
            file
        };
        file.seek(SeekFrom::Start(buf.start))?;
        file.write_all(&buf.bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        if buf.new_file {
            let _ = fs::remove_file(&path);
        } else if let Ok(file) = open_existing(&path, true) {
            let _ = file.set_len(buf.start);
            let _ = file.sync_all();
        }
    }
    result
}

fn rollback(dirs: &[Anchor; 6], written: &[&Buf]) {
    for buf in written.iter().rev() {
        let path = segment_path(&dirs[buf.category as usize], buf.segment);
        if buf.new_file {
            let _ = fs::remove_file(path);
        } else if let Ok(file) = open_existing(&path, true) {
            let _ = file.set_len(buf.start);
            let _ = file.sync_all();
        }
    }
}

fn append_journal(dir: &Anchor, framed: &[u8]) -> io::Result<()> {
    let path = dir.io_path().join(JOURNAL);
    let mut file = open_existing(&path, true)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len))?;
    if let Err(error) = file.write_all(framed).and_then(|_| file.sync_all()) {
        let _ = file.set_len(len);
        return Err(error);
    }
    Ok(())
}

fn segment_path(dir: &Anchor, segment: u32) -> std::path::PathBuf {
    dir.io_path().join(format!("s{segment:08}.wsg"))
}

fn parse_segment_name(name: &str) -> Option<u32> {
    let id = name.strip_prefix('s')?.strip_suffix(".wsg")?;
    if id.len() == 8 && id.bytes().all(|byte| byte.is_ascii_digit()) {
        id.parse().ok()
    } else {
        None
    }
}

fn directory_bytes(dir: &Anchor) -> io::Result<u64> {
    let mut total = 0u64;
    for entry in fs::read_dir(dir.io_path())? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.file_type().is_file() {
            return Err(invalid("unexpected entry in hybrid storage"));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| invalid("capacity overflow"))?;
    }
    Ok(total)
}

fn sync_dir(dir: &Anchor) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    File::open(dir.io_path())?.sync_all()?;
    #[cfg(not(target_os = "linux"))]
    let _ = dir;
    Ok(())
}

fn encode_object(object: &ObjectRecord) -> io::Result<Vec<u8>> {
    let body =
        serde_json::to_vec(&object.body).map_err(|_| input("object body is not valid JSON"))?;
    if body.len() > MAX_BODY {
        return Err(input("object body exceeds 256 KiB"));
    }
    let mut out = Vec::new();
    out.push(1);
    put_str(&mut out, object.id.as_str());
    put_str(&mut out, object.kind.as_str());
    put_u32(&mut out, body.len() as u32);
    out.extend_from_slice(&body);
    Ok(out)
}

fn decode_object(payload: &[u8]) -> io::Result<ObjectRecord> {
    let mut reader = Reader::new(payload);
    if reader.u8()? != 1 {
        return Err(invalid("structured record is not an object"));
    }
    let id = stored_id(&reader.str(256)?)?;
    let kind = stored_id(&reader.str(256)?)?;
    let len = reader.u32()? as usize;
    if len > MAX_BODY {
        return Err(invalid("object body exceeds 256 KiB"));
    }
    let body = reader.bytes(len)?;
    reader.finish()?;
    let body: Map<String, Value> =
        serde_json::from_slice(body).map_err(|_| invalid("object body is not a JSON object"))?;
    Ok(ObjectRecord { id, kind, body })
}

fn encode_relation(relation: &Relation) -> Vec<u8> {
    let mut out = vec![2];
    put_str(&mut out, relation.from.as_str());
    put_str(&mut out, relation.kind.as_str());
    put_str(&mut out, relation.to.as_str());
    out
}

fn decode_relation(payload: &[u8]) -> io::Result<Relation> {
    let mut reader = Reader::new(payload);
    if reader.u8()? != 2 {
        return Err(invalid("structured record is not a relation"));
    }
    let relation = Relation {
        from: stored_id(&reader.str(256)?)?,
        kind: stored_id(&reader.str(256)?)?,
        to: stored_id(&reader.str(256)?)?,
    };
    reader.finish()?;
    Ok(relation)
}

fn encode_metric(series: &Id, points: &[Point]) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    put_str(&mut out, series.as_str());
    let pairs: Vec<_> = points
        .iter()
        .map(|point| (point.unix_ns, point.value))
        .collect();
    out.extend(series::compress(&pairs));
    if out.len() > MAX_RECORD {
        return Err(input("metric record exceeds the 1 MiB storage limit"));
    }
    Ok(out)
}

fn decode_metric(payload: &[u8]) -> io::Result<(String, Vec<(i64, i64)>)> {
    let mut reader = Reader::new(payload);
    let series = reader.str(256)?;
    let points = series::decompress(&payload[payload.len() - reader.remaining()..])?;
    Ok((series, points))
}

fn encode_snapshot(snapshot: &Snapshot) -> io::Result<Vec<u8>> {
    let doc = SnapshotDoc {
        id: snapshot.id.as_str().to_owned(),
        base: snapshot.base.as_ref().map(|id| id.as_str().to_owned()),
        entries: snapshot
            .entries
            .iter()
            .map(|entry| EntryDoc {
                path: entry.path.clone(),
                state: entry.state.as_str().to_owned(),
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&doc).map_err(|_| input("snapshot cannot be encoded"))?;
    if bytes.len() > MAX_RECORD {
        return Err(input("snapshot exceeds the 1 MiB storage limit"));
    }
    Ok(bytes)
}

fn decode_snapshot(payload: &[u8]) -> io::Result<Snapshot> {
    let doc: SnapshotDoc =
        serde_json::from_slice(payload).map_err(|_| invalid("snapshot record is invalid"))?;
    let mut entries = Vec::with_capacity(doc.entries.len());
    let mut paths = BTreeSet::new();
    for entry in doc.entries {
        snapshot_path(&entry.path).map_err(|_| invalid("snapshot path is invalid"))?;
        if !paths.insert(entry.path.clone()) {
            return Err(invalid("snapshot repeats a path"));
        }
        entries.push(SnapshotEntry {
            path: entry.path,
            state: stored_id(&entry.state)?,
        });
    }
    Ok(Snapshot {
        id: stored_id(&doc.id)?,
        base: doc.base.as_deref().map(stored_id).transpose()?,
        entries,
    })
}

fn encode_retention(class: DataClass, days: Option<u32>) -> Vec<u8> {
    let mut out = vec![class.code(), u8::from(days.is_some())];
    put_u32(&mut out, days.unwrap_or(0));
    out
}

fn decode_retention(payload: &[u8]) -> io::Result<(DataClass, Option<u32>)> {
    let mut reader = Reader::new(payload);
    let class = DataClass::from_code(reader.u8()?)?;
    let mode = reader.u8()?;
    let days = reader.u32()?;
    reader.finish()?;
    match mode {
        0 if days == 0 => Ok((class, None)),
        1 if days >= 1 => Ok((class, Some(days))),
        _ => Err(invalid("retention record is invalid")),
    }
}

fn encode_commit(record: &CommitRec) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    put_u64(&mut out, record.seq);
    put_u32(&mut out, record.waters.len() as u32);
    for &(category, segment, len) in &record.waters {
        out.push(category);
        put_u32(&mut out, segment);
        put_u64(&mut out, len);
    }
    put_u32(&mut out, record.refs.len() as u32);
    for ref_ in &record.refs {
        out.push(ref_.category);
        put_u32(&mut out, ref_.segment);
        put_u64(&mut out, ref_.offset);
        put_u32(&mut out, ref_.len);
        put_u32(&mut out, ref_.crc);
    }
    Ok(out)
}

fn decode_commit(payload: &[u8]) -> io::Result<CommitRec> {
    let mut reader = Reader::new(payload);
    let seq = reader.u64()?;
    let waters_len = reader.u32()? as usize;
    if waters_len > 10_000 {
        return Err(invalid("commit lists too many segments"));
    }
    let mut waters = Vec::with_capacity(waters_len);
    for _ in 0..waters_len {
        waters.push((reader.u8()?, reader.u32()?, reader.u64()?));
    }
    let refs_len = reader.u32()? as usize;
    if refs_len > 100_000 {
        return Err(invalid("commit lists too many records"));
    }
    let mut refs = Vec::with_capacity(refs_len);
    for _ in 0..refs_len {
        refs.push(RefRec {
            category: reader.u8()?,
            segment: reader.u32()?,
            offset: reader.u64()?,
            len: reader.u32()?,
            crc: reader.u32()?,
        });
    }
    reader.finish()?;
    Ok(CommitRec { seq, waters, refs })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotDoc {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<String>,
    entries: Vec<EntryDoc>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryDoc {
    path: String,
    state: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureNote {
    id: String,
    source: String,
    reason: String,
    tagged_unix_ns: i64,
    confidence: String,
}

impl CaptureNote {
    fn from_tag(id: &Id, tag: &RiskTag) -> Self {
        Self {
            id: id.as_str().to_owned(),
            source: tag.source.as_str().to_owned(),
            reason: tag.reason.as_str().to_owned(),
            tagged_unix_ns: tag.unix_ns,
            confidence: tag.confidence.as_str().to_owned(),
        }
    }

    fn tag(&self) -> io::Result<RiskTag> {
        Ok(RiskTag {
            source: stored_id(&self.source)?,
            reason: stored_id(&self.reason)?,
            unix_ns: self.tagged_unix_ns,
            confidence: Confidence::parse(&self.confidence)?,
        })
    }
}

fn decode_note(comment: &str) -> io::Result<CaptureNote> {
    serde_json::from_str(comment).map_err(|_| invalid("capture comment is not a risk tag"))
}

#[cfg(test)]
mod tests;

use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Map, Value, json};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, ObjectRef, Record,
    SignedNanos, Source, Version,
};

use super::{
    Batch, CAPTURES, CaptureWrite, Confidence, DataClass, Hybrid, INDEXES, METRICS, ObjectRecord,
    Point, RecordRef, Relation, RiskTag, STRUCTURED, Snapshot, SnapshotEntry,
};
#[cfg(target_os = "linux")]
use crate::StorageState;
use crate::platform::Anchor;
use crate::{Core, Filter, Limits, RunState, Runtime, Settings};

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "winstrata-hybrid-{}",
            crate::platform::fresh_id().unwrap()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn user(&self) -> Anchor {
        Anchor::open(&self.0)
            .unwrap()
            .child(std::ffi::OsStr::new("user"), true, true)
            .unwrap()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn id(value: &str) -> Id {
    Id::new(value).unwrap()
}

fn object(name: &str, token: &str) -> ObjectRecord {
    let mut body = Map::new();
    body.insert("token".into(), Value::String(token.into()));
    ObjectRecord {
        id: id(name),
        kind: id("app"),
        body,
    }
}

fn source() -> Source {
    Source {
        device: id("device-1"),
        provider: id("provider.events"),
        epoch: id("epoch-1"),
    }
}

fn observation(sequence: u64, event_type: &str, subject: Option<&str>, unix_ns: i64) -> Event {
    Event {
        version: Version::CURRENT,
        source: source(),
        sequence: Counter::new(sequence).unwrap(),
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: id(event_type),
            subject: subject.map(|value| ObjectRef {
                kind: id("software"),
                id: id(value),
            }),
            user: None,
            process: None,
            attribution: Attribution::Observed {
                evidence: id("collector"),
            },
            completeness: Completeness::Complete,
        },
    }
}

fn tag(reason: &str) -> RiskTag {
    RiskTag {
        source: id("strata-tag"),
        reason: id(reason),
        unix_ns: 50,
        confidence: Confidence::Stated,
    }
}

fn count_token(store: &Hybrid, token: &str) -> usize {
    let mut count = 0;
    for category in 0..6 {
        for entry in fs::read_dir(store.category_path(category)).unwrap() {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            count += token_hits(&bytes, token);
        }
    }
    count
}

fn segment_count(store: &Hybrid, category: u8) -> usize {
    fs::read_dir(store.category_path(category))
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_str()
                .unwrap()
                .ends_with(".wsg")
        })
        .count()
}

#[test]
fn writes_query_segments_index_and_capacity() {
    let temp = Temp::new();
    let user = temp.user();
    let mut store = Hybrid::create(&user, 256).unwrap();
    assert_eq!(store.segment_bytes(), 256);
    assert!(store.reserve(1).is_ok());
    assert_eq!(
        store.reserve(u64::MAX).unwrap_err().kind(),
        std::io::ErrorKind::StorageFull
    );

    let mut batch = Batch::new();
    for index in 0..8 {
        let mut record = object(
            &format!("obj-{index}"),
            &format!("payload-token-{index:04}"),
        );
        record
            .body
            .insert("pad".into(), Value::String("x".repeat(220)));
        batch.put_object(record).unwrap();
    }
    batch
        .put_relation(Relation {
            from: id("obj-0"),
            kind: id("runs"),
            to: id("obj-1"),
        })
        .unwrap();
    batch
        .append_event(observation(1, "process.start", Some("obj-0"), 1_000))
        .unwrap();
    batch
        .append_event(observation(3, "process.sample", None, 1_500))
        .unwrap();
    batch
        .append_event(Event {
            version: Version::CURRENT,
            source: source(),
            sequence: Counter::new(2).unwrap(),
            time: EventTime {
                unix_ns: SignedNanos::new(1_200),
                precision_ns: None,
            },
            record: Record::Gap {
                reason: GapReason::Lost,
                range: None,
            },
        })
        .unwrap();
    let points: Vec<_> = (0..64)
        .map(|index| Point {
            unix_ns: 10_000 + index,
            value: 5,
        })
        .collect();
    batch
        .append_metrics(id("cpu.percent"), points.clone())
        .unwrap();
    batch
        .append_capture(CaptureWrite {
            id: id("pkt-1"),
            tag: Some(tag("risk.connection")),
            link_type: 1,
            timestamp_ns: 2_000,
            bytes: b"\x45\x00tagged-packet".to_vec(),
        })
        .unwrap();
    batch
        .put_snapshot(Snapshot {
            id: id("snap-full"),
            base: None,
            entries: vec![
                SnapshotEntry {
                    path: r"C:\App\bin".into(),
                    state: id("present"),
                },
                SnapshotEntry {
                    path: "/var/log/app".into(),
                    state: id("present"),
                },
            ],
        })
        .unwrap();
    batch
        .put_snapshot(Snapshot {
            id: id("snap-inc"),
            base: Some(id("snap-full")),
            entries: vec![SnapshotEntry {
                path: r"C:\App\bin".into(),
                state: id("changed"),
            }],
        })
        .unwrap();
    let commit = store.commit(batch).unwrap();
    assert_eq!(commit, 1);

    let mut same = Batch::new();
    same.put_relation(Relation {
        from: id("obj-0"),
        kind: id("runs"),
        to: id("obj-1"),
    })
    .unwrap();
    assert_eq!(store.commit(same).unwrap(), 1);
    assert!(store.append_duplicate_sequence().is_err());

    assert!(segment_count(&store, STRUCTURED) >= 2);
    for index in 0..8 {
        let record = store
            .get_object(&id(&format!("obj-{index}")))
            .unwrap()
            .unwrap();
        assert_eq!(
            record.body["token"],
            json!(format!("payload-token-{index:04}"))
        );
        assert_eq!(count_token(&store, &format!("payload-token-{index:04}")), 1);
    }
    assert_eq!(store.relations_from(&id("obj-0")).unwrap().len(), 1);
    assert_eq!(
        store.event(&source(), 1).unwrap().unwrap().sequence.get(),
        1
    );
    let ranged = store.events_in_time(&source(), 1_000, 1_200).unwrap();
    assert_eq!(ranged.len(), 2);
    let metrics = store.metrics(&id("cpu.percent"), 10_010, 10_012).unwrap();
    assert_eq!(
        metrics,
        vec![
            Point {
                unix_ns: 10_010,
                value: 5
            },
            Point {
                unix_ns: 10_011,
                value: 5
            },
            Point {
                unix_ns: 10_012,
                value: 5
            },
        ]
    );
    let metric_bytes = fs::read_dir(store.category_path(METRICS))
        .unwrap()
        .map(|entry| fs::metadata(entry.unwrap().path()).unwrap().len())
        .sum::<u64>();
    assert!(
        metric_bytes < 64 * 16,
        "compressed metrics occupied {metric_bytes}"
    );

    let capture = store.capture(&id("pkt-1")).unwrap().unwrap();
    assert_eq!(capture.bytes, b"\x45\x00tagged-packet");
    assert_eq!(capture.link_type, 1);
    assert_eq!(capture.tag.reason, id("risk.connection"));
    let capture_file = fs::read_dir(store.category_path(CAPTURES))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        &fs::read(&capture_file).unwrap()[..4],
        &[0x0a, 0x0d, 0x0d, 0x0a]
    );

    let view = store.materialize_snapshot(&id("snap-inc")).unwrap();
    assert_eq!(view.get(r"C:\App\bin"), Some(&id("changed")));
    assert_eq!(view.get("/var/log/app"), Some(&id("present")));
    assert!(!temp.0.join("not-created-by-snapshot").exists());
    assert_eq!(count_token(&store, "FILE-BYTES-SHOULD-NOT-EXIST"), 0);

    let hits = store.lookup(&id("obj-0")).unwrap();
    assert!(
        hits.iter()
            .any(|hit| matches!(hit, RecordRef::Object { .. }))
    );
    assert!(
        hits.iter()
            .any(|hit| matches!(hit, RecordRef::Relation { .. }))
    );
    assert!(
        hits.iter()
            .any(|hit| matches!(hit, RecordRef::Event { sequence: 1, .. }))
    );

    let report = store.capacity().unwrap();
    let mut expected = [0; 6];
    for category in 0..6 {
        expected[category as usize] = fs::read_dir(store.category_path(category))
            .unwrap()
            .map(|entry| fs::symlink_metadata(entry.unwrap().path()).unwrap().len())
            .sum();
    }
    assert_eq!(
        [
            report.structured,
            report.events,
            report.metrics,
            report.captures,
            report.snapshots,
            report.indexes
        ],
        expected
    );
    assert!(report.captures > 0);
    assert_eq!(report.total(), expected.iter().sum::<u64>());
    drop(store);

    let mut reopened = Hybrid::open(&user).unwrap();
    assert_eq!(
        reopened.get_object(&id("obj-3")).unwrap().unwrap().body["token"],
        json!("payload-token-0003")
    );
    assert_eq!(reopened.recovery().rolled_back_commits, 0);
    let mut revised = Batch::new();
    let mut record = object("obj-0", "payload-token-revised");
    record
        .body
        .insert("pad".into(), Value::String("y".repeat(20)));
    revised.put_object(record).unwrap();
    reopened.commit(revised).unwrap();
    assert_eq!(
        reopened.get_object(&id("obj-0")).unwrap().unwrap().body["token"],
        json!("payload-token-revised")
    );
    assert_eq!(count_token(&reopened, "payload-token-0000"), 1);
    assert_eq!(count_token(&reopened, "payload-token-revised"), 1);
}

trait DuplicateProbe {
    fn append_duplicate_sequence(&mut self) -> std::io::Result<u64>;
}

impl DuplicateProbe for Hybrid {
    fn append_duplicate_sequence(&mut self) -> std::io::Result<u64> {
        let mut batch = Batch::new();
        batch
            .append_event(observation(1, "process.start", None, 1_000))
            .unwrap();
        self.commit(batch)
    }
}

#[test]
fn corruption_is_rejected_and_reopen_rolls_back() {
    let temp = Temp::new();
    let user = temp.user();
    let mut store = Hybrid::create(&user, 65_536).unwrap();
    let mut first = Batch::new();
    first.put_object(object("kept", "kept-token-aaa")).unwrap();
    store.commit(first).unwrap();
    let mut second = Batch::new();
    second
        .put_object(object("damaged", "damaged-token-bbb"))
        .unwrap();
    store.commit(second).unwrap();
    assert!(store.get_object(&id("damaged")).unwrap().is_some());

    let path = flip_token(store.category_path(STRUCTURED), "damaged-token-bbb");
    let error = store.get_object(&id("damaged")).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(store.get_object(&id("kept")).unwrap().is_some());
    drop(store);

    let restored = Hybrid::open(&user).unwrap();
    assert!(restored.recovery().rolled_back_commits >= 1);
    assert!(restored.recovery().truncated_bytes > 0);
    assert!(restored.get_object(&id("kept")).unwrap().is_some());
    assert!(restored.get_object(&id("damaged")).unwrap().is_none());
    let bytes = fs::read(fs::canonicalize(&path).unwrap_or(path)).unwrap();
    assert_eq!(token_hits(&bytes, "damaged-token-bbb"), 0);
}

#[test]
fn torn_journal_and_segment_tail_roll_back_without_dropping_prior_commit() {
    let temp = Temp::new();
    let user = temp.user();
    let mut store = Hybrid::create(&user, 65_536).unwrap();
    let mut batch = Batch::new();
    batch.put_object(object("kept", "kept-token-ccc")).unwrap();
    store.commit(batch).unwrap();
    let journal = fs::canonicalize(store.category_path(INDEXES))
        .unwrap()
        .join("journal.wsj");
    let segment = fs::canonicalize(
        fs::read_dir(store.category_path(STRUCTURED))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap();
    let mut journal_file = fs::OpenOptions::new().append(true).open(&journal).unwrap();
    journal_file.write_all(b"TORN!").unwrap();
    journal_file.sync_all().unwrap();
    let mut segment_file = fs::OpenOptions::new().append(true).open(&segment).unwrap();
    segment_file.write_all(b"TORN-SEGMENT").unwrap();
    segment_file.sync_all().unwrap();
    drop(journal_file);
    drop(segment_file);
    drop(store);

    let restored = Hybrid::open(&user).unwrap();
    assert_eq!(restored.recovery().rolled_back_commits, 0);
    assert!(restored.recovery().truncated_bytes >= 5 + b"TORN-SEGMENT".len() as u64);
    assert!(restored.get_object(&id("kept")).unwrap().is_some());
    assert_eq!(token_hits(&fs::read(&journal).unwrap(), "TORN!"), 0);
    assert_eq!(token_hits(&fs::read(&segment).unwrap(), "TORN-SEGMENT"), 0);
}

#[test]
fn untagged_packets_retention_and_failed_batch_do_not_extend_durable_bytes() {
    let temp = Temp::new();
    let user = temp.user();
    let mut store = Hybrid::create(&user, 65_536).unwrap();
    let mut batch = Batch::new();
    let error = batch.append_capture(CaptureWrite {
        id: id("pkt-hidden"),
        tag: None,
        link_type: 1,
        timestamp_ns: 10,
        bytes: b"raw".to_vec(),
    });
    assert_eq!(error.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    batch.set_retention(DataClass::Events, Some(14)).unwrap();
    batch
        .append_event(observation(1, "process.start", None, 10))
        .unwrap();
    store.commit(batch).unwrap();
    let before = store.capacity().unwrap();
    assert_eq!(store.retention(DataClass::Events), Some(14));
    assert_eq!(store.retention(DataClass::Captures), None);
    assert_eq!(before.captures, 0);
    assert_eq!(segment_count(&store, CAPTURES), 0);

    let mut rejected = Batch::new();
    rejected
        .append_event(observation(1, "process.start", None, 11))
        .unwrap();
    assert!(store.commit(rejected).is_err());
    assert_eq!(store.capacity().unwrap().events, before.events);
    drop(store);

    let restored = Hybrid::open(&user).unwrap();
    assert_eq!(restored.retention(DataClass::Events), Some(14));
    assert!(restored.event(&source(), 1).unwrap().is_some());
    assert_eq!(restored.capacity().unwrap().events, before.events);
}

#[test]
fn session_stores_events_from_the_bus_and_migrates_the_record_once() {
    let temp = Temp::new();
    let locator = temp.0.join("locator");
    let source_root = temp.0.join("source");
    let destination = temp.0.join("destination");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&destination).unwrap();
    let watched = temp.0.join("watched");
    fs::create_dir(&watched).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&source_root).unwrap();
    core.update_settings(&Settings {
        start_at_login: false,
        monitored_roots: vec![watched],
    })
    .unwrap();
    core.start_live().unwrap();
    let runtime = Runtime::start(Limits::standard()).unwrap();
    let subscription = runtime.bus().subscribe(1, Filter::default()).unwrap();
    let event = observation(1, "process.start", Some("obj-bus"), 42);
    runtime.bus().publish(event.clone()).unwrap();
    let delivered = subscription
        .recv_batch(1, std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(delivered.len(), 1);
    let mut store = core.open_hybrid().unwrap();
    let mut batch = Batch::new();
    batch
        .put_object(object("obj-bus", "bus-object-token"))
        .unwrap();
    batch.append_event((*delivered[0]).clone()).unwrap();
    core.commit_hybrid(&mut store, batch).unwrap();
    assert_eq!(store.event(&source(), 1).unwrap().unwrap(), event);
    assert_eq!(core.run_state(), RunState::Live);
    drop(store);
    runtime.shutdown();
    core.stop();
    let user = core.current_user().to_owned();
    core.select_directory(&destination).unwrap();
    let migrated = core.open_hybrid().unwrap();
    assert_eq!(
        migrated.get_object(&id("obj-bus")).unwrap().unwrap().body["token"],
        json!("bus-object-token")
    );
    assert_eq!(count_token(&migrated, "bus-object-token"), 1);
    drop(migrated);
    assert!(directory_contains(
        &source_root.join(&user).join("data"),
        "bus-object-token"
    ));
    assert!(directory_contains(
        &destination.join(&user).join("data"),
        "bus-object-token"
    ));
}

#[test]
fn measured_event_batch_round_trips() {
    let temp = Temp::new();
    let user = temp.user();
    let mut store = Hybrid::create(&user, 1024 * 1024).unwrap();
    let mut batch = Batch::new();
    for sequence in 1..=2_000 {
        batch
            .append_event(observation(
                sequence,
                "process.sample",
                None,
                sequence as i64,
            ))
            .unwrap();
    }
    let started = Instant::now();
    store.commit(batch).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(
        store.event(&source(), 1).unwrap().unwrap().sequence.get(),
        1
    );
    assert_eq!(
        store
            .event(&source(), 2_000)
            .unwrap()
            .unwrap()
            .sequence
            .get(),
        2_000
    );
    assert!(store.event(&source(), 2_001).unwrap().is_none());
    eprintln!("hybrid_commit_ms={}", elapsed.as_millis());
    assert!(elapsed.as_secs() < 30, "commit took {elapsed:?}");
}

#[cfg(target_os = "linux")]
#[test]
fn unwritable_journal_suspends_live_mode_and_keeps_the_prior_commit() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Temp::new();
    let locator = temp.0.join("locator");
    let root = temp.0.join("root");
    fs::create_dir(&locator).unwrap();
    fs::create_dir(&root).unwrap();
    let watched = temp.0.join("watched");
    fs::create_dir(&watched).unwrap();
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&root).unwrap();
    core.update_settings(&Settings {
        start_at_login: false,
        monitored_roots: vec![watched],
    })
    .unwrap();
    core.start_live().unwrap();
    let mut store = core.open_hybrid().unwrap();
    let mut batch = Batch::new();
    batch.put_object(object("kept", "suspend-token")).unwrap();
    core.commit_hybrid(&mut store, batch).unwrap();
    let journal = store.category_path(INDEXES).join("journal.wsj");
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o400)).unwrap();
    let mut next = Batch::new();
    next.put_object(object("blocked", "blocked-token")).unwrap();
    let error = core.commit_hybrid(&mut store, next).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(matches!(
        core.storage_state(),
        StorageState::NeedsDirectory { .. }
    ));
    assert_eq!(core.run_state(), RunState::Stopped);
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o600)).unwrap();
    core.select_directory(&root).unwrap();
    assert!(matches!(core.storage_state(), StorageState::Ready));
    let reopened = core.open_hybrid().unwrap();
    assert!(reopened.get_object(&id("kept")).unwrap().is_some());
    assert!(reopened.get_object(&id("blocked")).unwrap().is_none());
}

fn token_hits(bytes: &[u8], token: &str) -> usize {
    bytes
        .windows(token.len())
        .filter(|window| *window == token.as_bytes())
        .count()
}

fn directory_contains(dir: &std::path::Path, token: &str) -> bool {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() && directory_contains(&path, token) {
            return true;
        }
        if path.is_file() && token_hits(&fs::read(&path).unwrap(), token) > 0 {
            return true;
        }
    }
    false
}

fn flip_token(dir: &std::path::Path, token: &str) -> PathBuf {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let mut bytes = fs::read(&path).unwrap();
        if let Some(position) = bytes
            .windows(token.len())
            .position(|window| window == token.as_bytes())
        {
            bytes[position] ^= 0xff;
            let real = fs::canonicalize(&path).unwrap();
            let mut file = fs::OpenOptions::new().write(true).open(&real).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(&bytes).unwrap();
            file.sync_all().unwrap();
            return real;
        }
    }
    panic!("token was not stored");
}

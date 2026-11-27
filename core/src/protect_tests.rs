use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde_json::{Map, Value, json};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, Id, ObjectRef, Record, SignedNanos,
    Source, Version,
};

use super::{
    BackupKind, Core, HybridRiskTag, RunState, Settings, StorageState,
    hybrid::{
        Batch, CaptureWrite, Confidence, DataClass, Hybrid, ObjectRecord, Point, Snapshot,
        SnapshotEntry,
    },
    platform, vault,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "winstrata-protect-{}",
            platform::fresh_id().unwrap()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn directory(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
}

impl Drop for Scratch {
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

fn observation(sequence: u64, unix_ns: i64) -> Event {
    Event {
        version: Version::CURRENT,
        source: source(),
        sequence: Counter::new(sequence).unwrap(),
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: id("process.start"),
            subject: Some(ObjectRef {
                kind: id("software"),
                id: id("obj-1"),
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

fn workspace(temp: &Scratch) -> (Core, PathBuf, PathBuf) {
    let locator = temp.directory("locator");
    let root = temp.directory("root");
    let watched = temp.directory("watched");
    let mut core = Core::open_at(&locator, None).unwrap();
    core.select_directory(&root).unwrap();
    core.update_settings(&Settings {
        start_at_login: false,
        monitored_roots: vec![watched],
    })
    .unwrap();
    (core, root, locator)
}

fn sample_batch(token: &str, sequence: u64) -> Batch {
    let mut batch = Batch::new();
    batch.put_object(object("obj-1", token)).unwrap();
    batch.append_event(observation(sequence, 1_000)).unwrap();
    batch
        .append_metrics(
            id("cpu.percent"),
            vec![Point {
                unix_ns: 10,
                value: 5,
            }],
        )
        .unwrap();
    batch
        .append_capture(CaptureWrite {
            id: id("pkt-1"),
            tag: Some(HybridRiskTag {
                source: id("strata-tag"),
                reason: id("risk.connection"),
                unix_ns: 50,
                confidence: Confidence::Stated,
            }),
            link_type: 1,
            timestamp_ns: 2_000,
            bytes: b"tagged-packet-013".to_vec(),
        })
        .unwrap();
    batch
        .put_snapshot(Snapshot {
            id: id("snap-1"),
            base: None,
            entries: vec![SnapshotEntry {
                path: "/var/log/app".into(),
                state: id("present"),
            }],
        })
        .unwrap();
    batch.set_retention(DataClass::Events, Some(14)).unwrap();
    batch
}

fn tree_contains(path: &Path, token: &str) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.is_dir() {
        fs::read_dir(path)
            .unwrap()
            .any(|entry| tree_contains(&entry.unwrap().path(), token))
    } else if metadata.is_file() {
        fs::read(path)
            .unwrap()
            .windows(token.len())
            .any(|window| window == token.as_bytes())
    } else {
        false
    }
}

fn segment_image(store: &Hybrid) -> Vec<u8> {
    let mut image = Vec::new();
    for category in 0..6 {
        let mut paths: Vec<_> = fs::read_dir(store.category_path(category))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        paths.sort();
        for path in paths {
            image.extend(fs::read(path).unwrap());
        }
    }
    image
}

const PASSWORD: &str = "pw-013-visible-secret";
const TOKEN: &str = "token-013-plaintext";

#[test]
fn encryption_hides_records_and_locking_keeps_monitoring_writes() {
    let temp = Scratch::new();
    let (mut core, root, locator) = workspace(&temp);
    assert_eq!(
        core.enable_encryption("short", 0).unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    core.start_live().unwrap();
    assert_eq!(
        core.enable_encryption(PASSWORD, 0).unwrap_err().kind(),
        ErrorKind::ResourceBusy
    );
    core.stop();
    core.enable_encryption(PASSWORD, 0).unwrap();
    let protection = core.protection().unwrap();
    assert!(protection.encrypted && !protection.locked);
    let user = core.current_user().to_owned();
    let mut store = core.open_hybrid().unwrap();
    core.commit_hybrid(&mut store, sample_batch(TOKEN, 1))
        .unwrap();
    assert_eq!(
        store.get_object(&id("obj-1")).unwrap().unwrap().body["token"],
        json!(TOKEN)
    );
    assert!(store.event(&source(), 1).unwrap().is_some());
    assert_eq!(store.metrics(&id("cpu.percent"), 0, 20).unwrap().len(), 1);
    assert!(store.capture(&id("pkt-1")).unwrap().is_some());
    assert!(store.snapshot(&id("snap-1")).unwrap().is_some());
    assert_eq!(store.retention(DataClass::Events), Some(14));
    assert!(!tree_contains(&root.join(&user).join("data"), TOKEN));
    assert!(!tree_contains(&root.join(&user), PASSWORD));
    drop(store);

    core.start_live().unwrap();
    assert_eq!(
        core.backup_full(&temp.0.join("live.wsb"))
            .unwrap_err()
            .kind(),
        ErrorKind::ResourceBusy
    );
    core.lock().unwrap();
    assert_eq!(core.run_state(), RunState::Live);
    assert!(matches!(core.storage_state(), StorageState::Ready));
    match core.open_hybrid() {
        Err(error) => assert!(vault::is_locked(&error)),
        Ok(_) => panic!("locked storage opened"),
    }
    core.unlock(PASSWORD).unwrap();
    let mut store = core.open_hybrid().unwrap();
    core.lock().unwrap();
    assert_eq!(core.run_state(), RunState::Live);
    let locked = store.get_object(&id("obj-1")).unwrap_err();
    assert!(vault::is_locked(&locked));
    assert!(matches!(core.storage_state(), StorageState::Ready));
    let mut next = Batch::new();
    next.put_object(object("obj-2", "token-013-after-lock"))
        .unwrap();
    core.commit_hybrid(&mut store, next).unwrap();
    assert_eq!(core.run_state(), RunState::Live);
    assert!(matches!(core.storage_state(), StorageState::Ready));
    assert!(vault::is_password_error(
        &core.unlock("wrong-password-value").unwrap_err()
    ));
    core.unlock(PASSWORD).unwrap();
    assert_eq!(
        store.get_object(&id("obj-1")).unwrap().unwrap().body["token"],
        json!(TOKEN)
    );
    assert_eq!(
        store.get_object(&id("obj-2")).unwrap().unwrap().body["token"],
        json!("token-013-after-lock")
    );
    core.stop();
    drop(store);

    drop(core);
    let mut restarted = Core::open_at(&locator, None).unwrap();
    match restarted.open_hybrid() {
        Err(error) => assert!(vault::is_locked(&error)),
        Ok(_) => panic!("locked storage opened"),
    }
    assert!(matches!(restarted.storage_state(), StorageState::Ready));
    restarted.unlock(PASSWORD).unwrap();
    let store = restarted.open_hybrid().unwrap();
    assert!(store.get_object(&id("obj-2")).unwrap().is_some());
}

#[test]
fn password_rotation_and_migration_preserve_plaintext() {
    let temp = Scratch::new();
    let (mut core, root, _) = workspace(&temp);
    core.enable_encryption(PASSWORD, 0).unwrap();
    let mut store = core.open_hybrid().unwrap();
    core.commit_hybrid(&mut store, sample_batch(TOKEN, 1))
        .unwrap();
    let before = segment_image(&store);
    core.change_password(PASSWORD, "pw-013-rotated-secret")
        .unwrap();
    assert!(vault::is_password_error(
        &core.unlock(PASSWORD).unwrap_err()
    ));
    core.lock().unwrap();
    core.unlock("pw-013-rotated-secret").unwrap();
    assert_eq!(
        store.get_object(&id("obj-1")).unwrap().unwrap().body["token"],
        json!(TOKEN)
    );
    core.rotate_keys(&mut store, "pw-013-rotated-secret")
        .unwrap();
    let after = segment_image(&store);
    assert_ne!(before, after);
    assert!(!tree_contains(store.category_path(0), TOKEN));
    assert_eq!(
        store.get_object(&id("obj-1")).unwrap().unwrap().body["token"],
        json!(TOKEN)
    );
    assert_eq!(
        store.capture(&id("pkt-1")).unwrap().unwrap().bytes,
        b"tagged-packet-013"
    );
    drop(store);

    let destination = temp.directory("moved");
    let user = core.current_user().to_owned();
    core.select_directory(&destination).unwrap();
    assert!(
        fs::read_dir(root.join(&user).join("data"))
            .unwrap()
            .next()
            .is_some()
    );
    assert!(!tree_contains(&root.join(&user), TOKEN));
    assert!(!tree_contains(&destination.join(&user), TOKEN));
    let store = core.open_hybrid().unwrap();
    assert_eq!(
        store.get_object(&id("obj-1")).unwrap().unwrap().body["token"],
        json!(TOKEN)
    );
    assert!(root.join(&user).join("vault.wsv").is_file());
    assert!(destination.join(&user).join("vault.wsv").is_file());
}

#[test]
fn backup_and_restore_keep_ciphertext_integrity() {
    let temp = Scratch::new();
    let (mut core, _root, _) = workspace(&temp);
    core.enable_encryption(PASSWORD, 0).unwrap();
    let device = core.device_id().unwrap();
    let mut store = core.open_hybrid().unwrap();
    core.commit_hybrid(&mut store, sample_batch(TOKEN, 1))
        .unwrap();
    drop(store);
    let full = temp.0.join("full.wsb");
    let full_report = core.backup_full(&full).unwrap();
    assert_eq!(full_report.kind, BackupKind::Full);
    assert_eq!(full_report.device_id, device);
    assert!(!tree_contains(&full, TOKEN));
    assert!(!tree_contains(&full, PASSWORD));
    let mut store = core.open_hybrid().unwrap();
    let mut extra = Batch::new();
    extra
        .put_object(object("obj-2", "token-013-second"))
        .unwrap();
    core.commit_hybrid(&mut store, extra).unwrap();
    drop(store);
    let incremental = temp.0.join("incremental.wsb");
    let incremental_report = core.backup_incremental(&incremental, &full).unwrap();
    assert_eq!(incremental_report.kind, BackupKind::Incremental);
    assert_eq!(incremental_report.base_commit, full_report.commit);
    assert!(incremental_report.commit > full_report.commit);
    assert_eq!(core.verify_backup(&full).unwrap().files, full_report.files);

    let mut tampered = fs::read(&full).unwrap();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x5a;
    let tampered_path = temp.0.join("tampered.wsb");
    fs::write(&tampered_path, &tampered).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tampered_path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(
        core.verify_backup(&tampered_path).unwrap_err().kind(),
        ErrorKind::InvalidData
    );

    let restore_root = temp.directory("restored");
    let restored = core.restore_full(&restore_root, &full).unwrap();
    assert_eq!(restored.source_device, device);
    let wrong = temp.directory("wrong-user");
    assert_eq!(
        super::backup::restore_full(&wrong, "another-user", &full)
            .unwrap_err()
            .kind(),
        ErrorKind::PermissionDenied
    );
    assert!(!wrong.join("another-user").exists());

    let locator = temp.directory("restored-locator");
    let mut recovered = Core::open_at(&locator, None).unwrap();
    recovered.select_directory(&restore_root).unwrap();
    match recovered.open_hybrid() {
        Err(error) => assert!(vault::is_locked(&error)),
        Ok(_) => panic!("locked storage opened"),
    }
    recovered.unlock(PASSWORD).unwrap();
    assert_eq!(recovered.device_id().unwrap(), device);
    let store = recovered.open_hybrid().unwrap();
    assert!(store.get_object(&id("obj-1")).unwrap().is_some());
    assert!(store.get_object(&id("obj-2")).unwrap().is_none());
    drop(store);
    recovered
        .restore_incremental(&restore_root, &full, &incremental)
        .unwrap();
    match recovered.open_hybrid() {
        Err(error) => assert!(vault::is_locked(&error)),
        Ok(_) => panic!("locked storage opened"),
    }
    recovered.unlock(PASSWORD).unwrap();
    let store = recovered.open_hybrid().unwrap();
    assert!(store.get_object(&id("obj-1")).unwrap().is_some());
    assert_eq!(
        store.get_object(&id("obj-2")).unwrap().unwrap().body["token"],
        json!("token-013-second")
    );
    assert!(!tree_contains(
        &restore_root.join(recovered.current_user()),
        TOKEN
    ));
}

#[test]
fn auto_lock_blocks_reads_without_blocking_the_next_commit() {
    let temp = Scratch::new();
    let (mut core, _, _) = workspace(&temp);
    core.enable_encryption(PASSWORD, 2).unwrap();
    let mut store = core.open_hybrid().unwrap();
    core.commit_hybrid(&mut store, sample_batch(TOKEN, 1))
        .unwrap();
    assert!(store.get_object(&id("obj-1")).unwrap().is_some());
    thread::sleep(Duration::from_millis(2_100));
    assert!(vault::is_locked(
        &store.get_object(&id("obj-1")).unwrap_err()
    ));
    let mut next = Batch::new();
    next.put_object(object("obj-2", "token-013-after-autolock"))
        .unwrap();
    core.commit_hybrid(&mut store, next).unwrap();
    core.unlock(PASSWORD).unwrap();
    assert!(store.get_object(&id("obj-1")).unwrap().is_some());
    assert!(store.get_object(&id("obj-2")).unwrap().is_some());
}

#[test]
fn encryption_rejects_a_store_that_already_has_records() {
    let temp = Scratch::new();
    let (mut core, _, _) = workspace(&temp);
    let mut store = core.open_hybrid().unwrap();
    core.commit_hybrid(&mut store, sample_batch(TOKEN, 1))
        .unwrap();
    drop(store);
    assert_eq!(
        core.enable_encryption(PASSWORD, 0).unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert!(tree_contains(
        &temp.0.join("root").join(core.current_user()).join("data"),
        TOKEN
    ));
}

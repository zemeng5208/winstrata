//! 首次引导、选定范围基线和实时/历史模式。
//!
//! 官方插件的采集实现仍不在这个计划里。这里只记录用户的启用选择，
//! 不把缺席的实现标成已经启动。救援不进入批量启用。预设脚本和自动快照保持关闭。

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use winstrata_protocols::{
    Counter, Event, EventTime, GapReason, Id, Record, SignedNanos, Source, Version,
};

use crate::file_registry::{Channel, CollectError, FileRegistry};
use crate::hybrid::{Batch, DataClass, ObjectRecord, Snapshot, SnapshotEntry};
use crate::plugins::OFFICIAL;
use crate::scope::{CollectionScope, PathClass};
use crate::storage;
use crate::{Core, EventBus, Limits, RunState};

const FILE_NAME: &str = "onboarding.json";
const VERSION: u32 = 1;
const RESCUE: &str = "strata.rescue";
const MCP: &str = "strata.mcp";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Onboarding {
    pub version: u32,
    pub step: String,
    pub complete: bool,
    pub skipped: Vec<String>,
    pub language: Option<String>,
    pub opted_in: Vec<String>,
    pub rescue: bool,
    pub mcp: bool,
    pub encryption: bool,
    pub baseline_id: Option<String>,
    pub baseline_label: Option<String>,
    pub retention_days: Option<u32>,
    pub scope: CollectionScope,
    pub scripts: bool,
    pub automatic_snapshots: bool,
    pub marker_count: u64,
}

impl Default for Onboarding {
    fn default() -> Self {
        Self {
            version: VERSION,
            step: "language".into(),
            complete: false,
            skipped: Vec::new(),
            language: None,
            opted_in: Vec::new(),
            rescue: false,
            mcp: false,
            encryption: false,
            baseline_id: None,
            baseline_label: None,
            retention_days: None,
            scope: CollectionScope::default(),
            scripts: false,
            automatic_snapshots: false,
            marker_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeMarker {
    pub reason: String,
    pub activity: String,
    pub sequence: u64,
    pub release_drivers: bool,
}

impl Core {
    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn collector_note(&self) -> &str {
        &self.collector_note
    }

    pub fn onboarding(&mut self) -> io::Result<Onboarding> {
        if !self.storage_ready() {
            return Ok(Onboarding::default());
        }
        let mut record = self.read_onboarding()?;
        record.scope.roots = self.settings()?.monitored_roots;
        Ok(record)
    }

    pub fn choose_location(
        &mut self,
        root: &Path,
        language: Option<&str>,
        skip_language: bool,
    ) -> io::Result<Onboarding> {
        // 调用方必须给出目录。这里不会在失败时改选 C: 或其他盘。
        self.select_directory(root)?;
        let mut record = self.read_onboarding().unwrap_or_default();
        if let Some(language) = language {
            record.language = Some(normalize_language(language)?);
            record.skipped.retain(|step| step != "language");
        } else if skip_language {
            record.language = None;
            push_skip(&mut record, "language");
        }
        if record.step == "language" || record.step == "location" {
            record.step = "plugins".into();
        }
        record.scope.roots = self.settings()?.monitored_roots;
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn skip_onboarding(&mut self, step: &str) -> io::Result<Onboarding> {
        let mut record = self.require_record()?;
        if record.step != step {
            return Err(invalid_input("that onboarding step is not current"));
        }
        match step {
            "language" | "plugins" | "retention" | "boot" | "encryption" | "mcp" | "baseline" => {}
            _ => return Err(invalid_input("location and scope cannot be skipped")),
        }
        push_skip(&mut record, step);
        record.step = next_step(step).to_owned();
        if record.step == "complete" {
            record.complete = true;
        }
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn set_onboarding_language(&mut self, language: &str) -> io::Result<Onboarding> {
        let mut record = self.require_record()?;
        record.language = Some(normalize_language(language)?);
        record.skipped.retain(|step| step != "language");
        if record.step == "language" {
            record.step = "location".into();
        }
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn opt_in(&mut self, ids: &[String]) -> io::Result<Onboarding> {
        let mut record = self.require_record()?;
        let mut chosen = Vec::new();
        for id in ids {
            if id == RESCUE {
                return Err(invalid_input("rescue is excluded from batch enablement"));
            }
            if !OFFICIAL.iter().any(|plugin| plugin.id == id) {
                return Err(invalid_input("unknown plugin"));
            }
            if !chosen.iter().any(|existing: &String| existing == id) {
                chosen.push(id.clone());
            }
        }
        record.opted_in = chosen;
        record.mcp = record.opted_in.iter().any(|id| id == MCP);
        record.rescue = false;
        record.skipped.retain(|step| step != "plugins");
        if record.step == "plugins" {
            record.step = "scope".into();
        }
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn set_rescue_opt_in(&mut self, enabled: bool) -> io::Result<Onboarding> {
        let mut record = self.require_record()?;
        record.rescue = enabled;
        record.opted_in.retain(|id| id != RESCUE);
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn set_collection_scope(&mut self, scope: CollectionScope) -> io::Result<Onboarding> {
        scope.validate()?;
        let mut record = self.require_record()?;
        let mut settings = self.settings()?;
        settings.monitored_roots = scope.roots.clone();
        self.update_settings(&settings)?;
        record.scope = scope;
        record.skipped.retain(|step| step != "scope");
        if record.step == "scope" {
            record.step = "retention".into();
        }
        self.write_onboarding(&record)?;
        self.apply_scope_to_collectors()?;
        Ok(record)
    }

    pub fn confirm_retention(&mut self, days: Option<u32>) -> io::Result<Onboarding> {
        if days == Some(0) {
            return Err(invalid_input(
                "retention days start at 1; use none for indefinite",
            ));
        }
        let mut record = self.require_record()?;
        if let Some(days) = days {
            let mut store = self.open_hybrid()?;
            let mut batch = Batch::new();
            for class in [
                DataClass::Structured,
                DataClass::Events,
                DataClass::Metrics,
                DataClass::Captures,
                DataClass::Snapshots,
            ] {
                batch.set_retention(class, Some(days))?;
            }
            self.commit_hybrid(&mut store, batch)?;
        }
        record.retention_days = days;
        record.skipped.retain(|step| step != "retention");
        if record.step == "retention" {
            record.step = "boot".into();
        }
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn set_boot_preference(&mut self, start_at_login: bool) -> io::Result<Onboarding> {
        let mut record = self.require_record()?;
        let mut settings = self.settings()?;
        settings.start_at_login = start_at_login;
        self.update_settings(&settings)?;
        record.skipped.retain(|step| step != "boot");
        if record.step == "boot" {
            record.step = "encryption".into();
        }
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn enable_onboarding_encryption(
        &mut self,
        password: &str,
        auto_lock_secs: u32,
    ) -> io::Result<Onboarding> {
        self.enable_encryption(password, auto_lock_secs)?;
        let mut record = self.require_record()?;
        record.encryption = true;
        record.skipped.retain(|step| step != "encryption");
        if record.step == "encryption" {
            record.step = "mcp".into();
        }
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn set_mcp(&mut self, enabled: bool) -> io::Result<Onboarding> {
        let mut record = self.require_record()?;
        record.mcp = enabled;
        if enabled {
            if !record.opted_in.iter().any(|id| id == MCP) {
                record.opted_in.push(MCP.into());
            }
        } else {
            record.opted_in.retain(|id| id != MCP);
        }
        record.skipped.retain(|step| step != "mcp");
        if record.step == "mcp" {
            record.step = "baseline".into();
        }
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn build_baseline(&mut self) -> io::Result<Onboarding> {
        let mut record = self.require_record()?;
        if record.scripts || record.automatic_snapshots {
            return Err(invalid_input(
                "preset scripts and automatic snapshots stay off",
            ));
        }
        record.scope.roots = self.settings()?.monitored_roots;
        if record.scope.roots.is_empty() {
            return Err(invalid_input("select at least one monitoring scope"));
        }
        let mut entries = Vec::new();
        for root in &record.scope.roots.clone() {
            walk_baseline(root, &record.scope, &mut entries)?;
        }
        let sequence = record.marker_count.saturating_add(1);
        let id = Id::new(format!("baseline-{sequence}"))
            .map_err(|_| invalid_input("baseline identifier is invalid"))?;
        let mut body = Map::new();
        body.insert("label".into(), Value::String("reconstructed".into()));
        body.insert("files".into(), Value::from(entries.len()));
        for source in [
            "software",
            "registry",
            "environment",
            "services",
            "drivers",
            "tasks",
        ] {
            body.insert(source.into(), Value::String("not_scanned".into()));
        }
        let mut batch = Batch::new();
        batch.put_object(ObjectRecord {
            id: id.clone(),
            kind: Id::new("baseline").map_err(|_| invalid_input("baseline kind is invalid"))?,
            body,
        })?;
        // 完整快照至少要有一项。范围内没有匹配文件时只保留对象，并在 files 中记 0。
        if !entries.is_empty() {
            batch.put_snapshot(Snapshot {
                id: id.clone(),
                base: None,
                entries,
            })?;
        }
        let mut store = self.open_hybrid()?;
        self.commit_hybrid(&mut store, batch)?;
        record.baseline_id = Some(id.as_str().to_owned());
        record.baseline_label = Some("reconstructed".into());
        record.marker_count = sequence;
        record.skipped.retain(|step| step != "baseline");
        record.step = "complete".into();
        record.complete = true;
        self.write_onboarding(&record)?;
        Ok(record)
    }

    pub fn begin_live(&mut self) -> io::Result<ModeMarker> {
        self.start_live()?;
        let note = self.start_collectors();
        self.collector_note = note.clone();
        self.record_marker(&note, "live", false)
    }

    pub fn pause_monitoring(&mut self) -> io::Result<ModeMarker> {
        if self.run_state() != RunState::Live || self.paused {
            return Err(invalid_input("pause applies to live monitoring"));
        }
        self.stop_collectors();
        self.paused = true;
        self.record_gap(GapReason::Paused, "paused", "pause", false)
    }

    pub fn resume_monitoring(&mut self) -> io::Result<ModeMarker> {
        if self.run_state() != RunState::Live || !self.paused {
            return Err(invalid_input("resume applies to paused monitoring"));
        }
        let note = self.start_collectors();
        self.collector_note = note;
        self.paused = false;
        self.record_marker("resumed", "resume", false)
    }

    pub fn switch_to_history(&mut self) -> io::Result<ModeMarker> {
        let was_live = self.run_state() == RunState::Live;
        self.stop_collectors();
        self.enter_history();
        if was_live {
            self.record_gap(GapReason::Paused, "paused", "history", true)
        } else {
            self.record_marker("paused", "history", true)
        }
    }

    pub fn stop_monitoring(&mut self) -> io::Result<ModeMarker> {
        let was_live = self.run_state() == RunState::Live;
        self.stop_collectors();
        self.stop();
        if was_live {
            self.record_gap(GapReason::Paused, "paused", "stop", true)
        } else {
            Ok(ModeMarker {
                reason: "stopped".into(),
                activity: "idle".into(),
                sequence: 0,
                release_drivers: false,
            })
        }
    }

    pub fn note_collection_gap(&mut self, reason: &str) -> io::Result<ModeMarker> {
        let (gap, activity) = match reason {
            "lost" => (GapReason::Lost, "loss"),
            "source_failure" => (GapReason::SourceFailure, "failure"),
            "filtered" => (GapReason::Filtered, "scope"),
            _ => return Err(invalid_input("gap reason is not loss, failure, or filtered")),
        };
        self.record_gap(gap, reason, activity, false)
    }

    pub fn markers(&mut self) -> io::Result<Vec<ModeMarker>> {
        if !self.storage_ready() {
            return Ok(Vec::new());
        }
        let record = self.read_onboarding()?;
        let store = self.open_hybrid()?;
        let mut markers = Vec::new();
        for sequence in 1..=record.marker_count {
            let id = Id::new(format!("mode-{sequence}"))
                .map_err(|_| invalid_data("stored mode identifier is invalid"))?;
            let Some(object) = store.get_object(&id)? else {
                continue;
            };
            let activity = object
                .body
                .get("activity")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let reason = object
                .body
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let release = object
                .body
                .get("releaseDrivers")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            markers.push(ModeMarker {
                reason: reason.to_owned(),
                activity: activity.to_owned(),
                sequence,
                release_drivers: release,
            });
        }
        Ok(markers)
    }

    fn storage_ready(&self) -> bool {
        matches!(self.storage_state(), crate::StorageState::Ready)
    }

    fn require_record(&mut self) -> io::Result<Onboarding> {
        if !self.storage_ready() {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                "select a storage directory",
            ));
        }
        let mut record = self.read_onboarding()?;
        if record.scripts || record.automatic_snapshots {
            return Err(invalid_data(
                "preset scripts and automatic snapshots stay off",
            ));
        }
        record.scope.roots = self.settings()?.monitored_roots;
        Ok(record)
    }

    fn read_onboarding(&mut self) -> io::Result<Onboarding> {
        let root = self
            .selected_root()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "select a storage directory"))?
            .to_path_buf();
        let dir = storage::data_dir(&root, self.current_user(), false)?;
        match crate::files::read_json::<Onboarding>(&dir, FILE_NAME) {
            Ok(record) if record.version == VERSION => Ok(record),
            Ok(_) => Err(invalid_data("unsupported onboarding record")),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Onboarding::default()),
            Err(error) => Err(error),
        }
    }

    fn write_onboarding(&mut self, record: &Onboarding) -> io::Result<()> {
        if record.scripts || record.automatic_snapshots || record.opted_in.iter().any(|id| id == RESCUE)
        {
            return Err(invalid_input(
                "preset scripts, automatic snapshots, and rescue batch enablement stay off",
            ));
        }
        let root = self
            .selected_root()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "select a storage directory"))?
            .to_path_buf();
        let dir = storage::data_dir(&root, self.current_user(), false)?;
        crate::files::write_json(&dir, FILE_NAME, record)
    }

    fn record_marker(
        &mut self,
        reason: &str,
        activity: &str,
        release_drivers: bool,
    ) -> io::Result<ModeMarker> {
        self.write_mode_object(reason, activity, release_drivers, None)
    }

    fn record_gap(
        &mut self,
        gap: GapReason,
        reason: &str,
        activity: &str,
        release_drivers: bool,
    ) -> io::Result<ModeMarker> {
        self.write_mode_object(reason, activity, release_drivers, Some(gap))
    }

    fn write_mode_object(
        &mut self,
        reason: &str,
        activity: &str,
        release_drivers: bool,
        gap: Option<GapReason>,
    ) -> io::Result<ModeMarker> {
        let mut record = self.require_record()?;
        let sequence = record.marker_count.saturating_add(1);
        let mut body = Map::new();
        body.insert("activity".into(), Value::String(activity.into()));
        body.insert("reason".into(), Value::String(reason.into()));
        body.insert("sequence".into(), Value::from(sequence));
        body.insert("releaseDrivers".into(), Value::Bool(release_drivers));
        let mut batch = Batch::new();
        batch.put_object(ObjectRecord {
            id: Id::new(format!("mode-{sequence}"))
                .map_err(|_| invalid_input("mode identifier is invalid"))?,
            kind: Id::new("mode").map_err(|_| invalid_input("mode kind is invalid"))?,
            body,
        })?;
        if let Some(gap) = gap {
            batch.append_event(mode_event(sequence, gap)?)?;
        }
        let mut store = self.open_hybrid()?;
        self.commit_hybrid(&mut store, batch)?;
        record.marker_count = sequence;
        self.write_onboarding(&record)?;
        Ok(ModeMarker {
            reason: reason.to_owned(),
            activity: activity.to_owned(),
            sequence,
            release_drivers,
        })
    }

    fn apply_scope_to_collectors(&mut self) -> io::Result<()> {
        if self.collectors.is_empty() {
            return Ok(());
        }
        if self.run == RunState::Live && !self.paused {
            self.collector_note = self.start_collectors();
            return Ok(());
        }
        self.stop_collectors();
        Ok(())
    }

    pub(crate) fn stop_collectors(&mut self) {
        self.collectors.clear();
    }

    pub(crate) fn start_collectors(&mut self) -> String {
        self.collectors.clear();
        let scope = match self.onboarding() {
            Ok(record) => record.scope,
            Err(error) => return format!("unavailable: {error}"),
        };
        if scope.roots.is_empty() {
            return "scope_empty".into();
        }
        let bus = match EventBus::new(Limits::standard()) {
            Ok(bus) => bus,
            Err(error) => return format!("unavailable: {error}"),
        };
        let device = match Id::new("local-collector") {
            Ok(device) => device,
            Err(_) => return "unavailable: invalid device".into(),
        };
        let mut ready = 0;
        let mut unsupported = 0;
        for root in &scope.roots {
            match open_files(&bus, &device, root, &scope) {
                Ok(collector) => {
                    ready += 1;
                    self.collectors.push(collector);
                }
                Err(error) if error.code() == "unsupported" => unsupported += 1,
                Err(error) => return format!("source_failure: {}", error.code()),
            }
        }
        for key in &scope.registry {
            let Some(root) = scope.roots.first() else {
                break;
            };
            match open_registry(&bus, &device, root, key, &scope) {
                Ok(collector) => {
                    ready += 1;
                    self.collectors.push(collector);
                }
                Err(error) if error.code() == "unsupported" => unsupported += 1,
                Err(error) => return format!("source_failure: {}", error.code()),
            }
        }
        if ready > 0 {
            "ready".into()
        } else if unsupported > 0 {
            "unsupported".into()
        } else {
            "scope_empty".into()
        }
    }
}

fn open_files(
    bus: &EventBus,
    device: &Id,
    root: &Path,
    scope: &CollectionScope,
) -> Result<FileRegistry, CollectError> {
    let mut registry = FileRegistry::open(bus.clone(), device.clone(), root, "Software\\WinStrata")?;
    registry.set_scope(scope)?;
    registry.subscribe(Channel::Files)?;
    Ok(registry)
}

fn open_registry(
    bus: &EventBus,
    device: &Id,
    root: &Path,
    key: &str,
    scope: &CollectionScope,
) -> Result<FileRegistry, CollectError> {
    let mut registry = FileRegistry::open(bus.clone(), device.clone(), root, key)?;
    registry.set_scope(scope)?;
    registry.subscribe(Channel::Registry)?;
    Ok(registry)
}

fn walk_baseline(root: &Path, scope: &CollectionScope, entries: &mut Vec<SnapshotEntry>) -> io::Result<()> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        if !scope.decide_path(&directory, PathClass::Directory) && directory != root {
            continue;
        }
        let listing = fs::read_dir(&directory)?;
        for item in listing {
            let item = item?;
            let path = item.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if scope.decide_path(&path, PathClass::Directory) {
                    pending.push(path);
                }
                continue;
            }
            if !metadata.is_file() || !scope.admits_file(&path) {
                continue;
            }
            if entries.len() >= 8192 {
                return Err(invalid_input("snapshot exceeds 8192 entries"));
            }
            let text = path
                .to_str()
                .ok_or_else(|| invalid_input("snapshot path is not unicode"))?;
            entries.push(SnapshotEntry {
                path: text.to_owned(),
                state: Id::new("reconstructed")
                    .map_err(|_| invalid_input("baseline label is invalid"))?,
            });
        }
    }
    Ok(())
}

fn mode_event(sequence: u64, reason: GapReason) -> io::Result<Event> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    Ok(Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new("local-modes").map_err(|_| invalid_input("mode source is invalid"))?,
            provider: Id::new("winstrata.modes")
                .map_err(|_| invalid_input("mode source is invalid"))?,
            epoch: Id::new("modes").map_err(|_| invalid_input("mode source is invalid"))?,
        },
        sequence: Counter::new(sequence).map_err(|_| invalid_input("mode sequence is invalid"))?,
        time: EventTime {
            unix_ns: SignedNanos::new(now),
            precision_ns: None,
        },
        record: Record::Gap { reason, range: None },
    })
}

fn normalize_language(language: &str) -> io::Result<String> {
    match language {
        "en" | "zh-CN" => Ok(language.to_owned()),
        _ => Err(invalid_input("language must be en or zh-CN")),
    }
}

fn next_step(step: &str) -> &'static str {
    match step {
        "language" => "location",
        "location" => "plugins",
        "plugins" => "scope",
        "scope" => "retention",
        "retention" => "boot",
        "boot" => "encryption",
        "encryption" => "mcp",
        "mcp" => "baseline",
        _ => "complete",
    }
}

fn push_skip(record: &mut Onboarding, step: &str) {
    if !record.skipped.iter().any(|item| item == step) {
        record.skipped.push(step.to_owned());
    }
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message)
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Core, RunState};
    use std::path::PathBuf;
    use std::fs;
    use winstrata_kernel::{ProcessRow, SystemSnapshot};
    use winstrata_protocols::Record;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "winstrata-017-{}-{}",
                std::process::id(),
                crate::platform::fresh_id().unwrap()
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

    fn opened() -> (Scratch, Core, PathBuf) {
        let temp = Scratch::new();
        let locator = temp.directory("locator");
        let root = temp.directory("data");
        let mut core = Core::open_at(&locator, None).unwrap();
        core.choose_location(&root, Some("en"), false).unwrap();
        (temp, core, root)
    }

    #[test]
    fn onboarding_records_choices_without_starting_plugins_or_substituting_a_drive() {
        let temp = Scratch::new();
        let locator = temp.directory("locator");
        let mut core = Core::open_at(&locator, None).unwrap();
        let missing = temp.0.join("missing-parent").join("store");
        assert!(core.choose_location(&missing, None, true).is_err());
        assert!(core.selected_root().is_none());
        let root = temp.directory("chosen");
        let record = core.choose_location(&root, None, true).unwrap();
        assert_eq!(record.step, "plugins");
        assert!(record.skipped.iter().any(|step| step == "language"));
        assert!(!core.settings().unwrap().start_at_login);
        let rejected = core.opt_in(&[RESCUE.into(), "strata.trace".into()]);
        assert!(rejected.unwrap_err().to_string().contains("rescue"));
        let chosen = core
            .opt_in(&["strata.trace".into(), "strata.mcp".into()])
            .unwrap();
        assert!(chosen.mcp);
        assert!(!chosen.opted_in.iter().any(|id| id == RESCUE));
        assert!(!chosen.rescue);
        let rescue = core.set_rescue_opt_in(true).unwrap();
        assert!(rescue.rescue);
        assert!(!rescue.opted_in.iter().any(|id| id == RESCUE));
        let skipped = core.skip_onboarding("scope");
        assert!(skipped.is_err());
    }

    #[test]
    fn baseline_scope_pause_history_and_loss_stay_distinct() {
        let (temp, mut core, _root) = opened();
        let watched = temp.directory("watched");
        let outside = temp.directory("outside");
        fs::write(watched.join("keep.txt"), b"keep").unwrap();
        fs::write(watched.join("skip.exe"), b"skip").unwrap();
        fs::write(outside.join("secret.txt"), b"secret").unwrap();
        let scope = CollectionScope {
            roots: vec![watched.clone()],
            file_types: vec!["txt".into()],
            watch_processes: true,
            ..CollectionScope::default()
        };
        core.skip_onboarding("plugins").unwrap();
        let record = core.set_collection_scope(scope).unwrap();
        assert_eq!(record.step, "retention");
        core.confirm_retention(None).unwrap();
        let boot = core.set_boot_preference(false).unwrap();
        assert_eq!(boot.step, "encryption");
        assert!(!core.settings().unwrap().start_at_login);
        core.skip_onboarding("encryption").unwrap();
        core.skip_onboarding("mcp").unwrap();
        let baseline = core.build_baseline().unwrap();
        assert_eq!(baseline.baseline_label.as_deref(), Some("reconstructed"));
        assert!(baseline.complete);
        assert!(!baseline.scripts && !baseline.automatic_snapshots);
        let store = core.open_hybrid().unwrap();
        let snapshot = store
            .snapshot(&Id::new(baseline.baseline_id.unwrap()).unwrap())
            .unwrap()
            .unwrap();
        let paths = snapshot
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.iter().any(|path| path.ends_with("keep.txt")));
        assert!(paths.iter().all(|path| !path.contains("secret")));
        assert!(paths.iter().all(|path| !path.ends_with("skip.exe")));
        assert!(snapshot.entries.iter().all(|entry| entry.state.as_str() == "reconstructed"));
        let object = store
            .get_object(&Id::new(snapshot.id.as_str()).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(object.body.get("software").and_then(Value::as_str), Some("not_scanned"));
        drop(store);

        let live = core.begin_live().unwrap();
        assert_eq!(core.run_state(), RunState::Live);
        assert_eq!(live.activity, "live");
        assert!(core.collector_note() == "unsupported" || core.collector_note() == "ready");
        let paused = core.pause_monitoring().unwrap();
        assert!(core.paused());
        assert_eq!(paused.reason, "paused");
        assert_eq!(paused.activity, "pause");
        assert!(!paused.release_drivers);
        let resumed = core.resume_monitoring().unwrap();
        assert_eq!(resumed.reason, "resumed");
        assert!(!core.paused());
        let history = core.switch_to_history().unwrap();
        assert_eq!(core.run_state(), RunState::History);
        assert_eq!(history.activity, "history");
        assert!(history.release_drivers);
        assert!(!core.paused());
        let loss = core.note_collection_gap("lost").unwrap();
        assert_eq!(loss.reason, "lost");
        assert_eq!(loss.activity, "loss");
        let failure = core.note_collection_gap("source_failure").unwrap();
        assert_eq!(failure.activity, "failure");
        let markers = core.markers().unwrap();
        assert!(markers.iter().any(|marker| marker.activity == "pause" && marker.reason == "paused"));
        assert!(markers.iter().any(|marker| marker.activity == "loss" && marker.reason == "lost"));
        assert!(markers.iter().any(|marker| marker.activity == "history"));
        let store = core.open_hybrid().unwrap();
        let event = store
            .event(&mode_source(), paused.sequence)
            .unwrap()
            .unwrap();
        match event.record {
            Record::Gap { reason, .. } => assert_eq!(reason, GapReason::Paused),
            other => panic!("pause was not a gap: {other:?}"),
        }
        let loss_event = store.event(&mode_source(), loss.sequence).unwrap().unwrap();
        match loss_event.record {
            Record::Gap { reason, .. } => assert_eq!(reason, GapReason::Lost),
            other => panic!("loss was rewritten: {other:?}"),
        }
        drop(store);

        let mut tracker = crate::process::ProcessTracker::new();
        let snapshot = SystemSnapshot {
            device: "dev-1".into(),
            boot: "boot-1".into(),
            sampled_unix_ns: 1_000,
            precision_ns: Some(1),
            processes: vec![
                process_row(4, "keep", Some(watched.join("keep.txt").to_str().unwrap())),
                process_row(5, "secret", Some(outside.join("secret.txt").to_str().unwrap())),
            ],
            services: Vec::new(),
            processes_truncated: false,
            services_truncated: false,
            services_unavailable: true,
            evidence: "test",
        };
        let scope = core.onboarding().unwrap().scope;
        let events = tracker.observe_scoped(&snapshot, &scope).unwrap();
        let pids = events
            .iter()
            .filter_map(|event| match &event.record {
                Record::Observation {
                    process: Some(process),
                    ..
                } => Some(process.pid),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(pids.contains(&4), "{events:?}");
        assert!(!pids.contains(&5), "{events:?}");
        assert!(!format!("{events:?}").contains("secret"));
        let _ = temp;
    }

    fn process_row(pid: u32, name: &str, image: Option<&str>) -> ProcessRow {
        ProcessRow {
            pid,
            parent_pid: 1,
            start_unix_ns: Some(10),
            name: name.into(),
            image: image.map(str::to_owned),
            threads: vec![1],
            modules: Vec::new(),
            modules_truncated: false,
            threads_truncated: false,
            limited: false,
        }
    }

    fn mode_source() -> Source {
        Source {
            device: Id::new("local-modes").unwrap(),
            provider: Id::new("winstrata.modes").unwrap(),
            epoch: Id::new("modes").unwrap(),
        }
    }
}

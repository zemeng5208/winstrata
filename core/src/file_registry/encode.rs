//! 把已解码的文件和注册表变化写成协议事件。
//!
//! 通知接口没有给出变更时刻的精度，因此时间只记录用户态观察到缓冲的时钟，
//! 并省略 `precision_ns`。放不进标识符的路径、权限或键值不截断冒充完整记录。

use winstrata_kernel::CollectError;
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, GapReason, Id, ObjectRef, Record,
    SignedNanos, Source, Version,
};

use super::decode::{FileNotice, ValueChange, ValueSample};

pub const FILE_PROVIDER: &str = "windows.read_directory_changes";
pub const REGISTRY_PROVIDER: &str = "windows.reg_notify";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    File,
    Directory,
    Reparse,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedPath {
    pub full_path: Option<String>,
    pub kind: PathKind,
    pub owner: Option<String>,
    pub sddl: Option<String>,
    pub escaped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fields {
    pub event_type: String,
    pub subject_kind: Option<String>,
    pub subject_id: Option<String>,
    pub user: Option<String>,
    pub evidence: String,
    pub partial: Option<String>,
}

pub struct Stream {
    pub device: Id,
    pub provider: Id,
    pub epoch: Id,
}

pub fn describe_file(notice: &FileNotice, observed: Option<&ObservedPath>) -> Vec<Fields> {
    match notice {
        FileNotice::Added(name) | FileNotice::Modified(name) => {
            let action = if matches!(notice, FileNotice::Added(_)) {
                "added"
            } else {
                "modified"
            };
            vec![live_fields(name, action, observed)]
        }
        FileNotice::Removed(name) => vec![path_fields(
            "path.removed",
            name,
            observed.and_then(|item| item.full_path.as_deref()),
            None,
            Some("security_unavailable"),
        )],
        FileNotice::Renamed { from, to } => {
            let destination = live_fields(to, "renamed_to", observed);
            let mut source = path_fields(
                "path.renamed_from",
                from,
                None,
                None,
                Some("security_unavailable"),
            );
            if destination.partial.as_deref() == Some("path_escapes_root") {
                source.partial = Some("path_escapes_root".into());
            }
            vec![source, destination]
        }
        FileNotice::Unpaired { name, old } => {
            let event_type = if *old {
                "path.renamed_from"
            } else {
                "path.renamed_to"
            };
            vec![path_fields(
                event_type,
                name,
                None,
                None,
                Some("rename_unpaired"),
            )]
        }
        FileNotice::Undecodable { action } => vec![Fields {
            event_type: "path.unreadable".into(),
            subject_kind: None,
            subject_id: None,
            user: None,
            evidence: FILE_PROVIDER.into(),
            partial: Some(format!("name_not_unicode_{action}")),
        }],
    }
}

fn live_fields(name: &str, action: &str, observed: Option<&ObservedPath>) -> Fields {
    let Some(observed) = observed else {
        return path_fields(
            &format!("path.{action}"),
            name,
            None,
            None,
            Some("metadata_unavailable"),
        );
    };
    if observed.escaped {
        return path_fields(
            &format!("path.{action}"),
            name,
            None,
            None,
            Some("path_escapes_root"),
        );
    }
    let (event_type, partial) = match observed.kind {
        PathKind::File => (format!("file.{action}"), None),
        PathKind::Directory => (format!("directory.{action}"), None),
        PathKind::Reparse => (format!("path.{action}"), Some("reparse_unresolved")),
        PathKind::Missing => (format!("path.{action}"), Some("metadata_unavailable")),
    };
    let security = match (&observed.owner, &observed.sddl, partial) {
        (Some(owner), Some(sddl), None) => Some((owner.as_str(), sddl.as_str())),
        (Some(owner), None, None) => {
            return path_fields(
                &event_type,
                name,
                observed.full_path.as_deref(),
                Some(owner.as_str()),
                Some("permissions_unavailable"),
            );
        }
        _ => None,
    };
    if let Some((owner, sddl)) = security {
        if Id::new(sddl).is_ok() {
            return path_fields(
                &event_type,
                name,
                observed.full_path.as_deref(),
                Some(owner),
                None,
            )
            .with_evidence(sddl);
        }
        return path_fields(
            &event_type,
            name,
            observed.full_path.as_deref(),
            Some(owner),
            Some("permissions_exceed_identifier"),
        );
    }
    path_fields(
        &event_type,
        name,
        observed.full_path.as_deref(),
        None,
        partial.or(Some("security_unavailable")),
    )
}

fn path_fields(
    event_type: &str,
    relative: &str,
    full_path: Option<&str>,
    user: Option<&str>,
    partial: Option<&str>,
) -> Fields {
    let (subject_id, partial) = match full_path {
        Some(path) if Id::new(path).is_ok() => (Some(path.to_owned()), partial),
        Some(_) if Id::new(relative).is_ok() => (
            Some(relative.to_owned()),
            Some("absolute_path_exceeds_identifier"),
        ),
        None if Id::new(relative).is_ok() => (Some(relative.to_owned()), partial),
        _ => (None, Some("path_exceeds_identifier")),
    };
    let partial = match (partial, user) {
        (None, Some(owner)) if Id::new(owner).is_err() => Some("owner_unavailable"),
        (partial, _) => partial,
    };
    let user = user
        .filter(|owner| Id::new(*owner).is_ok())
        .map(str::to_owned);
    Fields {
        event_type: event_type.to_owned(),
        subject_kind: subject_id
            .as_ref()
            .map(|_| subject_kind(event_type).to_owned()),
        subject_id,
        user,
        evidence: FILE_PROVIDER.into(),
        partial: partial.map(str::to_owned),
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

impl Fields {
    fn with_evidence(mut self, evidence: &str) -> Self {
        if let Ok(evidence) = Id::new(evidence) {
            self.evidence = evidence.as_str().to_owned();
        } else if self.partial.is_none() {
            self.partial = Some("permissions_exceed_identifier".into());
        }
        self
    }
}

pub fn describe_registry(subkey: &str, change: &ValueChange) -> Fields {
    let (event_type, sample, before, after) = match change {
        ValueChange::Added(sample) => ("registry.value_added", sample, None, Some(sample)),
        ValueChange::Removed(sample) => ("registry.value_removed", sample, Some(sample), None),
        ValueChange::Changed { before, after } => {
            ("registry.value_changed", after, Some(before), Some(after))
        }
    };
    let subject = format!("HKCU\\{subkey}\\{}", sample.name);
    let subject_id = Id::new(&subject).ok().map(|id| id.as_str().to_owned());
    let detailed = registry_evidence(before, after);
    let (evidence, mut partial) = match detailed {
        Some(text) => (text, None),
        None => (
            REGISTRY_PROVIDER.to_owned(),
            Some("value_exceeds_identifier"),
        ),
    };
    if subject_id.is_none() {
        partial = Some("identifier_limit");
    }
    Fields {
        event_type: event_type.to_owned(),
        subject_kind: subject_id.as_ref().map(|_| "registry_value".to_owned()),
        subject_id,
        user: None,
        evidence,
        partial: partial.map(str::to_owned),
    }
}

pub fn describe_registry_incomplete() -> Fields {
    Fields {
        event_type: "registry.key_changed".into(),
        subject_kind: None,
        subject_id: None,
        user: None,
        evidence: REGISTRY_PROVIDER.into(),
        partial: Some("snapshot_incomplete".into()),
    }
}

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

fn hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(data.len() * 2);
    for byte in data {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
}

pub fn observation(
    stream: &Stream,
    sequence: u64,
    unix_ns: i64,
    fields: &Fields,
    user: Option<&Id>,
) -> Result<Event, CollectError> {
    let evidence =
        Id::new(&fields.evidence).map_err(|_| CollectError::fault("invalid evidence"))?;
    let partial = fields
        .partial
        .as_deref()
        .map(Id::new)
        .transpose()
        .map_err(|_| CollectError::fault("invalid partial reason"))?;
    let subject = match (&fields.subject_kind, &fields.subject_id) {
        (Some(kind), Some(id)) => Some(ObjectRef {
            kind: Id::new(kind).map_err(|_| CollectError::fault("invalid subject"))?,
            id: Id::new(id).map_err(|_| CollectError::fault("invalid subject"))?,
        }),
        (None, None) => None,
        _ => return Err(CollectError::fault("invalid subject")),
    };
    let record_user = match user {
        Some(id) => Some(id.clone()),
        None => fields
            .user
            .as_deref()
            .map(Id::new)
            .transpose()
            .map_err(|_| CollectError::fault("invalid user"))?,
    };
    Ok(Event {
        version: Version::CURRENT,
        source: Source {
            device: stream.device.clone(),
            provider: stream.provider.clone(),
            epoch: stream.epoch.clone(),
        },
        sequence: Counter::new(sequence).map_err(|_| CollectError::fault("invalid sequence"))?,
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new(&fields.event_type)
                .map_err(|_| CollectError::fault("invalid event type"))?,
            subject,
            user: record_user,
            process: None,
            attribution: Attribution::Observed { evidence },
            completeness: match partial {
                Some(reason) => Completeness::Partial { reason },
                None => Completeness::Complete,
            },
        },
    })
}

pub fn gap(
    stream: &Stream,
    sequence: u64,
    unix_ns: i64,
    reason: GapReason,
) -> Result<Event, CollectError> {
    Ok(Event {
        version: Version::CURRENT,
        source: Source {
            device: stream.device.clone(),
            provider: stream.provider.clone(),
            epoch: stream.epoch.clone(),
        },
        sequence: Counter::new(sequence).map_err(|_| CollectError::fault("invalid sequence"))?,
        time: EventTime {
            unix_ns: SignedNanos::new(unix_ns),
            precision_ns: None,
        },
        record: Record::Gap {
            reason,
            range: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_registry::decode::FileNotice;

    fn observed(kind: PathKind, owner: &str, sddl: &str) -> ObservedPath {
        ObservedPath {
            full_path: Some(r"C:\watch\a.txt".into()),
            kind,
            owner: Some(owner.into()),
            sddl: Some(sddl.into()),
            escaped: false,
        }
    }

    #[test]
    fn file_security_is_kept_only_when_it_fits() {
        let notice = FileNotice::Added("a.txt".into());
        let sddl = "O:S-1-5-21-1D:P(A;;FA;;;S-1-5-21-1)";
        let fields = describe_file(&notice, Some(&observed(PathKind::File, "S-1-5-21-1", sddl)));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].event_type, "file.added");
        assert_eq!(fields[0].subject_id.as_deref(), Some(r"C:\watch\a.txt"));
        assert_eq!(fields[0].user.as_deref(), Some("S-1-5-21-1"));
        assert_eq!(fields[0].evidence, sddl);
        assert_eq!(fields[0].partial, None);

        let long = "D:".to_owned() + &"A".repeat(300);
        let fields = describe_file(
            &notice,
            Some(&observed(PathKind::File, "S-1-5-21-1", &long)),
        );
        assert_eq!(fields[0].evidence, FILE_PROVIDER);
        assert_eq!(
            fields[0].partial.as_deref(),
            Some("permissions_exceed_identifier")
        );
        assert!(!fields[0].evidence.contains('A'));
    }

    #[test]
    fn removal_does_not_invent_security_or_file_content() {
        let fields = describe_file(&FileNotice::Removed("a.txt".into()), None);
        assert_eq!(fields[0].event_type, "path.removed");
        assert_eq!(fields[0].partial.as_deref(), Some("security_unavailable"));
        assert_eq!(fields[0].evidence, FILE_PROVIDER);
        assert_eq!(fields[0].user, None);
    }

    #[test]
    fn registry_values_keep_exact_before_and_after_bytes() {
        let added = ValueChange::Added(ValueSample {
            name: "Probe".into(),
            kind: 4,
            data: vec![0x11, 0, 0, 0],
        });
        let fields = describe_registry(r"Software\WinStrata", &added);
        assert_eq!(fields.event_type, "registry.value_added");
        assert_eq!(
            fields.subject_id.as_deref(),
            Some(r"HKCU\Software\WinStrata\Probe")
        );
        assert_eq!(fields.evidence, "t=4;a=11000000");
        assert_eq!(fields.partial, None);

        let changed = ValueChange::Changed {
            before: ValueSample {
                name: "Probe".into(),
                kind: 4,
                data: vec![0x11, 0, 0, 0],
            },
            after: ValueSample {
                name: "Probe".into(),
                kind: 4,
                data: vec![0x22, 0, 0, 0],
            },
        };
        let fields = describe_registry(r"Software\WinStrata", &changed);
        assert_eq!(fields.evidence, "t=4;b=11000000;a=22000000");

        let huge = ValueChange::Added(ValueSample {
            name: "Probe".into(),
            kind: 3,
            data: vec![0xab; 200],
        });
        let fields = describe_registry(r"Software\WinStrata", &huge);
        assert_eq!(fields.evidence, REGISTRY_PROVIDER);
        assert_eq!(fields.partial.as_deref(), Some("value_exceeds_identifier"));
        assert!(!fields.evidence.contains("ab"));
    }

    #[test]
    fn encoded_observation_omits_unknown_precision() {
        let fields = describe_file(&FileNotice::Removed("a.txt".into()), None);
        let event = observation(
            &Stream {
                device: Id::new("device-1").unwrap(),
                provider: Id::new(FILE_PROVIDER).unwrap(),
                epoch: Id::new("fs-1").unwrap(),
            },
            1,
            20,
            &fields[0],
            None,
        )
        .unwrap();
        let json = event.to_json().unwrap();
        assert!(!json.contains("precision_ns"));
        assert!(json.contains("\"sequence\":\"1\""));
        assert!(json.contains(FILE_PROVIDER));
    }
}

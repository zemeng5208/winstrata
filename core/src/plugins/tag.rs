//! 计划 029 的标记判定。
//!
//! 报文是否落盘只交给 `PacketLedger`。这里没有全量落盘，禁用标记也不会打开它。
//! 调用方带入的 actor、时间和 safety 会被丢掉。宿主重新填写。

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, Id, ObjectRef, Record, SignedNanos,
    Source, Version,
};

use super::PluginError;
use crate::network::{DiskPolicy, PacketLedger, PersistFailure, RiskTag};

pub(super) const PLUGIN: &str = "strata.tag";

pub(super) const MANIFEST: &str = concat!(
    r#"{"version":{"major":1,"minor":0},"id":"strata.tag","name":"StrataTag","#,
    r#""plugin_version":{"major":1,"minor":0,"patch":0},"kind":"builtin","runtime":"native","#,
    r#""capabilities":["tag.apply","tag.query","tag.policy"],"permissions":["event.publish","#,
    r#""event.subscribe","storage.private","history.read"],"dependencies":[],"events":["#,
    r#""tag.applied","tag.rejected","tag.late"],"rescue":false}"#,
);

pub(super) fn beside(runtime: &Path) -> Option<PathBuf> {
    super::collector_beside(runtime, "winstrata-strata-tag")
}

pub(super) struct Prepared {
    pub(super) request: String,
    pub(super) path: Option<PathBuf>,
    pub(super) policy: Option<(DiskPolicy, usize)>,
}

pub(super) fn actor_for(caller: &str, user: &str) -> (&'static str, String) {
    match caller {
        PLUGIN => ("user", user.to_owned()),
        "strata.mcp" => ("agent", caller.to_owned()),
        _ => ("plugin", caller.to_owned()),
    }
}

pub(super) fn prepare_apply(
    actor: &str,
    actor_id: &str,
    payload: &str,
    paused: bool,
    now: i64,
    ledger: &mut PacketLedger,
    capture: Option<PathBuf>,
) -> Result<Prepared, PluginError> {
    let value = object(payload)?;
    let kind = text(&value, "kind");
    if !matches!(
        kind,
        "event" | "packet" | "file" | "process" | "software" | "registry" | "system"
    ) {
        return Err(PluginError::new(
            "invalid_input",
            "tag subject kind is not recognized",
        ));
    }
    let subject = text(&value, "subject");
    if !field(subject, 256) {
        return Err(PluginError::new(
            "invalid_input",
            "tag subject is not a token",
        ));
    }
    let reason = text(&value, "reason");
    let source = text(&value, "source");
    let confidence = text(&value, "confidence");
    let evidence = text(&value, "evidence");
    let attribution = text(&value, "attribution");
    if !matches!(attribution, "observed" | "inferred" | "unknown") {
        return Err(PluginError::new(
            "invalid_input",
            "tag attribution is not recognized",
        ));
    }
    if !evidence.is_empty() && !field(evidence, 256) {
        return Err(PluginError::new(
            "invalid_input",
            "tag evidence is not a token",
        ));
    }
    if !field(actor_id, 256) || !matches!(actor, "user" | "script" | "plugin" | "agent") {
        return Err(PluginError::new(
            "invalid_input",
            "tag actor is not a token",
        ));
    }
    let packet = value.get("packet").and_then(Value::as_u64).unwrap_or(0);
    if paused {
        return record(
            actor,
            actor_id,
            kind,
            subject,
            reason,
            source,
            confidence,
            evidence,
            attribution,
            now,
            packet,
            "paused",
            false,
            None,
            None,
        );
    }
    if kind == "packet" {
        if packet == 0 {
            return Err(PluginError::new("invalid_input", "packet id is missing"));
        }
        let path =
            capture.ok_or_else(|| PluginError::new("invalid_input", "packet id is missing"))?;
        let tag = if reason.is_empty() && source.is_empty() && confidence.is_empty() {
            None
        } else {
            Some(RiskTag {
                reason: reason.to_owned(),
                source: source.to_owned(),
                confidence: confidence.to_owned(),
                time_unix_ns: now,
            })
        };
        let (code, wrote) = match ledger.persist(packet, tag.as_ref(), &path) {
            Ok(()) => ("applied", true),
            Err(PersistFailure::MemoryOnly) => ("memory_only", false),
            Err(PersistFailure::Untagged) => ("untagged", false),
            Err(PersistFailure::TagRejected) => ("tag_rejected", false),
            Err(PersistFailure::UnknownPacket) => ("unknown_packet", false),
            Err(PersistFailure::Unavailable) => ("unavailable", false),
            Err(PersistFailure::Io(error)) => return Err(error.into()),
        };
        return record(
            actor,
            actor_id,
            kind,
            subject,
            reason,
            source,
            confidence,
            evidence,
            attribution,
            now,
            packet,
            code,
            wrote,
            if wrote { Some(path) } else { None },
            None,
        );
    }
    let code = if reason.is_empty() && source.is_empty() && confidence.is_empty() {
        "untagged"
    } else if token(reason) && token(source) && token(confidence) {
        "applied"
    } else {
        "tag_rejected"
    };
    record(
        actor,
        actor_id,
        kind,
        subject,
        reason,
        source,
        confidence,
        evidence,
        attribution,
        now,
        0,
        code,
        false,
        None,
        None,
    )
}

pub(super) fn prepare_policy(
    actor: &str,
    actor_id: &str,
    payload: &str,
    now: i64,
    current_cap: usize,
) -> Result<Prepared, PluginError> {
    let value = object(payload)?;
    let mode = text(&value, "mode");
    let policy = match mode {
        "memory_only" => DiskPolicy::MemoryOnly,
        "tagged_only" => DiskPolicy::TaggedOnly,
        "all" | "persist" | "persist_all" => {
            return Err(PluginError::new(
                "policy_rejected",
                "tag policy cannot persist every packet",
            ));
        }
        _ => {
            return Err(PluginError::new(
                "invalid_input",
                "tag policy mode is not recognized",
            ));
        }
    };
    let cap = match value.get("cap").and_then(Value::as_u64) {
        None => current_cap,
        Some(value) if (1..=262_144).contains(&value) => value as usize,
        Some(_) => {
            return Err(PluginError::new(
                "invalid_input",
                "tag memory cap is outside 1 to 262144",
            ));
        }
    };
    record(
        actor,
        actor_id,
        "policy",
        mode,
        mode,
        actor,
        "stated",
        &format!("cap={cap}"),
        "observed",
        now,
        0,
        "policy",
        false,
        None,
        Some((policy, cap)),
    )
}

pub(super) fn event_type(code: &str) -> Option<&'static str> {
    match code {
        "applied" => Some("tag.applied"),
        "unavailable" => Some("tag.late"),
        "policy" => None,
        "untagged" | "memory_only" | "tag_rejected" | "unknown_packet" | "paused" => {
            Some("tag.rejected")
        }
        _ => None,
    }
}

pub(super) fn observation(
    epoch: &str,
    sequence: u64,
    event_type: &str,
    record: &Value,
) -> Result<Event, PluginError> {
    let kind = text(record, "kind");
    let subject = text(record, "subject");
    let evidence = text(record, "evidence");
    let attribution_name = text(record, "attribution");
    let actor = text(record, "actor");
    let actor_id = text(record, "actor_id");
    let evidence_id = if evidence.is_empty() {
        Id::new("tag").map_err(invalid)?
    } else {
        Id::new(evidence).map_err(invalid)?
    };
    let attribution = match attribution_name {
        "observed" => Attribution::Observed {
            evidence: evidence_id,
        },
        "inferred" => Attribution::Inferred {
            evidence: evidence_id,
        },
        _ => Attribution::Unknown,
    };
    let user = if actor == "user" {
        Some(Id::new(actor_id).map_err(invalid)?)
    } else {
        None
    };
    Ok(Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new("local-plugin-host").map_err(invalid)?,
            provider: Id::new(PLUGIN).map_err(invalid)?,
            epoch: Id::new(epoch).map_err(invalid)?,
        },
        sequence: Counter::new(sequence).map_err(invalid)?,
        time: EventTime {
            unix_ns: SignedNanos::new(super::unix_ns()),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new(event_type).map_err(invalid)?,
            subject: Some(ObjectRef {
                kind: Id::new(kind).map_err(invalid)?,
                id: Id::new(subject).map_err(invalid)?,
            }),
            user,
            process: None,
            attribution,
            completeness: Completeness::Complete,
        },
    })
}

fn record(
    actor: &str,
    actor_id: &str,
    kind: &str,
    subject: &str,
    reason: &str,
    source: &str,
    confidence: &str,
    evidence: &str,
    attribution: &str,
    now: i64,
    packet: u64,
    code: &str,
    wrote: bool,
    path: Option<PathBuf>,
    policy: Option<(DiskPolicy, usize)>,
) -> Result<Prepared, PluginError> {
    let request = json!({
        "ok": true,
        "code": code,
        "actor": actor,
        "actor_id": actor_id,
        "kind": kind,
        "subject": subject,
        "reason": reason,
        "source": source,
        "confidence": confidence,
        "evidence": evidence,
        "attribution": attribution,
        "time_unix_ns": now,
        "safety": "none",
        "wrote": wrote,
        "packet": packet,
    })
    .to_string();
    if request.len() > 1024 || request.chars().any(char::is_control) {
        return Err(PluginError::new(
            "invalid_record",
            "tag record does not fit a plugin frame",
        ));
    }
    Ok(Prepared {
        request,
        path,
        policy,
    })
}

fn object(payload: &str) -> Result<Value, PluginError> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|_| PluginError::new("invalid_input", "tag payload is not an object"))?;
    if !value.is_object() {
        return Err(PluginError::new(
            "invalid_input",
            "tag payload is not an object",
        ));
    }
    Ok(value)
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn token(value: &str) -> bool {
    field(value, 128)
}

fn field(value: &str, max: usize) -> bool {
    (1..=max).contains(&value.len())
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn invalid(error: winstrata_protocols::ProtocolError) -> PluginError {
    PluginError::new("invalid_record", error.to_string())
}

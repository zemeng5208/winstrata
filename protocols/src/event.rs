use crate::{Counter, Id, ProtocolError, SignedNanos, Version, object};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Deserializer, Serialize};

/// 事件键由设备、来源、来源实例和序号组成。时间戳允许因校时而回退。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub version: Version,
    pub source: Source,
    pub sequence: Counter,
    pub time: EventTime,
    pub record: Record,
}

/// 对外的 Serde 入口在读写时均验证事件；原始字段转换仅供协议内部使用。
#[derive(Serialize, Deserialize)]
#[serde(remote = "Event")]
pub(crate) struct EventWire {
    #[serde(deserialize_with = "object::required")]
    version: Version,
    #[serde(deserialize_with = "object::required")]
    source: Source,
    sequence: Counter,
    #[serde(deserialize_with = "object::required")]
    time: EventTime,
    #[serde(deserialize_with = "object::tagged")]
    record: Record,
}

pub(crate) fn read<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Event, D::Error> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct UncheckedEvent(#[serde(with = "EventWire")] Event);

    let UncheckedEvent(event) = object::required(deserializer)?;
    Ok(event)
}

impl Event {
    /// 可直接作为来源序列的去重键，无需复制标识符。
    pub fn key(&self) -> (&Source, Counter) {
        (&self.source, self.sequence)
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.version.check()?;
        if let Record::Gap {
            range: Some(range), ..
        } = &self.record
            && (range.first.get() > range.last.get() || range.last.get() >= self.sequence.get())
        {
            return Err(ProtocolError::invalid(
                "record.range must satisfy first <= last < sequence",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Source {
    pub device: Id,
    pub provider: Id,
    pub epoch: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventTime {
    pub unix_ns: SignedNanos,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision_ns: Option<Counter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Record {
    Observation {
        event_type: Id,
        #[serde(
            default,
            deserialize_with = "object::optional",
            skip_serializing_if = "Option::is_none"
        )]
        subject: Option<ObjectRef>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user: Option<Id>,
        #[serde(
            default,
            deserialize_with = "object::optional",
            skip_serializing_if = "Option::is_none"
        )]
        process: Option<ProcessRef>,
        #[serde(deserialize_with = "object::tagged")]
        attribution: Attribution,
        #[serde(deserialize_with = "object::tagged")]
        completeness: Completeness,
    },
    Gap {
        #[serde(deserialize_with = "read_gap_reason")]
        reason: GapReason,
        #[serde(
            default,
            deserialize_with = "object::optional",
            skip_serializing_if = "Option::is_none"
        )]
        range: Option<SequenceRange>,
    },
}

/// 进程标识依赖来源设备、启动实例、进程号和进程启动时间。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessRef {
    pub boot: Id,
    pub pid: u32,
    pub started_unix_ns: SignedNanos,
}

/// 对象类型和标识在事件的来源设备内解析。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectRef {
    pub kind: Id,
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Attribution {
    Observed { evidence: Id },
    Inferred { evidence: Id },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    Partial { reason: Id },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapReason {
    Filtered,
    Sampled,
    Paused,
    SourceFailure,
    Lost,
}

fn read_gap_reason<'de, D: Deserializer<'de>>(deserializer: D) -> Result<GapReason, D::Error> {
    GapReason::deserialize(String::deserialize(deserializer)?.into_deserializer())
}

/// 闭区间，两端均属于同一来源实例。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceRange {
    pub first: Counter,
    pub last: Counter,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, from_value, json, to_value};

    fn gap(reason: &str) -> Value {
        json!({
            "version": { "major": 1, "minor": 0 },
            "source": { "device": "test-device", "provider": "test-provider", "epoch": "run-1" },
            "sequence": "9",
            "time": { "unix_ns": "-730" },
            "record": { "kind": "gap", "reason": reason }
        })
    }

    #[test]
    fn each_gap_reason_preserves_an_unknown_range_and_precision() {
        for reason in ["filtered", "sampled", "paused", "source_failure", "lost"] {
            let wire = gap(reason);
            let event = from_value::<Event>(wire.clone()).unwrap();
            assert_eq!(to_value(event).unwrap(), wire);
        }
    }

    #[test]
    fn a_gap_can_report_an_earlier_range_or_a_single_sequence() {
        for (first, last) in [("1", "5"), ("8", "8")] {
            let mut wire = gap("lost");
            wire["record"]["range"] = json!({ "first": first, "last": last });
            let event = from_value::<Event>(wire.clone()).unwrap();
            assert_eq!(to_value(event).unwrap(), wire);
        }
    }

    #[test]
    fn gap_bounds_must_be_ordered_and_precede_the_reporting_sequence() {
        for (first, last) in [("7", "6"), ("1", "9"), ("9", "10")] {
            let mut wire = gap("lost");
            wire["record"]["range"] = json!({ "first": first, "last": last });
            assert!(from_value::<Event>(wire).is_err());
        }
    }

    #[test]
    fn nested_objects_require_field_names_and_allow_unknown_optional_arrays() {
        let mut observation = gap("lost");
        observation["record"] = json!({
            "kind": "observation", "event_type": "test.event",
            "subject": {"kind":"file", "id":"sample"},
            "process": {"boot":"boot-1", "pid":7, "started_unix_ns":"100"},
            "attribution": {"kind":"observed", "evidence":"test.source"},
            "completeness": {"kind":"complete", "future_values": [1, {"field": []}]}
        });
        assert!(Event::from_json(&observation.to_string()).is_ok());
        for (path, array) in [
            ("/version", json!([1, 0])),
            ("/source", json!(["test-device", "test-provider", "run-1"])),
            ("/time", json!(["-730", "1"])),
            ("/record", json!(["gap", "lost", null])),
            ("/record/subject", json!(["file", "sample"])),
            ("/record/process", json!(["boot-1", 7, "100"])),
            ("/record/attribution", json!(["observed", "test.source"])),
            ("/record/completeness", json!(["complete"])),
        ] {
            let mut malformed = observation.clone();
            *malformed.pointer_mut(path).unwrap() = array;
            assert!(Event::from_json(&malformed.to_string()).is_err(), "{path}");
            assert!(from_value::<Event>(malformed).is_err(), "{path}");
        }
        let mut malformed = gap("lost");
        malformed["record"]["range"] = json!(["2", "5"]);
        assert!(Event::from_json(&malformed.to_string()).is_err());
    }

    #[test]
    fn duplicate_named_fields_are_rejected_before_their_values_are_lost() {
        let duplicate = gap("lost").to_string().replace(
            "\"provider\":\"test-provider\"",
            "\"provider\":\"test-provider\",\"provider\":\"second-provider\"",
        );
        assert!(Event::from_json(&duplicate).is_err());
        let duplicate = gap("lost").to_string().replace(
            "\"kind\":\"gap\"",
            "\"kind\":\"gap\",\"kind\":\"observation\"",
        );
        assert!(Event::from_json(&duplicate).is_err());
    }

    #[test]
    fn variant_names_are_strings_at_every_depth() {
        let mut wire = gap("lost");
        wire["record"]["reason"] = json!({"lost": null});
        assert!(Event::from_json(&wire.to_string()).is_err(), "reason");
        wire["record"] = json!({
            "kind":"observation", "event_type":"test.event",
            "attribution":{"kind":2}, "completeness":{"kind":"complete"}
        });
        assert!(
            Event::from_json(&wire.to_string()).is_err(),
            "attribution.kind"
        );
        wire["record"]["attribution"]["kind"] = json!("unknown");
        wire["record"]["completeness"]["kind"] = json!(0);
        assert!(
            Event::from_json(&wire.to_string()).is_err(),
            "completeness.kind"
        );
    }
}

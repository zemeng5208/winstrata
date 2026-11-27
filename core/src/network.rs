#![forbid(unsafe_code)]

//! 网络观测的内存留存。
//!
//! 默认策略不写磁盘。显式切换到按标记留存后，也只写入被点名的那一个报文，
//! 且标记必须同时带有理由、来源和可信状态。未标记报文不会先写后删。

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::Path;

use winstrata_protocols::{
    Attribution, Completeness, Counter, Event, EventTime, Id, ObjectRef, ProtocolError, Record,
    SignedNanos, Source, Version,
};

pub const DEFAULT_PACKET_CAP: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskPolicy {
    MemoryOnly,
    TaggedOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketClass {
    Plain,
    Encrypted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiskTag {
    pub reason: String,
    pub source: String,
    pub confidence: String,
    pub time_unix_ns: i64,
}

#[derive(Debug)]
pub enum PersistFailure {
    MemoryOnly,
    Untagged,
    TagRejected,
    UnknownPacket,
    Unavailable,
    Io(io::Error),
}

impl PersistFailure {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MemoryOnly => "memory_only",
            Self::Untagged => "untagged",
            Self::TagRejected => "tag_rejected",
            Self::UnknownPacket => "unknown_packet",
            Self::Unavailable => "unavailable",
            Self::Io(_) => "io",
        }
    }
}

impl PartialEq for PersistFailure {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Io(left), Self::Io(right)) => left.kind() == right.kind(),
            (Self::MemoryOnly, Self::MemoryOnly)
            | (Self::Untagged, Self::Untagged)
            | (Self::TagRejected, Self::TagRejected)
            | (Self::UnknownPacket, Self::UnknownPacket)
            | (Self::Unavailable, Self::Unavailable) => true,
            _ => false,
        }
    }
}

impl Eq for PersistFailure {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowFact {
    pub transport: &'static str,
    pub local: SocketAddr,
    pub remote: SocketAddr,
    pub direction: &'static str,
    pub basis: &'static str,
    pub pid: Option<u32>,
    pub payload: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceFact {
    pub name: String,
    pub kind: String,
    pub admin_up: bool,
    pub operational: Option<bool>,
    pub address_count: usize,
}

struct Staged {
    id: u64,
    class: PacketClass,
    bytes: Vec<u8>,
    original_len: usize,
    released: bool,
}

pub struct PacketLedger {
    policy: DiskPolicy,
    cap_bytes: usize,
    next_id: u64,
    packets: VecDeque<Staged>,
    discarded: u64,
}

impl PacketLedger {
    pub fn new(policy: DiskPolicy, cap_bytes: usize) -> Self {
        Self {
            policy,
            cap_bytes,
            next_id: 1,
            packets: VecDeque::new(),
            discarded: 0,
        }
    }

    pub fn memory_only() -> Self {
        Self::new(DiskPolicy::MemoryOnly, DEFAULT_PACKET_CAP)
    }

    pub fn discarded(&self) -> u64 {
        self.discarded
    }

    pub fn retained_bytes(&self) -> usize {
        self.packets.iter().map(|packet| packet.bytes.len()).sum()
    }

    pub fn stage(&mut self, class: PacketClass, bytes: Vec<u8>) -> u64 {
        let original_len = bytes.len();
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let keep = self.reserve(original_len);
        let stored = if keep { bytes } else { Vec::new() };
        let released = original_len > 0 && stored.is_empty();
        if released {
            self.discarded += 1;
        }
        self.packets.push_back(Staged {
            id,
            class,
            bytes: stored,
            original_len,
            released,
        });
        id
    }

    pub fn packet_class(&self, id: u64) -> Option<PacketClass> {
        self.packets
            .iter()
            .find(|packet| packet.id == id)
            .map(|packet| packet.class)
    }

    fn reserve(&mut self, incoming: usize) -> bool {
        if incoming == 0 || incoming > self.cap_bytes {
            return false;
        }
        while self.retained_bytes() + incoming > self.cap_bytes {
            let Some(index) = self
                .packets
                .iter()
                .position(|packet| !packet.bytes.is_empty())
            else {
                return false;
            };
            self.packets[index].bytes.clear();
            self.packets[index].released = true;
            self.discarded += 1;
        }
        true
    }

    /// 只有显式的按标记策略会创建目标文件。失败的已授权写入会删掉自己的半截文件。
    pub fn persist(
        &self,
        id: u64,
        tag: Option<&RiskTag>,
        path: &Path,
    ) -> Result<(), PersistFailure> {
        let Some(packet) = self.packets.iter().find(|packet| packet.id == id) else {
            return Err(PersistFailure::UnknownPacket);
        };
        if self.policy == DiskPolicy::MemoryOnly {
            return Err(PersistFailure::MemoryOnly);
        }
        let Some(tag) = tag else {
            return Err(PersistFailure::Untagged);
        };
        if !qualifies(tag) {
            return Err(PersistFailure::TagRejected);
        }
        if packet.released || packet.bytes.is_empty() || packet.bytes.len() != packet.original_len {
            return Err(PersistFailure::Unavailable);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(PersistFailure::Io)?;
        if let Err(error) = file.write_all(&packet.bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(path);
            return Err(PersistFailure::Io(error));
        }
        Ok(())
    }
}

fn qualifies(tag: &RiskTag) -> bool {
    token(&tag.reason) && token(&tag.source) && token(&tag.confidence)
}

fn token(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

pub fn flow_event(
    device: &str,
    epoch: &str,
    sequence: u64,
    time_unix_ns: i64,
    fact: &FlowFact,
) -> Result<Event, ProtocolError> {
    let evidence = format!(
        "{}:{}-{}:{}:{}:pid={}:{}",
        fact.transport,
        fact.local,
        fact.remote,
        fact.direction,
        fact.basis,
        fact.pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "absent".to_owned()),
        fact.payload
    );
    let reason = if fact.pid.is_none() {
        "process_unavailable"
    } else {
        fact.payload
    };
    let observed = fact.basis == "observed";
    let inferred = fact.basis == "inferred" || fact.pid.is_some();
    observation(
        device,
        epoch,
        sequence,
        time_unix_ns,
        "net.flow",
        "flow",
        &evidence,
        &evidence,
        observed,
        inferred,
        reason,
    )
}

pub fn interface_event(
    device: &str,
    epoch: &str,
    sequence: u64,
    time_unix_ns: i64,
    fact: &InterfaceFact,
) -> Result<Event, ProtocolError> {
    let evidence = format!(
        "name={}:kind={}:admin_up={}:operational={}:addresses={}",
        fact.name,
        fact.kind,
        fact.admin_up as u8,
        fact.operational
            .map(|value| if value { "up" } else { "down" })
            .unwrap_or("unknown"),
        fact.address_count
    );
    let reason = if fact.operational.is_none() {
        "operstate_unknown"
    } else {
        "complete"
    };
    let partial = fact.operational.is_none();
    if partial {
        observation(
            device,
            epoch,
            sequence,
            time_unix_ns,
            "net.interface",
            "interface",
            &fact.name,
            &evidence,
            true,
            false,
            reason,
        )
    } else {
        observation_complete(
            device,
            epoch,
            sequence,
            time_unix_ns,
            "net.interface",
            "interface",
            &fact.name,
            &evidence,
        )
    }
}

fn observation(
    device: &str,
    epoch: &str,
    sequence: u64,
    time_unix_ns: i64,
    event_type: &str,
    subject_kind: &str,
    subject_id: &str,
    evidence: &str,
    observed: bool,
    inferred: bool,
    partial_reason: &str,
) -> Result<Event, ProtocolError> {
    let attribution = if observed {
        Attribution::Observed {
            evidence: Id::new(evidence)?,
        }
    } else if inferred {
        Attribution::Inferred {
            evidence: Id::new(evidence)?,
        }
    } else {
        Attribution::Unknown
    };
    record(
        device,
        epoch,
        sequence,
        time_unix_ns,
        event_type,
        subject_kind,
        subject_id,
        attribution,
        Completeness::Partial {
            reason: Id::new(partial_reason)?,
        },
    )
}

fn observation_complete(
    device: &str,
    epoch: &str,
    sequence: u64,
    time_unix_ns: i64,
    event_type: &str,
    subject_kind: &str,
    subject_id: &str,
    evidence: &str,
) -> Result<Event, ProtocolError> {
    record(
        device,
        epoch,
        sequence,
        time_unix_ns,
        event_type,
        subject_kind,
        subject_id,
        Attribution::Observed {
            evidence: Id::new(evidence)?,
        },
        Completeness::Complete,
    )
}

fn record(
    device: &str,
    epoch: &str,
    sequence: u64,
    time_unix_ns: i64,
    event_type: &str,
    subject_kind: &str,
    subject_id: &str,
    attribution: Attribution,
    completeness: Completeness,
) -> Result<Event, ProtocolError> {
    Ok(Event {
        version: Version::CURRENT,
        source: Source {
            device: Id::new(device)?,
            provider: Id::new("winstrata.net")?,
            epoch: Id::new(epoch)?,
        },
        sequence: Counter::new(sequence)?,
        time: EventTime {
            unix_ns: SignedNanos::new(time_unix_ns),
            precision_ns: None,
        },
        record: Record::Observation {
            event_type: Id::new(event_type)?,
            subject: Some(ObjectRef {
                kind: Id::new(subject_kind)?,
                id: Id::new(subject_id)?,
            }),
            user: None,
            process: None,
            attribution,
            completeness,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, TcpListener};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use winstrata_protocols::{Completeness, Event, Record};

    use crate::{EventBus, Filter, Limits};

    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "winstrata-net-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }

        fn empty(&self) -> bool {
            fs::read_dir(&self.0).unwrap().next().is_none()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tag() -> RiskTag {
        RiskTag {
            reason: "test-risk".into(),
            source: "unit".into(),
            confidence: "high".into(),
            time_unix_ns: 1,
        }
    }

    #[test]
    fn default_policy_writes_nothing_and_a_tag_keeps_only_the_selected_packet() {
        let scratch = Scratch::new();
        let payload = vec![0x16u8, 0x03, 0x01, 0x00, 0x01, 0x00];
        let other = b"plain-context".to_vec();
        let mut memory = PacketLedger::memory_only();
        let id = memory.stage(PacketClass::Encrypted, payload.clone());
        let destination = scratch.path("ignored.bin");
        assert_eq!(
            memory.persist(id, Some(&tag()), &destination).unwrap_err(),
            PersistFailure::MemoryOnly
        );
        assert!(scratch.empty());

        let mut ledger = PacketLedger::new(DiskPolicy::TaggedOnly, DEFAULT_PACKET_CAP);
        let selected = ledger.stage(PacketClass::Encrypted, payload.clone());
        let rest = ledger.stage(PacketClass::Plain, other.clone());
        assert_eq!(ledger.packet_class(selected), Some(PacketClass::Encrypted));
        assert_eq!(ledger.packet_class(rest), Some(PacketClass::Plain));
        assert_eq!(
            ledger.persist(selected, None, &destination).unwrap_err(),
            PersistFailure::Untagged
        );
        assert!(scratch.empty());
        assert_eq!(
            ledger
                .persist(
                    selected,
                    Some(&RiskTag {
                        reason: " ".into(),
                        source: "unit".into(),
                        confidence: "high".into(),
                        time_unix_ns: 1,
                    }),
                    &destination
                )
                .unwrap_err(),
            PersistFailure::TagRejected
        );
        assert!(scratch.empty());
        ledger
            .persist(selected, Some(&tag()), &destination)
            .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), payload);
        let second = scratch.path("second.bin");
        assert_eq!(
            ledger.persist(rest, None, &second).unwrap_err(),
            PersistFailure::Untagged
        );
        assert!(!second.exists());
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
        let combined = fs::read(&destination).unwrap();
        assert!(!combined.windows(other.len()).any(|window| window == other));
    }

    #[test]
    fn a_released_packet_cannot_be_written_after_the_cap_drops_it() {
        let scratch = Scratch::new();
        let mut ledger = PacketLedger::new(DiskPolicy::TaggedOnly, 10);
        let first = ledger.stage(PacketClass::Plain, vec![1; 8]);
        let second = ledger.stage(PacketClass::Plain, vec![2; 8]);
        assert!(ledger.discarded() >= 1);
        assert_eq!(
            ledger
                .persist(first, Some(&tag()), &scratch.path("late.bin"))
                .unwrap_err(),
            PersistFailure::Unavailable
        );
        assert!(scratch.empty());
        ledger
            .persist(second, Some(&tag()), &scratch.path("kept.bin"))
            .unwrap();
        assert_eq!(fs::read(scratch.path("kept.bin")).unwrap(), vec![2; 8]);
    }

    #[test]
    fn live_exchange_projects_real_flows_and_does_not_spill_packets() {
        use winstrata_kernel::{Basis, Direction, InterfaceKind, Observer, PacketBody};

        let scratch = Scratch::new();
        let observer = Observer::open();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let server = listener.local_addr().unwrap();
        let mut client = std::net::TcpStream::connect(server).unwrap();
        let (mut accepted, _) = listener.accept().unwrap();
        let client_addr = client.local_addr().unwrap();
        client.set_nodelay(true).unwrap();
        accepted.set_nodelay(true).unwrap();
        let payload = [0x16u8, 0x03, 0x01, 0x00, 0x05, 0x01, 0x00, 0x00, 0x01, 0x00];
        client.write_all(&payload).unwrap();
        let mut received = [0u8; 16];
        let size = accepted.read(&mut received).unwrap();
        assert_eq!(&received[..size], &payload);
        let snapshot = observer.collect(Duration::from_millis(400), 64).unwrap();
        assert!(
            snapshot
                .interfaces
                .iter()
                .any(|interface| interface.kind == InterfaceKind::Loopback)
        );
        assert!(
            snapshot
                .interfaces
                .iter()
                .any(|interface| interface.kind != InterfaceKind::Loopback)
        );
        let outbound = snapshot
            .endpoints
            .iter()
            .find(|endpoint| {
                endpoint.local == client_addr
                    && endpoint.remote == server
                    && endpoint.direction == Direction::Outbound(Basis::Inferred)
            })
            .expect("outbound flow");
        let fact = FlowFact {
            transport: outbound.transport.as_str(),
            local: outbound.local,
            remote: outbound.remote,
            direction: outbound.direction.as_str(),
            basis: outbound.direction.basis(),
            pid: outbound.pid,
            payload: outbound.payload,
        };
        let event = flow_event("local-test", "run-1", 1, 10, &fact).unwrap();
        let encoded = event.to_json().unwrap();
        let decoded = Event::from_json(&encoded).unwrap();
        assert_eq!(decoded, event);
        match &decoded.record {
            Record::Observation {
                event_type,
                attribution,
                completeness,
                ..
            } => {
                assert_eq!(event_type.as_str(), "net.flow");
                assert!(matches!(
                    attribution,
                    winstrata_protocols::Attribution::Inferred { .. }
                ));
                assert!(matches!(
                    completeness,
                    Completeness::Partial { reason } if reason.as_str() == "table_has_no_payload"
                ));
            }
            Record::Gap { .. } => panic!("flow became a gap"),
        }
        let bus = EventBus::new(Limits::standard()).unwrap();
        let subscription = bus.subscribe(1, Filter::default()).unwrap();
        bus.publish(event).unwrap();
        let batch = subscription
            .recv_batch(8, Duration::from_millis(200))
            .unwrap();
        assert_eq!(batch.len(), 1);
        let loopback = snapshot
            .interfaces
            .iter()
            .find(|interface| interface.kind == InterfaceKind::Loopback)
            .unwrap();
        let interface = interface_event(
            "local-test",
            "run-1",
            2,
            11,
            &InterfaceFact {
                name: loopback.name.clone(),
                kind: loopback.kind.label(),
                admin_up: loopback.admin_up,
                operational: loopback.operational,
                address_count: loopback.addresses.len(),
            },
        )
        .unwrap();
        let decoded = Event::from_json(&interface.to_json().unwrap()).unwrap();
        assert_eq!(decoded, interface);
        match &decoded.record {
            Record::Observation {
                event_type,
                completeness,
                ..
            } => {
                assert_eq!(event_type.as_str(), "net.interface");
                match (loopback.operational, completeness) {
                    (None, Completeness::Partial { reason }) => {
                        assert_eq!(reason.as_str(), "operstate_unknown")
                    }
                    (Some(_), Completeness::Complete) => {}
                    _ => panic!("interface completeness does not match the observed operstate"),
                }
            }
            Record::Gap { .. } => panic!("interface became a gap"),
        }

        let saw_tls = snapshot.packets.iter().any(|packet| {
            matches!(
                &packet.body,
                PacketBody::Encrypted { evidence, .. } if *evidence == "tls-record-header"
            ) && packet.source == client_addr
                && packet.destination == server
                && !packet.body.truncated()
        });
        if !saw_tls {
            assert!(snapshot.gaps.iter().any(|gap| {
                (gap.reason == "capture_inaccessible" && gap.os_code.is_some())
                    || gap.reason == "capture_no_payload"
            }));
        }
        let mut memory = PacketLedger::memory_only();
        let bytes = snapshot
            .packets
            .iter()
            .find(|packet| matches!(packet.body, PacketBody::Encrypted { .. }))
            .map(|packet| packet.body.frame().to_vec())
            .unwrap_or_else(|| payload.to_vec());
        let id = memory.stage(PacketClass::Encrypted, bytes);
        assert_eq!(
            memory
                .persist(id, Some(&tag()), &scratch.path("capture.bin"))
                .unwrap_err(),
            PersistFailure::MemoryOnly
        );
        assert!(scratch.empty());
        let _ = (client, accepted, listener);
    }
}

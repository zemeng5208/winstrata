//! 用户态网络接口、传输端点和有界报文采样。
//!
//! 这里不加载 `strata-net`，也不把报文写入磁盘。连接表没有载荷；
//! 只有采样套接字实际读到的帧才会进入内存中的 `packets`。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

pub use parse::MAX_FRAME_BYTES;

pub const MAX_ENDPOINTS: usize = 8_192;
pub const MAX_CAPTURE_PACKETS: usize = 64;
pub const MAX_CAPTURE_WAIT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Observed,
    Inferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceKind {
    Loopback,
    Ethernet,
    Bridge,
    Tunnel,
    Wireless,
    Virtual,
    Other(u32),
}

impl InterfaceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Loopback => "loopback",
            Self::Ethernet => "ethernet",
            Self::Bridge => "bridge",
            Self::Tunnel => "tunnel",
            Self::Wireless => "wireless",
            Self::Virtual => "virtual",
            Self::Other(_) => "other",
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Other(value) => format!("other:{value}"),
            kind => kind.as_str().to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub kind: InterfaceKind,
    pub admin_up: bool,
    pub operational: Option<bool>,
    pub addresses: Vec<IpAddr>,
    pub mac: Option<[u8; 6]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Tcp,
    Udp,
}

impl Transport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Listening,
    Inbound(Basis),
    Outbound(Basis),
    Unknown,
}

impl Direction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Listening => "listening",
            Self::Inbound(_) => "inbound",
            Self::Outbound(_) => "outbound",
            Self::Unknown => "unknown",
        }
    }

    pub const fn basis(self) -> &'static str {
        match self {
            Self::Listening | Self::Inbound(Basis::Observed) | Self::Outbound(Basis::Observed) => {
                "observed"
            }
            Self::Inbound(Basis::Inferred) | Self::Outbound(Basis::Inferred) => "inferred",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TcpState {
    Listen,
    SynSent,
    SynRecv,
    Established,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub transport: Transport,
    pub local: SocketAddr,
    pub remote: SocketAddr,
    pub direction: Direction,
    pub pid: Option<u32>,
    /// 连接表本身不携带载荷。采样到的字节在 `Snapshot::packets`。
    pub payload: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gap {
    pub reason: &'static str,
    pub os_code: Option<i32>,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PacketBody {
    Encrypted {
        evidence: &'static str,
        frame: Vec<u8>,
        original_len: usize,
    },
    Plain {
        frame: Vec<u8>,
        original_len: usize,
    },
}

impl PacketBody {
    pub fn frame(&self) -> &[u8] {
        match self {
            Self::Encrypted { frame, .. } | Self::Plain { frame, .. } => frame,
        }
    }

    pub fn original_len(&self) -> usize {
        match self {
            Self::Encrypted { original_len, .. } | Self::Plain { original_len, .. } => {
                *original_len
            }
        }
    }

    pub fn truncated(&self) -> bool {
        self.frame().len() != self.original_len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedPacket {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub body: PacketBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub interfaces: Vec<Interface>,
    pub endpoints: Vec<Endpoint>,
    pub packets: Vec<CapturedPacket>,
    pub gaps: Vec<Gap>,
}

pub(crate) struct TableEndpoint {
    pub transport: Transport,
    pub local: SocketAddr,
    pub remote: SocketAddr,
    pub state: Option<TcpState>,
    pub pid: Option<u32>,
}

pub struct Observer {
    capture: PlatformCapture,
}

#[cfg(target_os = "linux")]
type PlatformCapture = linux::Capture;
#[cfg(windows)]
type PlatformCapture = windows::Capture;

impl Observer {
    /// 打开采样套接字（若操作系统拒绝则只记录缺口）并保留到 `collect`。
    /// 调用方应在此之后产生流量，再收集连接表。
    pub fn open() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            capture: linux::Capture::open(),
            #[cfg(windows)]
            capture: windows::Capture::open(),
        }
    }

    pub fn collect(self, wait: Duration, packet_limit: usize) -> io::Result<Snapshot> {
        if wait > MAX_CAPTURE_WAIT || packet_limit > MAX_CAPTURE_PACKETS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "capture wait or packet limit exceeds the fixed ceiling",
            ));
        }
        #[cfg(target_os = "linux")]
        return linux::collect(self.capture, wait, packet_limit);
        #[cfg(windows)]
        return windows::collect(self.capture, wait, packet_limit);
    }
}

pub(crate) fn finish(
    mut snapshot: Snapshot,
    packets: Vec<CapturedPacket>,
    capture: Option<Gap>,
) -> Snapshot {
    snapshot.packets = packets;
    snapshot.gaps.push(Gap {
        reason: "driver_image_absent",
        os_code: None,
        count: 1,
    });
    snapshot.gaps.push(Gap {
        reason: "payload_requires_capture",
        os_code: None,
        count: 1,
    });
    if let Some(gap) = capture {
        snapshot.gaps.push(gap);
    }
    snapshot
}

pub(crate) fn direction_for(
    transport: Transport,
    state: Option<TcpState>,
    local: SocketAddr,
    remote: SocketAddr,
    listeners: &[(IpAddr, u16)],
) -> Direction {
    if transport != Transport::Tcp {
        return Direction::Unknown;
    }
    match state {
        Some(TcpState::Listen) => Direction::Listening,
        Some(TcpState::SynSent) => Direction::Outbound(Basis::Observed),
        Some(TcpState::SynRecv) => Direction::Inbound(Basis::Observed),
        Some(TcpState::Established | TcpState::Other) => {
            let local_listen = listener_covers(listeners, local);
            let remote_listen = listener_covers(listeners, remote);
            match (local_listen, remote_listen) {
                (true, false) => Direction::Inbound(Basis::Inferred),
                (false, true) => Direction::Outbound(Basis::Inferred),
                _ => Direction::Unknown,
            }
        }
        None => Direction::Unknown,
    }
}

fn listener_covers(listeners: &[(IpAddr, u16)], endpoint: SocketAddr) -> bool {
    listeners.iter().any(|(address, port)| {
        *port == endpoint.port() && (address.is_unspecified() || *address == endpoint.ip())
    })
}

pub(crate) fn materialize(tables: Vec<TableEndpoint>) -> Vec<Endpoint> {
    let listeners: Vec<(IpAddr, u16)> = tables
        .iter()
        .filter(|endpoint| endpoint.state == Some(TcpState::Listen))
        .map(|endpoint| (endpoint.local.ip(), endpoint.local.port()))
        .collect();
    tables
        .into_iter()
        .map(|endpoint| {
            let direction = direction_for(
                endpoint.transport,
                endpoint.state,
                endpoint.local,
                endpoint.remote,
                &listeners,
            );
            Endpoint {
                transport: endpoint.transport,
                local: endpoint.local,
                remote: endpoint.remote,
                direction,
                pid: endpoint.pid,
                payload: "table_has_no_payload",
            }
        })
        .collect()
}

#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn captured_from_frame(frame: &[u8]) -> Option<CapturedPacket> {
    captured_from_parsed(frame, parse::parse_frame(frame)?)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn captured_from_ip(packet: &[u8]) -> Option<CapturedPacket> {
    captured_from_parsed(packet, parse::parse_ip(packet)?)
}

fn captured_from_parsed(bytes: &[u8], parsed: parse::ParsedFrame<'_>) -> Option<CapturedPacket> {
    if parsed.payload.is_empty() {
        return None;
    }
    let original_len = bytes.len();
    let kept = bytes.len().min(MAX_FRAME_BYTES);
    let stored = bytes[..kept].to_vec();
    let body = if parsed.tcp && parse::is_tls_record(parsed.payload) {
        PacketBody::Encrypted {
            evidence: "tls-record-header",
            frame: stored,
            original_len,
        }
    } else {
        PacketBody::Plain {
            frame: stored,
            original_len,
        }
    };
    Some(CapturedPacket {
        source: parsed.source,
        destination: parsed.destination,
        body,
    })
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn windows_port(value: u32) -> u16 {
    u16::from_be((value & 0xffff) as u16)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn windows_ipv4(value: u32) -> std::net::Ipv4Addr {
    std::net::Ipv4Addr::from(u32::from_be(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    #[test]
    fn established_flows_follow_the_observed_listener() {
        let listeners = [(IpAddr::V4(Ipv4Addr::LOCALHOST), 80)];
        assert_eq!(
            direction_for(
                Transport::Tcp,
                Some(TcpState::Established),
                addr(40_000),
                addr(80),
                &listeners
            ),
            Direction::Outbound(Basis::Inferred)
        );
        assert_eq!(
            direction_for(
                Transport::Tcp,
                Some(TcpState::Established),
                addr(80),
                addr(40_000),
                &listeners
            ),
            Direction::Inbound(Basis::Inferred)
        );
        assert_eq!(
            direction_for(
                Transport::Tcp,
                Some(TcpState::SynSent),
                addr(40_000),
                addr(80),
                &[]
            ),
            Direction::Outbound(Basis::Observed)
        );
        assert_eq!(
            direction_for(
                Transport::Tcp,
                Some(TcpState::Established),
                addr(40_000),
                addr(81),
                &listeners
            ),
            Direction::Unknown
        );
        assert_eq!(
            direction_for(Transport::Udp, None, addr(1), addr(2), &listeners),
            Direction::Unknown
        );
    }

    #[test]
    fn a_wildcard_listener_covers_a_concrete_local_address() {
        let listeners = [(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 443)];
        assert_eq!(
            direction_for(
                Transport::Tcp,
                Some(TcpState::Established),
                SocketAddr::from((Ipv4Addr::new(172, 30, 0, 2), 443)),
                SocketAddr::from((Ipv4Addr::new(1, 1, 1, 1), 50_000)),
                &listeners
            ),
            Direction::Inbound(Basis::Inferred)
        );
    }

    #[test]
    fn windows_table_fields_use_the_documented_byte_order() {
        assert_eq!(windows_port(0x5000), 80);
        assert_eq!(
            windows_ipv4(0x0100_007f),
            std::net::Ipv4Addr::new(127, 0, 0, 1)
        );
    }

    #[test]
    fn tls_record_on_a_frame_is_encrypted_and_syn_frames_are_dropped() {
        let mut frame = vec![0u8; 14];
        frame[12] = 0x08;
        frame[13] = 0x00;
        frame.extend_from_slice(&[0x45, 0x00, 0x00, 0x3c]);
        frame.extend_from_slice(&[0, 0, 0, 0, 0x40, 0x06, 0, 0]);
        frame.extend_from_slice(&[127, 0, 0, 1, 127, 0, 0, 1]);
        let header_len = 20u8;
        frame.extend_from_slice(&40_000u16.to_be_bytes());
        frame.extend_from_slice(&80u16.to_be_bytes());
        frame.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        frame.push((header_len / 4) << 4);
        frame.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0]);
        assert!(captured_from_frame(&frame).is_none());
        let payload = [0x16u8, 0x03, 0x01, 0x00, 0x05, 0x01, 0x00, 0x00, 0x01, 0x00];
        frame.extend_from_slice(&payload);
        let total = (frame.len() - 14) as u16;
        frame[16] = (total >> 8) as u8;
        frame[17] = total as u8;
        let packet = captured_from_frame(&frame).unwrap();
        assert_eq!(packet.source.port(), 40_000);
        assert_eq!(packet.destination.port(), 80);
        match packet.body {
            PacketBody::Encrypted {
                evidence,
                frame: kept,
                original_len,
            } => {
                assert_eq!(evidence, "tls-record-header");
                assert_eq!(original_len, kept.len());
                assert!(kept.windows(payload.len()).any(|window| window == payload));
            }
            PacketBody::Plain { .. } => panic!("tls record was classified as plaintext"),
        }
    }

    #[test]
    fn live_exchange_reports_directions_interfaces_and_the_capture_gap() {
        use std::io::{Read, Write};
        use std::net::{Ipv4Addr, TcpListener};
        use std::process;
        use std::time::Duration;

        let directory =
            std::env::temp_dir().join(format!("winstrata-net-kernel-{}", process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory).unwrap();
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
        let loopback = snapshot
            .interfaces
            .iter()
            .find(|interface| interface.kind == InterfaceKind::Loopback)
            .expect("loopback");
        let other = snapshot
            .interfaces
            .iter()
            .find(|interface| interface.kind != InterfaceKind::Loopback)
            .expect("non-loopback");
        assert_ne!(loopback.name, other.name);
        assert_ne!(loopback.kind, other.kind);
        assert!(
            loopback
                .addresses
                .contains(&std::net::IpAddr::V4(Ipv4Addr::LOCALHOST))
        );
        assert!(
            snapshot
                .gaps
                .iter()
                .any(|gap| gap.reason == "driver_image_absent")
        );
        assert!(
            snapshot
                .gaps
                .iter()
                .any(|gap| gap.reason == "payload_requires_capture")
        );
        let pid = process::id();
        assert!(snapshot.endpoints.iter().any(|endpoint| {
            endpoint.local == server
                && endpoint.remote.port() == 0
                && endpoint.direction == Direction::Listening
                && endpoint.pid == Some(pid)
                && endpoint.payload == "table_has_no_payload"
        }));
        assert!(snapshot.endpoints.iter().any(|endpoint| {
            endpoint.local == client_addr
                && endpoint.remote == server
                && endpoint.direction == Direction::Outbound(Basis::Inferred)
                && endpoint.pid == Some(pid)
        }));
        assert!(snapshot.endpoints.iter().any(|endpoint| {
            endpoint.local == server
                && endpoint.remote == client_addr
                && endpoint.direction == Direction::Inbound(Basis::Inferred)
                && endpoint.pid == Some(pid)
        }));
        let encrypted = snapshot.packets.iter().any(|packet| {
            matches!(
                &packet.body,
                PacketBody::Encrypted { evidence, .. } if *evidence == "tls-record-header"
            ) && packet.source == client_addr
                && packet.destination == server
                && !packet.body.truncated()
        });
        if !encrypted {
            assert!(
                snapshot.gaps.iter().any(|gap| {
                    (gap.reason == "capture_inaccessible" && gap.os_code.is_some())
                        || gap.reason == "capture_no_payload"
                }),
                "missing capture gap: {:?}",
                snapshot
                    .gaps
                    .iter()
                    .map(|gap| gap.reason)
                    .collect::<Vec<_>>()
            );
        }
        assert!(std::fs::read_dir(&directory).unwrap().next().is_none());
        let _ = std::fs::remove_dir_all(&directory);
        let _ = (client, accepted, listener);
    }
}

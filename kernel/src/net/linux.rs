#![allow(unsafe_code)]

use std::collections::HashMap;
use std::fs;
use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::Path;
use std::time::{Duration, Instant};

use super::{
    CapturedPacket, Gap, Interface, InterfaceKind, Snapshot, TableEndpoint, TcpState, Transport,
    captured_from_frame, finish, materialize,
};

pub(crate) struct Capture {
    socket: Option<PacketSocket>,
    failure: Option<Gap>,
}

impl Capture {
    pub(crate) fn open() -> Self {
        match PacketSocket::open() {
            Ok(socket) => Self {
                socket: Some(socket),
                failure: None,
            },
            Err(error) => Self {
                socket: None,
                failure: Some(Gap {
                    reason: "capture_inaccessible",
                    os_code: error.raw_os_error(),
                    count: 1,
                }),
            },
        }
    }
}

pub(crate) fn collect(capture: Capture, wait: Duration, limit: usize) -> io::Result<Snapshot> {
    let opened = capture.socket.is_some();
    let packets = match &capture.socket {
        Some(socket) => socket.drain(wait, limit)?,
        None => Vec::new(),
    };
    let failure = capture_gap(capture.failure, opened, packets.is_empty());
    let snapshot = read_snapshot()?;
    Ok(finish(snapshot, packets, failure))
}

fn capture_gap(failure: Option<Gap>, opened: bool, empty: bool) -> Option<Gap> {
    if failure.is_some() {
        return failure;
    }
    if opened && empty {
        return Some(Gap {
            reason: "capture_no_payload",
            os_code: None,
            count: 1,
        });
    }
    None
}

struct PacketSocket(i32);

impl PacketSocket {
    fn open() -> io::Result<Self> {
        // 协议号使用网络字节序。CLOEXEC 避免采样描述符被意外继承。
        let protocol = (libc::ETH_P_ALL as u16).to_be() as i32;
        let fd = unsafe {
            libc::socket(
                libc::AF_PACKET,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                protocol,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let socket = Self(fd);
        let buffer = 1 << 20;
        let _ = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                (&buffer as *const i32).cast(),
                4,
            )
        };
        Ok(socket)
    }

    fn drain(&self, wait: Duration, limit: usize) -> io::Result<Vec<CapturedPacket>> {
        let mut packets = Vec::new();
        let deadline = Instant::now() + wait;
        let mut buffer = vec![0u8; 65_536];
        while packets.len() < limit {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let mut poll = libc::pollfd {
                fd: self.0,
                events: libc::POLLIN,
                revents: 0,
            };
            let timeout = i32::try_from(remaining.as_millis()).unwrap_or(i32::MAX);
            let ready = unsafe { libc::poll(&mut poll, 1, timeout) };
            if ready == 0 {
                break;
            }
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            let size = unsafe { libc::recv(self.0, buffer.as_mut_ptr().cast(), buffer.len(), 0) };
            if size < 0 {
                let error = io::Error::last_os_error();
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) {
                    continue;
                }
                return Err(error);
            }
            if size == 0 {
                break;
            }
            if let Some(packet) = captured_from_frame(&buffer[..size as usize]) {
                packets.push(packet);
            }
        }
        Ok(packets)
    }
}

impl Drop for PacketSocket {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

fn read_snapshot() -> io::Result<Snapshot> {
    let mut gaps = Vec::new();
    let interfaces = interfaces()?;
    if interfaces.is_empty() {
        gaps.push(Gap {
            reason: "no_interfaces",
            os_code: None,
            count: 1,
        });
    }
    let (owners, owner_gaps) = socket_owners();
    gaps.extend(owner_gaps);
    let mut tables = Vec::new();
    read_proc_table(
        "/proc/net/tcp",
        false,
        Transport::Tcp,
        &mut tables,
        &mut gaps,
        true,
    )?;
    read_proc_table(
        "/proc/net/tcp6",
        true,
        Transport::Tcp,
        &mut tables,
        &mut gaps,
        false,
    )?;
    read_proc_table(
        "/proc/net/udp",
        false,
        Transport::Udp,
        &mut tables,
        &mut gaps,
        false,
    )?;
    read_proc_table(
        "/proc/net/udp6",
        true,
        Transport::Udp,
        &mut tables,
        &mut gaps,
        false,
    )?;
    for endpoint in &mut tables {
        match owners.get(&endpoint.inode) {
            Some(Owner::One(pid)) => endpoint.pid = Some(*pid),
            Some(Owner::Shared) => {
                endpoint.pid = None;
                push_count(&mut gaps, "shared_socket_inode");
            }
            None => endpoint.pid = None,
        }
    }
    let mut endpoints = materialize(tables.into_iter().map(TableEndpoint::from).collect());
    if endpoints.len() > super::MAX_ENDPOINTS {
        let omitted = (endpoints.len() - super::MAX_ENDPOINTS) as u64;
        endpoints.truncate(super::MAX_ENDPOINTS);
        gaps.push(Gap {
            reason: "endpoint_sample_truncated",
            os_code: None,
            count: omitted,
        });
    }
    Ok(Snapshot {
        interfaces,
        endpoints,
        packets: Vec::new(),
        gaps,
    })
}

struct ProcRow {
    local: SocketAddr,
    remote: SocketAddr,
    state: Option<TcpState>,
    inode: u64,
    pid: Option<u32>,
    transport: Transport,
}

impl From<ProcRow> for TableEndpoint {
    fn from(row: ProcRow) -> Self {
        Self {
            transport: row.transport,
            local: row.local,
            remote: row.remote,
            state: row.state,
            pid: row.pid,
        }
    }
}

fn read_proc_table(
    path: &str,
    v6: bool,
    transport: Transport,
    rows: &mut Vec<ProcRow>,
    gaps: &mut Vec<Gap>,
    required: bool,
) -> io::Result<()> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound && !required => {
            gaps.push(Gap {
                reason: "transport_table_absent",
                os_code: None,
                count: 1,
            });
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        match parse_proc_line(line, v6, transport) {
            Some(row) => rows.push(row),
            None => push_count(gaps, "malformed_endpoint"),
        }
    }
    Ok(())
}

fn parse_proc_line(line: &str, v6: bool, transport: Transport) -> Option<ProcRow> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 10 {
        return None;
    }
    let local = parse_socket(fields[1], v6)?;
    let remote = parse_socket(fields[2], v6)?;
    let state = if transport == Transport::Tcp {
        Some(linux_state(fields[3])?)
    } else {
        None
    };
    let inode = fields[9].parse().ok()?;
    Some(ProcRow {
        local,
        remote,
        state,
        inode,
        pid: None,
        transport,
    })
}

fn parse_socket(field: &str, v6: bool) -> Option<SocketAddr> {
    let (address, port) = field.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let ip = if v6 {
        IpAddr::V6(parse_ipv6(address)?)
    } else {
        IpAddr::V4(parse_ipv4(address)?)
    };
    Some(SocketAddr::new(ip, port))
}

fn parse_ipv4(hex: &str) -> Option<Ipv4Addr> {
    if hex.len() != 8 {
        return None;
    }
    let word = u32::from_str_radix(hex, 16).ok()?;
    Some(Ipv4Addr::from(u32::from_be(word)))
}

fn parse_ipv6(hex: &str) -> Option<Ipv6Addr> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for index in 0..4 {
        let word = u32::from_str_radix(&hex[index * 8..index * 8 + 8], 16).ok()?;
        bytes[index * 4..index * 4 + 4].copy_from_slice(&u32::from_be(word).to_be_bytes());
    }
    Some(Ipv6Addr::from(bytes))
}

fn linux_state(text: &str) -> Option<TcpState> {
    Some(match text {
        "01" => TcpState::Established,
        "02" => TcpState::SynSent,
        "03" => TcpState::SynRecv,
        "0A" => TcpState::Listen,
        "04" | "05" | "06" | "07" | "08" | "09" | "0B" => TcpState::Other,
        _ => return None,
    })
}

enum Owner {
    One(u32),
    Shared,
}

fn socket_owners() -> (HashMap<u64, Owner>, Vec<Gap>) {
    let mut owners = HashMap::new();
    let mut unreadable = 0u64;
    let Ok(processes) = fs::read_dir("/proc") else {
        return (
            owners,
            vec![Gap {
                reason: "process_table_unreadable",
                os_code: None,
                count: 1,
            }],
        );
    };
    for process in processes.flatten() {
        let Ok(pid) = process.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let directory = process.path().join("fd");
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                unreadable += 1;
                continue;
            }
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            let Some(inode) = socket_inode(&target.to_string_lossy()) else {
                continue;
            };
            owners
                .entry(inode)
                .and_modify(|owner| {
                    if !matches!(owner, Owner::One(existing) if *existing == pid) {
                        *owner = Owner::Shared;
                    }
                })
                .or_insert(Owner::One(pid));
        }
    }
    let mut gaps = Vec::new();
    if unreadable > 0 {
        gaps.push(Gap {
            reason: "foreign_process_fds_unreadable",
            os_code: None,
            count: unreadable,
        });
    }
    (owners, gaps)
}

fn socket_inode(target: &str) -> Option<u64> {
    target
        .strip_prefix("socket:[")
        .and_then(|value| value.strip_suffix(']'))
        .and_then(|value| value.parse().ok())
}

fn interfaces() -> io::Result<Vec<Interface>> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: head 是 getifaddrs 的输出指针。成功后由 freeifaddrs 释放整张链表。
    let result = unsafe { libc::getifaddrs(&mut head) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    let mut grouped: Vec<Interface> = Vec::new();
    let mut cursor = head;
    while !cursor.is_null() {
        // SAFETY: cursor 仍属于尚未释放的 getifaddrs 链表。
        let node = unsafe { &*cursor };
        if !node.ifa_name.is_null() {
            let name = unsafe { std::ffi::CStr::from_ptr(node.ifa_name) }
                .to_string_lossy()
                .into_owned();
            if valid_if_name(&name) {
                let flags = node.ifa_flags;
                let interface = grouped.iter_mut().find(|item| item.name == name);
                let interface = if let Some(interface) = interface {
                    interface
                } else {
                    grouped.push(Interface {
                        name: name.clone(),
                        kind: interface_kind(&name, flags),
                        admin_up: flags & libc::IFF_UP as u32 != 0,
                        operational: operstate(&name),
                        addresses: Vec::new(),
                        mac: None,
                    });
                    grouped.last_mut().expect("the interface was just inserted")
                };
                if let Some(address) = interface_address(node.ifa_addr) {
                    if !interface.addresses.contains(&address) {
                        interface.addresses.push(address);
                    }
                }
                if interface.mac.is_none() {
                    interface.mac = mac_address(node.ifa_addr);
                }
            }
        }
        cursor = node.ifa_next;
    }
    unsafe { libc::freeifaddrs(head) };
    Ok(grouped)
}

fn interface_address(pointer: *const libc::sockaddr) -> Option<IpAddr> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: getifaddrs 保证非空地址指向对应族的 sockaddr。
    let family = unsafe { (*pointer).sa_family as i32 };
    match family {
        libc::AF_INET => {
            let value = unsafe { &*(pointer.cast::<libc::sockaddr_in>()) };
            Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(
                value.sin_addr.s_addr,
            ))))
        }
        libc::AF_INET6 => {
            let value = unsafe { &*(pointer.cast::<libc::sockaddr_in6>()) };
            Some(IpAddr::V6(Ipv6Addr::from(value.sin6_addr.s6_addr)))
        }
        _ => None,
    }
}

fn mac_address(pointer: *const libc::sockaddr) -> Option<[u8; 6]> {
    if pointer.is_null() {
        return None;
    }
    let family = unsafe { (*pointer).sa_family as i32 };
    if family != libc::AF_PACKET {
        return None;
    }
    let value = unsafe { &*(pointer.cast::<libc::sockaddr_ll>()) };
    if value.sll_halen != 6 {
        return None;
    }
    let mut mac = [0u8; 6];
    mac.copy_from_slice(&value.sll_addr[..6]);
    Some(mac)
}

fn interface_kind(name: &str, flags: u32) -> InterfaceKind {
    if flags & libc::IFF_LOOPBACK as u32 != 0 {
        return InterfaceKind::Loopback;
    }
    if Path::new(&format!("/sys/class/net/{name}/bridge")).is_dir() {
        return InterfaceKind::Bridge;
    }
    if flags & libc::IFF_POINTOPOINT as u32 != 0 {
        return InterfaceKind::Tunnel;
    }
    match read_type(name) {
        Some(1) => InterfaceKind::Ethernet,
        Some(772) => InterfaceKind::Loopback,
        Some(value) => InterfaceKind::Other(value),
        None => InterfaceKind::Other(0),
    }
}

fn read_type(name: &str) -> Option<u32> {
    let text = fs::read_to_string(format!("/sys/class/net/{name}/type")).ok()?;
    text.trim().parse().ok()
}

fn operstate(name: &str) -> Option<bool> {
    match fs::read_to_string(format!("/sys/class/net/{name}/operstate"))
        .ok()
        .as_deref()
        .map(str::trim)
    {
        Some("up") => Some(true),
        Some("down") => Some(false),
        _ => None,
    }
}

fn valid_if_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
}

fn push_count(gaps: &mut Vec<Gap>, reason: &'static str) {
    if let Some(gap) = gaps.iter_mut().find(|gap| gap.reason == reason) {
        gap.count += 1;
    } else {
        gaps.push(Gap {
            reason,
            os_code: None,
            count: 1,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

    use super::{TcpState, Transport, parse_proc_line};

    #[test]
    fn proc_lines_decode_loopback_listener_and_ipv6() {
        let line = "0: 0100007F:0050 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 12345 1 00000000 100 0 0 10 0";
        let row = parse_proc_line(line, false, Transport::Tcp).unwrap();
        assert_eq!(row.local, SocketAddr::from((Ipv4Addr::LOCALHOST, 80)));
        assert_eq!(row.remote.port(), 0);
        assert_eq!(row.state, Some(TcpState::Listen));
        assert_eq!(row.inode, 12_345);
        let v6 = "0: 00000000000000000000000001000000:01BB 00000000000000000000000000000000:0000 01 00000000:00000000 00:00000000 00000000 1000 0 9 1 00000000 100 0 0 10 0";
        let row = parse_proc_line(v6, true, Transport::Tcp).unwrap();
        assert_eq!(row.local, SocketAddr::from((Ipv6Addr::LOCALHOST, 443)));
        assert_eq!(row.state, Some(TcpState::Established));
        assert!(parse_proc_line("not a row", false, Transport::Tcp).is_none());
    }
}

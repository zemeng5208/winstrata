#![allow(unsafe_code)]

use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    GetExtendedTcpTable, GetExtendedUdpTable, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211,
    IF_TYPE_PPP, IF_TYPE_PROP_VIRTUAL, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL,
    IP_ADAPTER_ADDRESSES_LH, MIB_TCP_STATE_ESTAB, MIB_TCP_STATE_LISTEN, MIB_TCP_STATE_SYN_RCVD,
    MIB_TCP_STATE_SYN_SENT, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_UDP6ROW_OWNER_PID,
    MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows_sys::Win32::NetworkManagement::Ndis::{
    IfOperStatusDown, IfOperStatusLowerLayerDown, IfOperStatusUp,
};
use windows_sys::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, FIONBIO, INVALID_SOCKET, IPPROTO_IP, SOCK_RAW, SOCKADDR,
    SOCKADDR_IN, SOCKADDR_IN6, SOCKET, WSACleanup, WSADATA, WSAEWOULDBLOCK, WSAGetLastError,
    WSAStartup, closesocket, ioctlsocket, recv, socket,
};

use super::{
    CapturedPacket, Gap, Interface, InterfaceKind, Snapshot, TableEndpoint, TcpState, Transport,
    captured_from_ip, finish, materialize, windows_ipv4, windows_port,
};

pub(crate) struct Capture {
    socket: Option<RawSocket>,
    failure: Option<Gap>,
}

impl Capture {
    pub(crate) fn open() -> Self {
        let startup = match Winsock::start() {
            Ok(startup) => startup,
            Err(error) => {
                return Self {
                    socket: None,
                    failure: Some(access_gap(error)),
                };
            }
        };
        match RawSocket::open(startup) {
            Ok(socket) => Self {
                socket: Some(socket),
                failure: None,
            },
            Err(error) => Self {
                socket: None,
                failure: Some(access_gap(error)),
            },
        }
    }
}

fn access_gap(error: io::Error) -> Gap {
    Gap {
        reason: "capture_inaccessible",
        os_code: error.raw_os_error(),
        count: 1,
    }
}

pub(crate) fn collect(capture: Capture, wait: Duration, limit: usize) -> io::Result<Snapshot> {
    let opened = capture.socket.is_some();
    let packets = match &capture.socket {
        Some(socket) => socket.drain(wait, limit)?,
        None => Vec::new(),
    };
    let failure = if capture.failure.is_some() {
        capture.failure
    } else if opened && packets.is_empty() {
        Some(Gap {
            reason: "capture_no_payload",
            os_code: None,
            count: 1,
        })
    } else {
        None
    };
    let snapshot = read_snapshot()?;
    Ok(finish(snapshot, packets, failure))
}

struct Winsock;

impl Winsock {
    fn start() -> io::Result<Self> {
        let mut data = unsafe { std::mem::zeroed::<WSADATA>() };
        let code = unsafe { WSAStartup(0x0202, &mut data) };
        if code != 0 {
            return Err(io::Error::from_raw_os_error(code));
        }
        Ok(Self)
    }
}

impl Drop for Winsock {
    fn drop(&mut self) {
        unsafe { WSACleanup() };
    }
}

struct RawSocket {
    handle: SOCKET,
    _winsock: Winsock,
}

impl RawSocket {
    fn open(winsock: Winsock) -> io::Result<Self> {
        // 不启用混杂模式。Windows 原始套接字也看不到环回，失败时只记录操作系统错误。
        let handle = unsafe { socket(AF_INET as i32, SOCK_RAW, IPPROTO_IP) };
        if handle == INVALID_SOCKET {
            return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
        }
        let socket = Self {
            handle,
            _winsock: winsock,
        };
        let mut enabled = 1u32;
        if unsafe { ioctlsocket(socket.handle, FIONBIO, &mut enabled) } != 0 {
            return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
        }
        Ok(socket)
    }

    fn drain(&self, wait: Duration, limit: usize) -> io::Result<Vec<CapturedPacket>> {
        let mut packets = Vec::new();
        let deadline = Instant::now() + wait;
        let mut buffer = vec![0u8; 65_536];
        while packets.len() < limit && Instant::now() < deadline {
            let size = unsafe {
                recv(
                    self.handle,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as i32,
                    0,
                )
            };
            if size > 0 {
                if let Some(packet) = captured_from_ip(&buffer[..size as usize]) {
                    packets.push(packet);
                }
                continue;
            }
            let code = unsafe { WSAGetLastError() };
            if code == WSAEWOULDBLOCK {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            if size == 0 {
                break;
            }
            return Err(io::Error::from_raw_os_error(code));
        }
        Ok(packets)
    }
}

impl Drop for RawSocket {
    fn drop(&mut self) {
        unsafe { closesocket(self.handle) };
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
    let mut tables = Vec::new();
    read_tcp(AF_INET as u32, false, &mut tables)?;
    read_tcp(AF_INET6 as u32, true, &mut tables)?;
    read_udp(AF_INET as u32, false, &mut tables)?;
    read_udp(AF_INET6 as u32, true, &mut tables)?;
    let mut endpoints = materialize(tables);
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

fn read_tcp(family: u32, v6: bool, rows: &mut Vec<TableEndpoint>) -> io::Result<()> {
    let buffer = grow(|pointer, size| unsafe {
        GetExtendedTcpTable(pointer, size, 0, family, TCP_TABLE_OWNER_PID_ALL, 0)
    })?;
    if v6 {
        for row in table_rows::<MIB_TCP6ROW_OWNER_PID>(&buffer)? {
            rows.push(TableEndpoint {
                transport: Transport::Tcp,
                local: SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                    windows_port(row.dwLocalPort),
                ),
                remote: SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::from(row.ucRemoteAddr)),
                    windows_port(row.dwRemotePort),
                ),
                state: Some(windows_state(row.dwState)),
                pid: Some(row.dwOwningPid),
            });
        }
    } else {
        for row in table_rows::<MIB_TCPROW_OWNER_PID>(&buffer)? {
            rows.push(TableEndpoint {
                transport: Transport::Tcp,
                local: SocketAddr::new(
                    IpAddr::V4(windows_ipv4(row.dwLocalAddr)),
                    windows_port(row.dwLocalPort),
                ),
                remote: SocketAddr::new(
                    IpAddr::V4(windows_ipv4(row.dwRemoteAddr)),
                    windows_port(row.dwRemotePort),
                ),
                state: Some(windows_state(row.dwState)),
                pid: Some(row.dwOwningPid),
            });
        }
    }
    Ok(())
}

fn read_udp(family: u32, v6: bool, rows: &mut Vec<TableEndpoint>) -> io::Result<()> {
    let buffer = grow(|pointer, size| unsafe {
        GetExtendedUdpTable(pointer, size, 0, family, UDP_TABLE_OWNER_PID, 0)
    })?;
    if v6 {
        for row in table_rows::<MIB_UDP6ROW_OWNER_PID>(&buffer)? {
            rows.push(TableEndpoint {
                transport: Transport::Udp,
                local: SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                    windows_port(row.dwLocalPort),
                ),
                remote: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
                state: None,
                pid: Some(row.dwOwningPid),
            });
        }
    } else {
        for row in table_rows::<MIB_UDPROW_OWNER_PID>(&buffer)? {
            rows.push(TableEndpoint {
                transport: Transport::Udp,
                local: SocketAddr::new(
                    IpAddr::V4(windows_ipv4(row.dwLocalAddr)),
                    windows_port(row.dwLocalPort),
                ),
                remote: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                state: None,
                pid: Some(row.dwOwningPid),
            });
        }
    }
    Ok(())
}

fn windows_state(value: u32) -> TcpState {
    match value as i32 {
        MIB_TCP_STATE_LISTEN => TcpState::Listen,
        MIB_TCP_STATE_SYN_SENT => TcpState::SynSent,
        MIB_TCP_STATE_SYN_RCVD => TcpState::SynRecv,
        MIB_TCP_STATE_ESTAB => TcpState::Established,
        _ => TcpState::Other,
    }
}

fn interfaces() -> io::Result<Vec<Interface>> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let buffer = grow(|pointer, size| unsafe {
        GetAdaptersAddresses(
            u32::from(AF_UNSPEC),
            flags,
            std::ptr::null(),
            pointer.cast(),
            size,
        )
    })?;
    if buffer.is_empty() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    let mut cursor = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    let end = unsafe { buffer.as_ptr().add(buffer.len()) };
    while !cursor.is_null() && (cursor as *const u8) < end {
        // SAFETY: GetAdaptersAddresses 把链表写进这块缓冲区，Next 要么为空，要么仍指向其中。
        let adapter = unsafe { &*cursor };
        let friendly = utf16(adapter.FriendlyName);
        let name = if usable(&friendly) {
            friendly
        } else {
            c_string(adapter.AdapterName)
        };
        if usable(&name) {
            let mut addresses = Vec::new();
            let mut unicast = adapter.FirstUnicastAddress;
            while !unicast.is_null() {
                let entry = unsafe { &*unicast };
                if let Some(address) =
                    sockaddr_ip(entry.Address.lpSockaddr, entry.Address.iSockaddrLength)
                {
                    if !addresses.contains(&address) {
                        addresses.push(address);
                    }
                }
                unicast = entry.Next;
            }
            let operational = operational(adapter.OperStatus);
            found.push(Interface {
                name,
                kind: interface_kind(adapter.IfType),
                admin_up: operational == Some(true),
                operational,
                addresses,
                mac: mac(adapter.PhysicalAddress, adapter.PhysicalAddressLength),
            });
        }
        cursor = adapter.Next;
    }
    Ok(found)
}

fn utf16(pointer: *const u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut length = 0usize;
    while length < 256 && unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(pointer, length) })
}

fn c_string(pointer: *const u8) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut length = 0usize;
    while length < 256 && unsafe { *pointer.add(length) } != 0 {
        length += 1;
    }
    String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(pointer, length) }).into_owned()
}

fn usable(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
}

fn sockaddr_ip(pointer: *const SOCKADDR, length: i32) -> Option<IpAddr> {
    if pointer.is_null() || length < 2 {
        return None;
    }
    let family = unsafe { (*pointer).sa_family };
    if family == AF_INET && length as usize >= std::mem::size_of::<SOCKADDR_IN>() {
        let value = unsafe { &*(pointer.cast::<SOCKADDR_IN>()) };
        return Some(IpAddr::V4(Ipv4Addr::from(u32::from_be(unsafe {
            value.sin_addr.S_un.S_addr
        }))));
    }
    if family == AF_INET6 && length as usize >= std::mem::size_of::<SOCKADDR_IN6>() {
        let value = unsafe { &*(pointer.cast::<SOCKADDR_IN6>()) };
        return Some(IpAddr::V6(Ipv6Addr::from(unsafe {
            value.sin6_addr.u.Byte
        })));
    }
    None
}

fn mac(bytes: [u8; 8], length: u32) -> Option<[u8; 6]> {
    if length == 6 {
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&bytes[..6]);
        Some(mac)
    } else {
        None
    }
}

fn interface_kind(if_type: u32) -> InterfaceKind {
    match if_type {
        IF_TYPE_SOFTWARE_LOOPBACK => InterfaceKind::Loopback,
        IF_TYPE_ETHERNET_CSMACD => InterfaceKind::Ethernet,
        IF_TYPE_IEEE80211 => InterfaceKind::Wireless,
        IF_TYPE_TUNNEL | IF_TYPE_PPP => InterfaceKind::Tunnel,
        IF_TYPE_PROP_VIRTUAL => InterfaceKind::Virtual,
        other => InterfaceKind::Other(other),
    }
}

fn operational(status: i32) -> Option<bool> {
    if status == IfOperStatusUp {
        Some(true)
    } else if status == IfOperStatusDown || status == IfOperStatusLowerLayerDown {
        Some(false)
    } else {
        None
    }
}

fn grow(mut call: impl FnMut(*mut core::ffi::c_void, *mut u32) -> u32) -> io::Result<Vec<u8>> {
    let mut size = 0u32;
    let mut buffer = Vec::<u8>::new();
    for _ in 0..6 {
        let code = call(
            if buffer.is_empty() {
                std::ptr::null_mut()
            } else {
                buffer.as_mut_ptr().cast()
            },
            &mut size,
        );
        if code == NO_ERROR && !buffer.is_empty() {
            return Ok(buffer);
        }
        if code == ERROR_INSUFFICIENT_BUFFER
            || code == ERROR_BUFFER_OVERFLOW
            || (code == NO_ERROR && buffer.is_empty())
        {
            if size == 0 {
                return Ok(Vec::new());
            }
            buffer.resize(size as usize, 0);
            continue;
        }
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    Err(io::Error::new(
        ErrorKind::InvalidData,
        "network table size did not settle",
    ))
}

fn table_rows<R>(buffer: &[u8]) -> io::Result<&[R]> {
    if buffer.is_empty() {
        return Ok(&[]);
    }
    if buffer.len() < 4 || std::mem::align_of::<R>() > 4 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "network table header is short",
        ));
    }
    let count = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    let start = 4usize;
    let bytes = count
        .checked_mul(std::mem::size_of::<R>())
        .and_then(|rows| start.checked_add(rows))
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "network table count overflow"))?;
    if buffer.len() < bytes {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "network table is shorter than its row count",
        ));
    }
    // 这些 owner-pid 表的第一行紧跟 dwNumEntries，行对齐不超过 4。
    let pointer = unsafe { buffer.as_ptr().add(start) as *const R };
    Ok(unsafe { std::slice::from_raw_parts(pointer, count) })
}

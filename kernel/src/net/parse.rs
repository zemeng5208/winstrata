//! 未打 VLAN 标签的以太网帧。只解析紧随其后的 IPv4/IPv6，且不跟扩展头。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

pub const MAX_FRAME_BYTES: usize = 2_048;

pub struct ParsedFrame<'a> {
    pub source: SocketAddr,
    pub destination: SocketAddr,
    pub tcp: bool,
    pub payload: &'a [u8],
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_ip(packet: &[u8]) -> Option<ParsedFrame<'_>> {
    match packet.first().copied().unwrap_or(0) >> 4 {
        4 => parse_ipv4(packet),
        6 => parse_ipv6(packet),
        _ => None,
    }
}

#[cfg_attr(windows, allow(dead_code))]
pub fn parse_frame(frame: &[u8]) -> Option<ParsedFrame<'_>> {
    if frame.len() < 14 {
        return None;
    }
    let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
    let packet = &frame[14..];
    match ethertype {
        0x0800 => parse_ipv4(packet),
        0x86dd => parse_ipv6(packet),
        _ => None,
    }
}

pub fn is_tls_record(payload: &[u8]) -> bool {
    if payload.len() < 5 || payload[0] != 0x16 || payload[1] != 0x03 {
        return false;
    }
    if !matches!(payload[2], 0x01 | 0x02 | 0x03 | 0x04) {
        return false;
    }
    let declared = u16::from_be_bytes([payload[3], payload[4]]) as usize;
    (1..=18_432).contains(&declared) && payload.len() >= 5 + declared
}

fn parse_ipv4(packet: &[u8]) -> Option<ParsedFrame<'_>> {
    if packet.len() < 20 || packet[0] >> 4 != 4 {
        return None;
    }
    let header = usize::from(packet[0] & 0x0f) * 4;
    if header < 20 || packet.len() < header {
        return None;
    }
    let source = IpAddr::V4(Ipv4Addr::new(
        packet[12], packet[13], packet[14], packet[15],
    ));
    let destination = IpAddr::V4(Ipv4Addr::new(
        packet[16], packet[17], packet[18], packet[19],
    ));
    parse_transport(packet[9], source, destination, &packet[header..])
}

fn parse_ipv6(packet: &[u8]) -> Option<ParsedFrame<'_>> {
    if packet.len() < 40 || packet[0] >> 4 != 6 {
        return None;
    }
    let mut source = [0u8; 16];
    let mut destination = [0u8; 16];
    source.copy_from_slice(&packet[8..24]);
    destination.copy_from_slice(&packet[24..40]);
    parse_transport(
        packet[6],
        IpAddr::V6(Ipv6Addr::from(source)),
        IpAddr::V6(Ipv6Addr::from(destination)),
        &packet[40..],
    )
}

fn parse_transport(
    protocol: u8,
    source: IpAddr,
    destination: IpAddr,
    segment: &[u8],
) -> Option<ParsedFrame<'_>> {
    let tcp = match protocol {
        6 => true,
        17 => false,
        _ => return None,
    };
    if segment.len() < 8 {
        return None;
    }
    let source = SocketAddr::new(source, u16::from_be_bytes([segment[0], segment[1]]));
    let destination = SocketAddr::new(destination, u16::from_be_bytes([segment[2], segment[3]]));
    let payload = if tcp {
        if segment.len() < 20 {
            return None;
        }
        let offset = usize::from(segment[12] >> 4) * 4;
        if offset < 20 || segment.len() < offset {
            return None;
        }
        &segment[offset..]
    } else {
        &segment[8..]
    };
    Some(ParsedFrame {
        source,
        destination,
        tcp,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_record_requires_a_consistent_header() {
        assert!(is_tls_record(&[
            0x16, 0x03, 0x01, 0x00, 0x05, 1, 0, 0, 1, 0
        ]));
        assert!(!is_tls_record(&[0x16, 0x03, 0x01, 0x00, 0x05, 1, 0]));
        assert!(!is_tls_record(&[0x17, 0x03, 0x03, 0x00, 0x01, 0]));
        assert!(!is_tls_record(&[0x16, 0x03, 0x05, 0x00, 0x01, 0]));
    }
}

//! 最小 PCAPNG：节头、带纳秒分辨率的接口描述，以及带注释的增强包块。
//!
//! 原始包字节只写在增强包块里。风险标记写在该块的标准注释选项中。

use std::io;

use super::format::{input, invalid};

const SHB: u32 = 0x0A0D_0D0A;
const IDB: u32 = 0x0000_0001;
const EPB: u32 = 0x0000_0006;
const ORDER: u32 = 0x1A2B_3C4D;
const IF_TSRESOL: u16 = 9;
const OPT_COMMENT: u16 = 1;
const SNAPLEN: u32 = 1024 * 1024;

fn pad(len: usize) -> usize {
    (4 - (len % 4)) % 4
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn finish_block(block_type: u32, body: &[u8]) -> Vec<u8> {
    let total = (8 + body.len() + 4) as u32;
    let mut out = Vec::with_capacity(total as usize);
    put_u32(&mut out, block_type);
    put_u32(&mut out, total);
    out.extend_from_slice(body);
    put_u32(&mut out, total);
    out
}

pub(crate) fn preamble(link_type: u16) -> Vec<u8> {
    let mut section = Vec::new();
    put_u32(&mut section, ORDER);
    put_u16(&mut section, 1);
    put_u16(&mut section, 0);
    section.extend_from_slice(&u64::MAX.to_le_bytes());
    put_u16(&mut section, 0);
    put_u16(&mut section, 0);
    let mut out = finish_block(SHB, &section);

    let mut interface = Vec::new();
    put_u16(&mut interface, link_type);
    put_u16(&mut interface, 0);
    put_u32(&mut interface, SNAPLEN);
    put_u16(&mut interface, IF_TSRESOL);
    put_u16(&mut interface, 1);
    interface.push(9);
    interface.extend_from_slice(&[0, 0, 0]);
    put_u16(&mut interface, 0);
    put_u16(&mut interface, 0);
    out.extend(finish_block(IDB, &interface));
    out
}

pub(crate) fn packet(timestamp_ns: u64, data: &[u8], comment: &str) -> io::Result<Vec<u8>> {
    if data.is_empty() || data.len() > SNAPLEN as usize {
        return Err(input("packet length is outside the capture limit"));
    }
    if comment.len() > u16::MAX as usize {
        return Err(input("capture comment exceeds the PCAPNG option limit"));
    }
    let mut body = Vec::new();
    put_u32(&mut body, 0);
    put_u32(&mut body, (timestamp_ns >> 32) as u32);
    put_u32(&mut body, timestamp_ns as u32);
    put_u32(&mut body, data.len() as u32);
    put_u32(&mut body, data.len() as u32);
    body.extend_from_slice(data);
    body.extend(std::iter::repeat_n(0, pad(data.len())));
    put_u16(&mut body, OPT_COMMENT);
    put_u16(&mut body, comment.len() as u16);
    body.extend_from_slice(comment.as_bytes());
    body.extend(std::iter::repeat_n(0, pad(comment.len())));
    put_u16(&mut body, 0);
    put_u16(&mut body, 0);
    Ok(finish_block(EPB, &body))
}

pub(crate) fn link_type(prefix: &[u8]) -> io::Result<u16> {
    let (blocks, _) = blocks(prefix)?;
    if blocks.len() < 2 || block_type(&blocks[0]) != SHB || block_type(&blocks[1]) != IDB {
        return Err(invalid("capture segment is missing its PCAPNG header"));
    }
    let body = block_body(&blocks[0])?;
    if body.len() < 8 || u32::from_le_bytes(body[..4].try_into().expect("4")) != ORDER {
        return Err(invalid("capture byte order is not little endian"));
    }
    let interface = block_body(&blocks[1])?;
    if interface.len() < 8 {
        return Err(invalid("capture interface block is truncated"));
    }
    Ok(u16::from_le_bytes(interface[..2].try_into().expect("2")))
}

pub(crate) fn parse_packet(block: &[u8]) -> io::Result<(u64, Vec<u8>, String)> {
    if block_type(block) != EPB {
        return Err(invalid("capture record is not an enhanced packet block"));
    }
    let body = block_body(block)?;
    if body.len() < 20 {
        return Err(invalid("enhanced packet block is truncated"));
    }
    let timestamp = (u64::from(u32::from_le_bytes(body[4..8].try_into().expect("4"))) << 32)
        | u64::from(u32::from_le_bytes(body[8..12].try_into().expect("4")));
    let captured = u32::from_le_bytes(body[12..16].try_into().expect("4")) as usize;
    let original = u32::from_le_bytes(body[16..20].try_into().expect("4")) as usize;
    if captured != original || captured == 0 {
        return Err(invalid("capture lengths are inconsistent"));
    }
    let data_end = 20usize
        .checked_add(captured)
        .ok_or_else(|| invalid("enhanced packet block is truncated"))?;
    if data_end > body.len() {
        return Err(invalid("enhanced packet block is truncated"));
    }
    let data = body[20..data_end].to_vec();
    let mut pos = data_end + pad(captured);
    let mut comment = None;
    while pos + 4 <= body.len() {
        let code = u16::from_le_bytes(body[pos..pos + 2].try_into().expect("2"));
        let len = u16::from_le_bytes(body[pos + 2..pos + 4].try_into().expect("2")) as usize;
        pos += 4;
        if code == 0 {
            if len != 0 || pos != body.len() {
                return Err(invalid("capture options are malformed"));
            }
            break;
        }
        let value_end = pos
            .checked_add(len)
            .filter(|end| *end <= body.len())
            .ok_or_else(|| invalid("capture option is truncated"))?;
        if code == OPT_COMMENT {
            let text = std::str::from_utf8(&body[pos..value_end])
                .map_err(|_| invalid("capture comment is not UTF-8"))?
                .to_owned();
            comment = Some(text);
        }
        pos = value_end + pad(len);
    }
    if pos != body.len() {
        return Err(invalid("capture options are malformed"));
    }
    Ok((
        timestamp,
        data,
        comment.ok_or_else(|| invalid("capture comment is missing"))?,
    ))
}

fn block_type(block: &[u8]) -> u32 {
    block
        .get(..4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4")))
        .unwrap_or(0)
}

fn block_body(block: &[u8]) -> io::Result<&[u8]> {
    if block.len() < 12 || block.len() % 4 != 0 {
        return Err(invalid("PCAPNG block length is invalid"));
    }
    let total = u32::from_le_bytes(block[4..8].try_into().expect("4")) as usize;
    let repeated = u32::from_le_bytes(block[block.len() - 4..].try_into().expect("4")) as usize;
    if total != block.len() || repeated != total {
        return Err(invalid("PCAPNG block length does not match"));
    }
    Ok(&block[8..block.len() - 4])
}

fn blocks(bytes: &[u8]) -> io::Result<(Vec<&[u8]>, usize)> {
    let mut found = Vec::new();
    let mut pos = 0;
    while pos + 8 <= bytes.len() && found.len() < 2 {
        let total = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().expect("4")) as usize;
        if total < 12 || total % 4 != 0 || pos + total > bytes.len() {
            return Err(invalid("PCAPNG block is truncated"));
        }
        found.push(&bytes[pos..pos + total]);
        pos += total;
    }
    Ok((found, pos))
}

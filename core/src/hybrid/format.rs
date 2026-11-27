//! 混合存储的帧、校验和与小端字段编码。
//!
//! 载荷只出现在分段文件里。日志帧保存偏移和 CRC，用来在打开时重建索引。

use std::io::{self, ErrorKind};

pub(crate) const FRAME_MAGIC: &[u8; 4] = b"WSR1";
pub(crate) const JOURNAL_MAGIC: &[u8; 4] = b"WSJ1";
pub(crate) const SEGMENT_MAGIC: &[u8; 4] = b"WSS1";
pub(crate) const JOURNAL_VERSION: u32 = 1;
pub(crate) const HEADER_LEN: usize = 12;
pub(crate) const MAX_RECORD: usize = 1024 * 1024;
pub(crate) const MAX_COMMIT: usize = 8 * 1024 * 1024;
pub(crate) const DEFAULT_SEGMENT_BYTES: u32 = 1024 * 1024;
pub(crate) const MIN_SEGMENT_BYTES: u32 = 256;
pub(crate) const MAX_SEGMENT_BYTES: u32 = 16 * 1024 * 1024;

pub(crate) fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

pub(crate) fn input(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message)
}

pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

pub(crate) fn frame(payload: &[u8]) -> io::Result<Vec<u8>> {
    frame_limited(
        payload,
        MAX_RECORD,
        "record exceeds the 1 MiB storage limit",
    )
}

pub(crate) fn commit_frame(payload: &[u8]) -> io::Result<Vec<u8>> {
    frame_limited(
        payload,
        MAX_COMMIT,
        "commit exceeds the 8 MiB storage limit",
    )
}

fn frame_limited(payload: &[u8], max: usize, message: &str) -> io::Result<Vec<u8>> {
    if payload.len() > max {
        return Err(input(message));
    }
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(FRAME_MAGIC);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&crc32(payload).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

pub(crate) fn journal_header(segment_bytes: u32) -> [u8; HEADER_LEN] {
    let mut header = [0; HEADER_LEN];
    header[..4].copy_from_slice(JOURNAL_MAGIC);
    header[4..8].copy_from_slice(&JOURNAL_VERSION.to_le_bytes());
    header[8..12].copy_from_slice(&segment_bytes.to_le_bytes());
    header
}

pub(crate) fn segment_header(category: u8, segment: u32) -> [u8; HEADER_LEN] {
    let mut header = [0; HEADER_LEN];
    header[..4].copy_from_slice(SEGMENT_MAGIC);
    header[4] = category;
    header[5..9].copy_from_slice(&segment.to_le_bytes());
    header
}

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub(crate) fn u8(&mut self) -> io::Result<u8> {
        let byte = *self
            .data
            .get(self.pos)
            .ok_or_else(|| invalid("truncated record"))?;
        self.pos += 1;
        Ok(byte)
    }

    pub(crate) fn u32(&mut self) -> io::Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
    }

    pub(crate) fn u64(&mut self) -> io::Result<u64> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("8 bytes")))
    }

    pub(crate) fn bytes(&mut self, len: usize) -> io::Result<&'a [u8]> {
        self.take(len)
    }

    pub(crate) fn str(&mut self, max: usize) -> io::Result<String> {
        let len = self.u32()? as usize;
        if len > max {
            return Err(invalid("stored string exceeds its limit"));
        }
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| invalid("stored string is not UTF-8"))
    }

    pub(crate) fn finish(&self) -> io::Result<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(invalid("record contains trailing bytes"))
        }
    }

    fn take(&mut self, len: usize) -> io::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| invalid("truncated record"))?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }
}

pub(crate) fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_str(out: &mut Vec<u8>, value: &str) {
    put_u32(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

/// 从 `start` 起读取完整帧。校验失败或尾部不足时返回已接受的结束位置。
pub(crate) fn read_frames(data: &[u8], start: usize) -> (Vec<(usize, Vec<u8>)>, usize) {
    let mut frames = Vec::new();
    let mut pos = start;
    while data.len() - pos >= HEADER_LEN {
        if data.get(pos..pos + 4) != Some(FRAME_MAGIC) {
            break;
        }
        let len = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().expect("4 bytes")) as usize;
        let crc = u32::from_le_bytes(data[pos + 8..pos + 12].try_into().expect("4 bytes"));
        if len > MAX_COMMIT {
            break;
        }
        let Some(end) = pos
            .checked_add(HEADER_LEN + len)
            .filter(|end| *end <= data.len())
        else {
            break;
        };
        let payload = &data[pos + HEADER_LEN..end];
        if crc32(payload) != crc {
            break;
        }
        frames.push((end, payload.to_vec()));
        pos = end;
    }
    (frames, pos)
}

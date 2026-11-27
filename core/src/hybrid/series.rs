//! 无损时序压缩：时间用差值的差值，数值用相邻差值，两者都做 zigzag 变长编码。

use std::io::{self, ErrorKind};

use super::format::{invalid, put_u32};

fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

fn unzigzag(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn read_varint(data: &[u8], pos: &mut usize) -> io::Result<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *data.get(*pos).ok_or_else(|| invalid("truncated metric"))?;
        *pos += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(io::Error::new(
        ErrorKind::InvalidData,
        "metric integer is too wide",
    ))
}

pub(crate) fn compress(points: &[(i64, i64)]) -> Vec<u8> {
    let mut out = Vec::new();
    put_u32(&mut out, points.len() as u32);
    let mut previous_time = 0i64;
    let mut previous_delta = 0i64;
    let mut previous_value = 0i64;
    for (index, &(time, value)) in points.iter().enumerate() {
        if index == 0 {
            put_varint(&mut out, zigzag(time));
            put_varint(&mut out, zigzag(value));
        } else {
            let delta = time.wrapping_sub(previous_time);
            put_varint(&mut out, zigzag(delta.wrapping_sub(previous_delta)));
            put_varint(&mut out, zigzag(value.wrapping_sub(previous_value)));
            previous_delta = delta;
        }
        previous_time = time;
        previous_value = value;
    }
    out
}

pub(crate) fn decompress(data: &[u8]) -> io::Result<Vec<(i64, i64)>> {
    if data.len() < 4 {
        return Err(invalid("truncated metric"));
    }
    let count = u32::from_le_bytes(data[..4].try_into().expect("4 bytes")) as usize;
    let mut pos = 4;
    let mut points = Vec::with_capacity(count);
    let mut previous_time = 0i64;
    let mut previous_delta = 0i64;
    let mut previous_value = 0i64;
    for index in 0..count {
        let time = if index == 0 {
            unzigzag(read_varint(data, &mut pos)?)
        } else {
            let delta = previous_delta.wrapping_add(unzigzag(read_varint(data, &mut pos)?));
            previous_delta = delta;
            previous_time.wrapping_add(delta)
        };
        let value = if index == 0 {
            unzigzag(read_varint(data, &mut pos)?)
        } else {
            previous_value.wrapping_add(unzigzag(read_varint(data, &mut pos)?))
        };
        previous_time = time;
        previous_value = value;
        points.push((time, value));
    }
    if pos != data.len() {
        return Err(invalid("metric contains trailing bytes"));
    }
    Ok(points)
}

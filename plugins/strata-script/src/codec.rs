//! 帧文本不能包含换行，源码和输出因此走 base64。

const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= data.len() {
        let chunk = u32::from(data[index]) << 16 | u32::from(data[index + 1]) << 8 | u32::from(data[index + 2]);
        push(&mut out, chunk, 4);
        index += 3;
    }
    let rest = data.len() - index;
    if rest == 1 {
        let chunk = u32::from(data[index]) << 16;
        push(&mut out, chunk, 2);
        out.push_str("==");
    } else if rest == 2 {
        let chunk = u32::from(data[index]) << 16 | u32::from(data[index + 1]) << 8;
        push(&mut out, chunk, 3);
        out.push('=');
    }
    out
}

pub(crate) fn decode(text: &str) -> Result<Vec<u8>, ()> {
    if text.len() > 24_000 || !text.len().is_multiple_of(4) {
        return Err(());
    }
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let mut value = 0u32;
        let mut pads = 0;
        for shift in (0..4).rev() {
            let byte = bytes[index];
            index += 1;
            if byte == b'=' {
                pads += 1;
                continue;
            }
            let Some(digit) = TABLE.iter().position(|item| *item == byte) else {
                return Err(());
            };
            value |= (digit as u32) << (shift * 6);
        }
        if pads > 2 || (pads > 0 && index != bytes.len()) {
            return Err(());
        }
        out.push((value >> 16) as u8);
        if pads < 2 {
            out.push((value >> 8) as u8);
        }
        if pads < 1 {
            out.push(value as u8);
        }
    }
    Ok(out)
}

fn push(out: &mut String, chunk: u32, count: usize) {
    for shift in 0..count {
        let index = ((chunk >> (18 - shift * 6)) & 0x3f) as usize;
        out.push(TABLE[index] as char);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_covers_padding() {
        for sample in [b"" as &[u8], b"a", b"ab", b"abc", b"print(6)\n", "值".as_bytes()] {
            assert_eq!(decode(&encode(sample)).unwrap(), sample);
        }
        assert!(decode("@@@@").is_err());
    }
}

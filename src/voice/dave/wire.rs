pub mod op {
    pub const PREPARE_TRANSITION: u8 = 21;
    pub const EXECUTE_TRANSITION: u8 = 22;
    pub const READY_FOR_TRANSITION: u8 = 23;
    pub const PREPARE_EPOCH: u8 = 24;
    pub const MLS_EXTERNAL_SENDER: u8 = 25;
    pub const MLS_KEY_PACKAGE: u8 = 26;
    pub const MLS_PROPOSALS: u8 = 27;
    pub const MLS_COMMIT_WELCOME: u8 = 28;
    pub const MLS_ANNOUNCE_COMMIT: u8 = 29;
    pub const MLS_WELCOME: u8 = 30;
    pub const MLS_INVALID_COMMIT_WELCOME: u8 = 31;
}

#[derive(Debug)]
pub struct InboundBinary {
    pub seq: Option<u16>,
    pub opcode: u8,

    pub transition_id: Option<u16>,

    pub body: Vec<u8>,
}

fn is_dave_opcode(b: u8) -> bool {
    (21..=31).contains(&b)
}

pub fn parse_inbound_binary(buf: &[u8]) -> Option<InboundBinary> {
    if buf.is_empty() {
        return None;
    }
    let (seq, opcode, mut rest) = if is_dave_opcode(buf[0]) {
        (None, buf[0], &buf[1..])
    } else {
        if buf.len() < 3 || !is_dave_opcode(buf[2]) {
            return None;
        }
        (
            Some(u16::from_be_bytes([buf[0], buf[1]])),
            buf[2],
            &buf[3..],
        )
    };

    let transition_id = if opcode == op::MLS_ANNOUNCE_COMMIT || opcode == op::MLS_WELCOME {
        if rest.len() < 2 {
            return None;
        }
        let tid = u16::from_be_bytes([rest[0], rest[1]]);
        rest = &rest[2..];
        Some(tid)
    } else {
        None
    };

    Some(InboundBinary {
        seq,
        opcode,
        transition_id,
        body: rest.to_vec(),
    })
}

pub fn frame_outbound(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(1 + payload.len());
    v.push(opcode);
    v.extend_from_slice(payload);
    v
}

pub fn uleb128_encode(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push(0x80 | (value as u8 & 0x7F));
        value >>= 7;
    }
    out.push(value as u8);
}

pub fn uleb128_decode(buf: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    for (i, &b) in buf.iter().enumerate() {
        result |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    None
}

pub fn mls_varint_encode(len: usize, out: &mut Vec<u8>) {
    let len = len as u64;
    if len < 0x40 {
        out.push(len as u8);
    } else if len < 0x4000 {
        out.push(0x40 | (len >> 8) as u8);
        out.push(len as u8);
    } else if len < 0x4000_0000 {
        out.push(0x80 | (len >> 24) as u8);
        out.push((len >> 16) as u8);
        out.push((len >> 8) as u8);
        out.push(len as u8);
    } else {
        out.push(0xC0 | (len >> 56) as u8);
        for s in (0..56).step_by(8).rev() {
            out.push((len >> s) as u8);
        }
    }
}

pub fn mls_varint_decode(buf: &[u8]) -> Option<(u64, usize)> {
    let first = *buf.first()?;
    match first >> 6 {
        0 => Some(((first & 0x3F) as u64, 1)),
        1 => {
            let b1 = *buf.get(1)?;
            Some(((((first & 0x3F) as u64) << 8) | b1 as u64, 2))
        }
        2 => {
            let mut v = ((first & 0x3F) as u64) << 24;
            v |= (*buf.get(1)? as u64) << 16;
            v |= (*buf.get(2)? as u64) << 8;
            v |= *buf.get(3)? as u64;
            Some((v, 4))
        }
        _ => {
            let mut v = ((first & 0x3F) as u64) << 56;
            for i in 1..8 {
                v |= (*buf.get(i)? as u64) << (8 * (7 - i));
            }
            Some((v, 8))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uleb128_roundtrip() {
        for v in [0u64, 1, 127, 128, 300, 16384, 2_000_000, u32::MAX as u64] {
            let mut buf = Vec::new();
            uleb128_encode(v, &mut buf);
            let (dec, n) = uleb128_decode(&buf).unwrap();
            assert_eq!(dec, v);
            assert_eq!(n, buf.len());
        }
    }

    #[test]
    fn mls_varint_roundtrip() {
        for v in [0usize, 63, 64, 16383, 16384, 1_000_000] {
            let mut buf = Vec::new();
            mls_varint_encode(v, &mut buf);
            let (dec, n) = mls_varint_decode(&buf).unwrap();
            assert_eq!(dec as usize, v);
            assert_eq!(n, buf.len());
        }
    }
}

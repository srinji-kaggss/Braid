//! `leb128` owns variable-length integer encoding and decoding for WebAssembly
//! and canonical binary serialization, enforcing INV-LEB128-MINIMAL: all
//! decoders reject non-minimal or overflowing encodings to prevent malleability.

use std::error::Error;
use std::fmt;

/// Why an LEB128 sequence could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Input ended unexpectedly before the terminating byte.
    UnexpectedEnd,
    /// Value exceeds the target integer width (overflow).
    Overflow,
    /// The encoding is not canonically minimal (e.g. redundant padding bytes).
    NonMinimal,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => write!(f, "unexpected end of LEB128 input"),
            Self::Overflow => write!(f, "LEB128 value exceeds integer capacity"),
            Self::NonMinimal => write!(f, "non-minimal LEB128 encoding rejected"),
        }
    }
}

impl Error for DecodeError {}

/// Encodes an unsigned 64-bit integer into unsigned LEB128 (varuint) bytes.
pub fn encode_u64(mut val: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (val & 0x7f) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if val == 0 {
            break;
        }
    }
}

/// Decodes an unsigned 64-bit integer from unsigned LEB128 bytes.
/// Returns the decoded integer and the number of bytes consumed.
pub fn decode_u64(input: &[u8]) -> Result<(u64, usize), DecodeError> {
    let mut result: u64 = 0;
    let mut shift: usize = 0;

    for (i, &byte) in input.iter().enumerate() {
        let val = (byte & 0x7f) as u64;

        if shift == 63 {
            if val > 1 || (byte & 0x80) != 0 {
                return Err(DecodeError::Overflow);
            }
        } else if shift > 63 {
            return Err(DecodeError::Overflow);
        }

        result |= val << shift;

        if (byte & 0x80) == 0 {
            // INV-LEB128-MINIMAL: non-zero values cannot have trailing 0x00 bytes,
            // and 0 cannot be encoded in more than 1 byte.
            if i > 0 && byte == 0x00 {
                return Err(DecodeError::NonMinimal);
            }
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err(DecodeError::UnexpectedEnd)
}

/// Encodes a signed 64-bit integer into signed LEB128 (varint) bytes.
pub fn encode_i64(mut val: i64, out: &mut Vec<u8>) {
    let mut more = true;
    while more {
        let mut byte = (val & 0x7f) as u8;
        val >>= 7;
        let sign_bit = (byte & 0x40) != 0;
        if (val == 0 && !sign_bit) || (val == -1 && sign_bit) {
            more = false;
        } else {
            byte |= 0x80;
        }
        out.push(byte);
    }
}

/// Decodes a signed 64-bit integer from signed LEB128 bytes.
/// Returns the decoded integer and the number of bytes consumed.
pub fn decode_i64(input: &[u8]) -> Result<(i64, usize), DecodeError> {
    let mut result: u64 = 0;
    let mut shift: usize = 0;
    let mut prev_byte: u8 = 0;

    for (i, &byte) in input.iter().enumerate() {
        let val = (byte & 0x7f) as u64;

        if shift == 63 {
            if (val != 0 && val != 0x7f) || (byte & 0x80) != 0 {
                return Err(DecodeError::Overflow);
            }
        } else if shift > 63 {
            return Err(DecodeError::Overflow);
        }

        result |= val << shift;

        if (byte & 0x80) == 0 {
            let mut signed_res = result as i64;
            if shift < 63 && (byte & 0x40) != 0 {
                signed_res |= (!0i64) << (shift + 7);
            }
            // Minimal encoding check for signed LEB128:
            if i > 0 {
                let prev_sign = (prev_byte & 0x40) != 0;
                if (!prev_sign && byte == 0x00) || (prev_sign && byte == 0x7f) {
                    return Err(DecodeError::NonMinimal);
                }
            }
            return Ok((signed_res, i + 1));
        }
        prev_byte = byte;
        shift += 7;
    }
    Err(DecodeError::UnexpectedEnd)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u64_roundtrip_vectors() {
        let cases = [0u64, 1, 127, 128, 255, 624485, u64::MAX];
        for val in cases {
            let mut buf = Vec::new();
            encode_u64(val, &mut buf);
            let (decoded, consumed) = decode_u64(&buf).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn i64_roundtrip_vectors() {
        let cases = [0i64, 1, -1, 63, -64, 127, -128, 624485, -624485, i64::MIN, i64::MAX];
        for val in cases {
            let mut buf = Vec::new();
            encode_i64(val, &mut buf);
            let (decoded, consumed) = decode_i64(&buf).unwrap();
            assert_eq!(decoded, val, "failed for {val}");
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn standard_wasm_vectors() {
        // Standard LEB128 624485 = 0xE5 0x8E 0x26
        let mut buf = Vec::new();
        encode_u64(624485, &mut buf);
        assert_eq!(buf, vec![0xe5, 0x8e, 0x26]);

        // -624485 = 0x9B 0xF1 0x59
        let mut sbuf = Vec::new();
        encode_i64(-624485, &mut sbuf);
        assert_eq!(sbuf, vec![0x9b, 0xf1, 0x59]);
    }

    #[test]
    fn rejects_non_minimal_padding() {
        // 0 encoded as 2 bytes: [0x80, 0x00]
        let non_minimal = [0x80, 0x00];
        assert_eq!(decode_u64(&non_minimal), Err(DecodeError::NonMinimal));
        assert_eq!(decode_i64(&non_minimal), Err(DecodeError::NonMinimal));

        // -1 encoded as 2 bytes: [0xFF, 0x7F]
        let non_minimal_signed = [0xff, 0x7f];
        assert_eq!(decode_i64(&non_minimal_signed), Err(DecodeError::NonMinimal));
    }

    #[test]
    fn rejects_overflow_and_never_panics() {
        // 11 continuation bytes
        let malformed = vec![0x80; 11];
        assert_eq!(decode_u64(&malformed), Err(DecodeError::Overflow));
        assert_eq!(decode_i64(&malformed), Err(DecodeError::Overflow));

        // 10 continuation bytes + 0x00
        let mut malformed_term = vec![0x80; 10];
        malformed_term.push(0x00);
        assert_eq!(decode_u64(&malformed_term), Err(DecodeError::Overflow));
        assert_eq!(decode_i64(&malformed_term), Err(DecodeError::Overflow));
    }
}

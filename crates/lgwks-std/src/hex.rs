//! `hex` owns lowercase base-16 transcoding and enforces INV-HEX-ROUNDTRIP:
//! `decode(encode(b)) == b` for every byte slice, and `decode` refuses any input
//! that is not an even-length run of ASCII hex digits.
//!
//! Retires the `hex` crate, declared in 11 manifests and reached from 21 lines
//! across the four repos. The call surface is exactly two functions —
//! `hex::encode` (22 occurrences) and `hex::decode` (8) — so this module is
//! complete rather than partial.
//!
//! The algorithm is hoisted from the estate's existing hand-rolled copy at
//! `Braid/crates/braid-ir/src/cid.rs:30-48` rather than written fresh. That
//! matters: `Cid` byte output is a G4 charter authority in Braid's `AGENTS.md`,
//! so `Cid::to_hex` routing through here must be byte-identical, and
//! `matches_the_braid_cid_encoding` below is the differential proof.

use std::error::Error;
use std::fmt;

// ── Encoding ────────────────────────────────────────────────────────────────

const LOWER: &[u8; 16] = b"0123456789abcdef";

/// Encodes bytes as lowercase hex, two characters per input byte.
pub fn encode(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(LOWER[(b >> 4) as usize] as char);
        out.push(LOWER[(b & 0x0f) as usize] as char);
    }
    out
}

// ── Decoding ────────────────────────────────────────────────────────────────

/// Why a hex string could not be decoded. Variant names are stable and
/// machine-readable; callers match on them rather than on message text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The input had an odd number of characters, so some byte is half-written.
    OddLength {
        /// Length of the offending input, in bytes.
        len: usize,
    },
    /// A character outside `[0-9a-fA-F]` appeared at this byte offset.
    NotHexDigit {
        /// Zero-based offset of the offending character.
        index: usize,
        /// The offending byte, reported verbatim for diagnosis.
        byte: u8,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddLength { len } => {
                write!(f, "hex input has odd length {len}; every byte needs two characters")
            }
            Self::NotHexDigit { index, byte } => {
                write!(f, "byte {byte:#04x} at offset {index} is not a hex digit")
            }
        }
    }
}

impl Error for DecodeError {}

/// Decodes a hex string into bytes. Uppercase digits are accepted, matching the
/// upstream `hex` crate this module retires.
pub fn decode(input: impl AsRef<[u8]>) -> Result<Vec<u8>, DecodeError> {
    let input = input.as_ref();
    if input.len() % 2 != 0 {
        return Err(DecodeError::OddLength { len: input.len() });
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    for (pair_index, pair) in input.chunks(2).enumerate() {
        let offset = pair_index * 2;
        let hi = nibble(pair[0]).ok_or(DecodeError::NotHexDigit { index: offset, byte: pair[0] })?;
        let lo = nibble(pair[1])
            .ok_or(DecodeError::NotHexDigit { index: offset + 1, byte: pair[1] })?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_emits_two_lowercase_characters_per_byte() {
        assert_eq!(encode([0x00, 0x0f, 0xff, 0xa5]), "000fffa5");
        assert_eq!(encode(b""), "");
    }

    #[test]
    fn roundtrip_holds_for_every_single_byte() {
        for b in 0u8..=255 {
            let encoded = encode([b]);
            assert_eq!(encoded.len(), 2, "byte {b} encoded to {encoded:?}");
            assert_eq!(decode(&encoded).unwrap(), vec![b]);
        }
    }

    #[test]
    fn decode_accepts_uppercase_digits() {
        assert_eq!(decode("DEADBEEF").unwrap(), decode("deadbeef").unwrap());
    }

    #[test]
    fn decode_refuses_odd_length() {
        assert_eq!(decode("abc"), Err(DecodeError::OddLength { len: 3 }));
    }

    #[test]
    fn decode_names_the_offending_offset() {
        assert_eq!(decode("00zz"), Err(DecodeError::NotHexDigit { index: 2, byte: b'z' }));
        assert_eq!(decode("0z"), Err(DecodeError::NotHexDigit { index: 1, byte: b'z' }));
    }

    /// Differential proof against `Braid/crates/braid-ir/src/cid.rs:30-48`,
    /// which formats each byte with `format!("{b:02x}")`. `Cid` output is a G4
    /// charter authority, so this encoding must not drift from it.
    #[test]
    fn matches_the_braid_cid_encoding() {
        let digest: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(7).wrapping_add(3));
        let braid: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(encode(digest), braid);
        assert_eq!(braid.len(), 64);
    }
}

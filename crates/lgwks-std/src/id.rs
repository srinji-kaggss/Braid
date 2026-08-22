//! `id` owns UUID version 4 generation and parsing, and enforces
//! INV-UUID-RFC4122: the version nibble is always `4` and the variant bits are
//! always `10`, so a value that this module produces is indistinguishable from
//! one the `uuid` crate produces.
//!
//! Retires the `uuid` crate, declared in 7 manifests and reached from 12 call
//! sites. Every one of them is `Uuid::new_v4()` followed by `to_string()`; no
//! other version and no namespace hashing appears anywhere, so v4 is the
//! complete surface.
//!
//! Entropy comes from [`crate::random`], the estate's single CSPRNG owner. A v4
//! UUID built from anything weaker is predictable, so generation returns a
//! `Result` rather than papering over an entropy fault.

use std::error::Error;
use std::fmt;

use crate::random::{self, EntropyError};

/// A 128-bit RFC 4122 identifier, stored as raw bytes in network order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid([u8; 16]);

impl Uuid {
    /// Generates a version 4 UUID from OS entropy.
    pub fn new_v4() -> Result<Self, EntropyError> {
        let mut raw: [u8; 16] = random::bytes()?;
        // INV-UUID-RFC4122: version 4 in the high nibble of octet 6, variant
        // `10` in the two high bits of octet 8.
        raw[6] = (raw[6] & 0x0f) | 0x40;
        raw[8] = (raw[8] & 0x3f) | 0x80;
        Ok(Self(raw))
    }

    /// The identifier's raw bytes in network order.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Builds an identifier from raw bytes without imposing version bits. Use
    /// this only to rehydrate a value that was generated elsewhere.
    pub fn from_bytes(raw: [u8; 16]) -> Self {
        Self(raw)
    }

    /// The RFC 4122 version nibble, or `None` for a value that carries no
    /// recognisable variant.
    pub fn version(&self) -> Option<u8> {
        if self.0[8] & 0xc0 == 0x80 {
            Some(self.0[6] >> 4)
        } else {
            None
        }
    }

    /// Parses the canonical hyphenated 8-4-4-4-12 lowercase or uppercase form.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
        let bytes = text.as_bytes();
        if bytes.len() != 36 {
            return Err(ParseError::WrongLength { len: bytes.len() });
        }
        let mut raw = [0u8; 16];
        let mut out = 0usize;
        let mut cursor = 0usize;
        for (group_index, width) in GROUPS.iter().enumerate() {
            if group_index > 0 {
                if bytes[cursor] != b'-' {
                    return Err(ParseError::MissingHyphen { index: cursor });
                }
                cursor += 1;
            }
            let group = &bytes[cursor..cursor + width];
            let decoded = crate::hex::decode(group)
                .map_err(|_| ParseError::NotHexDigit { index: cursor })?;
            raw[out..out + decoded.len()].copy_from_slice(&decoded);
            out += decoded.len();
            cursor += width;
        }
        Ok(Self(raw))
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let h = crate::hex::encode(self.0);
        write!(f, "{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
    }
}

/// Why a string is not a canonical UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The canonical form is exactly 36 characters.
    WrongLength {
        /// Length of the offending input, in bytes.
        len: usize,
    },
    /// A group separator was expected at this offset.
    MissingHyphen {
        /// Zero-based offset where a `-` was required.
        index: usize,
    },
    /// A group contained a character outside `[0-9a-fA-F]`.
    NotHexDigit {
        /// Zero-based offset of the group that failed to decode.
        index: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { len } => write!(f, "uuid must be 36 characters, got {len}"),
            Self::MissingHyphen { index } => write!(f, "expected '-' at offset {index}"),
            Self::NotHexDigit { index } => write!(f, "group at offset {index} is not hex"),
        }
    }
}

impl Error for ParseError {}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_value_carries_version_four_and_the_rfc_variant() {
        let id = Uuid::new_v4().expect("OS entropy unavailable");
        assert_eq!(id.version(), Some(4));
        assert_eq!(id.as_bytes()[8] & 0xc0, 0x80);
    }

    #[test]
    fn display_is_hyphenated_lowercase_in_eight_four_four_four_twelve() {
        let id = Uuid::from_bytes([
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0x4c, 0xde, 0x8f, 0x01, 0x23, 0x45, 0x67, 0x89,
            0xab, 0xcd,
        ]);
        assert_eq!(id.to_string(), "01234567-89ab-4cde-8f01-23456789abcd");
    }

    #[test]
    fn parse_reverses_display() {
        let id = Uuid::new_v4().expect("OS entropy unavailable");
        assert_eq!(Uuid::parse(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn parse_accepts_uppercase() {
        let lower = "01234567-89ab-4cde-8f01-23456789abcd";
        assert_eq!(Uuid::parse(&lower.to_uppercase()).unwrap(), Uuid::parse(lower).unwrap());
    }

    #[test]
    fn parse_refuses_a_missing_hyphen() {
        // Length stays at 36 so the hyphen check, not the length check, fires.
        assert_eq!(
            Uuid::parse("01234567x89ab-4cde-8f01-23456789abcd"),
            Err(ParseError::MissingHyphen { index: 8 })
        );
    }

    #[test]
    fn parse_refuses_the_wrong_length() {
        assert_eq!(Uuid::parse("abc"), Err(ParseError::WrongLength { len: 3 }));
    }

    #[test]
    fn parse_refuses_a_non_hex_group() {
        assert_eq!(
            Uuid::parse("0123456z-89ab-4cde-8f01-23456789abcd"),
            Err(ParseError::NotHexDigit { index: 0 })
        );
    }

    #[test]
    fn successive_identifiers_differ() {
        let a = Uuid::new_v4().expect("OS entropy unavailable");
        let b = Uuid::new_v4().expect("OS entropy unavailable");
        assert_ne!(a, b);
    }
}

//! `encoding` owns the two byte-to-text encodings the estate uses outside hex
//! and enforces INV-ENCODING-STRICT-DECODE: both decoders refuse malformed
//! input with a typed error rather than skipping bytes, so a corrupted value
//! cannot round-trip into something plausible.
//!
//! Retires `percent-encoding`, declared in 1 manifest and reached from 1 call
//! site. Both encodings are table-driven transcoders with no
//! correctness-critical security property of their own, which is what puts them
//! in the ELIMINATE tier rather than VENDOR.
//!
//! `base64` has **no caller in the estate today** — 0 manifests, 0 call sites.
//! It is here anyway, and deliberately: the purpose of a stdlib+ is that the
//! answer at rung 2 of the admission ladder is already "yes", so the question
//! "can I add `base64`?" never reaches rung 6. That is the one case where
//! building before the third caller is correct, and it is stated rather than
//! disguised as demand.

// ── base64 ──────────────────────────────────────────────────────────────────

/// RFC 4648 standard base64 with `+`, `/`, and `=` padding.
pub mod base64 {
    use std::error::Error;
    use std::fmt;

    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /// Encodes bytes as padded standard base64.
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied().unwrap_or(0);
            let b2 = chunk.get(2).copied().unwrap_or(0);
            out.push(ALPHABET[(b0 >> 2) as usize] as char);
            out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[(b2 & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    /// Why a base64 string could not be decoded.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum DecodeError {
        /// Padded base64 arrives in four-character quanta.
        BadLength {
            /// Length of the offending input, in bytes.
            len: usize,
        },
        /// A character outside the standard alphabet appeared.
        NotInAlphabet {
            /// Zero-based offset of the offending character.
            index: usize,
            /// The offending byte, reported verbatim.
            byte: u8,
        },
        /// `=` appeared anywhere but the last one or two positions.
        MisplacedPadding {
            /// Zero-based offset of the offending `=`.
            index: usize,
        },
    }

    impl fmt::Display for DecodeError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::BadLength { len } => {
                    write!(f, "base64 length {len} is not a multiple of 4")
                }
                Self::NotInAlphabet { index, byte } => {
                    write!(f, "byte {byte:#04x} at offset {index} is not base64")
                }
                Self::MisplacedPadding { index } => {
                    write!(f, "padding '=' at offset {index} is not at the end")
                }
            }
        }
    }

    impl Error for DecodeError {}

    /// Decodes padded standard base64.
    pub fn decode(input: impl AsRef<[u8]>) -> Result<Vec<u8>, DecodeError> {
        let input = input.as_ref();
        if input.is_empty() {
            return Ok(Vec::new());
        }
        if input.len() % 4 != 0 {
            return Err(DecodeError::BadLength { len: input.len() });
        }
        let padding = input.iter().rev().take_while(|&&b| b == b'=').count();
        if padding > 2 {
            return Err(DecodeError::MisplacedPadding { index: input.len() - padding });
        }
        let body = &input[..input.len() - padding];
        if let Some(offset) = body.iter().position(|&b| b == b'=') {
            return Err(DecodeError::MisplacedPadding { index: offset });
        }

        let mut out = Vec::with_capacity(input.len() / 4 * 3);
        let mut accumulator = 0u32;
        let mut filled = 0u32;
        for (index, &byte) in body.iter().enumerate() {
            let value =
                sextet(byte).ok_or(DecodeError::NotInAlphabet { index, byte })?;
            accumulator = (accumulator << 6) | u32::from(value);
            filled += 6;
            if filled >= 8 {
                filled -= 8;
                out.push((accumulator >> filled) as u8);
            }
        }
        if filled > 0 && (accumulator & ((1 << filled) - 1)) != 0 {
            return Err(DecodeError::MisplacedPadding { index: body.len() });
        }
        Ok(out)
    }

    fn sextet(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
}

// ── percent ─────────────────────────────────────────────────────────────────

/// RFC 3986 percent-encoding.
pub mod percent {
    use std::error::Error;
    use std::fmt;

    /// Encodes every byte that is not an RFC 3986 unreserved character, which
    /// is the component-safe set: reserved delimiters such as `/` and `?` are
    /// escaped, so the result is safe to place in a single URL component.
    pub fn encode_component(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for &byte in text.as_bytes() {
            if is_unreserved(byte) {
                out.push(byte as char);
            } else {
                out.push('%');
                out.push(upper_nibble(byte >> 4));
                out.push(upper_nibble(byte & 0x0f));
            }
        }
        out
    }

    fn is_unreserved(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
    }

    fn upper_nibble(value: u8) -> char {
        b"0123456789ABCDEF"[value as usize] as char
    }

    /// Why a percent-encoded string could not be decoded.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum DecodeError {
        /// A `%` was not followed by two hex digits.
        TruncatedEscape {
            /// Zero-based offset of the offending `%`.
            index: usize,
        },
        /// A `%` was followed by something other than hex digits.
        NotHexDigit {
            /// Zero-based offset of the offending `%`.
            index: usize,
        },
        /// The decoded bytes are not valid UTF-8.
        NotUtf8,
    }

    impl fmt::Display for DecodeError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::TruncatedEscape { index } => {
                    write!(f, "escape at offset {index} needs two hex digits")
                }
                Self::NotHexDigit { index } => {
                    write!(f, "escape at offset {index} is not hex")
                }
                Self::NotUtf8 => f.write_str("decoded bytes are not valid UTF-8"),
            }
        }
    }

    impl Error for DecodeError {}

    /// Decodes percent escapes back into text.
    pub fn decode(text: &str) -> Result<String, DecodeError> {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != b'%' {
                out.push(bytes[i]);
                i += 1;
                continue;
            }
            let escape =
                bytes.get(i + 1..i + 3).ok_or(DecodeError::TruncatedEscape { index: i })?;
            let decoded = crate::hex::decode(escape).map_err(|_| DecodeError::NotHexDigit {
                index: i,
            })?;
            out.push(decoded[0]);
            i += 3;
        }
        String::from_utf8(out).map_err(|_| DecodeError::NotUtf8)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4648 section 10 test vectors, verbatim.
    #[test]
    fn base64_matches_the_rfc_4648_vectors() {
        let vectors = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in vectors {
            assert_eq!(base64::encode(plain), encoded, "encoding {plain:?}");
            assert_eq!(base64::decode(encoded).unwrap(), plain.as_bytes(), "decoding {encoded:?}");
        }
    }

    #[test]
    fn base64_roundtrips_every_byte_value() {
        let all: Vec<u8> = (0u8..=255).collect();
        assert_eq!(base64::decode(base64::encode(&all)).unwrap(), all);
    }

    #[test]
    fn base64_refuses_a_length_that_is_not_a_quantum() {
        assert_eq!(base64::decode("Zm9"), Err(base64::DecodeError::BadLength { len: 3 }));
    }

    #[test]
    fn base64_refuses_an_alien_character() {
        assert_eq!(
            base64::decode("Zm9v*mFy"),
            Err(base64::DecodeError::NotInAlphabet { index: 4, byte: b'*' })
        );
    }

    #[test]
    fn base64_refuses_interior_padding() {
        assert_eq!(
            base64::decode("Zm=vYmFy"),
            Err(base64::DecodeError::MisplacedPadding { index: 2 })
        );
    }

    #[test]
    fn percent_escapes_everything_reserved() {
        assert_eq!(percent::encode_component("a b/c?d"), "a%20b%2Fc%3Fd");
        assert_eq!(percent::encode_component("azAZ09-_.~"), "azAZ09-_.~");
    }

    #[test]
    fn percent_roundtrips_multibyte_text() {
        let text = "path/to/thing — ünïcödé";
        assert_eq!(percent::decode(&percent::encode_component(text)).unwrap(), text);
    }

    #[test]
    fn percent_decode_accepts_lowercase_escapes() {
        assert_eq!(percent::decode("a%2fb").unwrap(), "a/b");
    }

    #[test]
    fn percent_refuses_a_truncated_escape() {
        assert_eq!(percent::decode("a%2"), Err(percent::DecodeError::TruncatedEscape { index: 1 }));
    }

    #[test]
    fn percent_refuses_a_non_hex_escape() {
        assert_eq!(percent::decode("a%zzb"), Err(percent::DecodeError::NotHexDigit { index: 1 }));
    }

    #[test]
    fn percent_refuses_escapes_that_decode_to_invalid_utf8() {
        assert_eq!(percent::decode("%FF%FE"), Err(percent::DecodeError::NotUtf8));
    }

    #[test]
    fn base64_rejects_non_zero_padding_bits() {
        // "Zg==" decodes to b"f" (0b01100110)
        // "Zh==" has bit 0 of the unused nibble set -> must be rejected
        assert_eq!(base64::decode("Zg==").unwrap(), vec![b'f']);
        assert!(matches!(base64::decode("Zh=="), Err(base64::DecodeError::MisplacedPadding { .. })));
    }
}

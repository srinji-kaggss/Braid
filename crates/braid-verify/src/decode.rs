//! The verifier's OWN strict decoder + re-encoder (D9 anti-trusting-trust).
//!
//! //why this duplicates `braid_ir::canon` on purpose: the serialization /
//! normalization layer is exactly where a generator and a verifier sharing
//! code develop a shared blind spot (threat T2 — a parse differential between
//! "what was verified" and "what runs"). The two implementations are written
//! independently against `spec/braid/` and held byte-equal by the KAT parity
//! suite (`tests/parity.rs`): if they EVER disagree on any vector, the build
//! is RED. Shared *types* (`braid_ir::Value`) are fine; shared *byte logic*
//! is not.

use braid_ir::Value;
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_DEPTH: usize = 64;

/// Error variants encountered during canonical CBOR decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Unexpected end of input buffer.
    Truncated {
        /// Byte offset where input ended prematurely.
        at: usize,
    },
    /// A forbidden CBOR construct was encountered.
    Forbidden {
        /// Byte offset of the forbidden construct.
        at: usize,
        /// Description of the forbidden element.
        reason: &'static str,
    },
    /// Value was encoded with non-minimal integer width.
    NonMinimal {
        /// Byte offset where non-minimal width was found.
        at: usize,
    },
    /// Map keys were not strictly ordered by canonical length-then-bytes.
    KeyOrder {
        /// Byte offset of out-of-order key.
        at: usize,
    },
    /// Text string contains invalid UTF-8 bytes.
    Utf8 {
        /// Byte offset of invalid UTF-8 string.
        at: usize,
    },
    /// Trailing unparsed bytes remain after top-level item.
    Trailing {
        /// Byte offset where trailing bytes began.
        at: usize,
    },
    /// Nesting depth exceeded [`MAX_DEPTH`].
    Depth {
        /// Byte offset where depth exceeded limit.
        at: usize,
    },
    /// Canonical re-encode did not match the input bytes byte-for-byte.
    NotBijective {
        /// Offset where bijection mismatch was detected.
        at: usize,
    },
    /// Integer value exceeded 64-bit signed representation.
    IntRange {
        /// Byte offset where integer was decoded.
        at: usize,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { at } => write!(f, "truncated input at offset {at}"),
            Self::Forbidden { at, reason } => {
                write!(f, "forbidden construct ({reason}) at offset {at}")
            }
            Self::NonMinimal { at } => write!(f, "non-minimal encoding at offset {at}"),
            Self::KeyOrder { at } => write!(f, "non-canonical key order at offset {at}"),
            Self::Utf8 { at } => write!(f, "invalid utf-8 at offset {at}"),
            Self::Trailing { at } => write!(f, "trailing bytes starting at offset {at}"),
            Self::Depth { at } => write!(f, "max nesting depth exceeded at offset {at}"),
            Self::NotBijective { at } => write!(f, "non-bijective encoding at offset {at}"),
            Self::IntRange { at } => write!(f, "integer out of range at offset {at}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Canonical key order (length, then bytes) — restated here, not imported.
fn key_lt(a: &str, b: &str) -> bool {
    (a.len(), a.as_bytes()) < (b.len(), b.as_bytes())
}

struct Cursor<'a> {
    buffer: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        let byte_val = *self
            .buffer
            .get(self.offset)
            .ok_or(DecodeError::Truncated { at: self.offset })?;
        self.offset += 1;
        Ok(byte_val)
    }

    fn check_bounds(&self, end_pos: usize) -> Result<(), DecodeError> {
        if end_pos > self.buffer.len() {
            Err(DecodeError::Truncated { at: self.offset })
        } else {
            Ok(())
        }
    }

    fn check_slice_len(&self, length: u64) -> Result<usize, DecodeError> {
        if let Ok(slice_len) = usize::try_from(length) {
            Ok(slice_len)
        } else {
            Err(DecodeError::Truncated { at: self.offset })
        }
    }

    fn read_slice(&mut self, length: u64) -> Result<&'a [u8], DecodeError> {
        let slice_len = self.check_slice_len(length)?;
        let end_pos = self
            .offset
            .checked_add(slice_len)
            .ok_or(DecodeError::Truncated { at: self.offset })?;
        self.check_bounds(end_pos)?;
        let slice_bytes = &self.buffer[self.offset..end_pos];
        self.offset = end_pos;
        Ok(slice_bytes)
    }

    fn read_arg_u16(&mut self) -> Result<u64, DecodeError> {
        let slice_bytes = self.read_slice(2)?;
        let val_u16 = u64::from(u16::from_be_bytes(slice_bytes.try_into().unwrap()));
        if val_u16 > 0xff {
            Ok(val_u16)
        } else {
            Err(DecodeError::NonMinimal { at: self.offset })
        }
    }

    fn read_arg_u32(&mut self) -> Result<u64, DecodeError> {
        let slice_bytes = self.read_slice(4)?;
        let val_u32 = u64::from(u32::from_be_bytes(slice_bytes.try_into().unwrap()));
        if val_u32 > 0xffff {
            Ok(val_u32)
        } else {
            Err(DecodeError::NonMinimal { at: self.offset })
        }
    }

    fn read_arg_u64(&mut self) -> Result<u64, DecodeError> {
        let slice_bytes = self.read_slice(8)?;
        let val_u64 = u64::from_be_bytes(slice_bytes.try_into().unwrap());
        if val_u64 > 0xffff_ffff {
            Ok(val_u64)
        } else {
            Err(DecodeError::NonMinimal { at: self.offset })
        }
    }

    fn read_arg(&mut self, first_byte: u8) -> Result<u64, DecodeError> {
        let additional_info = first_byte & 0x1f;
        match additional_info {
            0..=23 => Ok(u64::from(additional_info)),
            24 => {
                let val_u8 = u64::from(self.read_u8()?);
                if val_u8 >= 24 {
                    Ok(val_u8)
                } else {
                    Err(DecodeError::NonMinimal { at: self.offset })
                }
            }
            25 => self.read_arg_u16(),
            26 => self.read_arg_u32(),
            27 => self.read_arg_u64(),
            _ => Err(DecodeError::Forbidden {
                at: self.offset,
                reason: "reserved/indefinite",
            }),
        }
    }

    fn read_text(&mut self, length: u64) -> Result<String, DecodeError> {
        let current_at = self.offset;
        let slice_bytes = self.read_slice(length)?;
        match std::str::from_utf8(slice_bytes) {
            Ok(valid_str) => Ok(valid_str.to_owned()),
            Err(utf_err) => Err(DecodeError::Utf8 {
                at: current_at + utf_err.valid_up_to(),
            }),
        }
    }

    fn check_list_bounds(&self, count: u64) -> Result<(), DecodeError> {
        let remaining_bytes = (self.buffer.len() - self.offset) as u64;
        if count > remaining_bytes {
            Err(DecodeError::Truncated { at: self.offset })
        } else {
            Ok(())
        }
    }

    fn read_list(&mut self, count: u64, depth: usize) -> Result<Value, DecodeError> {
        self.check_list_bounds(count)?;
        let mut list_items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let item_val = self.read_val(depth + 1)?;
            list_items.push(item_val);
        }
        Ok(Value::List(list_items))
    }

    fn read_key_string(&mut self) -> Result<String, DecodeError> {
        let key_head = self.read_u8()?;
        if key_head >> 5 != 3 {
            Err(DecodeError::Forbidden {
                at: self.offset,
                reason: "map key",
            })
        } else {
            let key_len = self.read_arg(key_head)?;
            self.read_text(key_len)
        }
    }

    fn check_key_order(
        &self,
        prev_key: &Option<String>,
        next_key: &str,
    ) -> Result<(), DecodeError> {
        if let Some(prev) = prev_key {
            if !key_lt(prev, next_key) {
                Err(DecodeError::KeyOrder { at: self.offset })
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn read_map_entry(
        &mut self,
        depth: usize,
        last_key: &mut Option<String>,
    ) -> Result<(String, Value), DecodeError> {
        let map_key = self.read_key_string()?;
        self.check_key_order(last_key, &map_key)?;
        let map_val = self.read_val(depth + 1)?;
        *last_key = Some(map_key.clone());
        Ok((map_key, map_val))
    }

    fn check_map_bounds(&self, count: u64) -> Result<(), DecodeError> {
        let remaining_pairs = ((self.buffer.len() - self.offset) / 2) as u64;
        if count > remaining_pairs {
            Err(DecodeError::Truncated { at: self.offset })
        } else {
            Ok(())
        }
    }

    fn read_map(&mut self, count: u64, depth: usize) -> Result<Value, DecodeError> {
        self.check_map_bounds(count)?;
        let mut map_items = BTreeMap::new();
        let mut last_key: Option<String> = None;
        for _ in 0..count {
            let (map_key, map_val) = self.read_map_entry(depth, &mut last_key)?;
            map_items.insert(map_key, map_val);
        }
        Ok(Value::Map(map_items))
    }

    fn read_simple_or_float(&mut self, first_byte: u8) -> Result<Value, DecodeError> {
        match first_byte {
            0xf4 => Ok(Value::Bool(false)),
            0xf5 => Ok(Value::Bool(true)),
            _ => Err(DecodeError::Forbidden {
                at: self.offset,
                reason: "simple/float",
            }),
        }
    }

    fn parse_positive_int(&self, arg_val: u64, at: usize) -> Result<Value, DecodeError> {
        if let Ok(val) = i64::try_from(arg_val) {
            Ok(Value::Int(val))
        } else {
            Err(DecodeError::IntRange { at })
        }
    }

    fn parse_negative_int(&self, arg_val: u64, at: usize) -> Result<Value, DecodeError> {
        if let Ok(val_pos) = i64::try_from(arg_val) {
            Ok(Value::Int(-1 - val_pos))
        } else {
            Err(DecodeError::IntRange { at })
        }
    }

    fn read_major_int(&self, major: u8, arg_val: u64, at: usize) -> Result<Value, DecodeError> {
        if major == 0 {
            self.parse_positive_int(arg_val, at)
        } else {
            self.parse_negative_int(arg_val, at)
        }
    }

    fn check_depth(&self, depth: usize) -> Result<(), DecodeError> {
        if depth > MAX_DEPTH {
            Err(DecodeError::Depth { at: self.offset })
        } else {
            Ok(())
        }
    }

    fn read_major_payload(
        &mut self,
        major: u8,
        arg_val: u64,
        depth: usize,
        at: usize,
    ) -> Result<Value, DecodeError> {
        match major {
            0 | 1 => self.read_major_int(major, arg_val, at),
            2 => Ok(Value::Bytes(self.read_slice(arg_val)?.to_vec())),
            3 => Ok(Value::Text(self.read_text(arg_val)?)),
            4 => self.read_list(arg_val, depth),
            5 => self.read_map(arg_val, depth),
            _ => Err(DecodeError::Forbidden {
                at,
                reason: "tag/unknown",
            }),
        }
    }

    fn read_val(&mut self, depth: usize) -> Result<Value, DecodeError> {
        self.check_depth(depth)?;
        let first_byte = self.read_u8()?;
        let major_type = first_byte >> 5;
        if major_type == 7 {
            self.read_simple_or_float(first_byte)
        } else {
            let arg_val = self.read_arg(first_byte)?;
            let current_at = self.offset;
            self.read_major_payload(major_type, arg_val, depth, current_at)
        }
    }
}

// ── independent re-encoder for the bijection check ──

fn emit_head(out: &mut Vec<u8>, major: u8, n: u64) {
    let major_shifted = major << 5;
    if n < 24 {
        out.push(major_shifted | n as u8);
    } else if n <= 0xff {
        out.extend_from_slice(&[major_shifted | 24, n as u8]);
    } else if n <= 0xffff {
        out.push(major_shifted | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(major_shifted | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(major_shifted | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn reencode_map(map_items: &BTreeMap<String, Value>, out: &mut Vec<u8>) {
    emit_head(out, 5, map_items.len() as u64);
    let mut keys: Vec<&String> = map_items.keys().collect();
    keys.sort_by_key(|k| (k.len(), k.as_bytes()));
    for k in keys {
        emit_head(out, 3, k.len() as u64);
        out.extend_from_slice(k.as_bytes());
        reencode(&map_items[k], out);
    }
}

fn reencode_list(items: &[Value], out: &mut Vec<u8>) {
    emit_head(out, 4, items.len() as u64);
    for item in items {
        reencode(item, out);
    }
}

fn reencode_int(i: i64, out: &mut Vec<u8>) {
    if i >= 0 {
        emit_head(out, 0, i as u64);
    } else {
        emit_head(out, 1, (-1i128 - i as i128) as u64);
    }
}

pub fn reencode(val: &Value, out: &mut Vec<u8>) {
    match val {
        Value::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Value::Int(i) => reencode_int(*i, out),
        Value::Bytes(b) => {
            emit_head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Value::Text(s) => {
            emit_head(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Value::List(items) => reencode_list(items, out),
        Value::Map(map_items) => reencode_map(map_items, out),
    }
}

fn verify_bijection(bytes: &[u8], decoded_val: &Value, at: usize) -> Result<(), DecodeError> {
    let mut reencoded = Vec::with_capacity(bytes.len());
    reencode(decoded_val, &mut reencoded);
    if reencoded != bytes {
        Err(DecodeError::NotBijective { at })
    } else {
        Ok(())
    }
}

fn check_cursor_at_end(offset: usize, total_len: usize) -> Result<(), DecodeError> {
    if offset != total_len {
        Err(DecodeError::Trailing { at: offset })
    } else {
        Ok(())
    }
}

/// Strict decode + independent bijection guard.
pub fn decode_canonical(bytes: &[u8]) -> Result<Value, DecodeError> {
    let mut cursor = Cursor {
        buffer: bytes,
        offset: 0,
    };
    let decoded_val = cursor.read_val(0)?;
    check_cursor_at_end(cursor.offset, bytes.len())?;
    verify_bijection(bytes, &decoded_val, cursor.offset)?;
    Ok(decoded_val)
}

//! Strict canonical CBOR decoder and bijection verifier.
//!
//! Enforces deterministic parsing rules and verifies bijection via re-encoding.

use super::encode::encode;
use super::error::{CanonError, MAX_DEPTH};
use super::key_cmp;
use crate::value::Value;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Stream reader for decoding CBOR byte sequences.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

/// Safely casts a 64-bit argument to `usize`, reporting truncation on overflow.
fn check_arg_as_usize(arg_val: u64, at: &'static str) -> Result<usize, CanonError> {
    match usize::try_from(arg_val) {
        Ok(val) => Ok(val),
        Err(_err) => Err(CanonError::Truncated { at }),
    }
}

/// Decodes an unsigned positive integer value into an IR integer.
fn decode_int_value(arg_val: u64) -> Result<Value, CanonError> {
    if arg_val > i64::MAX as u64 {
        Err(CanonError::IntRange {
            at: "decode_int_value",
        })
    } else {
        Ok(Value::Int(arg_val as i64))
    }
}

/// Decodes a negative integer value into an IR integer.
fn decode_negint_value(arg_val: u64) -> Result<Value, CanonError> {
    if arg_val > i64::MAX as u64 {
        Err(CanonError::IntRange {
            at: "decode_negint_value",
        })
    } else {
        Ok(Value::Int(-1i64 - arg_val as i64))
    }
}

/// Checks that remaining buffer capacity is sufficient.
fn check_remaining_capacity(
    buf_len: usize,
    pos: usize,
    needed: usize,
    at: &'static str,
) -> Result<(), CanonError> {
    match buf_len.checked_sub(pos) {
        Some(remaining) if remaining >= needed => Ok(()),
        _ => Err(CanonError::Truncated { at }),
    }
}

/// Checks slice bounds for reader take operations.
fn check_take_bounds(end: usize, buf_len: usize) -> Result<(), CanonError> {
    if end > buf_len {
        Err(CanonError::Truncated {
            at: "Reader::take_bounds",
        })
    } else {
        Ok(())
    }
}

fn calculate_needed_map_capacity(count: usize) -> Result<usize, CanonError> {
    match count.checked_mul(2) {
        Some(n) => Ok(n),
        None => Err(CanonError::Truncated {
            at: "Reader::decode_map_bounds",
        }),
    }
}

/// Verifies map buffer capacity for entries.
fn check_map_capacity(buf_len: usize, pos: usize, arg_val: u64) -> Result<usize, CanonError> {
    let count = check_arg_as_usize(arg_val, "Reader::decode_map_overflow")?;
    let needed = calculate_needed_map_capacity(count)?;
    check_remaining_capacity(buf_len, pos, needed, "Reader::decode_map_bounds")?;
    Ok(count)
}

/// Verifies that a map key head is major type 3 (UTF-8 text).
fn check_map_key_major(key_head: u8) -> Result<(), CanonError> {
    if key_head >> 5 != 3 {
        Err(CanonError::ForbiddenForm {
            form: "non-text map key",
            at: "Reader::read_map_key",
        })
    } else {
        Ok(())
    }
}

/// Enforces canonical strictly-increasing key ordering.
fn check_map_key_order(prev: &Option<String>, key_str: &str) -> Result<(), CanonError> {
    if let Some(p) = prev {
        if key_cmp(p, key_str) != core::cmp::Ordering::Less {
            Err(CanonError::KeyOrder {
                at: "Reader::check_map_key_order",
            })
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

/// Decodes simple values for major type 7.
fn decode_simple_major7(head_byte: u8) -> Result<Value, CanonError> {
    match head_byte {
        0xf4 => Ok(Value::Bool(false)),
        0xf5 => Ok(Value::Bool(true)),
        _ => Err(CanonError::ForbiddenForm {
            form: "float/null/simple",
            at: "Reader::value_major7",
        }),
    }
}

/// Guards against stack exhaustion via bounded recursion depth.
fn check_recursion_depth(depth: usize) -> Result<(), CanonError> {
    if depth > MAX_DEPTH {
        Err(CanonError::DepthExceeded {
            at: "Reader::value_depth",
        })
    } else {
        Ok(())
    }
}

impl<'a> Reader<'a> {
    /// Reads a single byte from the buffer.
    fn byte(&mut self) -> Result<u8, CanonError> {
        let byte_val = *self.buf.get(self.pos).ok_or(CanonError::Truncated {
            at: "Reader::byte",
        })?;
        self.pos += 1;
        Ok(byte_val)
    }

    /// Takes `n` bytes from the buffer advancing position.
    fn take(&mut self, n: usize) -> Result<&'a [u8], CanonError> {
        let end = self.pos.checked_add(n).ok_or(CanonError::Truncated {
            at: "Reader::take_overflow",
        })?;
        check_take_bounds(end, self.buf.len())?;
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Reads an 8-bit argument with minimal encoding check.
    fn read_arg_u8(&mut self) -> Result<u64, CanonError> {
        let byte_val = self.byte()? as u64;
        if byte_val < 24 {
            Err(CanonError::NonMinimalInt {
                at: "Reader::read_arg_u8",
            })
        } else {
            Ok(byte_val)
        }
    }

    /// Reads a 16-bit argument with minimal encoding check.
    fn read_arg_u16(&mut self) -> Result<u64, CanonError> {
        let slice = self.take(2)?;
        let mut arr = [0u8; 2];
        arr.copy_from_slice(slice);
        let arg_val = u16::from_be_bytes(arr) as u64;
        if arg_val <= 0xff {
            Err(CanonError::NonMinimalInt {
                at: "Reader::read_arg_u16",
            })
        } else {
            Ok(arg_val)
        }
    }

    /// Reads a 32-bit argument with minimal encoding check.
    fn read_arg_u32(&mut self) -> Result<u64, CanonError> {
        let slice = self.take(4)?;
        let mut arr = [0u8; 4];
        arr.copy_from_slice(slice);
        let arg_val = u32::from_be_bytes(arr) as u64;
        if arg_val <= 0xffff {
            Err(CanonError::NonMinimalInt {
                at: "Reader::read_arg_u32",
            })
        } else {
            Ok(arg_val)
        }
    }

    /// Reads a 64-bit argument with minimal encoding check.
    fn read_arg_u64(&mut self) -> Result<u64, CanonError> {
        let slice = self.take(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(slice);
        let arg_val = u64::from_be_bytes(arr);
        if arg_val <= 0xffff_ffff {
            Err(CanonError::NonMinimalInt {
                at: "Reader::read_arg_u64",
            })
        } else {
            Ok(arg_val)
        }
    }

    /// Parses argument integer from head byte.
    fn arg(&mut self, head_byte: u8) -> Result<u64, CanonError> {
        let ai = head_byte & 0x1f;
        match ai {
            0..=23 => Ok(ai as u64),
            24 => self.read_arg_u8(),
            25 => self.read_arg_u16(),
            26 => self.read_arg_u32(),
            27 => self.read_arg_u64(),
            _ => Err(CanonError::ForbiddenForm {
                form: "indefinite/reserved head",
                at: "Reader::arg",
            }),
        }
    }

    /// Decodes UTF-8 text string value.
    fn decode_text(&mut self, arg_val: u64) -> Result<Value, CanonError> {
        let len = check_arg_as_usize(arg_val, "Reader::decode_text_overflow")?;
        let raw = self.take(len)?;
        match core::str::from_utf8(raw) {
            Ok(text_str) => Ok(Value::Text(text_str.to_string())),
            Err(_utf8_err) => Err(CanonError::Utf8 {
                at: "Reader::decode_text",
            }),
        }
    }

    /// Decodes ordered list of values.
    fn decode_list(&mut self, arg_val: u64, depth: usize) -> Result<Value, CanonError> {
        let count = check_arg_as_usize(arg_val, "Reader::decode_list_overflow")?;
        check_remaining_capacity(
            self.buf.len(),
            self.pos,
            count,
            "Reader::decode_list",
        )?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(self.value(depth + 1)?);
        }
        Ok(Value::List(items))
    }

    /// Reads a map key string.
    fn read_map_key(&mut self) -> Result<String, CanonError> {
        let key_head = self.byte()?;
        check_map_key_major(key_head)?;
        let key_len = self.arg(key_head)?;
        let key_usize = check_arg_as_usize(key_len, "Reader::read_map_key_len")?;
        let raw = self.take(key_usize)?;
        match core::str::from_utf8(raw) {
            Ok(key_str) => Ok(key_str.to_string()),
            Err(_utf8_err) => Err(CanonError::Utf8 {
                at: "Reader::read_map_key_utf8",
            }),
        }
    }

    /// Decodes a single map key-value entry.
    fn decode_map_entry(
        &mut self,
        prev: &mut Option<String>,
        depth: usize,
    ) -> Result<(String, Value), CanonError> {
        let key_str = self.read_map_key()?;
        check_map_key_order(prev, &key_str)?;
        let val = self.value(depth + 1)?;
        *prev = Some(key_str.clone());
        Ok((key_str, val))
    }

    /// Decodes map of key-value pairs.
    fn decode_map(&mut self, arg_val: u64, depth: usize) -> Result<Value, CanonError> {
        let count = check_map_capacity(self.buf.len(), self.pos, arg_val)?;
        let mut map = BTreeMap::new();
        let mut prev: Option<String> = None;
        for _ in 0..count {
            let (key_str, val) = self.decode_map_entry(&mut prev, depth)?;
            map.insert(key_str, val);
        }
        Ok(Value::Map(map))
    }

    /// Decodes scalar payloads (int, negint, bytes, text).
    fn decode_scalar_payload(&mut self, major: u8, arg_val: u64) -> Result<Value, CanonError> {
        match major {
            0 => decode_int_value(arg_val),
            1 => decode_negint_value(arg_val),
            2 => {
                let len = check_arg_as_usize(arg_val, "Reader::decode_bytes_overflow")?;
                Ok(Value::Bytes(self.take(len)?.to_vec()))
            }
            3 => self.decode_text(arg_val),
            _ => Err(CanonError::ForbiddenForm {
                form: "non-scalar major",
                at: "Reader::decode_scalar_payload",
            }),
        }
    }

    /// Decodes compound payloads (list, map).
    fn decode_compound_payload(
        &mut self,
        major: u8,
        arg_val: u64,
        depth: usize,
    ) -> Result<Value, CanonError> {
        match major {
            4 => self.decode_list(arg_val, depth),
            5 => self.decode_map(arg_val, depth),
            _ => Err(CanonError::ForbiddenForm {
                form: "forbidden major",
                at: "Reader::decode_compound_payload",
            }),
        }
    }

    /// Decodes major type payloads dispatching to scalar or compound decoders.
    fn decode_major_payload(
        &mut self,
        major: u8,
        arg_val: u64,
        depth: usize,
    ) -> Result<Value, CanonError> {
        match major {
            0..=3 => self.decode_scalar_payload(major, arg_val),
            4..=5 => self.decode_compound_payload(major, arg_val, depth),
            _ => Err(CanonError::ForbiddenForm {
                form: "tag or invalid major",
                at: "Reader::decode_major_payload",
            }),
        }
    }

    /// Parses a single value at the current position.
    fn value(&mut self, depth: usize) -> Result<Value, CanonError> {
        check_recursion_depth(depth)?;
        let head_byte = self.byte()?;
        let major = head_byte >> 5;
        if major == 7 {
            decode_simple_major7(head_byte)
        } else {
            let arg_val = self.arg(head_byte)?;
            self.decode_major_payload(major, arg_val, depth)
        }
    }
}

/// Verifies all bytes were consumed by the decoder.
fn check_all_bytes_consumed(reader_pos: usize, total_len: usize) -> Result<(), CanonError> {
    if reader_pos != total_len {
        Err(CanonError::TrailingBytes {
            at: "decode_strict",
        })
    } else {
        Ok(())
    }
}

/// Enforces the bijection guard by re-encoding and checking equality.
fn check_bijection_guard(parsed_val: &Value, expected_bytes: &[u8]) -> Result<(), CanonError> {
    if encode(parsed_val) != expected_bytes {
        Err(CanonError::NotBijective {
            at: "decode_strict",
        })
    } else {
        Ok(())
    }
}

/// Strict decode + bijection guard: decodes canonical bytes to a Value.
pub fn decode_strict(bytes: &[u8]) -> Result<Value, CanonError> {
    let mut reader = Reader { buf: bytes, pos: 0 };
    let parsed_val = reader.value(0)?;
    check_all_bytes_consumed(reader.pos, bytes.len())?;
    check_bijection_guard(&parsed_val, bytes)?;
    Ok(parsed_val)
}

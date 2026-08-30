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
pub const MAX_WIRE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_VALUE_NODES: u64 = 262_144;
pub const MAX_TEXT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_BYTE_STRING_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_LIST_ITEMS: u64 = 262_144;
pub const MAX_MAP_ENTRIES: u64 = 262_144;
pub const MAX_KEY_BYTES: u64 = 16 * 1024 * 1024;

/// Aggregate resource envelope enforced before canonical values are allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    WireBytes,
    ValueNodes,
    TextBytes,
    ByteStringBytes,
    ListItems,
    MapEntries,
    KeyBytes,
}

/// Counts established by the allocation-free canonical byte preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreflightStats {
    pub wire_bytes: usize,
    pub value_nodes: u64,
    pub text_bytes: u64,
    pub byte_string_bytes: u64,
    pub list_items: u64,
    pub map_entries: u64,
    pub key_bytes: u64,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Copy)]
struct PreflightLimits {
    wire_bytes: usize,
    value_nodes: u64,
    text_bytes: u64,
    byte_string_bytes: u64,
    list_items: u64,
    map_entries: u64,
    key_bytes: u64,
    depth: usize,
}

const DEFAULT_LIMITS: PreflightLimits = PreflightLimits {
    wire_bytes: MAX_WIRE_BYTES,
    value_nodes: MAX_VALUE_NODES,
    text_bytes: MAX_TEXT_BYTES,
    byte_string_bytes: MAX_BYTE_STRING_BYTES,
    list_items: MAX_LIST_ITEMS,
    map_entries: MAX_MAP_ENTRIES,
    key_bytes: MAX_KEY_BYTES,
    depth: MAX_DEPTH,
};

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
    /// A pre-allocation aggregate resource ceiling was exceeded.
    LimitExceeded {
        /// Resource whose aggregate ceiling was exceeded.
        kind: LimitKind,
        /// Observed or declared aggregate.
        actual: u64,
        /// Configured aggregate ceiling.
        limit: u64,
        /// Byte offset where the over-limit value was observed.
        at: usize,
    },
    /// Checked offset or counter arithmetic overflowed.
    ArithmeticOverflow {
        /// Byte offset where the overflow was detected.
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
            Self::LimitExceeded {
                kind,
                actual,
                limit,
                at,
            } => write!(
                f,
                "{kind:?} limit exceeded at offset {at}: {actual} > {limit}"
            ),
            Self::ArithmeticOverflow { at } => {
                write!(f, "checked arithmetic overflow at offset {at}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Canonical key order (length, then bytes) — restated here, not imported.
fn key_lt(a: &str, b: &str) -> bool {
    (a.len(), a.as_bytes()) < (b.len(), b.as_bytes())
}

fn key_bytes_lt(a: &[u8], b: &[u8]) -> bool {
    (a.len(), a) < (b.len(), b)
}

#[derive(Clone, Copy)]
enum ContainerKind {
    List,
    Map,
}

#[derive(Clone, Copy)]
struct Frame<'a> {
    kind: ContainerKind,
    remaining: u64,
    expecting_key: bool,
    previous_key: Option<&'a [u8]>,
}

const EMPTY_FRAME: Frame<'static> = Frame {
    kind: ContainerKind::List,
    remaining: 0,
    expecting_key: false,
    previous_key: None,
};

struct PreflightScanner<'a> {
    bytes: &'a [u8],
    offset: usize,
    stack: [Frame<'a>; MAX_DEPTH + 1],
    stack_len: usize,
    root_started: bool,
    stats: PreflightStats,
    limits: PreflightLimits,
}

impl<'a> PreflightScanner<'a> {
    fn new(bytes: &'a [u8], limits: PreflightLimits) -> Result<Self, DecodeError> {
        if bytes.len() > limits.wire_bytes {
            let limit = u64::try_from(limits.wire_bytes)
                .map_err(|_| DecodeError::ArithmeticOverflow { at: 0 })?;
            return Err(DecodeError::LimitExceeded {
                kind: LimitKind::WireBytes,
                actual: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                limit,
                at: 0,
            });
        }
        Ok(Self {
            bytes,
            offset: 0,
            stack: [EMPTY_FRAME; MAX_DEPTH + 1],
            stack_len: 0,
            root_started: false,
            stats: PreflightStats {
                wire_bytes: bytes.len(),
                value_nodes: 0,
                text_bytes: 0,
                byte_string_bytes: 0,
                list_items: 0,
                map_entries: 0,
                key_bytes: 0,
                max_depth: 0,
            },
            limits,
        })
    }

    fn read_u8(&mut self) -> Result<u8, DecodeError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(DecodeError::Truncated { at: self.offset })?;
        self.offset = self
            .offset
            .checked_add(1)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
        Ok(byte)
    }

    fn read_slice(&mut self, length: u64) -> Result<&'a [u8], DecodeError> {
        let length = usize::try_from(length)
            .map_err(|_| DecodeError::ArithmeticOverflow { at: self.offset })?;
        let end = self
            .offset
            .checked_add(length)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeError::Truncated { at: self.offset })?;
        self.offset = end;
        Ok(slice)
    }

    fn read_argument(&mut self, head: u8) -> Result<u64, DecodeError> {
        match head & 0x1f {
            inline @ 0..=23 => Ok(u64::from(inline)),
            24 => {
                let value = u64::from(self.read_u8()?);
                if value < 24 {
                    Err(DecodeError::NonMinimal { at: self.offset })
                } else {
                    Ok(value)
                }
            }
            25 => {
                let bytes = self.read_slice(2)?;
                let value = u64::from(u16::from_be_bytes(bytes.try_into().unwrap()));
                if value <= u64::from(u8::MAX) {
                    Err(DecodeError::NonMinimal { at: self.offset })
                } else {
                    Ok(value)
                }
            }
            26 => {
                let bytes = self.read_slice(4)?;
                let value = u64::from(u32::from_be_bytes(bytes.try_into().unwrap()));
                if value <= u64::from(u16::MAX) {
                    Err(DecodeError::NonMinimal { at: self.offset })
                } else {
                    Ok(value)
                }
            }
            27 => {
                let bytes = self.read_slice(8)?;
                let value = u64::from_be_bytes(bytes.try_into().unwrap());
                if value <= u64::from(u32::MAX) {
                    Err(DecodeError::NonMinimal { at: self.offset })
                } else {
                    Ok(value)
                }
            }
            _ => Err(DecodeError::Forbidden {
                at: self.offset,
                reason: "reserved/indefinite",
            }),
        }
    }

    fn add_counter(
        counter: &mut u64,
        amount: u64,
        limit: u64,
        kind: LimitKind,
        at: usize,
    ) -> Result<(), DecodeError> {
        let actual = counter
            .checked_add(amount)
            .ok_or(DecodeError::ArithmeticOverflow { at })?;
        if actual > limit {
            return Err(DecodeError::LimitExceeded {
                kind,
                actual,
                limit,
                at,
            });
        }
        *counter = actual;
        Ok(())
    }

    fn record_value(&mut self, depth: usize, at: usize) -> Result<(), DecodeError> {
        if depth > self.limits.depth {
            return Err(DecodeError::Depth { at });
        }
        Self::add_counter(
            &mut self.stats.value_nodes,
            1,
            self.limits.value_nodes,
            LimitKind::ValueNodes,
            at,
        )?;
        self.stats.max_depth = self.stats.max_depth.max(depth);
        Ok(())
    }

    fn ensure_minimum_payload(&self, count: u64, bytes_per_item: u64) -> Result<(), DecodeError> {
        let minimum = count
            .checked_mul(bytes_per_item)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
        let remaining_bytes = self
            .bytes
            .len()
            .checked_sub(self.offset)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
        let remaining = u64::try_from(remaining_bytes)
            .map_err(|_| DecodeError::ArithmeticOverflow { at: self.offset })?;
        if minimum > remaining {
            Err(DecodeError::Truncated { at: self.offset })
        } else {
            Ok(())
        }
    }

    fn push_container(&mut self, kind: ContainerKind, count: u64) -> Result<(), DecodeError> {
        if count == 0 {
            return Ok(());
        }
        let slot = self
            .stack
            .get_mut(self.stack_len)
            .ok_or(DecodeError::Depth { at: self.offset })?;
        *slot = Frame {
            kind,
            remaining: count,
            expecting_key: matches!(kind, ContainerKind::Map),
            previous_key: None,
        };
        self.stack_len = self
            .stack_len
            .checked_add(1)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
        Ok(())
    }

    fn scan_text_payload(&mut self, length: u64, key: bool) -> Result<&'a [u8], DecodeError> {
        let at = self.offset;
        if key {
            Self::add_counter(
                &mut self.stats.key_bytes,
                length,
                self.limits.key_bytes,
                LimitKind::KeyBytes,
                at,
            )?;
        } else {
            Self::add_counter(
                &mut self.stats.text_bytes,
                length,
                self.limits.text_bytes,
                LimitKind::TextBytes,
                at,
            )?;
        }
        let payload = self.read_slice(length)?;
        if let Err(error) = std::str::from_utf8(payload) {
            let invalid_at = at
                .checked_add(error.valid_up_to())
                .ok_or(DecodeError::ArithmeticOverflow { at })?;
            return Err(DecodeError::Utf8 { at: invalid_at });
        }
        Ok(payload)
    }

    fn scan_key(&mut self) -> Result<&'a [u8], DecodeError> {
        let at = self.offset;
        let head = self.read_u8()?;
        if head >> 5 != 3 {
            return Err(DecodeError::Forbidden {
                at,
                reason: "map key",
            });
        }
        let length = self.read_argument(head)?;
        self.scan_text_payload(length, true)
    }

    fn scan_value(&mut self, depth: usize) -> Result<(), DecodeError> {
        let at = self.offset;
        self.record_value(depth, at)?;
        let head = self.read_u8()?;
        let major = head >> 5;
        if major == 7 {
            return match head {
                0xf4 | 0xf5 => Ok(()),
                _ => Err(DecodeError::Forbidden {
                    at,
                    reason: "simple/float",
                }),
            };
        }

        let argument = self.read_argument(head)?;
        match major {
            0 => {
                i64::try_from(argument).map_err(|_| DecodeError::IntRange { at })?;
                Ok(())
            }
            1 => {
                i64::try_from(argument).map_err(|_| DecodeError::IntRange { at })?;
                Ok(())
            }
            2 => {
                Self::add_counter(
                    &mut self.stats.byte_string_bytes,
                    argument,
                    self.limits.byte_string_bytes,
                    LimitKind::ByteStringBytes,
                    at,
                )?;
                self.read_slice(argument)?;
                Ok(())
            }
            3 => {
                self.scan_text_payload(argument, false)?;
                Ok(())
            }
            4 => {
                Self::add_counter(
                    &mut self.stats.list_items,
                    argument,
                    self.limits.list_items,
                    LimitKind::ListItems,
                    at,
                )?;
                self.ensure_minimum_payload(argument, 1)?;
                self.push_container(ContainerKind::List, argument)
            }
            5 => {
                Self::add_counter(
                    &mut self.stats.map_entries,
                    argument,
                    self.limits.map_entries,
                    LimitKind::MapEntries,
                    at,
                )?;
                self.ensure_minimum_payload(argument, 2)?;
                self.push_container(ContainerKind::Map, argument)
            }
            _ => Err(DecodeError::Forbidden {
                at,
                reason: "tag/unknown",
            }),
        }
    }

    fn pop_completed(&mut self) {
        while self.stack_len > 0 {
            let frame = self.stack[self.stack_len - 1];
            let complete = frame.remaining == 0
                && (!matches!(frame.kind, ContainerKind::Map) || frame.expecting_key);
            if !complete {
                break;
            }
            self.stack_len -= 1;
        }
    }

    fn scan(mut self) -> Result<PreflightStats, DecodeError> {
        loop {
            self.pop_completed();
            if self.root_started && self.stack_len == 0 {
                break;
            }

            if self.stack_len > 0 {
                let frame_index = self.stack_len - 1;
                let frame = self.stack[frame_index];
                if matches!(frame.kind, ContainerKind::Map) && frame.expecting_key {
                    let key = self.scan_key()?;
                    if let Some(previous) = frame.previous_key
                        && !key_bytes_lt(previous, key)
                    {
                        return Err(DecodeError::KeyOrder { at: self.offset });
                    }
                    self.stack[frame_index].previous_key = Some(key);
                    self.stack[frame_index].expecting_key = false;
                    continue;
                }

                self.stack[frame_index].remaining = frame
                    .remaining
                    .checked_sub(1)
                    .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
                if matches!(frame.kind, ContainerKind::Map) {
                    self.stack[frame_index].expecting_key = true;
                }
            } else {
                self.root_started = true;
            }

            self.scan_value(self.stack_len)?;
        }

        if self.offset != self.bytes.len() {
            return Err(DecodeError::Trailing { at: self.offset });
        }
        Ok(self.stats)
    }
}

fn preflight_with_limits(
    bytes: &[u8],
    limits: PreflightLimits,
) -> Result<PreflightStats, DecodeError> {
    PreflightScanner::new(bytes, limits)?.scan()
}

/// Validate canonical bytes and all aggregate resource ceilings without heap
/// allocation. Only borrowed slices, scalar counters, and a fixed-size stack
/// are used before this function returns successfully.
pub fn preflight_canonical(bytes: &[u8]) -> Result<PreflightStats, DecodeError> {
    preflight_with_limits(bytes, DEFAULT_LIMITS)
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
        self.offset = self
            .offset
            .checked_add(1)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
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
        usize::try_from(length).map_err(|_| DecodeError::ArithmeticOverflow { at: self.offset })
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
                at: current_at
                    .checked_add(utf_err.valid_up_to())
                    .ok_or(DecodeError::ArithmeticOverflow { at: current_at })?,
            }),
        }
    }

    fn check_list_bounds(&self, count: u64) -> Result<(), DecodeError> {
        let remaining = self
            .buffer
            .len()
            .checked_sub(self.offset)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
        let remaining_bytes = u64::try_from(remaining)
            .map_err(|_| DecodeError::ArithmeticOverflow { at: self.offset })?;
        if count > remaining_bytes {
            Err(DecodeError::Truncated { at: self.offset })
        } else {
            Ok(())
        }
    }

    fn read_list(&mut self, count: u64, depth: usize) -> Result<Value, DecodeError> {
        self.check_list_bounds(count)?;
        let capacity = usize::try_from(count)
            .map_err(|_| DecodeError::ArithmeticOverflow { at: self.offset })?;
        let mut list_items = Vec::with_capacity(capacity);
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
        let remaining = self
            .buffer
            .len()
            .checked_sub(self.offset)
            .ok_or(DecodeError::ArithmeticOverflow { at: self.offset })?;
        let remaining_pairs = u64::try_from(remaining / 2)
            .map_err(|_| DecodeError::ArithmeticOverflow { at: self.offset })?;
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

fn check_cursor_at_end(offset: usize, total_len: usize) -> Result<(), DecodeError> {
    if offset != total_len {
        Err(DecodeError::Trailing { at: offset })
    } else {
        Ok(())
    }
}

/// Allocation-free canonical preflight followed by bounded owned projection.
///
/// The preflight validates the exact byte grammar, minimal integer widths,
/// borrowed map-key order, and complete input consumption. The owned decoder
/// then independently parses the same bytes. This establishes byte bijection
/// without allocating a second full-size encoded buffer.
pub fn decode_canonical(bytes: &[u8]) -> Result<Value, DecodeError> {
    preflight_canonical(bytes)?;
    let mut cursor = Cursor {
        buffer: bytes,
        offset: 0,
    };
    let decoded_val = cursor.read_val(0)?;
    check_cursor_at_end(cursor.offset, bytes.len())?;
    Ok(decoded_val)
}

#[cfg(test)]
mod preflight_tests {
    use super::*;

    fn test_limits() -> PreflightLimits {
        PreflightLimits {
            wire_bytes: 1024,
            value_nodes: 64,
            text_bytes: 64,
            byte_string_bytes: 64,
            list_items: 64,
            map_entries: 64,
            key_bytes: 64,
            depth: 8,
        }
    }

    #[test]
    fn counts_every_resource_without_materializing_values() {
        // {"a": [h'ff', "hi"], "b": 1}
        let bytes = [
            0xa2, 0x61, b'a', 0x82, 0x41, 0xff, 0x62, b'h', b'i', 0x61, b'b', 0x01,
        ];
        let stats = preflight_with_limits(&bytes, test_limits()).unwrap();

        assert_eq!(stats.wire_bytes, bytes.len());
        assert_eq!(stats.value_nodes, 5);
        assert_eq!(stats.text_bytes, 2);
        assert_eq!(stats.byte_string_bytes, 1);
        assert_eq!(stats.list_items, 2);
        assert_eq!(stats.map_entries, 2);
        assert_eq!(stats.key_bytes, 2);
        assert_eq!(stats.max_depth, 2);
    }

    #[test]
    fn many_tiny_values_hit_the_node_budget() {
        let mut limits = test_limits();
        limits.value_nodes = 4;
        assert!(matches!(
            preflight_with_limits(&[0x84, 0xf4, 0xf4, 0xf4, 0xf4], limits),
            Err(DecodeError::LimitExceeded {
                kind: LimitKind::ValueNodes,
                actual: 5,
                limit: 4,
                ..
            })
        ));
    }

    #[test]
    fn aggregate_text_budget_spans_multiple_valid_items() {
        let mut limits = test_limits();
        limits.text_bytes = 3;
        assert!(matches!(
            preflight_with_limits(&[0x82, 0x62, b'a', b'a', 0x62, b'b', b'b'], limits),
            Err(DecodeError::LimitExceeded {
                kind: LimitKind::TextBytes,
                actual: 4,
                limit: 3,
                ..
            })
        ));
    }

    #[test]
    fn aggregate_byte_string_budget_spans_multiple_valid_items() {
        let mut limits = test_limits();
        limits.byte_string_bytes = 3;
        assert!(matches!(
            preflight_with_limits(&[0x82, 0x42, 1, 2, 0x42, 3, 4], limits),
            Err(DecodeError::LimitExceeded {
                kind: LimitKind::ByteStringBytes,
                actual: 4,
                limit: 3,
                ..
            })
        ));
    }

    #[test]
    fn nested_lists_hit_the_aggregate_item_budget() {
        let mut limits = test_limits();
        limits.list_items = 5;
        assert!(matches!(
            preflight_with_limits(&[0x82, 0x82, 0xf5, 0xf5, 0x82, 0xf5, 0xf5], limits),
            Err(DecodeError::LimitExceeded {
                kind: LimitKind::ListItems,
                actual: 6,
                limit: 5,
                ..
            })
        ));
    }

    #[test]
    fn aggregate_map_entries_and_key_bytes_have_separate_budgets() {
        let bytes = [0xa2, 0x61, b'a', 0x01, 0x61, b'b', 0x02];
        let mut entry_limits = test_limits();
        entry_limits.map_entries = 1;
        assert!(matches!(
            preflight_with_limits(&bytes, entry_limits),
            Err(DecodeError::LimitExceeded {
                kind: LimitKind::MapEntries,
                actual: 2,
                limit: 1,
                ..
            })
        ));

        let mut key_limits = test_limits();
        key_limits.key_bytes = 1;
        assert!(matches!(
            preflight_with_limits(&bytes, key_limits),
            Err(DecodeError::LimitExceeded {
                kind: LimitKind::KeyBytes,
                actual: 2,
                limit: 1,
                ..
            })
        ));
    }

    #[test]
    fn borrowed_map_keys_must_be_canonically_ordered() {
        let unordered = [0xa2, 0x61, b'b', 0x01, 0x61, b'a', 0x02];
        assert!(matches!(
            preflight_with_limits(&unordered, test_limits()),
            Err(DecodeError::KeyOrder { .. })
        ));
    }

    #[test]
    fn nonempty_container_beyond_depth_limit_is_refused() {
        let mut limits = test_limits();
        limits.depth = 2;
        assert!(matches!(
            preflight_with_limits(&[0x81, 0x81, 0x81, 0xf5], limits),
            Err(DecodeError::Depth { .. })
        ));
    }

    #[test]
    fn hostile_declared_count_is_refused_before_iteration() {
        let mut bytes = vec![0x9b];
        bytes.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(matches!(
            preflight_canonical(&bytes),
            Err(DecodeError::LimitExceeded {
                kind: LimitKind::ListItems,
                actual: u64::MAX,
                limit: MAX_LIST_ITEMS,
                ..
            })
        ));
    }

    #[test]
    fn truncated_payload_is_refused_by_checked_slice_bounds() {
        assert!(matches!(
            preflight_with_limits(&[0x44, 1, 2], test_limits()),
            Err(DecodeError::Truncated { .. })
        ));
    }
}

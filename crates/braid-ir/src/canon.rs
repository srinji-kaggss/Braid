//! Canonical encoding: a deterministic CBOR subset (D8).
//!
//! One value ⇒ exactly one byte string; one byte string ⇒ at most one value.
//! The strict decoder rejects every non-canonical form *and* `decode_strict`
//! additionally re-encodes and compares as a belt-and-braces **bijection
//! guard** (threat T3 — the A4.8 governance-ledger byte-malleability lesson:
//! "signatures verified" is worthless if the bytes are malleable).
//!
//! Subset rules (reject everything else, fail-closed L9):
//! - majors 0/1 (ints, minimal-length heads only), 2 (bytes), 3 (UTF-8 text),
//!   4 (array), 5 (map), and ONLY simple values true/false from major 7.
//! - NO floats, NO tags (major 6), NO indefinite lengths, NO null/undefined.
//! - map keys: text only, strictly increasing in length-then-bytewise order
//!   (RFC 8949 deterministic order for definite-length text keys), no dups.
//! - no trailing bytes; nesting depth capped.

use crate::value::Value;
use std::collections::BTreeMap;

/// Maximum nesting depth (fail-closed resource bound, not a tunable).
pub const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonError {
    /// Input ended early / declared length exceeds remaining bytes.
    Truncated,
    /// A head/feature outside the subset (float, tag, indefinite, null, …).
    ForbiddenForm(&'static str),
    /// An integer head that is not the minimal-length encoding.
    NonMinimalInt,
    /// Map keys out of canonical order or duplicated.
    KeyOrder,
    /// Text bytes are not valid UTF-8.
    Utf8,
    /// Bytes remain after the single top-level value.
    TrailingBytes,
    /// Nesting exceeds [`MAX_DEPTH`].
    DepthExceeded,
    /// Decoded value re-encoded to different bytes (bijection guard).
    NotBijective,
    /// Integer outside i64 range.
    IntRange,
}

/// Canonical key order: length first, then bytewise — equals RFC 8949
/// deterministic encoding order for definite-length text keys (the length is
/// in the head byte(s), so encoded-byte comparison sees it first).
pub fn key_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.as_bytes().cmp(b.as_bytes()))
}

// ───────────────────────────── encoder ─────────────────────────────

fn put_head(out: &mut Vec<u8>, major: u8, n: u64) {
    let m = major << 5;
    match n {
        0..=23 => out.push(m | n as u8),
        24..=0xff => {
            out.push(m | 24);
            out.push(n as u8);
        }
        0x100..=0xffff => {
            out.push(m | 25);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(m | 26);
            out.extend_from_slice(&(n as u32).to_be_bytes());
        }
        _ => {
            out.push(m | 27);
            out.extend_from_slice(&n.to_be_bytes());
        }
    }
}

fn encode_into(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Int(i) => {
            if *i >= 0 {
                put_head(out, 0, *i as u64);
            } else {
                // CBOR major 1 encodes -1 - n.
                put_head(out, 1, (-1i128 - *i as i128) as u64);
            }
        }
        Value::Bytes(b) => {
            put_head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Value::Text(s) => {
            put_head(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Value::List(items) => {
            put_head(out, 4, items.len() as u64);
            for it in items {
                encode_into(it, out);
            }
        }
        Value::Map(m) => {
            put_head(out, 5, m.len() as u64);
            // //why re-sort here instead of trusting BTreeMap: BTreeMap orders
            // plain-bytewise ("aa" < "z"); canonical order is length-first
            // ("z" < "aa"). The boundary owns the order so storage can't.
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| key_cmp(a, b));
            for k in keys {
                put_head(out, 3, k.len() as u64);
                out.extend_from_slice(k.as_bytes());
                encode_into(&m[k], out);
            }
        }
    }
}

/// Encode a value to its single canonical byte form.
pub fn encode(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(v, &mut out);
    out
}

// ───────────────────────────── strict decoder ─────────────────────────────

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, CanonError> {
        let b = *self.buf.get(self.pos).ok_or(CanonError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], CanonError> {
        let end = self.pos.checked_add(n).ok_or(CanonError::Truncated)?;
        if end > self.buf.len() {
            return Err(CanonError::Truncated);
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    /// Read the argument of an already-consumed head byte, enforcing
    /// minimal-length integer encoding. Never called for major 7 — simple
    /// values carry no argument in the subset.
    fn arg(&mut self, b: u8) -> Result<u64, CanonError> {
        let ai = b & 0x1f;
        let n = match ai {
            0..=23 => ai as u64,
            24 => {
                let v = self.byte()? as u64;
                if v < 24 {
                    return Err(CanonError::NonMinimalInt);
                }
                v
            }
            25 => {
                let v = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as u64;
                if v <= 0xff {
                    return Err(CanonError::NonMinimalInt);
                }
                v
            }
            26 => {
                let v = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as u64;
                if v <= 0xffff {
                    return Err(CanonError::NonMinimalInt);
                }
                v
            }
            27 => {
                let v = u64::from_be_bytes(self.take(8)?.try_into().unwrap());
                if v <= 0xffff_ffff {
                    return Err(CanonError::NonMinimalInt);
                }
                v
            }
            // 28..=30 reserved, 31 = indefinite — all outside the subset.
            _ => return Err(CanonError::ForbiddenForm("indefinite/reserved head")),
        };
        Ok(n)
    }

    fn value(&mut self, depth: usize) -> Result<Value, CanonError> {
        if depth > MAX_DEPTH {
            return Err(CanonError::DepthExceeded);
        }
        let b = self.byte()?;
        let major = b >> 5;
        // Major 7 carries no argument in the subset: classify the head byte
        // itself so a float head is a FLOAT error, not an int-encoding one.
        if major == 7 {
            return match b {
                0xf4 => Ok(Value::Bool(false)),
                0xf5 => Ok(Value::Bool(true)),
                _ => Err(CanonError::ForbiddenForm("float/null/simple")),
            };
        }
        let n = self.arg(b)?;
        match major {
            0 => {
                if n > i64::MAX as u64 {
                    return Err(CanonError::IntRange);
                }
                Ok(Value::Int(n as i64))
            }
            1 => {
                // value = -1 - n; representable iff n <= i64::MAX.
                if n > i64::MAX as u64 {
                    return Err(CanonError::IntRange);
                }
                Ok(Value::Int(-1i64 - n as i64))
            }
            2 => Ok(Value::Bytes(self.take(n as usize)?.to_vec())),
            3 => {
                let raw = self.take(n as usize)?;
                let s = std::str::from_utf8(raw).map_err(|_| CanonError::Utf8)?;
                Ok(Value::Text(s.to_string()))
            }
            4 => {
                // Each element occupies ≥1 byte: a declared count beyond the
                // remaining bytes is a forged-length alloc bomb — reject before
                // reserving anything.
                if n as usize > self.buf.len() - self.pos {
                    return Err(CanonError::Truncated);
                }
                let mut items = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    items.push(self.value(depth + 1)?);
                }
                Ok(Value::List(items))
            }
            5 => {
                if (n as usize) > (self.buf.len() - self.pos) / 2 {
                    return Err(CanonError::Truncated);
                }
                let mut map = BTreeMap::new();
                let mut prev: Option<String> = None;
                for _ in 0..n {
                    let kb = self.byte()?;
                    if kb >> 5 != 3 {
                        return Err(CanonError::ForbiddenForm("non-text map key"));
                    }
                    let kn = self.arg(kb)?;
                    let raw = self.take(kn as usize)?;
                    let k = std::str::from_utf8(raw)
                        .map_err(|_| CanonError::Utf8)?
                        .to_string();
                    if let Some(p) = &prev {
                        if key_cmp(p, &k) != std::cmp::Ordering::Less {
                            return Err(CanonError::KeyOrder);
                        }
                    }
                    let v = self.value(depth + 1)?;
                    prev = Some(k.clone());
                    map.insert(k, v);
                }
                Ok(Value::Map(map))
            }
            6 => Err(CanonError::ForbiddenForm("tag")),
            // Major 7 handled before arg parsing; 3-bit major is exhaustive.
            _ => unreachable!("major is 3 bits"),
        }
    }
}

/// Strict decode + **bijection guard**: the decoded value must re-encode to
/// the exact input bytes, else the input is rejected as non-canonical even if
/// it parsed. //why both (strictness should already imply it): the guard is
/// the *independent* check that survives a decoder bug — mutation-tested by
/// `tests/malleability.rs`.
pub fn decode_strict(bytes: &[u8]) -> Result<Value, CanonError> {
    let mut r = Reader { buf: bytes, pos: 0 };
    let v = r.value(0)?;
    if r.pos != bytes.len() {
        return Err(CanonError::TrailingBytes);
    }
    if encode(&v) != bytes {
        return Err(CanonError::NotBijective);
    }
    Ok(v)
}

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

pub const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    Forbidden(&'static str),
    NonMinimal,
    KeyOrder,
    Utf8,
    Trailing,
    Depth,
    NotBijective,
    IntRange,
}

/// Canonical key order (length, then bytes) — restated here, not imported.
fn key_lt(a: &str, b: &str) -> bool {
    (a.len(), a.as_bytes()) < (b.len(), b.as_bytes())
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Result<u8, DecodeError> {
        let v = *self.b.get(self.i).ok_or(DecodeError::Truncated)?;
        self.i += 1;
        Ok(v)
    }

    fn slice(&mut self, n: u64) -> Result<&'a [u8], DecodeError> {
        let n = usize::try_from(n).map_err(|_| DecodeError::Truncated)?;
        let end = self.i.checked_add(n).ok_or(DecodeError::Truncated)?;
        if end > self.b.len() {
            return Err(DecodeError::Truncated);
        }
        let s = &self.b[self.i..end];
        self.i = end;
        Ok(s)
    }

    fn arg(&mut self, first: u8) -> Result<u64, DecodeError> {
        let ai = first & 0x1f;
        match ai {
            0..=23 => Ok(ai as u64),
            24 => {
                let v = self.u8()? as u64;
                (v >= 24).then_some(v).ok_or(DecodeError::NonMinimal)
            }
            25 => {
                let v = u16::from_be_bytes(self.slice(2)?.try_into().unwrap()) as u64;
                (v > 0xff).then_some(v).ok_or(DecodeError::NonMinimal)
            }
            26 => {
                let v = u32::from_be_bytes(self.slice(4)?.try_into().unwrap()) as u64;
                (v > 0xffff).then_some(v).ok_or(DecodeError::NonMinimal)
            }
            27 => {
                let v = u64::from_be_bytes(self.slice(8)?.try_into().unwrap());
                (v > 0xffff_ffff)
                    .then_some(v)
                    .ok_or(DecodeError::NonMinimal)
            }
            _ => Err(DecodeError::Forbidden("reserved/indefinite")),
        }
    }

    fn text(&mut self, n: u64) -> Result<String, DecodeError> {
        std::str::from_utf8(self.slice(n)?)
            .map(str::to_owned)
            .map_err(|_| DecodeError::Utf8)
    }

    fn val(&mut self, depth: usize) -> Result<Value, DecodeError> {
        if depth > MAX_DEPTH {
            return Err(DecodeError::Depth);
        }
        let first = self.u8()?;
        let major = first >> 5;
        if major == 7 {
            // Only true/false from major 7 — heads 0xf4/0xf5 carry no argument.
            return match first {
                0xf4 => Ok(Value::Bool(false)),
                0xf5 => Ok(Value::Bool(true)),
                _ => Err(DecodeError::Forbidden("simple/float")),
            };
        }
        let n = self.arg(first)?;
        match major {
            0 => i64::try_from(n)
                .map(Value::Int)
                .map_err(|_| DecodeError::IntRange),
            1 => i64::try_from(n)
                .map(|x| Value::Int(-1 - x))
                .map_err(|_| DecodeError::IntRange),
            2 => Ok(Value::Bytes(self.slice(n)?.to_vec())),
            3 => Ok(Value::Text(self.text(n)?)),
            4 => {
                if n > (self.b.len() - self.i) as u64 {
                    return Err(DecodeError::Truncated);
                }
                let mut out = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    out.push(self.val(depth + 1)?);
                }
                Ok(Value::List(out))
            }
            5 => {
                if n > ((self.b.len() - self.i) / 2) as u64 {
                    return Err(DecodeError::Truncated);
                }
                let mut m = BTreeMap::new();
                let mut last: Option<String> = None;
                for _ in 0..n {
                    let kh = self.u8()?;
                    if kh >> 5 != 3 {
                        return Err(DecodeError::Forbidden("map key"));
                    }
                    let kn = self.arg(kh)?;
                    let k = self.text(kn)?;
                    if let Some(p) = &last {
                        if !key_lt(p, &k) {
                            return Err(DecodeError::KeyOrder);
                        }
                    }
                    let v = self.val(depth + 1)?;
                    last = Some(k.clone());
                    m.insert(k, v);
                }
                Ok(Value::Map(m))
            }
            6 => Err(DecodeError::Forbidden("tag")),
            _ => unreachable!(),
        }
    }
}

// ── independent re-encoder for the bijection check ──

fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.extend_from_slice(&[m | 24, n as u8]);
    } else if n <= 0xffff {
        out.push(m | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

pub fn reencode(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Value::Int(i) if *i >= 0 => head(out, 0, *i as u64),
        Value::Int(i) => head(out, 1, (-1i128 - *i as i128) as u64),
        Value::Bytes(b) => {
            head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Value::Text(s) => {
            head(out, 3, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Value::List(items) => {
            head(out, 4, items.len() as u64);
            for i in items {
                reencode(i, out);
            }
        }
        Value::Map(m) => {
            head(out, 5, m.len() as u64);
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by_key(|k| (k.len(), k.as_bytes()));
            for k in keys {
                head(out, 3, k.len() as u64);
                out.extend_from_slice(k.as_bytes());
                reencode(&m[k], out);
            }
        }
    }
}

/// Strict decode + independent bijection guard.
pub fn decode_canonical(bytes: &[u8]) -> Result<Value, DecodeError> {
    let mut c = Cursor { b: bytes, i: 0 };
    let v = c.val(0)?;
    if c.i != bytes.len() {
        return Err(DecodeError::Trailing);
    }
    let mut re = Vec::with_capacity(bytes.len());
    reencode(&v, &mut re);
    if re != bytes {
        return Err(DecodeError::NotBijective);
    }
    Ok(v)
}

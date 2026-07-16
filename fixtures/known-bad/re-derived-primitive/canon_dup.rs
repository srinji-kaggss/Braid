//! Re-derived primitive — SLOP FIXTURE (do not ship).
//!
//! This is a COPY of `canon.rs` encode logic with a deliberate divergence:
//! map keys are emitted in BTreeMap (bytewise) order instead of RFC 8949
//! length-first order. This is the exact "second authority system" D9/D11
//! forbid — a re-derived canonical encoder that disagrees on byte form.
//!
//! When this module is present, the test below goes RED: the two encoders
//! disagree on multi-key maps. This proves the canonical form has one
//! authority, not two.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::value::Value;

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

/// The re-derived encoder — uses BTreeMap order (bytewise), NOT length-first.
pub fn encode_v2(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into_v2(v, &mut out);
    out
}

fn encode_into_v2(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Int(i) => {
            if *i >= 0 {
                put_head(out, 0, *i as u64);
            } else {
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
                encode_into_v2(it, out);
            }
        }
        Value::Map(m) => {
            put_head(out, 5, m.len() as u64);
            // BUG: BTreeMap iteration order is bytewise, not length-first.
            // This is the "slight difference" that IS the bug.
            for (k, v) in m.iter() {
                put_head(out, 3, k.len() as u64);
                out.extend_from_slice(k.as_bytes());
                encode_into_v2(v, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::encode_v2;
    use crate::canon::encode;
    use crate::value::Value;
    use alloc::collections::BTreeMap;
    use alloc::string::String;

    #[test]
    fn re_derived_encoder_matches_canonical() {
        // Multi-key map where length-first and bytewise orderings differ.
        let mut m = BTreeMap::new();
        m.insert(String::from("z"), Value::Int(1));
        m.insert(String::from("aa"), Value::Int(2));
        m.insert(String::from("a"), Value::Int(3));
        let v = Value::Map(m);

        let canonical = encode(&v);
        let re_derived = encode_v2(&v);

        // If these agree, there is one canonical form.
        // If they disagree, there are two — the D9/D11 violation.
        assert_eq!(
            canonical, re_derived,
            "re-derived encoder disagrees with canonical on multi-key map \
             — a second authority system exists (D9/D11 violation)"
        );
    }
}

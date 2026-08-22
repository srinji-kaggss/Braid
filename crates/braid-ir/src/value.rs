//! The closed value universe of the IR (PRD §4.1).
//!
//! //why no float variant exists at all (D8/T8): IEEE floats break canonical
//! encoding (NaN payloads, -0.0) and cross-platform replay. Fixed-point lives
//! in `Int` with term-level scaling. The absence of the variant — not a
//! runtime check — is what makes a float unrepresentable.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A value in the Braid IR. Closed; every variant has exactly one canonical
/// byte form under [`crate::canon`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    /// Fixed-point integer (scaling declared by the consuming term's spec).
    Int(i64),
    Bytes(Vec<u8>),
    Text(String),
    List(Vec<Value>),
    /// String-keyed map. Stored deduplicated; canonical emission order is
    /// length-then-bytewise on the key (see `canon::key_cmp`), NOT `BTreeMap`'s
    /// plain bytewise order — the encoder re-sorts at the boundary.
    Map(BTreeMap<String, Value>),
}

impl Value {
    /// Convenience constructor for map literals in builders/tests.
    pub fn map(entries: Vec<(&str, Value)>) -> Value {
        Value::Map(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    /// Typed field access for struct decoding (missing key = error at the
    /// caller; absence is never defaulted — fail-closed L9).
    pub fn get_field<'a>(&'a self, key: &str) -> Option<&'a Value> {
        match self {
            Value::Map(m) => m.get(key),
            _ => None,
        }
    }

    /// Reject the value unless it is a map whose keys are all drawn from
    /// `allowed`. //why this is a security check, not a convenience: a
    /// struct projection that silently ignores unknown keys lets distinct
    /// byte strings collapse to one struct — the bytes↔Value bijection guard
    /// passes (the Value keeps the extra key) while Value→struct drops it, so
    /// the CID commits to the re-encoded projection, NOT the admitted bytes.
    /// That is byte-malleability below the top level (the A4.8 / D8 lesson).
    /// Every nested map in the IR must call this.
    pub fn require_only_keys(&self, allowed: &[&str]) -> bool {
        match self {
            Value::Map(m) => m.keys().all(|k| allowed.contains(&k.as_str())),
            _ => false,
        }
    }
}

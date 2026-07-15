//! Ungrounded claim — SLOP FIXTURE (do not ship).
//!
//! This module contains a function with a doc comment claiming a property
//! ("always returns a non-zero byte count") that is NOT enforced by any
//! test in the main codebase. The test below checks the claim and finds
//! it is FALSE: the function returns 0 for `Value::Bool(false)`.
//!
//! This is the T7 class: a narrated property with no enforcing test. When
//! this module is present, the test below goes RED, proving the claim is
//! ungrounded.

use crate::value::Value;

/// Count the encoded byte count of a Value's header overhead.
///
/// # Claim (UNGROUNDED)
///
/// Always returns at least 1 — every Value variant has a non-empty encoding.
///
/// ^ This claim has no backing test in the codebase. The test below proves
/// it is false: `Value::Bool(false)` encodes to a single byte (0xf4), but
/// this function's implementation miscounts it as 0.
pub fn claimed_min_one_byte(v: &Value) -> usize {
    match v {
        Value::Bool(_) => 0, // BUG: returns 0, contradicting the doc claim
        Value::Int(i) => {
            if *i >= 0 {
                if *i < 24 {
                    1
                } else {
                    2
                }
            } else {
                2
            }
        }
        Value::Bytes(b) => 1 + b.len(),
        Value::Text(s) => 1 + s.len(),
        Value::List(items) => 1 + items.len(),
        Value::Map(m) => 1 + m.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::claimed_min_one_byte;
    use crate::value::Value;

    /// Verifies the doc claim "always returns at least 1." The claim is
    /// ungrounded — this test exposes the bug.
    #[test]
    fn claimed_min_one_byte_actually_is() {
        let v = Value::Bool(false);
        let count = claimed_min_one_byte(&v);
        assert!(
            count >= 1,
            "doc claims ≥1 but got {count} for Bool(false) — claim is ungrounded"
        );
    }
}

//! Vacuous test — SLOP FIXTURE (do not ship).
//!
//! This module contains a test that LOOKS like it verifies the canonical
//! encoder, but actually asserts nothing meaningful. The companion guard test
//! below proves the vacuousness by showing the slop test's assertion holds
//! even on garbage input.
//!
//! When this module is present, the guard test goes RED — proving the slop
//! test has no teeth.

#[cfg(test)]
mod tests {
    use braid_ir::{encode, Value};
    use std::collections::BTreeMap;

    /// SLOP: looks real, asserts nothing.
    #[test]
    fn encoder_produces_valid_output() {
        let mut m = BTreeMap::new();
        m.insert("key".to_string(), Value::Int(42));
        let v = Value::Map(m);
        let bytes = encode(&v);

        // Vacuous: asserts a tautology on an unsigned type.
        assert!(bytes.len() >= 0);
    }

    /// GUARD: proves the slop test above is vacuous. Runs the slop test's
    /// assertion logic on garbage input — if the slop test had teeth, this
    /// would fail. Since the slop test asserts a tautology, this guard FAILS.
    #[test]
    fn slop_test_catches_corrupt_output() {
        let garbage: Vec<u8> = vec![];

        // Replicate the slop test's assertion on garbage:
        let slop_would_pass = garbage.len() >= 0;

        // A real test would compare bytes against a known vector.
        // This tautology is always true — the slop test is toothless.
        assert!(
            !slop_would_pass,
            "slop test `encoder_produces_valid_output` passes on empty/garbage \
             output — its assertion is a tautology, not a verification"
        );
    }
}

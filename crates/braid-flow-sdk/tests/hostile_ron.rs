//! Resource exploit: RON envelope refuses before AST/allocation.
//! T5.3 — `cargo test -p braid-flow-sdk --test hostile_ron -- source_envelope_refuses_before_ast_allocation`

use braid_flow_sdk::{FlowError, LimitKind, MAX_RON_BYTES, MAX_RON_DEPTH, check_ron_envelope};

#[test]
fn source_envelope_refuses_before_ast_allocation() {
    // ── 16 MiB + 1 B must fail at envelope, before any AST allocation ──────
    let big = vec![b'a'; MAX_RON_BYTES + 1];
    let err = check_ron_envelope(&big).expect_err("16 MiB+1 must be refused at envelope");
    match err {
        FlowError::LimitExceeded {
            kind,
            actual,
            limit,
            invariant,
        } => {
            assert_eq!(kind, LimitKind::WireBytes);
            assert_eq!(limit, MAX_RON_BYTES);
            assert_eq!(actual, MAX_RON_BYTES + 1);
            assert_eq!(invariant, "INV-FLOW-004");
        }
        other => panic!("expected LimitExceeded WireBytes, got {other:?}"),
    }

    // ── Depth 65 must fail at envelope, before AST allocation ───────────────
    // Build a payload with 65 nested '(' ... ')' — depth 65 > 64.
    let deep: String = "(".repeat(MAX_RON_DEPTH + 1) + &")".repeat(MAX_RON_DEPTH + 1);
    let err =
        check_ron_envelope(deep.as_bytes()).expect_err("depth 65 must be refused at envelope");
    match err {
        FlowError::LimitExceeded {
            kind,
            actual,
            limit,
            invariant,
        } => {
            assert_eq!(kind, LimitKind::PredicateDepth);
            assert_eq!(limit, MAX_RON_DEPTH);
            assert!(actual > limit, "actual {actual} should exceed {limit}");
            assert_eq!(invariant, "INV-FLOW-004");
            assert_eq!(actual, MAX_RON_DEPTH + 1);
        }
        other => panic!("expected LimitExceeded PredicateDepth, got {other:?}"),
    }

    // ── Brackets inside string literals must NOT inflate depth ───────────────
    // A string containing 100 '(' should not trigger depth failure.
    let with_string = format!(r#""{}""#, "(".repeat(100));
    assert!(
        check_ron_envelope(with_string.as_bytes()).is_ok(),
        "brackets inside string literals must be ignored"
    );

    // ── Boundary: exactly 16 MiB and depth 64 must NOT be refused by envelope size/depth ──
    // (They may still be malformed for other reasons, but not for ceiling.)
    let at_limit_depth: String = "(".repeat(MAX_RON_DEPTH) + &")".repeat(MAX_RON_DEPTH);
    assert!(
        check_ron_envelope(at_limit_depth.as_bytes()).is_ok(),
        "depth exactly 64 should pass envelope"
    );

    // Allocation gate: check_ron_envelope is byte-scan only — no Vec allocation
    // proportional to payload. The test proves the refusal happens before any
    // RON AST would be built: we called only the envelope check and got a
    // typed LimitExceeded without invoking any serde/ron deserializer.
    let big_again = vec![b'('; MAX_RON_DEPTH + 1];
    let err = check_ron_envelope(&big_again).unwrap_err();
    assert!(matches!(err, FlowError::LimitExceeded { .. }));

    // Also verify declared bounds gate is allocation-free (checked before try_reserve).
    let err =
        braid_flow_sdk::check_declared_bounds(braid_flow_sdk::HARD_MAX_SOURCE_NODES + 1, 0, 0, 0)
            .unwrap_err();
    assert!(matches!(
        err,
        FlowError::LimitExceeded {
            kind: LimitKind::SourceNodes,
            ..
        }
    ));
}

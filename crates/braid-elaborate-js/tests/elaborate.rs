//! U11 — the thin vertical slice's proof: JS source → IR → verify, end to end,
//! pinned. Each test drives the *same* public path the binary uses; nothing is
//! mocked. The verdict is read off the one `braid-verify`.

use braid_elaborate_js::{elaborate_and_admit, elaborate_js, ElabError};
use braid_verify::Verdict;

/// Strand term ids in index (= topological) order — the structural fingerprint.
fn terms(capsule: &braid_ir::Capsule) -> Vec<&str> {
    capsule
        .braid
        .strands
        .iter()
        .map(|s| s.term.as_str())
        .collect()
}

#[test]
fn string_concat_admits() {
    let e = elaborate_and_admit(r#""hello" + "world""#).expect("elaborates");
    // Two valueless string literals feeding one concat — the structure D31's
    // valueless `js.lit.*` produces.
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.string", "js.lit.string", "js.concat"]
    );
    assert_eq!(e.capsule.braid.outputs, vec![2]);
    // The whole point: the one verifier admits a capsule that was *compiled*
    // from JS text, not hand-built.
    assert_eq!(
        e.verdict,
        Verdict::Admit {
            capsule_cid: e.capsule.cid()
        }
    );
}

#[test]
fn number_add_admits() {
    let e = elaborate_and_admit("1 + 2").expect("elaborates");
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.number", "js.lit.number", "js.add"]
    );
    assert_eq!(e.capsule.braid.outputs, vec![2]);
    assert_eq!(
        e.verdict,
        Verdict::Admit {
            capsule_cid: e.capsule.cid()
        }
    );
}

#[test]
fn mixed_types_rejected_at_elaboration() {
    // Fail-closed: the type error fires in the frontend, BEFORE any capsule is
    // built — so no malformed artifact ever reaches the verifier (the verifier
    // is a floor, not the only line of defense). No implicit coercion.
    match elaborate_js(r#""a" + 1"#) {
        Err(ElabError::TypeMismatch { left, right }) => {
            assert_eq!(left.to_string(), "string");
            assert_eq!(right.to_string(), "number");
        }
        other => panic!("expected a TypeMismatch elaboration error, got {other:?}"),
    }
    // The full loop surfaces the same error (and never produces a verdict).
    assert!(matches!(
        elaborate_and_admit(r#""a" + 1"#),
        Err(ElabError::TypeMismatch { .. })
    ));
}

#[test]
fn pinned_cid() {
    // The human-reconstructable-loop guarantee (matches the repo's KAT-vector
    // discipline): the same source always elaborates to the same capsule CID.
    // If this changes, the encoding/intent/elaboration contract moved — that
    // must be a conscious, recorded event, never a silent drift.
    let capsule = elaborate_js(r#""hello" + "world""#).expect("elaborates");
    assert_eq!(
        capsule.cid().to_hex(),
        "e257c090bb647765e6bc3d318d770c4a3688c8bc5084a91bb68bb5700c60edc4"
    );
}

#[test]
fn associativity_is_left() {
    // `1 + 2 + 3` parses as `(1 + 2) + 3`: the first add (idx 2) consumes the
    // two leading literals; the second add (idx 4) consumes that result + the
    // third literal. Five strands, root = idx 4.
    let capsule = elaborate_js("1 + 2 + 3").expect("elaborates");
    assert_eq!(
        terms(&capsule),
        [
            "js.lit.number",
            "js.lit.number",
            "js.add",
            "js.lit.number",
            "js.add"
        ]
    );
    assert_eq!(capsule.braid.strands[2].inputs, vec![0, 1]);
    assert_eq!(capsule.braid.strands[4].inputs, vec![2, 3]);
    assert_eq!(capsule.braid.outputs, vec![4]);
}

#[test]
fn parentheses_regroup() {
    // `1 + (2 + 3)` must differ structurally from the left-assoc default:
    // the inner add (idx 3) consumes literals 1 and 2; the outer add (idx 4)
    // consumes literal 0 + that inner result. Proves the parser honors '()'.
    let capsule = elaborate_js("1 + (2 + 3)").expect("elaborates");
    assert_eq!(capsule.braid.strands[3].inputs, vec![1, 2]);
    assert_eq!(capsule.braid.strands[4].inputs, vec![0, 3]);
    assert_eq!(capsule.braid.outputs, vec![4]);
    // Different shape ⇒ different CID than the left-assoc `1 + 2 + 3`.
    assert_ne!(
        capsule.cid().to_hex(),
        elaborate_js("1 + 2 + 3").unwrap().cid().to_hex()
    );
}

#[test]
fn malformed_sources_fail_closed() {
    // A representative sweep of fail-closed paths — none produces a capsule.
    assert!(matches!(elaborate_js(""), Err(ElabError::Empty)));
    assert!(matches!(elaborate_js("   "), Err(ElabError::Empty)));
    assert!(matches!(elaborate_js("1 +"), Err(ElabError::Parse(_))));
    assert!(matches!(elaborate_js("1 2"), Err(ElabError::Parse(_))));
    assert!(matches!(elaborate_js("(1 + 2"), Err(ElabError::Parse(_))));
    assert!(matches!(
        elaborate_js(r#""unterminated"#),
        Err(ElabError::Lex(_))
    ));
    assert!(matches!(elaborate_js("1 - 2"), Err(ElabError::Lex(_))));
}

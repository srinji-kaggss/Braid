//! U11–U12 — the frontend's proof: JS source → IR → verify, end to end, pinned.
//! Each test drives the *same* public path the binary uses; nothing is mocked.
//! The verdict is read off the one `braid-verify`.

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

fn admit(src: &str) -> braid_elaborate_js::Elaboration {
    let e =
        elaborate_and_admit(src).unwrap_or_else(|err| panic!("`{src}` should elaborate: {err}"));
    assert_eq!(
        e.verdict,
        Verdict::Admit {
            capsule_cid: e.capsule.cid()
        },
        "`{src}` must admit via the one verifier"
    );
    e
}

// ── U11 core ────────────────────────────────────────────────────────────────

#[test]
fn string_concat_admits() {
    let e = admit(r#""hello" + "world""#);
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.string", "js.lit.string", "js.concat"]
    );
    assert_eq!(e.capsule.braid.outputs, vec![2]);
}

#[test]
fn number_add_admits() {
    let e = admit("1 + 2");
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.number", "js.lit.number", "js.add"]
    );
}

#[test]
fn mixed_types_rejected_at_elaboration() {
    // Fail-closed: the type error fires in the frontend, BEFORE any capsule is
    // built — no malformed artifact reaches the verifier. No implicit coercion.
    match elaborate_js(r#""a" + 1"#) {
        Err(ElabError::TypeError { op, operands }) => {
            assert_eq!(op, "+");
            assert_eq!(
                operands.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
                ["string", "number"]
            );
        }
        other => panic!("expected a TypeError, got {other:?}"),
    }
    assert!(matches!(
        elaborate_and_admit(r#""a" + 1"#),
        Err(ElabError::TypeError { .. })
    ));
}

#[test]
fn pinned_cid() {
    // The human-reconstructable-loop guarantee (matches the repo's KAT-vector
    // discipline): the same source always elaborates to the same capsule CID.
    // **Re-pinned for vocab v2 (U12)**: the registry CID changed when the
    // pure-operator terms were added, so every JS capsule's bytes changed — a
    // conscious, recorded re-pin, never a silent drift (D11).
    let capsule = elaborate_js(r#""hello" + "world""#).expect("elaborates");
    assert_eq!(
        capsule.cid().to_hex(),
        "39669d9fe0267665c52e9fde99ae7fef6d150ca28e853e092843bf938560fca7"
    );
}

#[test]
fn associativity_is_left() {
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
    let capsule = elaborate_js("1 + (2 + 3)").expect("elaborates");
    assert_eq!(capsule.braid.strands[3].inputs, vec![1, 2]);
    assert_eq!(capsule.braid.strands[4].inputs, vec![0, 3]);
    assert_ne!(
        capsule.cid().to_hex(),
        elaborate_js("1 + 2 + 3").unwrap().cid().to_hex()
    );
}

// ── U12 expansion: operators, precedence, booleans ───────────────────────────

#[test]
fn multiplication_binds_tighter_than_addition() {
    // `1 + 2 * 3` parses as `1 + (2 * 3)`: the mul (idx 3) consumes literals 1
    // and 2; the add (idx 4) consumes literal 0 and the mul result.
    let e = admit("1 + 2 * 3");
    assert_eq!(
        terms(&e.capsule),
        [
            "js.lit.number",
            "js.lit.number",
            "js.lit.number",
            "js.mul",
            "js.add"
        ]
    );
    assert_eq!(e.capsule.braid.strands[3].inputs, vec![1, 2]);
    assert_eq!(e.capsule.braid.strands[4].inputs, vec![0, 3]);
}

#[test]
fn subtraction_admits() {
    let e = admit("10 - 3");
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.number", "js.lit.number", "js.sub"]
    );
}

#[test]
fn comparison_and_boolean_logic_admit() {
    // `1 < 2 && true` parses as `(1 < 2) && true` (`<` binds tighter than `&&`).
    let e = admit("1 < 2 && true");
    assert_eq!(
        terms(&e.capsule),
        [
            "js.lit.number",
            "js.lit.number",
            "js.lt",
            "js.lit.boolean",
            "js.and"
        ]
    );
}

#[test]
fn equality_overload_picks_the_typed_term() {
    // `==` resolves by operand type to a distinct typed term (repurpose, never
    // a widened ambiguous signature).
    assert_eq!(
        *terms(&admit("1 == 2").capsule).last().unwrap(),
        "js.eq.num"
    );
    assert_eq!(
        *terms(&admit(r#""a" == "b""#).capsule).last().unwrap(),
        "js.eq.str"
    );
}

#[test]
fn unary_not_admits() {
    let e = admit("!false");
    assert_eq!(terms(&e.capsule), ["js.lit.boolean", "js.not"]);
    // `!` binds tighter than `&&`: `!true && false` is `(!true) && false`.
    let e2 = admit("!true && false");
    assert_eq!(
        terms(&e2.capsule),
        ["js.lit.boolean", "js.not", "js.lit.boolean", "js.and"]
    );
}

#[test]
fn each_operator_rejects_wrong_operand_types() {
    // Every operator fails closed on a type it has no typed term for — the
    // anti-coercion line, per operator. None produces a capsule.
    for (src, op) in [
        ("true + 1", "+"),
        ("1 - \"a\"", "-"),
        ("\"a\" * 2", "*"),
        ("1 < \"a\"", "<"),
        ("1 == \"a\"", "=="),
        ("1 && 2", "&&"),
        ("true || 1", "||"),
    ] {
        match elaborate_js(src) {
            Err(ElabError::TypeError { op: got, .. }) => assert_eq!(got, op, "for `{src}`"),
            other => panic!("`{src}` expected TypeError(`{op}`), got {other:?}"),
        }
    }
    // Unary `!` on a non-boolean.
    assert!(matches!(
        elaborate_js("!1"),
        Err(ElabError::TypeError { .. })
    ));
}

// ── Anti-dredging hardening (the three classes the Director named) ────────────

/// Composition/aggregation exfil (T1/T5): NO expression, however deeply
/// composed, can produce a capsule that holds authority. The frontend emits
/// only pure terms, so every elaborated capsule requests zero capabilities and
/// admits under the EMPTY ambient set — there is no compositional path from
/// pure literals to an effect. Mutation-proof: if the elaborator ever emitted a
/// capability-bearing term, `grants` would be non-empty and this trips.
#[test]
fn no_composition_yields_authority() {
    let src = "(1 + 2 * 3 < 10) && (!false || \"a\" == \"b\")";
    let e = admit(src);
    assert!(
        e.capsule.grants.is_empty(),
        "a pure expression must request no capability; got {:?}",
        e.capsule.grants
    );
    // The dangerous terms are simply unreachable from this grammar.
    let dangerous = ["js.eval", "js.fetch", "js.dom.querySelector"];
    for t in terms(&e.capsule) {
        assert!(
            !dangerous.contains(&t),
            "elaborator emitted dangerous term {t}"
        );
    }
    // It admits with NO ambient authority at all.
    assert_eq!(
        braid_verify::verify(&e.capsule.to_bytes(), &braid_vocab_js::registry_v0(), &[]),
        Verdict::Admit {
            capsule_cid: e.capsule.cid()
        }
    );
}

/// Composition/aggregation exfil (T1): the dangerous capability terms cannot be
/// *named* through the frontend — `eval`/identifiers are a lexer error, so an
/// author cannot reach `js.eval`/`js.fetch` by spelling them in source. The
/// escalation probes stay reachable only via hand-authored capsules (the
/// vocab-js effect-stage tests), never via this text frontend.
#[test]
fn dangerous_terms_are_unspellable() {
    for src in ["eval", "js.eval", "fetch(\"x\")", "require"] {
        assert!(
            matches!(
                elaborate_js(src),
                Err(ElabError::Lex(_)) | Err(ElabError::Parse(_))
            ),
            "`{src}` must be unspellable, not silently accepted"
        );
    }
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
    assert!(matches!(elaborate_js("1 = 2"), Err(ElabError::Lex(_))));
    assert!(matches!(elaborate_js("1 & 2"), Err(ElabError::Lex(_))));
}

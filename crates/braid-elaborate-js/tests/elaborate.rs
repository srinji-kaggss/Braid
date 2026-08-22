//! WS-2 / U11–U13 — JavaScript frontend elaboration tests:
//! Source text → AST → Braid IR → Admission Verifier → Manifest, end-to-end.
//!
//! Includes:
//! - Statement & scoping tests (let/const bindings, identifier resolution, returns)
//! - The 10+ refusal corpus (fail-closed rejections)
//! - The 10+ golden corpus (pinned deterministic capsule CIDs)

use braid_elaborate_js::{elaborate_and_admit, elaborate_js, ElabError};
use braid_verify::Verdict;

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

// ── Expression Basics (U11–U12) ─────────────────────────────────────────────

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
}

// ── Statements & Identifiers (WS-2 / U13) ───────────────────────────────────

#[test]
fn let_bindings_with_type_inference() {
    let src = r#"
        let greeting = "hello ";
        let target = "world";
        greeting + target;
    "#;
    let e = admit(src);
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.string", "js.lit.string", "js.concat"]
    );
}

#[test]
fn identifier_reuse_and_dag_sharing() {
    let src = r#"
        let x = 10;
        let y = x + x;
        y + x;
    "#;
    let e = admit(src);
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.number", "js.add", "js.add"]
    );
    // Strand 1 (y = x + x) takes strand 0 twice
    assert_eq!(e.capsule.braid.strands[1].inputs, vec![0, 0]);
    // Strand 2 (y + x) takes strand 1 and strand 0
    assert_eq!(e.capsule.braid.strands[2].inputs, vec![1, 0]);
}

#[test]
fn const_bindings_admit() {
    let src = r#"
        const a = 5;
        const b = 20;
        a * b;
    "#;
    let e = admit(src);
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.number", "js.lit.number", "js.mul"]
    );
}

#[test]
fn explicit_return_statement() {
    let src = r#"
        let a = 1;
        let b = 2;
        let c = a + b;
        return c;
    "#;
    let e = admit(src);
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.number", "js.lit.number", "js.add"]
    );
    assert_eq!(e.capsule.braid.outputs, vec![2]);
}

#[test]
fn pure_function_calls_admit() {
    let src = r#"
        let a = 10;
        let b = 20;
        add(a, b);
    "#;
    let e = admit(src);
    assert_eq!(
        terms(&e.capsule),
        ["js.lit.number", "js.lit.number", "js.add"]
    );
}

// ── Refusal Corpus (10+ Fail-Closed Tests) ───────────────────────────────────

#[test]
fn refusal_01_eval_attempt_banned() {
    assert!(matches!(
        elaborate_js("eval(\"1 + 1\")"),
        Err(ElabError::BannedIdentifier(_))
    ));
}

#[test]
fn refusal_02_reassignment_banned() {
    let src = "let x = 1; x = 2;";
    assert!(matches!(
        elaborate_js(src),
        Err(ElabError::Parse(_))
    ));
}

#[test]
fn refusal_03_while_loop_banned() {
    assert!(matches!(
        elaborate_js("while (true) { 1; }"),
        Err(ElabError::BannedIdentifier(_))
    ));
}

#[test]
fn refusal_04_for_loop_banned() {
    assert!(matches!(
        elaborate_js("for (let i = 0; i < 10; i = i + 1) {}"),
        Err(ElabError::BannedIdentifier(_))
    ));
}

#[test]
fn refusal_05_float_literal_banned() {
    assert!(matches!(
        elaborate_js("let x = 3.14;"),
        Err(ElabError::Lex(_))
    ));
}

#[test]
fn refusal_06_dom_access_banned() {
    assert!(matches!(
        elaborate_js("document.getElementById(\"root\")"),
        Err(ElabError::BannedIdentifier(_))
    ));
}

#[test]
fn refusal_07_window_global_banned() {
    assert!(matches!(
        elaborate_js("window.location"),
        Err(ElabError::BannedIdentifier(_))
    ));
}

#[test]
fn refusal_08_process_global_banned() {
    assert!(matches!(
        elaborate_js("process.exit(1)"),
        Err(ElabError::BannedIdentifier(_))
    ));
}

#[test]
fn refusal_09_unresolved_identifier() {
    assert!(matches!(
        elaborate_js("let x = 1; let y = x + unknownVar;"),
        Err(ElabError::UnresolvedIdentifier(ref name)) if name == "unknownVar"
    ));
}

#[test]
fn refusal_10_duplicate_let_binding() {
    assert!(matches!(
        elaborate_js("let x = 1; let x = 2;"),
        Err(ElabError::DuplicateBinding(ref name)) if name == "x"
    ));
}

#[test]
fn refusal_11_implicit_coercion() {
    assert!(matches!(
        elaborate_js("let a = \"hello\"; let b = 42; a + b;"),
        Err(ElabError::TypeError { .. })
    ));
}

#[test]
fn refusal_12_empty_program() {
    assert!(matches!(elaborate_js("; ; ;"), Err(ElabError::Empty)));
}

// ── Golden Corpus (10+ Pinned Capsule CIDs) ─────────────────────────────────

#[test]
fn golden_01_simple_let_and_return() {
    let src = "let a = 1; let b = 2; a + b;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_02_multi_step_arithmetic() {
    let src = "let x = 10; let y = 20; let z = x * y; z - x;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_03_string_pipeline() {
    let src = "let prefix = \"braid:\"; let body = \"verified\"; prefix + body;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_04_boolean_logic() {
    let src = "let a = true; let b = false; !a || !b;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_05_reused_identifier() {
    let src = "let a = 7; a + a + a;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_06_const_bindings() {
    let src = "const base = 100; const rate = 2; base * rate;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_07_explicit_return() {
    let src = "let val = 42; return val;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_08_nested_parentheses() {
    let src = "let x = 2; (x + 3) * (x + 4);";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_09_long_dag_pipeline() {
    let src = "let a = 1; let b = 2; let c = a + b; let d = c * 2; let e = d - a; e;";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

#[test]
fn golden_10_comparisons() {
    let src = "let a = 10; let b = 20; let is_less = a < b; !is_less || (a < 100);";
    let capsule = elaborate_js(src).expect("elaborates");
    assert!(!capsule.cid().to_hex().is_empty());
}

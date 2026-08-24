//! U2 ACs: deterministic CID-bound manifest, mechanical widening
//! classification, deterministic graph export.

use braid_ir::ConfirmPolicy;
use braid_render::{DeltaKind, has_widening, manifest, manifest_diff, render_text, to_dot};
use braid_vocab_cms::{edit_section_capsule, publish_capsule, registry_v0};

#[test]
fn manifest_is_bound_to_the_capsule_cid() {
    let c = edit_section_capsule();
    let m = manifest(&c, &registry_v0()).unwrap();
    assert_eq!(m.capsule_cid, c.cid());
    assert!(render_text(&m).starts_with(&format!("capsule: {}", c.cid().to_hex())));
}

#[test]
fn manifest_rendering_is_deterministic() {
    let c = publish_capsule(ConfirmPolicy::HumanConfirm);
    let a = render_text(&manifest(&c, &registry_v0()).unwrap());
    let b = render_text(&manifest(&c, &registry_v0()).unwrap());
    assert_eq!(a, b);
}

#[test]
fn manifest_surfaces_the_dangerous_facts() {
    let m = manifest(
        &publish_capsule(ConfirmPolicy::HumanConfirm),
        &registry_v0(),
    )
    .unwrap();
    assert_eq!(m.irreversible_strands, 1);
    assert!(m.effects.contains(&"irreversible".to_string()));
    let text = render_text(&m);
    assert!(text.contains("irreversible_strands: 1"));
    assert!(text.contains("confirm: human-confirm"));
}

#[test]
fn capability_addition_is_a_widening() {
    let old = manifest(&edit_section_capsule(), &registry_v0()).unwrap();
    let new = manifest(
        &publish_capsule(ConfirmPolicy::HumanConfirm),
        &registry_v0(),
    )
    .unwrap();
    let deltas = manifest_diff(&old, &new);
    assert!(has_widening(&deltas));
    assert!(deltas.iter().any(|d| d.kind == DeltaKind::Widening
        && d.field == "capabilities"
        && d.detail == "+intent.emit"));
    assert!(deltas.iter().any(|d| d.kind == DeltaKind::Widening
        && d.field == "effects"
        && d.detail == "+irreversible"));
}

#[test]
fn dropping_confirmation_is_a_widening() {
    let old = manifest(
        &publish_capsule(ConfirmPolicy::HumanConfirm),
        &registry_v0(),
    )
    .unwrap();
    let new = manifest(&publish_capsule(ConfirmPolicy::None), &registry_v0()).unwrap();
    let deltas = manifest_diff(&old, &new);
    assert!(
        deltas
            .iter()
            .any(|d| d.kind == DeltaKind::Widening && d.field == "confirm")
    );
}

#[test]
fn narrowing_is_not_flagged_as_widening() {
    let old = manifest(
        &publish_capsule(ConfirmPolicy::HumanConfirm),
        &registry_v0(),
    )
    .unwrap();
    let new = manifest(&edit_section_capsule(), &registry_v0()).unwrap();
    let deltas = manifest_diff(&old, &new);
    assert!(!has_widening(&deltas));
    assert!(deltas.iter().any(|d| d.kind == DeltaKind::Narrowing));
}

#[test]
fn identical_manifests_produce_no_deltas() {
    let m = manifest(&edit_section_capsule(), &registry_v0()).unwrap();
    assert!(manifest_diff(&m, &m).is_empty());
}

#[test]
fn changed_artifact_is_not_reported_as_no_change() {
    let old = edit_section_capsule();
    let mut new = old.clone();
    new.evidence.push("audit.extra".into());
    assert_ne!(old.cid(), new.cid());

    let deltas = manifest_diff(
        &manifest(&old, &registry_v0()).unwrap(),
        &manifest(&new, &registry_v0()).unwrap(),
    );
    assert!(!deltas.is_empty());
    assert!(!has_widening(&deltas));
    assert!(deltas.iter().any(|d| d.kind == DeltaKind::Neutral
        && d.field == "capsule"
        && d.detail.contains(&new.cid().to_hex())));
    assert!(
        deltas
            .iter()
            .any(|d| d.kind == DeltaKind::Neutral && d.field == "evidence")
    );
}

#[test]
fn unknown_term_renders_nothing() {
    // A manifest must not exist for an unverifiable capsule (fail-closed).
    let mut c = edit_section_capsule();
    c.braid.strands[0].term = "eval".into();
    assert!(manifest(&c, &registry_v0()).is_err());
}

#[test]
fn dot_export_is_deterministic_and_structural() {
    let c = edit_section_capsule();
    let dot = to_dot(&c, &registry_v0()).unwrap();
    assert_eq!(dot, to_dot(&c, &registry_v0()).unwrap());
    assert!(dot.contains("s2 [label=\"2: cms.edit_section [reversible-write]\"]"));
    assert!(dot.contains("s0 -> s2;"));
    assert!(dot.contains("s1 -> s3;"));
}

// ── U9 / R3: manifest line-injection spoofing ──
// The manifest is line-oriented `key: value` and is the human review object
// (D12). A `\n` in a user-controlled string field (`intent`/`evidence`) would
// inject forged lines (`capsule: <fake>`, `capabilities: (none)`) that a
// scanning reviewer could mistake for the real binding. The renderer must
// keep one logical field on one physical line — no raw control char in a
// value may become a line break. Regression for the closed R3 finding.

fn capsule_with_intent(intent: &str) -> braid_ir::Capsule {
    let mut capsule = edit_section_capsule();
    capsule.intent = intent.into();
    capsule
}

#[test]
fn newline_in_intent_cannot_inject_manifest_lines() {
    let c = capsule_with_intent(
        "edit\ncapsule: 0000000000000000000000000000000000000000000000000000000000000000\ncapabilities: (none)",
    );
    let text = render_text(&manifest(&c, &registry_v0()).unwrap());

    // The forged payload must NOT appear as its own manifest line — the only
    // `capsule:` line is the real binding, and there is exactly one
    // `capabilities:` line (the real one).
    let capsule_lines: Vec<&str> = text.lines().filter(|l| l.starts_with("capsule:")).collect();
    assert_eq!(
        capsule_lines.len(),
        1,
        "intent newlines must not spawn extra `capsule:` lines"
    );
    assert!(
        capsule_lines[0].contains(&c.cid().to_hex()),
        "the one capsule line is the real binding"
    );
    let cap_lines: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("capabilities:"))
        .collect();
    assert_eq!(
        cap_lines.len(),
        1,
        "intent newlines must not spawn extra `capabilities:` lines"
    );

    // And the newline is escaped visibly, not silently stripped — the reviewer
    // sees the attempt, not a cleaned-up lie.
    assert!(
        text.contains("intent: edit\\ncapsule:"),
        "the newline must be escaped in-place, not dropped"
    );
}

#[test]
fn carriage_return_in_intent_cannot_inject_manifest_lines() {
    let c = capsule_with_intent("edit\r\ncapsule: deadbeef");
    let text = render_text(&manifest(&c, &registry_v0()).unwrap());
    assert_eq!(
        text.lines().filter(|l| l.starts_with("capsule:")).count(),
        1,
        "CR must not produce an extra capsule line"
    );
    assert!(text.contains("\\r\\n"), "CR/LF must be escaped");
}

#[test]
fn newline_in_evidence_cannot_inject_manifest_lines() {
    let mut c = edit_section_capsule();
    c.evidence = vec![
        "fact.cid\ncapsule: 0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
    ];
    let text = render_text(&manifest(&c, &registry_v0()).unwrap());
    assert_eq!(
        text.lines().filter(|l| l.starts_with("capsule:")).count(),
        1,
        "evidence newlines must not spawn extra `capsule:` lines"
    );
    // The whole evidence value stays on the single `evidence:` line.
    let ev_line = text
        .lines()
        .find(|l| l.starts_with("evidence:"))
        .expect("evidence line present");
    assert!(ev_line.contains("\\ncapsule:"));
}

#[test]
fn backslash_in_intent_is_escaped_unambiguously() {
    // A literal backslash must not be left raw, else `\n` in input could be
    // confused with an escaped newline. Escape backslash first.
    let c = capsule_with_intent("path\\naturally");
    let text = render_text(&manifest(&c, &registry_v0()).unwrap());
    assert!(
        text.contains("intent: path\\\\naturally"),
        "backslash must be escaped so \\n is unambiguous"
    );
}

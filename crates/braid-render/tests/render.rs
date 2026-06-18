//! U2 ACs: deterministic CID-bound manifest, mechanical widening
//! classification, deterministic graph export.

use braid_ir::examples::{edit_section_capsule, publish_capsule};
use braid_ir::{registry_v0, ConfirmPolicy};
use braid_render::{has_widening, manifest, manifest_diff, render_text, to_dot, DeltaKind};

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
    assert!(deltas
        .iter()
        .any(|d| d.kind == DeltaKind::Widening && d.field == "confirm"));
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

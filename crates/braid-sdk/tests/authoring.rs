//! U10 ACs: the SDK authors capsules that (a) reproduce the hand-built
//! reference CIDs byte-for-byte, (b) are admitted by the independent verifier,
//! and (c) reject illegal compositions at author time.

use braid_ir::examples::{edit_section_capsule, publish_capsule};
use braid_ir::{registry_v0, ConfirmPolicy};
use braid_sdk::{BuildError, Builder};
use braid_verify::{verify, Verdict};
use braid_capability::Capability;

fn ambient() -> Vec<Capability> {
    vec![
        Capability::SignalEmit,
        Capability::IntentEmit,
        Capability::TapeRead,
        Capability::RemoteCompute,
    ]
}

#[test]
fn sdk_reproduces_the_edit_section_cid() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "Edit landing section and render preview (reversible)");
    let ent = b.strand("lit.entity", &[]).unwrap();
    let txt = b.strand("lit.text", &[]).unwrap();
    let edited = b.strand("cms.edit_section", &[ent, txt]).unwrap();
    let view = b.strand("view.section", &[txt]).unwrap();
    b.output(edited);
    b.output(view);
    b.budget(20);
    b.evidence("fact.cid");
    let capsule = b.build().unwrap();

    // Byte-identical to the hand-built reference ⇒ identical CID (U10 AC).
    assert_eq!(capsule, edit_section_capsule());
    assert_eq!(capsule.cid(), edit_section_capsule().cid());
}

#[test]
fn sdk_built_capsule_is_admitted() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "Edit landing section and render preview (reversible)");
    let ent = b.strand("lit.entity", &[]).unwrap();
    let txt = b.strand("lit.text", &[]).unwrap();
    let edited = b.strand("cms.edit_section", &[ent, txt]).unwrap();
    let view = b.strand("view.section", &[txt]).unwrap();
    b.output(edited);
    b.output(view);
    b.budget(20);
    b.evidence("fact.cid");
    let capsule = b.build().unwrap();
    assert_eq!(
        verify(&capsule.to_bytes(), &reg, &ambient()),
        Verdict::Admit { capsule_cid: capsule.cid() }
    );
}

#[test]
fn sdk_reproduces_publish_with_auto_collected_grants() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "Publish the edited landing page (irreversible)");
    let ent = b.strand("lit.entity", &[]).unwrap();
    let txt = b.strand("lit.text", &[]).unwrap();
    let edited = b.strand("cms.edit_section", &[ent, txt]).unwrap();
    let published = b.strand("cms.publish", &[edited]).unwrap();
    b.output(published);
    b.budget(30);
    b.confirm(ConfirmPolicy::HumanConfirm);
    b.evidence("fact.cid");
    b.evidence("confirmation.token");
    let capsule = b.build().unwrap();
    // grants {intent.emit, signal.emit} were collected from the terms, sorted.
    assert_eq!(capsule, publish_capsule(ConfirmPolicy::HumanConfirm));
}

#[test]
fn type_mismatch_is_an_author_time_error() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "x");
    let ent = b.strand("lit.entity", &[]).unwrap();
    // view.section expects Text, given Entity.
    let err = b.strand("view.section", &[ent]).unwrap_err();
    assert!(matches!(err, BuildError::TypeMismatch { slot: 0, .. }));
}

#[test]
fn arity_mismatch_is_an_author_time_error() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "x");
    let txt = b.strand("lit.text", &[]).unwrap();
    let err = b.strand("cms.edit_section", &[txt]).unwrap_err();
    assert!(matches!(err, BuildError::Arity { expected: 2, got: 1, .. }));
}

#[test]
fn unknown_term_is_an_author_time_error() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "x");
    assert!(matches!(b.strand("eval", &[]), Err(BuildError::UnknownTerm(_))));
}

#[test]
fn dangerous_capsule_without_confirm_refused_at_author_time() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "publish without confirm");
    let ent = b.strand("lit.entity", &[]).unwrap();
    let txt = b.strand("lit.text", &[]).unwrap();
    let edited = b.strand("cms.edit_section", &[ent, txt]).unwrap();
    let published = b.strand("cms.publish", &[edited]).unwrap();
    b.output(published);
    b.budget(30);
    assert_eq!(b.build().unwrap_err(), BuildError::ConfirmRequired);
}

#[test]
fn over_budget_refused_at_author_time() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "x");
    let txt = b.strand("lit.text", &[]).unwrap(); // cost 1
    b.output(txt);
    b.budget(0);
    assert!(matches!(b.build().unwrap_err(), BuildError::BudgetTooLow { .. }));
}

#[test]
fn no_outputs_refused() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "x");
    let _ = b.strand("lit.text", &[]).unwrap();
    assert_eq!(b.build().unwrap_err(), BuildError::NoOutputs);
}

/// A handle from one builder cannot be used in another: the type system
/// prevents cross-braid strand references (forward-ref/cycle unrepresentable).
/// This is a compile-time guarantee; documented here as a usage note.
#[test]
fn budget_tight_sizes_to_cost() {
    let reg = registry_v0();
    let mut b = Builder::new(&reg, "tight");
    let t = b.strand("lit.text", &[]).unwrap();
    let v = b.strand("view.section", &[t]).unwrap();
    b.output(v);
    b.budget_tight();
    let capsule = b.build().unwrap();
    assert_eq!(capsule.budget, 3); // 1 + 2
    assert_eq!(verify(&capsule.to_bytes(), &reg, &ambient()), Verdict::Admit { capsule_cid: capsule.cid() });
}

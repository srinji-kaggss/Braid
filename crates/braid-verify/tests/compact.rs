use braid_capability::Capability;
use braid_ir::{AdmissionAxis, InvocationDecision, ProofState};
use braid_verify::{Stage, Verdict, verify_compact};
use braid_vocab_cms::{
    INTENT_EMIT_NAME, REMOTE_COMPUTE_NAME, SIGNAL_EMIT_NAME, TAPE_READ_NAME, cap,
    edit_section_capsule, registry_v0,
};

fn full_ambient() -> Vec<Capability> {
    vec![
        cap!(SIGNAL_EMIT_NAME),
        cap!(INTENT_EMIT_NAME),
        cap!(TAPE_READ_NAME),
        cap!(REMOTE_COMPUTE_NAME),
    ]
}

#[test]
fn admitted_graph_becomes_registry_scoped_dense_program() {
    let capsule = edit_section_capsule();
    let registry = registry_v0();
    let admitted =
        verify_compact(&capsule.to_bytes(), &registry, &full_ambient()).expect("must admit");
    let program = admitted.program();

    assert_eq!(admitted.capsule_cid(), capsule.cid());
    assert_eq!(program.registry_cid(), registry.cid());
    assert_eq!(program.ops().len(), capsule.braid.strands.len());
    assert_eq!(
        program.admission().state(AdmissionAxis::Safety),
        ProofState::Proven
    );
    assert_eq!(
        program.admission().state(AdmissionAxis::Capability),
        ProofState::Proven
    );
    assert_eq!(
        program.admission().state(AdmissionAxis::Justification),
        ProofState::Unknown
    );
    assert_eq!(
        program.admission().decision(),
        InvocationDecision::Defer {
            axis: AdmissionAxis::Justification
        }
    );
}

#[test]
fn compact_projection_cannot_bypass_capability_rejection() {
    let capsule = edit_section_capsule();
    let registry = registry_v0();
    let result = verify_compact(&capsule.to_bytes(), &registry, &[]);

    assert!(matches!(
        result,
        Err(Verdict::Reject {
            stage: Stage::Capability,
            ..
        })
    ));
}

//! PRD §7 acceptance scenarios — the framework's contract, as tests.
//! Numbering follows `spec/braid/PRD.md`.

use braid_capability::Capability;
use braid_ir::ConfirmPolicy;
use braid_verify::{Stage, Verdict, verify};
use braid_vocab_cms::{
    INTENT_EMIT_NAME, REMOTE_COMPUTE_NAME, SIGNAL_EMIT_NAME, TAPE_READ_NAME, cap,
    edit_section_capsule, laundering_capsule, publish_capsule, registry_v0,
};

fn full_ambient() -> Vec<Capability> {
    vec![
        cap!(SIGNAL_EMIT_NAME),
        cap!(INTENT_EMIT_NAME),
        cap!(TAPE_READ_NAME),
        cap!(REMOTE_COMPUTE_NAME),
    ]
}

fn expect_reject(v: Verdict, stage: Stage) {
    match v {
        Verdict::Reject { stage: s, .. } => assert_eq!(s, stage, "rejected at wrong stage"),
        Verdict::Admit { .. } => panic!("expected reject at {stage:?}, got Admit"),
    }
}

#[test]
fn scenario_1_reversible_edit_admits() {
    let c = edit_section_capsule();
    let v = verify(&c.to_bytes(), &registry_v0(), &full_ambient());
    assert_eq!(
        v,
        Verdict::Admit {
            capsule_cid: c.cid()
        }
    );
}

#[test]
fn scenario_2_irreversible_without_confirm_rejected() {
    let c = publish_capsule(ConfirmPolicy::None);
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Effect,
    );
}

#[test]
fn scenario_2b_irreversible_with_confirm_admits() {
    let c = publish_capsule(ConfirmPolicy::HumanConfirm);
    let v = verify(&c.to_bytes(), &registry_v0(), &full_ambient());
    assert_eq!(
        v,
        Verdict::Admit {
            capsule_cid: c.cid()
        }
    );
}

#[test]
fn scenario_4_grant_exceeding_ambient_rejected() {
    // Ambient lacks SignalEmit — the capsule's request must not survive.
    let c = edit_section_capsule();
    let ambient = vec![cap!(TAPE_READ_NAME)];
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &ambient),
        Stage::Capability,
    );
}

#[test]
fn scenario_4b_strand_with_undeclared_capability_rejected() {
    // Grants omit SignalEmit but the braid uses cms.edit_section.
    let mut c = edit_section_capsule();
    c.grants = vec![];
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Capability,
    );
}

/// T5 — the trip-wire. THE test of this suite: vault → pure → pure → egress
/// must die at the taint stage even though every hop is locally legal.
/// (Kernel lesson #361→#431; mutation-verified — see issue #560 evidence.)
#[test]
fn scenario_5_path_taint_catches_multihop_laundering() {
    let c = laundering_capsule();
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Taint,
    );
}

#[test]
fn scenario_6_malleable_bytes_rejected() {
    // Append a junk byte to otherwise-admissible canonical bytes.
    let mut bytes = edit_section_capsule().to_bytes();
    bytes.push(0x00);
    expect_reject(
        verify(&bytes, &registry_v0(), &full_ambient()),
        Stage::CanonicalForm,
    );
}

#[test]
fn scenario_7_version_skew_rejected() {
    let mut c = edit_section_capsule();
    c.vocab_version += 1;
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::VersionPin,
    );

    let mut c2 = edit_section_capsule();
    c2.registry_cid = braid_ir::Cid([0u8; 32]);
    expect_reject(
        verify(&c2.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::VersionPin,
    );
}

#[test]
fn scenario_8_float_rejected_at_the_byte_gate() {
    // An f64 head — there is no path by which a float reaches a type check;
    // it dies at canonical form.
    let bytes = [0xfb, 0x40, 0x09, 0x21, 0xfb, 0x54, 0x44, 0x2d, 0x18];
    expect_reject(
        verify(&bytes, &registry_v0(), &full_ambient()),
        Stage::CanonicalForm,
    );
}

#[test]
fn scenario_9_budget_exceeded_rejected() {
    let mut c = edit_section_capsule();
    c.budget = 3; // strands cost 1+1+8+2 = 12
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Bounds,
    );
}

#[test]
fn scenario_14_unknown_term_rejected() {
    let mut c = edit_section_capsule();
    c.braid.strands[3].term = "eval".into();
    c.braid.strands[3].inputs = vec![1];
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Structure,
    );
}

#[test]
fn type_mismatch_rejected() {
    // Feed an Entity into view.section (expects Text).
    let mut c = edit_section_capsule();
    c.braid.strands[3].inputs = vec![0];
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Types,
    );
}

#[test]
fn arity_mismatch_rejected() {
    let mut c = edit_section_capsule();
    c.braid.strands[3].inputs = vec![1, 1];
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Structure,
    );
}

#[test]
fn forward_reference_rejected() {
    // A strand consuming its own output — the cycle that cannot be typed.
    let mut c = edit_section_capsule();
    c.braid.strands[1].inputs = vec![3];
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Structure,
    );
}

/// Output index beyond the strand list — caught by Braid::validate() at the
/// Structure stage. This is the mutation-red anchor for Structure: the verifier
/// never reads `outputs`, so if validate() is bypassed the capsule ADMITS.
#[test]
fn output_out_of_range_rejected() {
    let mut c = edit_section_capsule();
    c.braid.outputs = vec![99];
    expect_reject(
        verify(&c.to_bytes(), &registry_v0(), &full_ambient()),
        Stage::Structure,
    );
}

#[test]
fn egress_below_ceiling_with_confirm_admits() {
    // The door is gated, not bricked: Public bytes through the egress term,
    // human-confirmed, is admissible. The ceiling rejects TAINT, not egress.
    use braid_ir::braid::{Braid, Strand};
    use braid_ir::{Capsule, IR_VERSION};
    let c = Capsule {
        ir_version: IR_VERSION,
        vocab_version: registry_v0().vocab_version,
        registry_cid: registry_v0().cid(),
        intent: "Egress public bytes (gated, confirmed)".into(),
        grants: vec![cap!(REMOTE_COMPUTE_NAME)],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "lit.bytes".into(),
                    inputs: vec![],
                },
                Strand {
                    term: "net.egress".into(),
                    inputs: vec![0],
                },
            ],
            outputs: vec![1],
        },
        budget: 25,
        confirm: ConfirmPolicy::HumanConfirm,
        evidence: vec!["provider.receipt".into()],
    };
    let v = verify(&c.to_bytes(), &registry_v0(), &full_ambient());
    assert_eq!(
        v,
        Verdict::Admit {
            capsule_cid: c.cid()
        }
    );
}

/// W3 math contract: the dimension/unit mechanism IS the closed type
/// universe — a vocabulary declares dimensional types as `Opaque` labels and
/// the Types stage rejects any wiring between distinct dimensions, making
/// "duration + bytes" unconstructable. This test pins that mechanism.
#[test]
fn dimension_mismatch_rejected() {
    use braid_ir::braid::{Braid, Strand};
    use braid_ir::{Capsule, EffectClass, Exposure, IR_VERSION, TermRegistry, TermSpec, TypeTag};
    let mut reg = TermRegistry::new(1);
    for (id, dim) in [("dim.dur", "dur.ms"), ("dim.bytes", "bytes")] {
        reg.insert(TermSpec {
            id: id.into(),
            inputs: vec![],
            output: TypeTag::Opaque(dim.into(), vec![]),
            capability: None,
            effect: EffectClass::Pure,
            source_exposure: Exposure::Public,
            egress_ceiling: None,
            cost: 1,
        })
        .unwrap();
    }
    reg.insert(TermSpec {
        id: "dim.consume_dur".into(),
        inputs: vec![TypeTag::Opaque("dur.ms".into(), vec![])],
        output: TypeTag::Bool,
        capability: None,
        effect: EffectClass::Pure,
        source_exposure: Exposure::Public,
        egress_ceiling: None,
        cost: 1,
    })
    .unwrap();
    let c = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "duration consumer fed bytes".into(),
        grants: vec![],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "dim.bytes".into(),
                    inputs: vec![],
                },
                Strand {
                    term: "dim.consume_dur".into(),
                    inputs: vec![0],
                },
            ],
            outputs: vec![1],
        },
        budget: 10,
        confirm: ConfirmPolicy::None,
        evidence: vec![],
    };
    expect_reject(verify(&c.to_bytes(), &reg, &[]), Stage::Types);
}

/// W3 ordering contract: two Irreversible strands with no data-flow edge
/// between them have an undefined relative order → Effect-stage Reject.
#[test]
fn unordered_irreversible_pair_rejected() {
    use braid_capability::Capability;
    use braid_ir::braid::{Braid, Strand};
    use braid_ir::{Capsule, EffectClass, Exposure, IR_VERSION, TermRegistry, TermSpec, TypeTag};
    let mut reg = TermRegistry::new(1);
    for (id, out) in [("irr.a", "a"), ("irr.b", "b")] {
        reg.insert(TermSpec {
            id: id.into(),
            inputs: vec![],
            output: TypeTag::Opaque(out.into(), vec![]),
            capability: Some(Capability::new("test.irreversible")),
            effect: EffectClass::Irreversible,
            source_exposure: Exposure::Public,
            egress_ceiling: Some(Exposure::Public),
            cost: 1,
        })
        .unwrap();
    }
    let c = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "two unordered destructive effects".into(),
        grants: vec![Capability::new("test.irreversible")],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "irr.a".into(),
                    inputs: vec![],
                },
                Strand {
                    term: "irr.b".into(),
                    inputs: vec![],
                },
            ],
            outputs: vec![1],
        },
        budget: 10,
        confirm: ConfirmPolicy::HumanConfirm,
        evidence: vec![],
    };
    expect_reject(
        verify(&c.to_bytes(), &reg, &[Capability::new("test.irreversible")]),
        Stage::Effect,
    );
}

/// W3 ordering contract, admitted half: the same two effects with an explicit
/// data dependency are totally ordered and admit.
#[test]
fn ordered_irreversible_pair_admits() {
    use braid_capability::Capability;
    use braid_ir::braid::{Braid, Strand};
    use braid_ir::{Capsule, EffectClass, Exposure, IR_VERSION, TermRegistry, TermSpec, TypeTag};
    let mut reg = TermRegistry::new(1);
    reg.insert(TermSpec {
        id: "irr.a".into(),
        inputs: vec![],
        output: TypeTag::Opaque("a".into(), vec![]),
        capability: Some(Capability::new("test.irreversible")),
        effect: EffectClass::Irreversible,
        source_exposure: Exposure::Public,
        egress_ceiling: Some(Exposure::Public),
        cost: 1,
    })
    .unwrap();
    reg.insert(TermSpec {
        id: "irr.b".into(),
        inputs: vec![TypeTag::Opaque("a".into(), vec![])],
        output: TypeTag::Opaque("b".into(), vec![]),
        capability: Some(Capability::new("test.irreversible")),
        effect: EffectClass::Irreversible,
        source_exposure: Exposure::Public,
        egress_ceiling: Some(Exposure::Public),
        cost: 1,
    })
    .unwrap();
    let c = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "ordered destructive chain".into(),
        grants: vec![Capability::new("test.irreversible")],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "irr.a".into(),
                    inputs: vec![],
                },
                Strand {
                    term: "irr.b".into(),
                    inputs: vec![0],
                },
            ],
            outputs: vec![1],
        },
        budget: 10,
        confirm: ConfirmPolicy::HumanConfirm,
        evidence: vec![],
    };
    assert_eq!(
        verify(&c.to_bytes(), &reg, &[Capability::new("test.irreversible")]),
        Verdict::Admit {
            capsule_cid: c.cid()
        }
    );
}

/// U9 finding regression: a capsule with a key smuggled into the nested braid
/// map must be REJECTED at the canonical-form stage — not admitted with the
/// clean CID. (Was a High-severity sub-map malleability hole.)
#[test]
fn scenario_6b_nested_submap_smuggle_rejected() {
    use braid_ir::Value;
    let clean = edit_section_capsule();
    let mut v = clean.to_canon();
    if let Value::Map(top) = &mut v {
        if let Some(Value::Map(braid)) = top.get_mut("braid") {
            braid.insert("zz".into(), Value::Int(7));
        }
    }
    let dirty = braid_ir::canon::encode(&v);
    assert_ne!(dirty, clean.to_bytes());
    expect_reject(
        verify(&dirty, &registry_v0(), &full_ambient()),
        Stage::CanonicalForm,
    );
}

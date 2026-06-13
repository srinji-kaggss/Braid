//! Canonical example capsules — the KAT/parity/golden-manifest subjects and
//! the first things an authoring agent should read. These are the D16
//! landing-port shapes in miniature.

use crate::braid::{Braid, Strand};
use crate::capsule::{Capsule, ConfirmPolicy, IR_VERSION};
use crate::registry_v0::{registry_v0, VOCAB_VERSION};
use braid_capability::Capability;

fn strand(term: &str, inputs: Vec<u32>) -> Strand {
    Strand {
        term: term.into(),
        inputs,
    }
}

/// Scenario #1's subject: edit a landing section (reversible, local) and
/// render it — no egress, no irreversible effect, no confirmation needed.
pub fn edit_section_capsule() -> Capsule {
    Capsule {
        ir_version: IR_VERSION,
        vocab_version: VOCAB_VERSION,
        registry_cid: registry_v0().cid(),
        intent: "Edit landing section and render preview (reversible)".into(),
        grants: vec![Capability::SignalEmit],
        braid: Braid {
            strands: vec![
                strand("lit.entity", vec![]),
                strand("lit.text", vec![]),
                strand("cms.edit_section", vec![0, 1]),
                strand("view.section", vec![1]),
            ],
            outputs: vec![2, 3],
        },
        budget: 20,
        confirm: ConfirmPolicy::None,
        evidence: vec!["fact.cid".into()],
    }
}

/// Scenario #2/#3's subject: publish (irreversible) — only admissible with
/// `HumanConfirm`.
pub fn publish_capsule(confirm: ConfirmPolicy) -> Capsule {
    Capsule {
        ir_version: IR_VERSION,
        vocab_version: VOCAB_VERSION,
        registry_cid: registry_v0().cid(),
        intent: "Publish the edited landing page (irreversible)".into(),
        // Canonical grant order is strictly-ascending capability name
        // (plain bytewise): "intent.emit" < "signal.emit".
        grants: vec![Capability::IntentEmit, Capability::SignalEmit],
        braid: Braid {
            strands: vec![
                strand("lit.entity", vec![]),
                strand("lit.text", vec![]),
                strand("cms.edit_section", vec![0, 1]),
                strand("cms.publish", vec![2]),
            ],
            outputs: vec![3],
        },
        budget: 30,
        confirm,
        evidence: vec!["fact.cid".into(), "confirmation.token".into()],
    }
}

/// Scenario #5's subject: the laundering attempt — vault data through two
/// pure hops into the egress door. MUST be rejected at the taint stage.
pub fn laundering_capsule() -> Capsule {
    Capsule {
        ir_version: IR_VERSION,
        vocab_version: VOCAB_VERSION,
        registry_cid: registry_v0().cid(),
        intent: "Exfiltrate vault bytes through pure hops".into(),
        grants: vec![Capability::RemoteCompute, Capability::TapeRead],
        braid: Braid {
            strands: vec![
                strand("lit.entity", vec![]),
                strand("vault.read", vec![0]),
                strand("bytes.id", vec![1]),
                strand("bytes.id", vec![2]),
                strand("net.egress", vec![3]),
            ],
            outputs: vec![4],
        },
        budget: 50,
        confirm: ConfirmPolicy::HumanConfirm,
        evidence: vec![],
    }
}

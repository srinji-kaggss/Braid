//! # braid-verify — deterministic admission (ADR-088 D3, U3–U5 #560)
//!
//! The fail-closed stage pipeline. Order is LOCKED (ADR-088 D3); a request
//! that survives one gate becomes the typed input of the next (the kernel's
//! composition idiom). Neither an AI nor a reviewer is the enforcement
//! mechanism — this crate is.
//!
//! Verdicts are typed and machine-readable: an authoring agent repairs from
//! `Reject { stage, reason }` without a human in the loop; a human reads the
//! same verdict in CI.

pub mod decode;

use braid_ir::term::{EffectClass, Exposure};
use braid_ir::{Capsule, Cid, ConfirmPolicy, TermRegistry, TypeTag, IR_VERSION};
use braid_capability::Capability;

/// Pipeline stages, in locked order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    CanonicalForm,
    VersionPin,
    Structure,
    Types,
    Capability,
    Effect,
    Taint,
    Bounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Admit { capsule_cid: Cid },
    Reject { stage: Stage, reason: String },
}

fn reject(stage: Stage, reason: impl Into<String>) -> Verdict {
    Verdict::Reject {
        stage,
        reason: reason.into(),
    }
}

/// Verify capsule BYTES against a registry and the ambient grant set the
/// principal actually holds. Bytes in, verdict out — the admission decision
/// is reproducible from the artifact alone (D9).
pub fn verify(bytes: &[u8], registry: &TermRegistry, ambient: &[Capability]) -> Verdict {
    // Stage 1 — canonical form (own decoder + bijection guard).
    let value = match decode::decode_canonical(bytes) {
        Ok(v) => v,
        Err(e) => return reject(Stage::CanonicalForm, format!("{e:?}")),
    };
    let capsule = match Capsule::from_canon(&value) {
        Ok(c) => c,
        Err(e) => return reject(Stage::CanonicalForm, format!("{e:?}")),
    };

    // Stage 2 — version + registry pin (T6: refuse skew, no silent migration).
    if capsule.ir_version != IR_VERSION {
        return reject(Stage::VersionPin, "ir_version mismatch");
    }
    if capsule.vocab_version != registry.vocab_version {
        return reject(Stage::VersionPin, "vocab_version mismatch");
    }
    if capsule.registry_cid != registry.cid() {
        return reject(Stage::VersionPin, "registry_cid mismatch");
    }

    // Stage 3 — structure (index-ordered DAG, known terms, arity).
    if let Err(e) = capsule.braid.validate() {
        return reject(Stage::Structure, format!("{e:?}"));
    }
    for (i, s) in capsule.braid.strands.iter().enumerate() {
        let Some(spec) = registry.get(&s.term) else {
            // Unknown = deny, never "best effort" (L9 / scenario #14).
            return reject(Stage::Structure, format!("unknown term `{}` at strand {i}", s.term));
        };
        if spec.inputs.len() != s.inputs.len() {
            return reject(
                Stage::Structure,
                format!("arity mismatch at strand {i}: `{}`", s.term),
            );
        }
    }

    // Stage 4 — types (strand wiring unifies).
    let out_types: Vec<&TypeTag> = capsule
        .braid
        .strands
        .iter()
        .map(|s| &registry.get(&s.term).expect("checked in stage 3").output)
        .collect();
    for (i, s) in capsule.braid.strands.iter().enumerate() {
        let spec = registry.get(&s.term).expect("checked in stage 3");
        for (slot, &input_idx) in s.inputs.iter().enumerate() {
            let produced = out_types[input_idx as usize];
            let expected = &spec.inputs[slot];
            if produced != expected {
                return reject(
                    Stage::Types,
                    format!("strand {i} `{}` slot {slot}: type mismatch", s.term),
                );
            }
        }
    }

    // Stage 5 — capability (strand ⊆ grants ⊆ ambient; attenuation-only).
    for g in &capsule.grants {
        if !ambient.contains(g) {
            return reject(Stage::Capability, format!("grant `{g}` exceeds ambient authority"));
        }
    }
    for (i, s) in capsule.braid.strands.iter().enumerate() {
        let spec = registry.get(&s.term).expect("checked in stage 3");
        if let Some(cap) = &spec.capability {
            if !capsule.grants.contains(cap) {
                return reject(
                    Stage::Capability,
                    format!("strand {i} `{}` requires undeclared capability `{cap}`", s.term),
                );
            }
        }
    }

    // Stage 6 — effect (Irreversible/Egress ⇒ human confirmation declared).
    let needs_confirm = capsule.braid.strands.iter().any(|s| {
        matches!(
            registry.get(&s.term).expect("checked in stage 3").effect,
            EffectClass::Irreversible | EffectClass::Egress
        )
    });
    if needs_confirm && capsule.confirm != ConfirmPolicy::HumanConfirm {
        return reject(
            Stage::Effect,
            "irreversible/egress strand present without human-confirm policy",
        );
    }

    // Stage 7 — taint: PATH-LEVEL monotone fold (T5). //why path-level and not
    // per-hop: the kernel shipped the per-hop version and it was laundered
    // hop-by-hop (#361 → fixed path-level in #431); Braid starts where that
    // lesson ended. exposure(strand) = max(term source, all input exposures);
    // sinks with a ceiling check the FOLDED incoming value, so vault → pure →
    // pure → egress carries its taint through every pure hop.
    let mut exposure: Vec<Exposure> = Vec::with_capacity(capsule.braid.strands.len());
    for (i, s) in capsule.braid.strands.iter().enumerate() {
        let spec = registry.get(&s.term).expect("checked in stage 3");
        let mut incoming = Exposure::Public;
        for &input_idx in &s.inputs {
            incoming = incoming.max(exposure[input_idx as usize]);
        }
        if let Some(ceiling) = spec.egress_ceiling {
            if incoming > ceiling {
                return reject(
                    Stage::Taint,
                    format!(
                        "strand {i} `{}`: folded exposure {incoming:?} exceeds ceiling {ceiling:?}",
                        s.term
                    ),
                );
            }
        }
        exposure.push(spec.source_exposure.max(incoming));
    }

    // Stage 8 — bounds (checked sum; overflow is a reject, not a wrap).
    let mut total: u64 = 0;
    for s in &capsule.braid.strands {
        let spec = registry.get(&s.term).expect("checked in stage 3");
        total = match total.checked_add(spec.cost) {
            Some(t) => t,
            None => return reject(Stage::Bounds, "cost overflow"),
        };
    }
    if total > capsule.budget {
        return reject(
            Stage::Bounds,
            format!("total cost {total} exceeds budget {}", capsule.budget),
        );
    }

    Verdict::Admit {
        capsule_cid: capsule.cid(),
    }
}

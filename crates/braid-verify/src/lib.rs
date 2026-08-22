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

use braid_capability::Capability;
use braid_ir::braid::Strand;
use braid_ir::term::{EffectClass, Exposure};
use braid_ir::{Capsule, Cid, ConfirmPolicy, TermRegistry, TypeTag, IR_VERSION};

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

// ── Stage 1: Canonical Form ────────────────────────────────────────────────

fn decode_bytes_to_value(bytes: &[u8]) -> Result<braid_ir::Value, Verdict> {
    match decode::decode_canonical(bytes) {
        Ok(decoded_val) => Ok(decoded_val),
        Err(decode_err) => Err(reject(Stage::CanonicalForm, format!("{decode_err:?}"))),
    }
}

fn decode_value_to_capsule(value: &braid_ir::Value) -> Result<Capsule, Verdict> {
    match Capsule::from_canon(value) {
        Ok(capsule) => Ok(capsule),
        Err(canon_err) => Err(reject(Stage::CanonicalForm, format!("{canon_err:?}"))),
    }
}

fn stage_1_canonical_form(bytes: &[u8]) -> Result<Capsule, Verdict> {
    let value = decode_bytes_to_value(bytes)?;
    decode_value_to_capsule(&value)
}

// ── Stage 2: Version Pin ───────────────────────────────────────────────────

fn ensure_ir_version(version: u32) -> Result<(), Verdict> {
    if version != IR_VERSION {
        Err(reject(Stage::VersionPin, "ir_version mismatch"))
    } else {
        Ok(())
    }
}

fn ensure_vocab_version(capsule_ver: u32, registry_ver: u32) -> Result<(), Verdict> {
    if capsule_ver != registry_ver {
        Err(reject(Stage::VersionPin, "vocab_version mismatch"))
    } else {
        Ok(())
    }
}

fn ensure_registry_cid(capsule_cid: Cid, registry_cid: Cid) -> Result<(), Verdict> {
    if capsule_cid != registry_cid {
        Err(reject(Stage::VersionPin, "registry_cid mismatch"))
    } else {
        Ok(())
    }
}

fn stage_2_version_pin(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    ensure_ir_version(capsule.ir_version)?;
    ensure_vocab_version(capsule.vocab_version, registry.vocab_version)?;
    ensure_registry_cid(capsule.registry_cid, registry.cid())?;
    Ok(())
}

// ── Stage 3: Structure ─────────────────────────────────────────────────────

fn ensure_known_term(
    strand_idx: usize,
    strand: &Strand,
    registry: &TermRegistry,
) -> Result<(), Verdict> {
    if registry.get(&strand.term).is_none() {
        Err(reject(
            Stage::Structure,
            format!("unknown term `{}` at strand {strand_idx}", strand.term),
        ))
    } else {
        Ok(())
    }
}

fn ensure_term_arity(
    strand_idx: usize,
    strand: &Strand,
    registry: &TermRegistry,
) -> Result<(), Verdict> {
    let spec = registry.get(&strand.term).expect("checked in ensure_known_term");
    if spec.inputs.len() != strand.inputs.len() {
        Err(reject(
            Stage::Structure,
            format!("arity mismatch at strand {strand_idx}: `{}`", strand.term),
        ))
    } else {
        Ok(())
    }
}

fn check_strand_structure(
    strand_idx: usize,
    strand: &Strand,
    registry: &TermRegistry,
) -> Result<(), Verdict> {
    ensure_known_term(strand_idx, strand, registry)?;
    ensure_term_arity(strand_idx, strand, registry)?;
    Ok(())
}

fn validate_braid_dag(capsule: &Capsule) -> Result<(), Verdict> {
    if let Err(dag_err) = capsule.braid.validate() {
        Err(reject(Stage::Structure, format!("{dag_err:?}")))
    } else {
        Ok(())
    }
}

fn stage_3_structure(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    validate_braid_dag(capsule)?;
    for (strand_idx, strand) in capsule.braid.strands.iter().enumerate() {
        check_strand_structure(strand_idx, strand, registry)?;
    }
    Ok(())
}

// ── Stage 4: Types ─────────────────────────────────────────────────────────

fn check_slot_type(
    strand_idx: usize,
    strand: &Strand,
    slot_idx: usize,
    produced: &TypeTag,
    expected: &TypeTag,
) -> Result<(), Verdict> {
    if produced != expected {
        Err(reject(
            Stage::Types,
            format!(
                "strand {strand_idx} `{}` slot {slot_idx}: type mismatch",
                strand.term
            ),
        ))
    } else {
        Ok(())
    }
}

fn check_strand_types(
    strand_idx: usize,
    strand: &Strand,
    registry: &TermRegistry,
    out_types: &[&TypeTag],
) -> Result<(), Verdict> {
    let spec = registry.get(&strand.term).expect("checked in stage 3");
    for (slot_idx, &input_idx) in strand.inputs.iter().enumerate() {
        let produced = out_types[input_idx as usize];
        let expected = &spec.inputs[slot_idx];
        check_slot_type(strand_idx, strand, slot_idx, produced, expected)?;
    }
    Ok(())
}

fn stage_4_types(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    let out_types: Vec<&TypeTag> = capsule
        .braid
        .strands
        .iter()
        .map(|s| &registry.get(&s.term).expect("checked in stage 3").output)
        .collect();

    for (strand_idx, strand) in capsule.braid.strands.iter().enumerate() {
        check_strand_types(strand_idx, strand, registry, &out_types)?;
    }
    Ok(())
}

// ── Stage 5: Capability ────────────────────────────────────────────────────

fn check_single_grant_ambient(grant: &Capability, ambient: &[Capability]) -> Result<(), Verdict> {
    if !ambient.contains(grant) {
        Err(reject(
            Stage::Capability,
            format!("grant `{grant}` exceeds ambient authority"),
        ))
    } else {
        Ok(())
    }
}

fn check_grants_ambient(grants: &[Capability], ambient: &[Capability]) -> Result<(), Verdict> {
    for grant in grants {
        check_single_grant_ambient(grant, ambient)?;
    }
    Ok(())
}

fn ensure_capability_granted(
    strand_idx: usize,
    term: &str,
    cap: &Capability,
    grants: &[Capability],
) -> Result<(), Verdict> {
    if !grants.contains(cap) {
        Err(reject(
            Stage::Capability,
            format!("strand {strand_idx} `{term}` requires undeclared capability `{cap}`"),
        ))
    } else {
        Ok(())
    }
}

fn check_strand_capability(
    strand_idx: usize,
    strand: &Strand,
    registry: &TermRegistry,
    grants: &[Capability],
) -> Result<(), Verdict> {
    let spec = registry.get(&strand.term).expect("checked in stage 3");
    if let Some(cap) = &spec.capability {
        ensure_capability_granted(strand_idx, &strand.term, cap, grants)?;
    }
    Ok(())
}

fn stage_5_capability(
    capsule: &Capsule,
    registry: &TermRegistry,
    ambient: &[Capability],
) -> Result<(), Verdict> {
    check_grants_ambient(&capsule.grants, ambient)?;
    for (strand_idx, strand) in capsule.braid.strands.iter().enumerate() {
        check_strand_capability(strand_idx, strand, registry, &capsule.grants)?;
    }
    Ok(())
}

// ── Stage 6: Effect ────────────────────────────────────────────────────────

fn check_confirm_policy(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    let needs_confirm = capsule.braid.strands.iter().any(|s| {
        matches!(
            registry.get(&s.term).expect("checked in stage 3").effect,
            EffectClass::Irreversible | EffectClass::Egress
        )
    });
    if needs_confirm && capsule.confirm != ConfirmPolicy::HumanConfirm {
        Err(reject(
            Stage::Effect,
            "irreversible/egress strand present without human-confirm policy",
        ))
    } else {
        Ok(())
    }
}

fn explore_inputs(
    strands: &[Strand],
    current_idx: usize,
    to_idx: usize,
    seen: &mut [bool],
    stack: &mut Vec<usize>,
) -> bool {
    for &target in &strands[current_idx].inputs {
        let target_idx = target as usize;
        if target_idx == to_idx {
            return true;
        }
        if target_idx < seen.len() && !seen[target_idx] {
            seen[target_idx] = true;
            stack.push(target_idx);
        }
    }
    false
}

fn reaches(strands: &[Strand], from_idx: usize, to_idx: usize) -> bool {
    if from_idx == to_idx {
        return true;
    }
    let mut seen = vec![false; strands.len()];
    let mut stack = vec![from_idx];
    while let Some(current_idx) = stack.pop() {
        if explore_inputs(strands, current_idx, to_idx, &mut seen, &mut stack) {
            return true;
        }
    }
    false
}

fn check_effect_pair(
    strands: &[Strand],
    first_idx: usize,
    second_idx: usize,
) -> Result<(), Verdict> {
    let upper = first_idx.max(second_idx);
    let lower = first_idx.min(second_idx);
    if !reaches(strands, upper, lower) {
        Err(reject(
            Stage::Effect,
            format!(
                "unordered irreversible/egress strands {first_idx} and {second_idx}: \
                 relative order undefined — wire an explicit dependency"
            ),
        ))
    } else {
        Ok(())
    }
}

fn check_effect_ordering(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    let effectful: Vec<usize> = capsule
        .braid
        .strands
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            matches!(
                registry.get(&s.term).expect("checked in stage 3").effect,
                EffectClass::Irreversible | EffectClass::Egress
            )
        })
        .map(|(i, _)| i)
        .collect();

    for (pos, &first_idx) in effectful.iter().enumerate() {
        for &second_idx in &effectful[pos + 1..] {
            check_effect_pair(&capsule.braid.strands, first_idx, second_idx)?;
        }
    }
    Ok(())
}

fn stage_6_effect(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    check_confirm_policy(capsule, registry)?;
    check_effect_ordering(capsule, registry)?;
    Ok(())
}

// ── Stage 7: Taint ─────────────────────────────────────────────────────────

fn fold_strand_incoming_exposure(strand: &Strand, exposure: &[Exposure]) -> Exposure {
    let mut incoming = Exposure::Public;
    for &input_idx in &strand.inputs {
        incoming = incoming.max(exposure[input_idx as usize]);
    }
    incoming
}

fn ensure_exposure_within_ceiling(
    strand_idx: usize,
    term: &str,
    incoming: Exposure,
    limit: Exposure,
) -> Result<(), Verdict> {
    if incoming > limit {
        Err(reject(
            Stage::Taint,
            format!(
                "strand {strand_idx} `{term}`: folded exposure {incoming:?} exceeds ceiling {limit:?}"
            ),
        ))
    } else {
        Ok(())
    }
}

fn check_egress_ceiling(
    strand_idx: usize,
    strand: &Strand,
    incoming: Exposure,
    ceiling: Option<Exposure>,
) -> Result<(), Verdict> {
    if let Some(limit) = ceiling {
        ensure_exposure_within_ceiling(strand_idx, &strand.term, incoming, limit)?;
    }
    Ok(())
}

fn check_strand_taint(
    strand_idx: usize,
    strand: &Strand,
    registry: &TermRegistry,
    exposure: &mut Vec<Exposure>,
) -> Result<(), Verdict> {
    let spec = registry.get(&strand.term).expect("checked in stage 3");
    let incoming = fold_strand_incoming_exposure(strand, exposure);
    check_egress_ceiling(strand_idx, strand, incoming, spec.egress_ceiling)?;
    exposure.push(spec.source_exposure.max(incoming));
    Ok(())
}

fn stage_7_taint(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    let mut exposure: Vec<Exposure> = Vec::with_capacity(capsule.braid.strands.len());
    for (strand_idx, strand) in capsule.braid.strands.iter().enumerate() {
        check_strand_taint(strand_idx, strand, registry, &mut exposure)?;
    }
    Ok(())
}

// ── Stage 8: Bounds ────────────────────────────────────────────────────────

fn check_cost_overflow(sum: Option<u64>) -> Result<u64, Verdict> {
    match sum {
        Some(val) => Ok(val),
        None => Err(reject(Stage::Bounds, "cost overflow")),
    }
}

fn accumulate_strand_cost(total: &mut u64, cost: u64) -> Result<(), Verdict> {
    let sum = total.checked_add(cost);
    *total = check_cost_overflow(sum)?;
    Ok(())
}

fn compute_total_cost(capsule: &Capsule, registry: &TermRegistry) -> Result<u64, Verdict> {
    let mut total: u64 = 0;
    for strand in &capsule.braid.strands {
        let spec = registry.get(&strand.term).expect("checked in stage 3");
        accumulate_strand_cost(&mut total, spec.cost)?;
    }
    Ok(total)
}

fn check_budget_limit(total_cost: u64, budget: u64) -> Result<(), Verdict> {
    if total_cost > budget {
        Err(reject(
            Stage::Bounds,
            format!("total cost {total_cost} exceeds budget {budget}"),
        ))
    } else {
        Ok(())
    }
}

fn stage_8_bounds(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    let total_cost = compute_total_cost(capsule, registry)?;
    check_budget_limit(total_cost, capsule.budget)?;
    Ok(())
}

// ── Pipeline Entry Point ────────────────────────────────────────────────────

fn verify_structure_and_types(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    stage_2_version_pin(capsule, registry)?;
    stage_3_structure(capsule, registry)?;
    stage_4_types(capsule, registry)?;
    Ok(())
}

fn verify_security_and_bounds(
    capsule: &Capsule,
    registry: &TermRegistry,
    ambient: &[Capability],
) -> Result<(), Verdict> {
    stage_5_capability(capsule, registry, ambient)?;
    stage_6_effect(capsule, registry)?;
    stage_7_taint(capsule, registry)?;
    stage_8_bounds(capsule, registry)?;
    Ok(())
}

fn run_pipeline(
    bytes: &[u8],
    registry: &TermRegistry,
    ambient: &[Capability],
) -> Result<Cid, Verdict> {
    let capsule = stage_1_canonical_form(bytes)?;
    verify_structure_and_types(&capsule, registry)?;
    verify_security_and_bounds(&capsule, registry, ambient)?;
    Ok(capsule.cid())
}

/// Verify capsule BYTES against a registry and the ambient grant set the
/// principal actually holds. Bytes in, verdict out — the admission decision
/// is reproducible from the artifact alone (D9).
pub fn verify(bytes: &[u8], registry: &TermRegistry, ambient: &[Capability]) -> Verdict {
    match run_pipeline(bytes, registry, ambient) {
        Ok(capsule_cid) => Verdict::Admit { capsule_cid },
        Err(verdict) => verdict,
    }
}

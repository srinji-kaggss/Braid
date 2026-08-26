//! # braid-verify — deterministic admission (ADR-088 D3, U3–U5 #560)
//!
//! The fail-closed stage pipeline. Order is LOCKED (ADR-088 D3); a request
//! that survives one gate becomes the typed input of the next. Neither an AI
//! nor a reviewer is the enforcement mechanism — this crate is.
//!
//! Verdicts are typed and machine-readable. [`verify_compact`] additionally
//! projects an admitted canonical graph into the dense token form used by the
//! future hot runtime without turning that cache into a second wire format.

#![forbid(unsafe_code)]

pub mod decode;

use braid_capability::Capability;
use braid_ir::braid::Strand;
use braid_ir::term::{EffectClass, Exposure};
use braid_ir::{
    AdmissionTriad, CAPSULE_DOMAIN, Capsule, Cid, ConfirmPolicy, IR_VERSION, ProofState,
    TermRegistry, TermTable, TokenError, TokenProgram, TypeTag,
};

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

/// Deterministic admission result for the canonical capsule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Admit { capsule_cid: Cid },
    Reject { stage: Stage, reason: String },
}

/// Ephemeral, non-serializable result of independent admission plus compact
/// projection.
///
/// Safety and Capability are proven by this verifier invocation under the
/// supplied registry and ambient capability set. Justification remains
/// [`ProofState::Unknown`] until the snapshot-bound Flow planner supplies that
/// proof. Consequently the embedded triad deterministically returns `Defer`,
/// never `Execute`, in v0.
///
/// This value is not an authority credential. In particular, serializing the
/// one-byte triad would not preserve the external authority snapshot that
/// produced it.
#[derive(Debug, PartialEq, Eq)]
pub struct AdmittedCapsule {
    program: TokenProgram,
}

impl AdmittedCapsule {
    /// Dense, CID-bound projection of the admitted graph.
    pub fn program(&self) -> &TokenProgram {
        &self.program
    }

    /// Canonical capsule identity.
    pub const fn capsule_cid(&self) -> Cid {
        self.program.capsule_cid()
    }

    /// Consume the wrapper and return the compact projection.
    ///
    /// The returned projection remains data, not an authority credential.
    pub fn into_program(self) -> TokenProgram {
        self.program
    }
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
    let spec = registry
        .get(&strand.term)
        .expect("checked in ensure_known_term");
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
        .map(|strand| &registry.get(&strand.term).expect("checked in stage 3").output)
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
    capability: &Capability,
    grants: &[Capability],
) -> Result<(), Verdict> {
    // Canonical capsule decoding enforces strict grant ordering, so binary
    // search avoids one linear string scan per effectful strand.
    if grants.binary_search(capability).is_err() {
        Err(reject(
            Stage::Capability,
            format!(
                "strand {strand_idx} `{term}` requires undeclared capability `{capability}`"
            ),
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
    if let Some(capability) = &spec.capability {
        ensure_capability_granted(strand_idx, &strand.term, capability, grants)?;
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

fn is_irreversible_or_egress(effect: EffectClass) -> bool {
    matches!(effect, EffectClass::Irreversible | EffectClass::Egress)
}

fn check_confirm_policy(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    let needs_confirm = capsule.braid.strands.iter().any(|strand| {
        is_irreversible_or_egress(
            registry
                .get(&strand.term)
                .expect("checked in stage 3")
                .effect,
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

fn latest_effect_from_inputs(
    strand: &Strand,
    latest_visible_effect: &[Option<usize>],
) -> Option<usize> {
    strand
        .inputs
        .iter()
        .filter_map(|&input| latest_visible_effect[input as usize])
        .max()
}

/// Prove that dangerous effects form one explicit dependency chain.
///
/// Because strands are topologically ordered, checking that every dangerous
/// strand depends on the immediately previous dangerous strand is sufficient:
/// transitivity then orders every earlier pair. This is O(V + E), replacing
/// the former O(k² × (V + E)) pairwise graph search and its per-search
/// allocations.
fn check_effect_ordering(capsule: &Capsule, registry: &TermRegistry) -> Result<(), Verdict> {
    let mut latest_visible_effect = Vec::with_capacity(capsule.braid.strands.len());
    let mut previous_effect = None;

    for (strand_index, strand) in capsule.braid.strands.iter().enumerate() {
        let incoming_latest = latest_effect_from_inputs(strand, &latest_visible_effect);
        let effect = registry
            .get(&strand.term)
            .expect("checked in stage 3")
            .effect;

        if is_irreversible_or_egress(effect) {
            if let Some(previous_index) = previous_effect
                && incoming_latest != Some(previous_index)
            {
                return Err(reject(
                    Stage::Effect,
                    format!(
                        "unordered irreversible/egress strands {previous_index} and \
                         {strand_index}: relative order undefined — wire an explicit dependency"
                    ),
                ));
            }
            previous_effect = Some(strand_index);
            latest_visible_effect.push(Some(strand_index));
        } else {
            latest_visible_effect.push(incoming_latest);
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
        Some(value) => Ok(value),
        None => Err(reject(Stage::Bounds, "cost overflow")),
    }
}

fn accumulate_strand_cost(total: &mut u64, cost: u64) -> Result<(), Verdict> {
    let sum = total.checked_add(cost);
    *total = check_cost_overflow(sum)?;
    Ok(())
}

fn compute_total_cost(capsule: &Capsule, registry: &TermRegistry) -> Result<u64, Verdict> {
    let mut total = 0u64;
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

// ── Compact projection ─────────────────────────────────────────────────────

fn token_error_stage(error: TokenError) -> Stage {
    match error {
        TokenError::RegistryTooLarge { .. } | TokenError::ProgramTooLarge { .. } => Stage::Bounds,
        TokenError::RegistryMismatch { .. }
        | TokenError::UnknownTerm { .. }
        | TokenError::InvalidInput { .. }
        | TokenError::InvalidOutput { .. } => Stage::Structure,
    }
}

fn compile_compact_program(
    bytes: &[u8],
    capsule: &Capsule,
    registry: &TermRegistry,
) -> Result<TokenProgram, Verdict> {
    let table = TermTable::new(registry).map_err(|error| {
        let stage = token_error_stage(error);
        reject(stage, error.to_string())
    })?;
    let capsule_cid = Cid::compute(CAPSULE_DOMAIN, bytes);
    let triad = AdmissionTriad::new(
        ProofState::Proven,
        ProofState::Proven,
        ProofState::Unknown,
    );
    TokenProgram::derive_bound(capsule, &table, capsule_cid, triad).map_err(|error| {
        let stage = token_error_stage(error);
        reject(stage, error.to_string())
    })
}

// ── Pipeline Entry Point ───────────────────────────────────────────────────

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

fn admit_capsule(
    bytes: &[u8],
    registry: &TermRegistry,
    ambient: &[Capability],
) -> Result<Capsule, Verdict> {
    let capsule = stage_1_canonical_form(bytes)?;
    verify_structure_and_types(&capsule, registry)?;
    verify_security_and_bounds(&capsule, registry, ambient)?;
    Ok(capsule)
}

/// Independently verify canonical bytes and return the compact, CID-bound
/// projection.
///
/// The returned triad has Safety=`Proven`, Capability=`Proven`, and
/// Justification=`Unknown`. A runtime must therefore defer until a
/// snapshot-bound planner supplies the third proof.
pub fn verify_compact(
    bytes: &[u8],
    registry: &TermRegistry,
    ambient: &[Capability],
) -> Result<AdmittedCapsule, Verdict> {
    let capsule = admit_capsule(bytes, registry, ambient)?;
    let program = compile_compact_program(bytes, &capsule, registry)?;
    Ok(AdmittedCapsule { program })
}

/// Verify capsule bytes against a registry and the ambient grant set the
/// principal actually holds. Bytes in, verdict out — the admission decision
/// is reproducible from the artifact and explicit authority input.
///
/// This verdict-only path does not allocate the dense execution projection.
pub fn verify(bytes: &[u8], registry: &TermRegistry, ambient: &[Capability]) -> Verdict {
    match admit_capsule(bytes, registry, ambient) {
        Ok(_) => Verdict::Admit {
            capsule_cid: Cid::compute(CAPSULE_DOMAIN, bytes),
        },
        Err(verdict) => verdict,
    }
}

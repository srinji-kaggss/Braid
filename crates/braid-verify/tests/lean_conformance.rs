//! D-SA5 — structural Stage⇄Atom conformance check.
//!
//! Scope, stated honestly (see `spec/braid/KEEL-RECONCILIATION.md` for the
//! full reconciliation table and rationale): this asserts that every
//! `braid-verify` pipeline `Stage` maps to exactly one evidence atom from the
//! Excellent Code Framework's twenty-atom vocabulary (Keel's
//! `schema/atoms.json`, canonically `lean/ExcellentCode/Framework.lean`), and
//! that the mapped atom id is a real, non-drifted member of that set.
//!
//! This is a **structural** check — the same kind Keel's own
//! `src/conformance.mjs` runs for `schema/concepts.json` against the Lean
//! skeleton (leaf-set/shape, not a compiled proof). It does NOT invoke the
//! Lean toolchain and does NOT prove per-stage semantic equivalence to the
//! Lean predicates; per-atom proof-grafting is explicitly future work per
//! `Framework.lean`'s own header. The behavioral evidence that each stage's
//! reject path is real and load-bearing is established separately by
//! `acceptance.rs` + `spec/braid/MUTATION-LEDGER.md` (PB-01 W2) — this test
//! does not duplicate that; it links the pipeline to the Lean vocabulary.
//!
//! If this test goes RED: either a `Stage` was added/renamed without updating
//! the mapping below (fix the mapping + `KEEL-RECONCILIATION.md`), or an atom
//! id was mistyped (fix the id).

use braid_verify::Stage;

/// The twenty evidence atom ids, transcribed verbatim from Keel's
/// `schema/atoms.json` (n=1..20), which `Framework.lean`'s `Atom` inductive
/// must equal per its own header. Point-in-time transcription, not a live
/// fetch — Keel is an external, pinned dependency (#20), so this list is
/// re-checked by hand whenever Keel's atom schema changes.
const ATOM_IDS: [&str; 20] = [
    "referential_truth",
    "specification_fidelity",
    "type_soundness",
    "precondition_correctness",
    "postcondition_correctness",
    "invariant_preservation",
    "totality_or_controlled_partiality",
    "boundary_completeness",
    "compositionality",
    "minimal_sufficient_complexity",
    "algorithmic_efficiency",
    "state_minimization",
    "data_model_truth",
    "error_semantics",
    "security_by_construction",
    "idempotence",
    "concurrency_correctness",
    "observability",
    "testability_falsifiability",
    "change_locality",
];

/// Exhaustive match: if `Stage` grows a variant, this fails to compile until
/// the new variant is given an atom mapping.
fn atom_for_stage(stage: Stage) -> &'static str {
    match stage {
        Stage::CanonicalForm => "referential_truth",
        Stage::VersionPin => "precondition_correctness",
        Stage::Structure => "specification_fidelity",
        Stage::Types => "type_soundness",
        Stage::Capability => "security_by_construction",
        Stage::Effect => "postcondition_correctness",
        Stage::Taint => "invariant_preservation",
        Stage::Bounds => "totality_or_controlled_partiality",
    }
}

const ALL_STAGES: [Stage; 8] = [
    Stage::CanonicalForm,
    Stage::VersionPin,
    Stage::Structure,
    Stage::Types,
    Stage::Capability,
    Stage::Effect,
    Stage::Taint,
    Stage::Bounds,
];

#[test]
fn every_stage_maps_to_a_real_atom() {
    for stage in ALL_STAGES {
        let atom = atom_for_stage(stage);
        assert!(
            ATOM_IDS.contains(&atom),
            "Stage::{:?} maps to {:?}, which is not in the transcribed 20-atom set — \
             typo, or Keel's atom schema drifted from KEEL-RECONCILIATION.md",
            stage,
            atom
        );
    }
}

#[test]
fn stage_atom_mapping_is_injective_on_core_atoms() {
    // Not a spec requirement that stages map 1:1 (several atoms have no
    // dedicated stage), but if two stages ever collapse onto the same atom
    // that's worth a human look — it likely means one of them is redundant
    // or the mapping needs to be refined.
    let mut seen = std::collections::HashSet::new();
    for stage in ALL_STAGES {
        let atom = atom_for_stage(stage);
        assert!(
            seen.insert(atom),
            "Stage::{:?} maps to atom {:?}, already claimed by another stage",
            stage,
            atom
        );
    }
}

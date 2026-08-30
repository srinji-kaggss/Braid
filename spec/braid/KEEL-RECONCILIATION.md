# Keel ↔ Braid Reconciliation

> **Current provenance (2026-08-30)**: `braid.profile.json` records the
> historical U-SA/D32 mapping, but the old Node profile adapter is not an active
> CI lane. Keel is **not** checked into this repo, current CI does not clone it,
> and `scripts/keel-floor.sh` now refuses unless a native `keel` binary is
> explicitly installed. Issue #78 owns the hermetic distribution and evidence
> migration. This note deliberately does not recreate a drifting `keel/` copy.

## Purpose

Keel provides Braid's Tier-2 semantic floor — the "stop slop" discipline,
operationalized as the Excellent Code Framework's twenty evidence atoms
(`~/keel/schema/atoms.json`, canonically `lean/ExcellentCode/Framework.lean`
in the Keel repo). This note reconciles Braid's `braid-verify` 8-stage
admission pipeline against that atom vocabulary.

## Reconciliation: Braid Verifier ↔ Keel Atoms

The `braid-verify` crate's 8 pipeline stages (`Stage` enum, locked order) each
correspond to one core evidence atom. The mapping below is the one asserted,
structurally, by `crates/braid-verify/tests/lean_conformance.rs` (D-SA5):

| Stage           | Atom                       | Why                                                                    |
|-----------------|-----------------------------|-------------------------------------------------------------------------|
| `CanonicalForm` | `referential_truth`        | Rejects malleable/non-canonical CBOR — the bytes must mean exactly what they encode, no ambiguity. |
| `VersionPin`    | `precondition_correctness` | IR/vocab version must match before any later stage runs.               |
| `Structure`     | `specification_fidelity`   | Braid must reference known terms, correct arity, no forward refs, in-range outputs. |
| `Types`         | `type_soundness`           | Direct match — term input/output types must check.                     |
| `Capability`    | `security_by_construction` | Grants may not exceed ambient capability — confinement by structure, not runtime trust. |
| `Effect`        | `postcondition_correctness`| Irreversible effects require an explicit confirm postcondition.        |
| `Taint`         | `invariant_preservation`   | Path-taint tracking preserves the no-laundering invariant across strands. |
| `Bounds`        | `totality_or_controlled_partiality` | Budget bounds keep the computation within a controlled-partial domain instead of running unbounded. |

## D-SA5 — what is and isn't machine-checked

Per `spec/braid/SAFETY_ASSURANCE_CI_SPEC.md` §9: "the Lean skeleton is
axiom-free for `excellent_not_hallucinated` but the *Rust verifier's stage
semantics matching the Lean predicates* is not yet machine-checked."

`lean_conformance.rs` closes the **structural** half of that gap, mirroring
exactly what Keel's own `src/conformance.mjs` does for `schema/concepts.json`
against the Lean skeleton (a leaf-set/shape check, not a compiled proof):

- Every `Stage` variant is mapped to exactly one atom id (exhaustive match —
  the mapping cannot silently miss a stage if the enum grows).
- Every mapped atom id is checked against the literal 20-atom set transcribed
  from `schema/atoms.json` / `Framework.lean`, so a typo or renamed atom is
  caught mechanically.

**What this does not do**: it does not invoke the Lean toolchain, and it does
not prove per-stage *semantic* equivalence to the Lean predicates (that
per-atom proof-grafting is explicitly future work in Keel's own
`Framework.lean` header — "purchasable", not yet done). The *behavioral*
evidence that each stage's reject path is real and load-bearing already
exists independently: `crates/braid-verify/tests/acceptance.rs` plus
`spec/braid/MUTATION-LEDGER.md` (PB-01 W2) mutation-test all 8 stages. This
note and its conformance test add the missing structural link between that
behavioral evidence and the Lean atom vocabulary — nothing more, and it says
so.

---

*Documented as part of PB-01 safety floor hardening (2026-07-15).*

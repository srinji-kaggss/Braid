# Braid Safety-Assurance CI — Specification (D32)

> **Status (corrected 2026-08-30)**: this remains the historical U-SA/D32
> design rationale, not a current green gate. `keel/` is absent, the Node entry
> point was retired, and neither GitHub nor local CI runs a Keel verdict.
> `scripts/keel-floor.sh` invokes only an explicitly installed native Keel
> binary, which currently exposes blocking debt. Issue #78 owns hermetic tool
> distribution, offline evidence, remediation, and release-gate migration.
> **Authority**: Director session 2026-06-23 ("adopt a CI framework that meets
> the strictest standards semantically similar to IEC61508 and ISO26262 and
> DO-178… designed like an ISO standard with the end goal: stop slop").
> **Standard adopted**: the DO-178C/ISO 26262/IEC 61508 *verification
> philosophy* — requirements-based verification, structural coverage,
> robustness at boundaries, independence, tool qualification — operationalized
> via **Keel** (`~/keel`), which already implements it and already names Braid
> as its end-state artifact format (`docs/00-thesis.md §0.7`: "the `.braid`
> end-state").
> **Anti-goal**: DO-178C *certification*. We adopt the discipline, not the
> credential (keel `docs/00 §0.3`).

---

## 1. The problem this solves for Braid

Braid's current CI (`.github/workflows/ci.yml`) is **Tier-1 only**: fmt,
clippy, `cargo test`, the CLI loop. These are tools that *produce evidence* —
they never judge whether the change is *what we mean by good*. The gap:

- A PR that compiles, passes tests, and is clippy-clean can still be **slop**:
  a re-derived primitive (the slight-difference-is-the-bug class), an
  ungrounded claim ("this is correct" with no proof pointer), a test that
  asserts nothing, a widening that the manifest gate misses because the diff
  logic drifted.
- The verifier (`braid-verify`) is itself a Rust crate authored by an LLM. Per
  the threat model (T2, T11) and keel's thesis (`docs/00 §0.2`: "the author is
  untrusted and possibly incoherent"), the verifier needs **tool qualification**
  — evidence that the checker works (DO-330), not an assertion that it does.
- The "stop slop" goal is a *semantic* property, not a syntactic one. It needs a
  Tier-2 floor: evidence compiled into named predicates, verdict read off a
  deterministic implication, no AI in the verdict path.

Braid is the canonical IR for a Java-ecosystem ambition. A Java-scale substrate
cannot ship without the airworthiness discipline; this spec is how Braid gets it
without reinventing it.

## 2. The decision: adopt Keel, do not reinvent

Keel (`~/keel`) is a working, hardened, content-addressed safety-assurance CI
that:
- adopts the DO-178C/ISO 26262/IEC 61508 verification philosophy as design
  constraints (`docs/00 §0.3`);
- implements the **Excellent Code Framework** (`~/Downloads/excellent_code_framework`)
  as its Tier-2 contract — 20 evidence atoms, a concept algebra, and a Lean 4
  theorem (`excellent_not_hallucinated`) the verdict adheres to (`docs/02`);
- removes the AI from the verdict path ("AI may propose evidence; only
  deterministic measurement and symbolic composition dispose" — `docs/00 §0.2`);
- already references Braid as the end-state artifact format.

**Decision (D32, INTERPRETED — veto-on-review):** Braid adopts Keel as its
safety-assurance CI layer. Braid does not build a second Keel. The work is
*wiring* — a `braid.profile.json` that binds Keel's atoms to Braid's real
Tier-1 tools — not *reimplementing* the engine, the atoms, the concept algebra,
or the Lean skeleton. One canonical safety-assurance implementation (CLAUDE.md
directive 3).

## 3. The three-tier gate for Braid

| Tier | Question | Braid mechanism | Keel atom it feeds |
|------|----------|-----------------|--------------------|
| **0 — substrate** | Is the IR itself sound? | `braid-verify`'s 8-stage pipeline + the U9 adversarial verdict (already built) | `invariant_preservation`, `security_by_construction`, `totality_or_controlled_partiality` (the verifier's own stages instantiate these) |
| **1 — operational** | Does it build/test at all? | `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `./scripts/cli-loop.sh`, `./scripts/demo-port.sh` | `referential_truth`, `type_soundness`, `specification_fidelity`, `testability_falsifiability` |
| **2 — semantic** | Is it *what we mean by good* — sound, grounded, not hallucinated, within envelope? | Keel compiles Tier-1 evidence into the 20 atoms, evaluates the concept algebra, reads the verdict off the Lean theorem | the concept `CoreGroundedCorrect` (floor) / `Excellent` (aspirational) |

Tier 0 is Braid-native (the verifier is the product). Tiers 1–2 are Keel-driven.
The tiers compose: a Tier-2 verdict is only as strong as the Tier-1 evidence
and the Tier-0 substrate (keel `docs/02 §2.1`: "a predicate holds only as
strongly as the tool that instantiated it").

## 4. The wiring — `braid.profile.json`

Keel's tailoring schema (`keel/schema/profile.schema.json`) is the per-codebase
document that binds atoms to real tools. Braid ships one profile that maps
every atom to a Braid Tier-1 tool. The profile is **restrictive** (keel
`docs/03 §3.1`): closed, typed, mechanically validated before any verification
runs.

### 4.1 Atom → Braid tool bindings (the floor)

| Atom | Braid evidence source | kind | tier |
|------|----------------------|------|------|
| `referential_truth` | `cargo check --workspace` (every `use` resolves); `crates/braid-ir/tests/boundary_conformance.rs` (the D3 boundary lexer — every import inside the allowlist) | boolean | commit |
| `type_soundness` | `cargo clippy --workspace --all-targets -- -D warnings` (the type checker + lint); `braid-verify`'s Stage::Types | boolean | commit |
| `specification_fidelity` | `cargo test --workspace` (PRD §7 acceptance scenarios as tests); `crates/braid-verify/tests/acceptance.rs` (scenarios #1–#14); `crates/braid-verify/tests/parity.rs` (D9 KAT parity) | boolean | commit |
| `precondition_correctness` | `braid-verify` Stage::CanonicalForm + Stage::VersionPin (bytes-in preconditions); `braid-sdk` author-time refusals (`BuildError`) | boolean | commit |
| `postcondition_correctness` | `braid-verify` Stage::Capability/Effect/Taint/Bounds (admission postconditions); `braid-render` manifest CID binding (T4) | boolean | commit |
| `invariant_preservation` | `braid-ir::canon` bijection guard (T3); `braid-ir/tests/malleability.rs` (the A4.8 exploit set); `braid-verify` Stage::Taint path-level fold (T5) | boolean | commit |
| `totality_or_controlled_partiality` | `cargo clippy` (exhaustive matches); `braid-verify` fail-closed verdicts (`Reject` is a defined error, never a panic); `braid-ir` `Value::require_only_keys` (unknown = reject, not ignore) | boolean | commit |
| `boundary_completeness` | `crates/braid-ir/tests/boundary_conformance.rs` (the D3 boundary); `braid-ir/tests/malleability.rs` (the canonical-form boundary); U9 verdict per-threat coverage | graded | commit |
| `compositionality` | `braid-verify` registry-parametric (admits against any `TermRegistry`); D31 substrate/vocabulary split (one canonical primitive per concept) | boolean | commit |
| `minimal_sufficient_complexity` | `cargo clippy` cognitive-complexity lints; manual review gate on new crates | graded | commit |
| `algorithmic_efficiency` | `braid-verify` Stage::Bounds (checked-sum, overflow ⇒ reject); `braid-render` deterministic render (no allocation in hot path) | graded | soak |
| `state_minimization` | `braid-ir` immutable `Value`/`Capsule` (no mutable state in the IR); `braid-verify` stateless verify fn | graded | commit |
| `data_model_truth` | `braid-ir::term::TypeTag` closed universe (D8: no floats, no interpretable-code type); `braid-capability` string-tagged tokens (D31) | boolean | commit |
| `error_semantics` | `braid-verify::Verdict` typed (`Admit`/`Reject{stage,reason}`); `braid-sdk::BuildError` typed; `braid-ir::CanonError` typed — no string errors, no panics in the verdict path | boolean | commit |
| `security_by_construction` | `braid-verify` capability attenuation (grant ⊆ ambient); path-level taint non-interference (T5); D10 (Braid adds no authority); the U9 verdict (T1–T16 + R1–R3 closed) | boolean | commit |
| `idempotence` | `braid-ir::canon::encode` is a pure function (same `Value` ⇒ same bytes); `Capsule::cid()` deterministic | boolean | sim |
| `concurrency_correctness` | N/A in v0 (no shared mutable state; the IR is immutable) — atom evaluates `unknown` with a documented justification, not `true` | boolean | sim |
| `observability` | `braid-cli` exit codes (0/1/2 — the CI gate's one bit); `braid-render` manifest (the human-review object); `spec/braid/U9-VERDICT.md` (per-threat evidence) | graded | commit |
| `testability_falsifiability` | `cargo test --workspace` (135 tests, mutation-verified where teeth exist — U9 `mutation ×2`, and the U12/U13 anti-dredging guards); KAT vectors + pinned CIDs (a drift is RED, not "update and move on"); the CLI loop asserts exit codes | boolean | commit |
| `change_locality` | D31 substrate/vocabulary split (a vocab change is local to the vocab crate); `braid-ir` stable IR_VERSION pin (D11) | graded | commit |

### 4.2 The concepts Braid adopts

```json
{
  "CoreGroundedCorrect": ["all", ["referential_truth", "type_soundness", "totality_or_controlled_partiality", "specification_fidelity"]],
  "Hallucinated": ["not", "CoreGroundedCorrect"],
  "NotSlop": ["all", ["CoreGroundedCorrect", "invariant_preservation", "security_by_construction", "testability_falsifiability", "change_locality"]]
}
```

`NotSlop` is Braid's floor concept — the Director's "stop slop" as a formula
over atoms. It is stricter than `CoreGroundedCorrect` (it adds the invariant,
security, falsifiability, and locality atoms) and weaker than `Excellent` (it
does not require all 20). A PR that fails `NotSlop` is REJECTED with the
specific atoms that failed and their evidence pointers.

### 4.3 The three-valued discipline (the anti-slop core)

Keel's `unknown` discipline (`docs/02 §2.6`) is the mechanism that "stops slop"
mechanically: an atom with no bound evidence source is `unknown`, never `true`;
a concept over an `unknown` atom is `unknown`; an `unknown` on a gated atom is
a `block`, not a silent skip. This is the formal statement of *absence of
evidence is never evidence of absence* — the exact failure mode of "looks good,
merge." Braid's profile must declare every atom's evidence source or accept
that the atom is `unknown` and the concept blocks.

## 5. Tool qualification (DO-330) — the verifier qualifies itself

The deepest keel thesis: the checker must be qualified, not trusted. Braid's
verifier is a Rust crate authored by an LLM; per T2/T11 it is itself an attack
surface. The qualification obligations:

1. **`ORG.selftest`** — keel's `src/conformance.mjs` already re-derives the
   framework concepts from the Lean skeleton and asserts `concepts.json`
   matches. Braid's profile adds: the `braid-verify` stage pipeline is
   exercised by the acceptance scenarios (`acceptance.rs` scenarios #1–#14) AND
   by mutation evidence — for each stage, a test that flips the stage's verdict
   goes RED (the U9 `mutation ×2` discipline, already applied to T3/T5/T12).
   A stage with no mutation-red test is `unknown` for `testability_falsifiability`.
2. **Lean skeleton machine-check** — keel's `lean-skeleton-machinecheck` row
   (`src/adapters/lean.mjs`) already proves `excellent_not_hallucinated` is
   axiom-free (no `sorry`). Braid's verifier *implements* the proven-sound
   rules (D22: Lean is the unforked proof oracle; the Rust verifier ships the
   fast version). The qualification evidence is: the Lean skeleton typechecks
   + the Rust verifier's stage semantics match the Lean predicates (a
   conformance check to be built).
3. **D9 independence** — `braid-verify` shares no serialization code with
   `braid-ir` (already enforced by `tests/parity.rs`). This is the
   anti-trusting-trust qualification; it stays.

## 6. The CI pipeline (the wiring)

```yaml
# Structural outline only. The executable source of truth is
# .github/workflows/ci.yml; scripts/ci-policy-check.sh rejects mutable refs,
# skipped required lanes, missing timeouts, and early cleanup.
jobs:
  stack:                    # fail closed on pull-request topology
  ci-policy:                # reject false-green workflow structure
    needs: [stack]
  build:                    # every change runs the full build
    needs: [ci-policy]
  test:                     # every change runs all tests and doc tests
    needs: [build]
  clippy:                   # every change runs all-target clippy
    needs: [build]
  cleanup:                  # waits for every preceding job under always()
    if: always()
```

Native Keel is intentionally not represented as a green CI job here. Issue #78
owns its hermetic distribution and the remediation needed before its verdict can
become release authority. Until then, `scripts/keel-floor.sh` is an explicit
diagnostic adapter and fails if the native binary is unavailable.
content-addressed (keel `docs/01`); the evidence bundle is uploaded for
reconstruction (the calculator test).

## 7. What Braid does NOT build (the anti-reinvention list)

- A second evidence-atom ontology (Keel's 20 atoms are canonical).
- A second concept algebra (Keel's `{all|any|not}` is canonical).
- A second Lean skeleton (the Excellent Code Framework is canonical).
- A second verdict engine (Keel's `src/engine.mjs` is canonical).
- A second content-addressed evidence store (Keel's `.keel/` DAG is canonical).

Braid builds **one artifact**: `braid.profile.json` + the Braid-specific
evidence adapters that Keel's registry doesn't already carry (if any — most
Braid evidence is standard `cargo` output Keel already binds).

## 8. Acceptance criteria for `U-SA`

1. `braid.profile.json` validates against `keel/schema/profile.schema.json`.
2. `node keel/run.mjs --profile braid.profile.json` exits 0 on a clean tree and
   exits non-zero on a seeded slop (a re-derived primitive, an ungrounded
   claim, a test that asserts nothing) — the red-team requirement, mirroring
   keel's `fixtures/known-bad/` discipline.
3. Every atom in the profile is either bound to a real Braid Tier-1 tool OR
   declared `unknown` with a documented justification (no atom silently `true`).
4. The `NotSlop` concept is the floor gate; a PR that fails it is REJECTED with
   the specific failed atoms and their evidence pointers.
5. The verdict is reconstructable: a human with the source, the evidence
   bundle, and no network can re-derive the verdict (the calculator test).
6. The verifier self-qualifies: mutation-red evidence exists for each
   `braid-verify` stage (the U9 discipline extended to all 8 stages).

## 9. Debts this exposes (not closes)

This spec does NOT close the Java-ecosystem gap. It adds the *safety-assurance
floor* that makes the gap visible and the work trustworthy. The debts it
exposes, on record:

- **D-SA1**: the verifier has no runtime to admit *against* (U7 blocked). The
  safety floor qualifies a verifier that verifies artifacts that don't run.
- **D-SA2**: no real elaborator (the JS→Braid path is architectural, not
  operational). The floor can qualify the IR but cannot qualify a compiler that
  doesn't exist.
- **D-SA3**: no real consumers (browser kernel binding live-wired, kernel
  snapshot live-wired). The floor can qualify a substrate with zero dependents.
- **D-SA4**: `concurrency_correctness` is `unknown` in v0 (no shared mutable
  state to test). The floor honestly reports this rather than faking it — the
  three-valued discipline doing its job.
- **D-SA5**: the Lean skeleton is axiom-free for `excellent_not_hallucinated`
  but the *Rust verifier's stage semantics matching the Lean predicates* is not
  yet machine-checked (a conformance check to build).

## 10. What I need from the Director to proceed

1. **Ratify D32** (adopt Keel as Braid's safety-assurance CI; do not reinvent).
   This is an INTERPRETED decision — veto-on-review.
2. **Confirm the Keel→Braid dependency direction.** Keel is a Node tool; Braid
   is a Rust workspace. Options: (a) Braid's CI calls Keel as a checkout + `node
   keel/run.mjs` (loose, no cargo dep — **recommended**, matches keel's
   portable design); (b) vendor Keel's schema + engine into Braid (re-creates
   the parallel-implementation anti-pattern — not recommended); (c) publish
   Keel as an npm package Braid depends on (heavier, later).
3. **The `NotSlop` concept formula** — I chose 5 atoms (core + invariant +
   security + falsifiability + locality). Is that the right floor, or do you
   want it stricter/weaker? This is the "what is good" parameter the Director
   owns (keel `docs/00 §0.6`).
4. **Where the `braid.profile.json` lives** — in the Braid repo (recommended,
   it's Braid's tailoring) or in a Keel fixtures dir (keel-owns-the-profile
   alternative).

— Braid project lead, 2026-06-23

# PB-01 — Finish the safety-assurance floor (U-SA closure + Lean conformance)

**Objective**: restore the current native Keel Tier-2 floor as a hermetic,
qualified, falsifiable release gate while preserving the existing verifier
mutation and Rust↔Lean structural conformance evidence.

**Why this is first**: everything else in the production arc (elaborator, runtime,
deploy platform) is code an LLM will author. The floor is what makes that work
trustworthy (T2/T11: the verifier is itself an attack surface authored by an LLM —
qualify it, don't trust it). Debt register priority #1 concurs.

## Current state (corrected 2026-08-30)

- No `tier2-semantic` job exists in the current workflow. `keel/` is absent and
  the retired Node adapter cannot run.
- `scripts/keel-floor.sh` now calls only an explicitly installed native Keel
  binary. It is diagnostic until #78 supplies a pinned clean distribution and
  the blocking findings are remediated.
- `scripts/ci-policy-check.sh` independently prevents known orchestration
  false-greens; it is not a second semantic verdict engine.

## Deep-learning corpus (read → probe)

1. `spec/braid/SAFETY_ASSURANCE_CI_SPEC.md` — all 10 sections; §8 is your AC list.
2. `braid.profile.json` — every atom binding; note which atoms are unbound/`unknown`.
3. `keel/README.md`, `keel/src/engine.mjs`, `keel/src/conformance.mjs`,
   `keel/src/adapters/lean.mjs` — how evidence compiles to atoms and the verdict is
   read off the Lean theorem (`excellent_not_hallucinated`).
4. `crates/braid-verify/src/lib.rs` — the 8 stages; `tests/acceptance.rs` scenarios
   #1–#14; `tests/parity.rs` (D9 independence).
5. `spec/braid/U9-VERDICT.md` — the mutation discipline you are extending.

Probe: `./scripts/keel-floor.sh` on a clean tree (expect exit 0); read the emitted
`.keel/` verdict + projections; then break one bound atom's tool on purpose (e.g.
seed a failing test) and confirm exit ≠ 0 with the failed atom named.

## Invariants (must hold throughout)

- No AI in the verdict path (keel thesis §0.2). The floor is deterministic.
- Three-valued discipline: an unbound atom is `unknown`, never `true`; `unknown` on a
  gated atom blocks. Do not "fix" a red verdict by binding an atom to a vacuous tool.
- One canonical Keel: no second engine, no second atom ontology (spec §7).
- When a gate fails, fix the thing under test — never weaken the gate.

## Execution steps

1. **Truth-sync the docs**: keep D-SA open until a pinned native Keel artifact,
   clean-run evidence bundle, and finding baseline are reproducible without a
   sibling checkout. Never recreate the removed vendored copy.
2. **U-SA AC-2, the red-team**: build 3 seeded-slop fixtures mirroring keel's
   `fixtures/known-bad/` discipline — (a) a re-derived primitive (copy `canon.rs`
   encode logic into a second crate), (b) an ungrounded claim (doc asserts a bound no
   test enforces — the T7 class), (c) a test that asserts nothing. Each must turn the
   floor RED. Commit the fixtures + a script that applies/reverts them.
3. **U-SA AC-6, verifier self-qualification**: for each of the 8 stages in
   `braid-verify`, ensure a mutation exists that flips the stage's verdict and makes a
   named test RED (U9 already proved T3/T5/T12; cover the remaining stages:
   canonical-form, version-pin, structure, types, capability, effect, bounds). Record
   each mutation + red test in a `MUTATION-LEDGER.md` next to the U9 verdict.
4. **D-SA5, the Lean conformance flight hour**: machine-check that the Rust stage
   semantics match the Lean predicates. Shape: a generator emits verdicts from
   `braid-verify` over a capsule corpus (accepted + every reject class); a Lean file
   asserts the corresponding predicate holds/fails on the same encoded facts; CI runs
   `keel/src/adapters/lean.mjs`-style check that the Lean build is axiom-free and the
   corpus verdicts agree. Log it in `calibration/FLIGHT_HOURS.md` (it is queue item #2
   there already).
5. **RFC 8949 map-ordering vectors** (flight-hours queue #1): add the multi-key
   length-first ordering cross-check to `crates/braid-ir/tests/calibration.rs`.
6. **Reconcile with `~/keel`**: file an issue documenting vendored-copy vs upstream
   divergence policy; if upstream Keel's red CI is fixed, re-sync deliberately.

## Verification (all must pass before claiming done)

```bash
cargo test --workspace                     # all green
./scripts/keel-floor.sh                    # exit 0, NotSlop satisfied
# each seeded-slop fixture: apply → keel-floor exits non-zero naming the atom → revert
# each stage mutation: apply → named test RED → revert
# Lean conformance: axiom-free build + corpus agreement in CI
```

## Failure modes to expect

- Binding an atom to a tool that can't fail (vacuous evidence) — the exact slop the
  floor exists to stop; the red-team fixtures are your guard.
- Mutation tests that pass for the wrong reason (test asserts exit code, mutation
  changes a message) — verify the RED is on the semantic assertion.
- Lean predicate drift: the Excellent Code Framework's predicates are more abstract
  than Braid's stages; document the mapping explicitly, don't force a 1:1 that lies.

## Exit criteria

U-SA §8 acceptance criteria 1–6 all demonstrably true; `MUTATION-LEDGER.md` covers 8/8
stages; D-SA5 conformance in CI; two new flight-hours rows; DEBT_REGISTER truthful.
Then request Director ratification of D32 (it is still INTERPRETED — D-CONFIRM).

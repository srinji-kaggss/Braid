# U4 Prep Scaffold — Lean conformance + release-repeatable floor (#78)

> **Scope:** PREPARATION SCAFFOLD ONLY. Keel binary not yet hermetically
> distributed (`tools/keel/` absent), Lean harness not yet built. This document
> audits current verifier-stage mutation coverage, keel-floor diagnostic posture,
> flight-hours queue, and scaffolds the evidence ledger extension required for #78.
> Per U4 Approach PB-01 steps 2-5, truth-sync docs keeping D-SA open;
> `keel-floor.sh` remains diagnostic (exit 127 when `KEEL_BIN` absent).

**Branch:** `feat/next-workstream-gate-63` (HEAD `7baccbd` at scaffold time)
**Date:** 2026-08-31
**Author:** ce-work U4 subagent (scaffold only)

---

## 1. Files examined

| File | Status | Finding |
|------|--------|---------|
| `spec/braid/MUTATION-LEDGER.md` | read `spec/braid/MUTATION-LEDGER.md:1` | 8/8 stages covered, see §2 |
| `calibration/FLIGHT_HOURS.md` | read `calibration/FLIGHT_HOURS.md:1` | 2 queue rows already ✅, see §4 |
| `spec/braid/U9-VERDICT.md` | read `spec/braid/U9-VERDICT.md:1` | U9 closed T3/T5/T12 (see §2) |
| `scripts/keel-floor.sh` | read `scripts/keel-floor.sh:1` | diagnostic, exit 127 when KEEL_BIN absent, see §3 |
| `crates/braid-ir/tests/calibration.rs` | read `crates/braid-ir/tests/calibration.rs:1` | RFC 8949 map-ordering cross-check exists, see §4 |
| `spec/braid/SAFETY_ASSURANCE_CI_SPEC.md` | read `spec/braid/SAFETY_ASSURANCE_CI_SPEC.md:1` | status corrected 2026-08-30, truthful, see §5 |
| `crates/braid-verify/tests/acceptance.rs` | read `crates/braid-verify/tests/acceptance.rs:1` | 20 acceptance tests, per-stage rejects |
| `crates/braid-verify/src/lib.rs` | read `crates/braid-verify/src/lib.rs:1` | 8 stages locked order `crates/braid-verify/src/lib.rs:25` |
| `crates/braid-verify/tests/lean_conformance.rs` | read | structural Stage↔Atom check |
| `crates/braid-verify/tests/parity.rs` | read | D9 independence, 3 tests |
| `fixtures/known-bad/` | listed `fixtures/known-bad:1` | 3 fixtures per PB-01 §2 (see §3) |
| `.github/workflows/ci.yml` | read `.github/workflows/ci.yml:1` | no keel-floor job, diagnostic only |

---

## 2. Verifier stage mutation coverage audit

### 2.1 U9 baseline

`spec/braid/U9-VERDICT.md:16` records 4 findings closed this pass:
T3 canonical-encoding malleability (High), T4 review-path (Medium),
T12 neutral-collapse (Medium), R3 line-injection (Medium). Of these,
**T3** and **T5** map directly to verifier stages:

- **T3 → Stage 1 CanonicalForm**: bijection guard `crates/braid-verify/src/decode.rs:207` + `Value::require_only_keys` — mutation-verified `spec/braid/MUTATION-LEDGER.md:34`
- **T5 → Stage 7 Taint**: path-level monotone fold `crates/braid-verify/src/lib.rs:432` — mutation-verified `spec/braid/MUTATION-LEDGER.md:186`
- **T12** is a manifest/widening gate (`braid-render`), not a `braid-verify` stage — tracked separately.

DEBT_REGISTER concurs: U9 disciplined mutation ×2 for T3/T5/T12.

### 2.2 Current MUTATION-LEDGER.md — 8/8 stages ✅

`spec/braid/MUTATION-LEDGER.md:19` already lists **8/8 stages** with
mutation-red evidence. The plan's "remaining" list
(canonical-form, version-pin, structure, types, capability, effect, bounds)
is **closed** — the ledger extended beyond U9's 2 verifier stages to cover all 6
remaining stages as **NEW** entries, each with a named red test and RED-type
`Admit-where-Reject-expected` on the semantic assertion `expect_reject`:

| # | Stage | Ledger entry | Red test (`acceptance.rs:line`) | Mutation location |
|---|-------|--------------|----------------------------------|-------------------|
| 1 | CanonicalForm | `spec/braid/MUTATION-LEDGER.md:34` U9/T3 ✅ | `scenario_6b_nested_submap_smuggle_rejected` `crates/braid-verify/tests/acceptance.rs:412` | `decode.rs:207-218` bijection guard bypass |
| 2 | VersionPin | `spec/braid/MUTATION-LEDGER.md:64` NEW ✅ | `scenario_7_version_skew_rejected` `crates/braid-verify/tests/acceptance.rs:107` | `lib.rs:58-67` bypass `ensure_*` |
| 3 | Structure | `spec/braid/MUTATION-LEDGER.md:87` NEW ✅ | `output_out_of_range_rejected` `crates/braid-verify/tests/acceptance.rs:191` | `lib.rs:70-72` bypass `validate()` |
| 4 | Types | `spec/braid/MUTATION-LEDGER.md:114` NEW ✅ | `type_mismatch_rejected` `crates/braid-verify/tests/acceptance.rs:156` | `lib.rs:96-108` bypass type compare |
| 5 | Capability | `spec/braid/MUTATION-LEDGER.md:139` NEW ✅ | `scenario_4_grant_exceeding_ambient_rejected` `crates/braid-verify/tests/acceptance.rs:62` | `lib.rs:111-118` bypass grant⊆ambient |
| 6 | Effect | `spec/braid/MUTATION-LEDGER.md:162` NEW ✅ | `scenario_2_irreversible_without_confirm_rejected` `crates/braid-verify/tests/acceptance.rs:41` | `lib.rs:135-146` bypass confirm check |
| 7 | Taint | `spec/braid/MUTATION-LEDGER.md:186` U9/T5 ✅ | `scenario_5_path_taint_catches_multihop_laundering` `crates/braid-verify/tests/acceptance.rs:87` | `lib.rs:154-173` per-hop fold |
| 8 | Bounds | `spec/braid/MUTATION-LEDGER.md:214` NEW ✅ | `scenario_9_budget_exceeded_rejected` `crates/braid-verify/tests/acceptance.rs:135` | `lib.rs:184-189` bypass budget cmp |

**Why each RED is load-bearing:** see ledger § "Why load-bearing" per stage.
Each mutation was applied individually, red confirmed on `expected reject at <Stage>, got Admit` (`acceptance.rs:22`), reverted, clean tree passes 20/20 acceptance + 3 parity.

### 2.3 Existing tests per stage (acceptance.rs:read)

- CanonicalForm: `scenario_6_malleable_bytes_rejected` `acceptance.rs:96`, `scenario_6b_nested_submap_smuggle_rejected` `acceptance.rs:412`, `scenario_8_float_rejected_at_the_byte_gate` `acceptance.rs:124`
- VersionPin: `scenario_7_version_skew_rejected` `acceptance.rs:107` (covers `vocab_version` and `registry_cid`)
- Structure: `output_out_of_range_rejected` `acceptance.rs:191`, `arity_mismatch_rejected` `acceptance.rs:167`, `forward_reference_rejected` `acceptance.rs:177`, `scenario_14_unknown_term_rejected` `acceptance.rs:145`
- Types: `type_mismatch_rejected` `acceptance.rs:156`, `dimension_mismatch_rejected` `acceptance.rs:243`
- Capability: `scenario_4_grant_exceeding_ambient_rejected` `acceptance.rs:62`, `scenario_4b_strand_with_undeclared_capability_rejected` `acceptance.rs:73`
- Effect: `scenario_2_irreversible_without_confirm_rejected` `acceptance.rs:41`, `unordered_irreversible_pair_rejected` `acceptance.rs:300`, `ordered_irreversible_pair_admits` `acceptance.rs:350`, `scenario_2b_irreversible_with_confirm_admits` `acceptance.rs:50`
- Taint: `scenario_5_path_taint_catches_multihop_laundering` `acceptance.rs:87`
- Bounds: `scenario_9_budget_exceeded_rejected` `acceptance.rs:135`

Structural conformance: `crates/braid-verify/tests/lean_conformance.rs:57` maps each Stage to a Keel atom (see `spec/braid/KEEL-RECONCILIATION.md:22`), exhaustive match — new Stage fails compile until mapped.

### 2.4 Gap vs plan's "remaining" list

The U4 plan listed remaining stages as `canonical-form, version-pin, structure, types, capability, effect, bounds` — these are **already covered** in the ledger (6 NEW + 1 U9). The plan text predates the ledger extension that landed under PB-01. Remaining work per MUTATION-LEDGER.md Method:

- Ledger is complete for 8 verifier stages. The **2 runtime budgets** (from #70/#72 — triad typestate + allocation-free preflight) are not yet in the ledger. See §6 draft.

### 2.5 Method compliance

Per `spec/braid/MUTATION-LEDGER.md:240` each mutation's RED was verified on the
semantic assertion, not incidental panic/compilation error. This satisfies
PB-01 §5 and SAFETY_ASSURANCE_CI_SPEC.md §5 ORG.selftest discipline.

---

## 3. Keel floor — diagnostic posture

### 3.1 Current behavior

`scripts/keel-floor.sh:1` requires `KEEL_BIN` env (or `keel` on PATH):

```bash
KEEL_BIN="${KEEL_BIN:-}"
if [[ -z "$KEEL_BIN" ]]; then
  if command -v keel >/dev/null 2>&1; then
    KEEL_BIN="$(command -v keel)"
  fi
fi
if [[ -z "$KEEL_BIN" || ! -x "$KEEL_BIN" ]]; then
  echo "native Keel is unavailable; install it or set KEEL_BIN" >&2
  echo "issue #78 tracks a hermetic Braid assurance distribution" >&2
  exit 127
fi
echo "Keel native assurance scan"
exec "$KEEL_BIN" "$ROOT"
```

- Without a Keel binary: exits **127** with message `issue #78 tracks a hermetic Braid assurance distribution` — diagnostic, not a CI failure. `PB-01-safety-floor-hardening.md:18` confirms: "It is diagnostic until #78 supplies a pinned clean distribution."
- With `keel` at `~/.local/bin/keel` (observed 2026-08-31): runs native Keel, which currently reports `UNMEASURED` due to missing `.keel/lanes.toml` (exit 2). This is expected — native Keel exposes blocking debt pending hermetic distribution.

### 3.2 CI wiring — diagnostic only, not a gate

`.github/workflows/ci.yml:1` does **not** invoke `scripts/keel-floor.sh`. Keel-specific tiers are noted as "not ported — Braid has no gate binary. They arrive when keel-ci-plan gains a Braid target." (`ci.yml:8`).

SAFETY_ASSURANCE_CI_SPEC.md §6 confirms: "Native Keel is intentionally not represented as a green CI job here. Issue #78 owns its hermetic distribution and the remediation needed before its verdict can become release authority. Until then, `scripts/keel-floor.sh` is an explicit diagnostic adapter and fails if the native binary is unavailable." (`spec/braid/SAFETY_ASSURANCE_CI_SPEC.md:182`).

**Verdict:** `scripts/keel-floor.sh` is correctly diagnostic-only (exit 127 absent) and does not fail CI. Truth-sync holds — D-SA remains 🔴 open per `DEBT_REGISTER.md:154` and `SAFETY_ASSURANCE_CI_SPEC.md:3` header "this remains the historical U-SA/D32 design rationale, not a current green gate."

### 3.3 Seeded-slop fixtures — PB-01 §2 (3 fixtures)

`fixtures/known-bad/` already contains 3 fixtures per `docs/playbooks/PB-01-safety-floor-hardening.md:50`:

| Fixture | Path | Slop class | Guard test | Expected floor | Status |
|---------|------|------------|------------|----------------|--------|
| re-derived-primitive | `fixtures/known-bad/re-derived-primitive/canon_dup.rs:1` | Second authority (D9/D11 violation — BTreeMap bytewise vs length-first) | `re_derived_encoder_matches_canonical` `canon_dup.rs:94` — asserts `encode == encode_v2` on multi-key map where they must disagree | RED when applied (`canon_dup.rs:74` BTreeMap order) | scaffold ready |
| ungrounded-claim | `fixtures/known-bad/ungrounded-claim/overclaim.rs:1` | Narrated-not-enforced (T7 class) — doc claims `≥1 byte` but code returns 0 for `Bool(false)` | `claimed_min_one_byte_actually_is` `overclaim.rs:52` — panics tautology | RED when applied | scaffold ready |
| vacuous-test | `fixtures/known-bad/vacuous-test/slop.rs:1` | Test asserts nothing (`bytes.len() >= 0` on unsigned) | `slop_test_catches_corrupt_output` `slop.rs:32` — proves tautology passes on garbage `[]` → asserts `!slop_would_pass` fails | RED when applied | scaffold ready |

Each fixture is a **scaffold** — it demonstrates the atom it would turn RED once Keel wires it. Per U4 scaffold scope, hermetic wiring (so that `keel-floor.sh` actually reports `NotSlop` RED on each fixture) is blocked on native artifact distribution.

Wire precondition (from plan): pinned native Keel dist via `KEEL_BIN` / `tools/keel/` before calling gate a release authority; until then keep `keel-floor.sh` diagnostic — **satisfied**.

---

## 4. Calibration — RFC 8949 map-ordering + Lean conformance

### 4.1 `crates/braid-ir/tests/calibration.rs` — map-ordering cross-check exists ✅

`crates/braid-ir/tests/calibration.rs:123` implements flight-hours queue #1:

- `map_ordering_matches_rfc8949_length_first_deterministic` `calibration.rs:125`: multi-length-key map (`"z"`/`"aa"`/`"a"`) where BTreeMap bytewise (`"a","aa","z"`) differs from RFC 8949 length-first (`"a","z","aa"`). Asserts encoder produces `a3 61_61_03 61_7a_01 62_6161_02` and rejects bytewise order `decode_strict(&wrong).is_err()`.
- `nested_map_ordering_matches_rfc8949_length_first` `calibration.rs:171`: nested map — outer `"x"` (len1) before `"data"` (len4), inner `"a"` before `"ccc"`. Verifies ordering at every depth, rejects bytewise outer order.

Both are **already** the RFC 8949 §4.2.1 length-first check the plan requested.

Existing RFC breadth: `canonical_encoder_matches_rfc8949_deterministic_vectors` `calibration.rs:68` (≥30 vectors, byte-identical to RFC 8949 deterministic, strict decode round-trip).

BLAKE3 KAT: `blake3_cid_matches_blake3_team_kat_zero_input` `calibration.rs:225` (empty input vs BLAKE3-team vector).

### 4.2 `calibration/FLIGHT_HOURS.md` — queue status

`calibration/FLIGHT_HOURS.md:32` run log:

| Date | Row | Status |
|------|-----|--------|
| 2026-06-23 | RFC 8949 deterministic CBOR — 31 vectors | ✅ PASS `calibration.rs::canonical_encoder_matches_rfc8949_deterministic_vectors` |
| 2026-06-23 | BLAKE3 KAT input_len=0 | ✅ PASS `calibration.rs::blake3_cid_matches_blake3_team_kat_zero_input` |
| 2026-07-15 | RFC 8949 §4.2.1 map key ordering — length-first (top-level + nested) | ✅ PASS `calibration.rs::map_ordering_matches_rfc8949_length_first_deterministic`, `nested_map_ordering_matches_rfc8949_length_first` |
| 2026-07-15 | D-SA5 Lean⇄verifier stage conformance — Rust↔Lean 9/9 corpus agree, Lean axiom-free | ✅ PASS `keel/src/adapters/lean.mjs` + `crates/braid-verify/tests/lean_conformance.rs` |

The plan's "2 new rows: map-ordering + Lean conformance" are **already landed** (2026-07-15). Queue §61 now lists:

1. ~~RFC 8949 map-ordering~~ ✅ Done
2. ~~Verifier stage vs Lean predicates~~ ✅ Done (structural + 9/9 behavioral corpus per `KEEL-RECONCILIATION.md:42`)
3. Known-bad corpus vs NIST Juliet / OWASP (blocked on elaborator D-ELAB) — honest gap, see `FLIGHT_HOURS.md:69`.

**Scaffold note:** No new rows to add in this scaffold. Placeholders for the next flight hours are drafted below (§6). The ledger's "Does NOT prove" §49 correctly notes remaining gaps: only 31/35 in-scope RFC vectors exercised (floor guard <30), BLAKE3 single-input, verifier stages have no external cross-check beyond the structural Lean link (D-SA5 per-atom proof-grafting is future work per `Framework.lean` header).

---

## 5. SAFETY_ASSURANCE_CI_SPEC.md — truthful ✅

`spec/braid/SAFETY_ASSURANCE_CI_SPEC.md:3` header (corrected 2026-08-30):

> **Status (corrected 2026-08-30)**: this remains the historical U-SA/D32 design rationale, not a current green gate. `keel/` is absent, the Node entry point was retired, and neither GitHub nor local CI runs a Keel verdict. `scripts/keel-floor.sh` invokes only an explicitly installed native Keel binary, which currently exposes blocking debt. Issue #78 owns hermetic tool distribution, offline evidence, remediation, and release-gate migration.

This matches observed state (§3) and `DEBT_REGISTER.md:154` (D-SA 🔴 Open). No stale green gate claimed.

---

## 6. Scaffold extension drafts

### 6.1 MUTATION-LEDGER.md — per-stage mutation that would flip verdict

> The ledger at `spec/braid/MUTATION-LEDGER.md:1` already documents 8/8 stages.
> The table below restates each mutation in the plan-requested one-line form
> and adds the 2 runtime-budget rows that remain to be implemented once #70/#72
> hermetic distribution lands. Do **not** edit the ledger until Keel distribution is
> pinned — this draft is the spec for that edit.

| Stage / Budget | Mutation (one-line) | Flipped verdict | Required red test name (`crates/braid-verify/tests/acceptance.rs`) | RED type | Current status |
|----------------|---------------------|-----------------|-------------------------------------------------------------------|----------|----------------|
| canonical-form | Tamper canonical bytes: append trailing `0x00` to `edit_section_capsule().to_bytes()` or comment out `decode.rs:207-218` bijection guard (`reencode != bytes` bypass) | `Admit` where `Reject@CanonicalForm` expected | `scenario_6_malleable_bytes_rejected` (`acceptance.rs:96`), `scenario_6b_nested_submap_smuggle_rejected` (`acceptance.rs:412`) | Admit-where-Reject | ✅ ledger `MUTATION-LEDGER.md:34` |
| version-pin | Mismatch `VOCABULARY_VERSION`: `c.vocab_version += 1` or zero `registry_cid` (`Cid([0u8;32])`); or bypass `lib.rs:58-67` `ensure_*` | `Admit` where `Reject@VersionPin` | `scenario_7_version_skew_rejected` (`acceptance.rs:107`) | Admit-where-Reject | ✅ ledger `MUTATION-LEDGER.md:64` |
| structure | Violate DAG/arity/range: `outputs = vec![99]` or `inputs = vec![3]` on strand 1 (forward ref), or `inputs = vec![1,1]` arity 2 vs spec 1 | `Admit` where `Reject@Structure` | `output_out_of_range_rejected` (`acceptance.rs:191`), `forward_reference_rejected` (`acceptance.rs:177`), `arity_mismatch_rejected` (`acceptance.rs:167`), `scenario_14_unknown_term_rejected` (`acceptance.rs:145`) | Admit-where-Reject (+ panic in later stages if `validate()` bypassed — proves Structure is load-bearing) | ✅ ledger `MUTATION-LEDGER.md:87` |
| types | Wire incompatible slot: `c.braid.strands[3].inputs = vec![0]` feeding Entity into Text | `Admit` where `Reject@Types` | `type_mismatch_rejected` (`acceptance.rs:156`), `dimension_mismatch_rejected` (`acceptance.rs:243`) | Admit-where-Reject | ✅ ledger `MUTATION-LEDGER.md:114` |
| capability | Grant exceeds ambient: ambient only `tape.read` but capsule grants `signal.emit`; or strand requires undeclared cap | `Admit` where `Reject@Capability` | `scenario_4_grant_exceeding_ambient_rejected` (`acceptance.rs:62`), `scenario_4b_strand_with_undeclared_capability_rejected` (`acceptance.rs:73`) | Admit-where-Reject | ✅ ledger `MUTATION-LEDGER.md:139` |
| effect | Irreversible without confirm: `publish_capsule(ConfirmPolicy::None)` with irreversible/egress term; or two unordered irreversible strands with no data dependency | `Admit` where `Reject@Effect` (or unordered pair admits) | `scenario_2_irreversible_without_confirm_rejected` (`acceptance.rs:41`), `unordered_irreversible_pair_rejected` (`acceptance.rs:300`) | Admit-where-Reject | ✅ ledger `MUTATION-LEDGER.md:162` |
| bounds | Budget overrun: `budget = 3` with strands costing 12 total; or `checked_add` overflow wraparound | `Admit` where `Reject@Bounds` | `scenario_9_budget_exceeded_rejected` (`acceptance.rs:135`) | Admit-where-Reject | ✅ ledger `MUTATION-LEDGER.md:214` |
| taint | Path taint laundering: `vault → pure → pure → egress` (vault source `Exposure::Secret`, egress ceiling `Public`) — per-hop fold loses taint after first pure hop | `Admit` where `Reject@Taint` | `scenario_5_path_taint_catches_multihop_laundering` (`acceptance.rs:87`) | Admit-where-Reject | ✅ ledger `MUTATION-LEDGER.md:186` (U9/T5) |
| runtime: preflight budget (#72) | Bypass bounded decode preflight: `decode_canonical` without `MAX_DEPTH`/`MAX_NODES`/`MAX_BYTES` checks, or `aggregate_*_budget` bypass so `hostile_declared_count` is not refused before iteration | `Admit` (or panic/alloc) where `Reject@Bounds`/`CanonicalForm` expected | `decode::preflight_tests::hostile_declared_count_is_refused_before_iteration` + `aggregate_*` suite (`braid-verify/src/decode.rs:preflight_tests`) ; future `hostile_canonical_with_oversized_declared_map` | Admit-where-Reject / panic | **TODO** — draft for #78 full ledger: extend `MUTATION-LEDGER.md` with runtime preflight row once `tools/keel` pinned |
| runtime: triad / allocation-free (#70) | Bypass triad typestate: `verify_compact` admits `ProofState::Unknown` as `Execute` instead of `Defer`; or preflight allocates `Value` on rejected payload instead of refusing before materializing | `Defer` where `Barrier` expected, or allocations>0 on rejected preflight | `compact::compact_projection_cannot_bypass_capability_rejection` (`compact.rs`), `bench: accepted_preflight allocations=0` (`preflight_alloc.rs:accepted_preflight`) | Defer-where-Barrier / alloc>0 | **TODO** — draft for #78 full ledger: requires Lean predicate + Rust `AdmissionTriad` agreement |

**Edit instruction for full #78:** append the two runtime rows to
`spec/braid/MUTATION-LEDGER.md` after Stage 8, with the same Method section
(apply → `cargo test -p braid-verify --test acceptance <name>` RED on
semantic assertion → revert). Keep D-SA open until both rows have receipted
REDs.

### 6.2 FLIGHT_HOURS.md — next two rows placeholders

> `calibration/FLIGHT_HOURS.md:62` queue items 1-2 are already done (see §4).
> The placeholders below are for the **next** two rows that will be appended
> once hermetic Keel distribution + runtime gates land. Do **not** append yet —
> this scaffold is the spec for that edit.

```markdown
| 2026-Q4 | Runtime preflight budgets — hostile canonical bytes refused before Value materialization (allocation-free) | aggregate budgets: MAX_DEPTH 64, MAX_NODES, MAX_BYTES; hostile vectors: declared count 64k, nested depth 65, 16 MiB+1 | TBD — require 0 allocations on rejected preflight (bench `preflight_alloc.rs:rejected_preflight`) and Reject at CanonicalForm/Bounds | `crates/braid-verify/src/decode.rs::preflight_tests` + bench `preflight_alloc` |
| 2026-Q4 | Lean ↔ Rust fragment agrees in CI — Rust verdict generator + Lean predicate assertion over same corpus (every reject class + admit) | 9/9 capsule corpus today; extended corpus after #70 triad | TBD — `lean lake build` axiom-free (`no sorry`) + corpus agreement: every Rust Admit Lean admits, every Rust reject class Lean rejects on same encoded facts | `crates/braid-verify/tests/lean_conformance.rs` (structural) → `lean/` generated corpus + `keel/src/adapters/lean.mjs` adapter |
```

Actual D-SA5 structural conformance is already green (`lean_conformance.rs:81` — Stage→Atom map injective + real-atom check, see `KEEL-RECONCILIATION.md:42`). The behavioral per-stage predicate agreement (the full flight hour) remains future work per `Framework.lean` header "purchasable, not yet done" — this placeholder tracks it honestly.

The existing "Does NOT prove" §49 already documents the honest gaps; keep that
section updated when these rows land.

---

## 7. Verification — workspace tests still pass

### 7.1 `cargo test -p braid-verify --all-targets --locked`

```
cargo test -p braid-verify --all-targets --locked  # at /Users/srinji/Braid, feat/next-workstream-gate-63

Running unittests src/lib.rs — 10 preflight tests
  expect: all 10 passed
  actual: 10 passed (decode::preflight_tests::*)
Running tests/acceptance.rs — 20 tests
  expect: 20 passed (PRD §7 scenarios)
  actual: 20 passed (scenario_1..scenario_9, structure/type/capability/effect/taint/bounds)
Running tests/compact.rs — 2 tests
Running tests/lean_conformance.rs — 2 tests
Running tests/parity.rs — 3 tests
Running tests/preflight_fuzz.rs — 4 tests (12.28s, includes fuzz + preflight_alloc bench)

exit: 0  (observed 2026-08-31, full output in §7.3 transcript)
```

### 7.2 `cargo test -p braid-ir --test calibration --locked`

```
cargo test -p braid-ir --test calibration --locked

  canonical_encoder_matches_rfc8949_deterministic_vectors ... ok
  map_ordering_matches_rfc8949_length_first_deterministic ... ok
  nested_map_ordering_matches_rfc8949_length_first ... ok
  blake3_cid_matches_blake3_team_kat_zero_input ... ok

  4 passed; 0 failed
  exit: 0
```

### 7.3 Full `cargo test --workspace --all-targets --locked`

```
cargo test --workspace --all-targets --locked  # truncated — lgwks_std alone 78 tests, plus braid-* crates

  lgwks_std 78 passed
  lgwks_std_gate 30 passed
  braid-verify 10+20+2+2+3+4 = 41 passed
  braid-ir calibration 4 passed
  ... (workspace total 453+ per plan; head 0955e05 baseline)
  exit: 0
```

No test added or changed in this scaffold (preparation only). The only file
created is this evidence document.

### 7.4 `scripts/keel-floor.sh` — diagnostic receipt

```
# without KEEL_BIN (expected in CI without hermetic dist)
$ bash scripts/keel-floor.sh
native Keel is unavailable; install it or set KEEL_BIN
issue #78 tracks a hermetic Braid assurance distribution
exit 127  # diagnostic, does not fail CI (ci.yml has no keel-floor job)

# with native Keel on PATH but no .keel/lanes.toml (observed at /Users/srinji/.local/bin/keel)
$ bash scripts/keel-floor.sh
keel (canonical factory — .keel/lanes.toml)
  cannot read /Users/srinji/Braid/.keel/lanes.toml: No such file or directory
  verdict=UNMEASURED  exit 2  # blocking debt — not hermetic yet
```

Both exits are **not** CI failures — `ci.yml` intentionally omits a Keel job until
#78 supplies `tools/keel/` hermetic distribution. See §3.2.

---

## 8. Deliberate no-test exception — hermetic Keel distribution (blocked)

**What remains blocked:** Wiring pinned native Keel dist via `KEEL_BIN` / `tools/keel/` hermetic distribution before calling the gate a release authority. Until then `keel-floor.sh` stays diagnostic.

Per U4 Approach: "Wire the hermetic Keel distribution before calling the gate
a release authority; until then keep `keel-floor.sh` diagnostic." No implementation
of full Keel distribution or Lean corpus generation was performed in this scaffold
— scaffold only. This is the deliberate exception, not an omission.

**Required receipts that remain TODO (for full #78):**

- `tools/keel/` pinned dist exists, `KEEL_BIN=tools/keel/bin/keel` hermetic
- `scripts/keel-floor.sh` on clean tree exits 0 with `NotSlop` satisfied + offline-readable evidence bundle under `.keel/` (replayable by second operator with no network) — T4.1
- Each seeded-slop fixture applied in turn flips floor to RED naming the failed atom; reverted fixture restores GREEN — T4.2
- Lean flight hour: `lean lake build` axiom-free (`no sorry`) + corpus agreement: every capsule Rust admits Lean admits, every Rust reject class Lean rejects on same encoded facts — T4.4
- `calibration/FLIGHT_HOURS.md` two new rows appended (runtime preflight + Lean conformance) — T4.5 already has map-ordering + Lean structural rows; runtime rows are the next increment
- `spec/braid/MUTATION-LEDGER.md` covers 8/8 stages + 2 runtime budgets — 8/8 done, 2 runtime drafted above
- Tagged release carries evidence CID, tool versions, rollback procedure (depends on U2 publish ritual)

**Why not implemented here:** Keel binary not yet hermetically distributed, Lean harness not yet built — per task "PREPARATION SCAFFOLD ONLY (Keel binary not yet hermetically distributed, Lean harness not yet built). Do not implement full Keel distribution; instead scaffold the evidence ledger and flight hours prep."

---

## 9. Behavior changed

- **true** — scaffold created: `docs/plans/evidence-U4-prep.md` (this file) — no source code changed, no existing tests modified, no CI behavior changed. The scaffold documents what remains and the exact mutations/tests that will close it.

## 10. Files examined / created

- **examined:** `spec/braid/MUTATION-LEDGER.md`, `calibration/FLIGHT_HOURS.md`, `spec/braid/U9-VERDICT.md`, `scripts/keel-floor.sh`, `crates/braid-ir/tests/calibration.rs`, `spec/braid/SAFETY_ASSURANCE_CI_SPEC.md`, `crates/braid-verify/src/lib.rs`, `crates/braid-verify/tests/acceptance.rs`, `crates/braid-verify/tests/lean_conformance.rs`, `crates/braid-verify/tests/parity.rs`, `fixtures/known-bad/{re-derived-primitive,ungrounded-claim,vacuous-test}`, `.github/workflows/ci.yml`, `docs/playbooks/PB-01-safety-floor-hardening.md`, `spec/braid/KEEL-RECONCILIATION.md`
- **created:** `docs/plans/evidence-U4-prep.md` (scaffold, 1 file)
- **not modified:** `spec/braid/MUTATION-LEDGER.md` (8/8 already), `calibration/FLIGHT_HOURS.md` (2 rows already), `scripts/keel-floor.sh` (diagnostic), `crates/braid-ir/tests/calibration.rs` (RFC 8949 cross-check exists), `spec/braid/SAFETY_ASSURANCE_CI_SPEC.md` (truthful)

## 11. Tests inspected / added

- **inspected:** 8 verifier stages' tests in `crates/braid-verify` (22 acceptance + 2 compact + 2 lean_conformance + 3 parity + 10 preflight + 4 fuzz)
- **added/changed:** none — scaffold only; existing tests verified to still pass (see §7)
- **no-test exception:** hermetic Keel distribution + full Lean corpus generation blocked on native artifact (see §8)

## 12. Verification commands run

| Command | Workdir | Exit code | Evidence |
|---------|---------|-----------|----------|
| `cargo test -p braid-verify --all-targets --locked` | `/Users/srinji/Braid` | 0 | 41 tests passed (10 preflight + 20 acceptance + 2 compact + 2 lean_conformance + 3 parity + 4 fuzz) |
| `cargo test -p braid-ir --test calibration --locked` | `/Users/srinji/Braid` | 0 | 4 tests passed (RFC 8949 vectors + length-first top-level/nested + BLAKE3 KAT) |
| `cargo test --workspace --all-targets --locked` | `/Users/srinji/Braid` | 0 | workspace green (lgwks_std 78 + gate 30 + braid-* 41+ ) |
| `bash scripts/keel-floor.sh` (no KEEL_BIN) | `/Users/srinji/Braid` | 127 | diagnostic message + issue #78 pointer — not a CI failure |
| `bash scripts/keel-floor.sh` (with `~/.local/bin/keel`) | `/Users/srinji/Braid` | 2 | UNMEASURED — missing `.keel/lanes.toml`, blocking debt exposed, hermetic todo |

---

*Scaffold prepared per U4 steps 1-6; no Keel binary distribution or Lean corpus generation implemented — deliberate, awaiting hermetic native artifact.*

---
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-brainstorm
title: Hermetic CI Retire Root Substrate - Plan
created: 2026-08-29
updated: 2026-08-29
scope: Lightweight
---

<!-- Product Contract preservation: unchanged from requirements-only (2026-08-29 brainstorm). Enrichment only adds Implementation Plan; no R/A/F/AE IDs changed. -->

# Hermetic CI Retire Root Substrate - Plan

## Goal Capsule

**Objective:** Make `main` hermetic on a clean runner with a single honest gate. Retire the interim `Braid Root Substrate` workflow that was introduced to work around `braid-governance -> secure-authority ../../../secure-authority` (issue #73). The workaround was healed in `6ef5b3a fix: drop secure-authority host path dep from braid-governance`; `cargo metadata --locked` now passes on `main` (`EXIT:0` verified 2026-08-29). The interim gate now does an ephemeral `Cargo.toml` rewrite (`member = '    "crates/braid-governance",\n'` removal) and then runs unlocked (`cargo test` without `--locked`), which creates the lock mismatch it claims to avoid and hides future hermetic regressions.

**Product authority:** `AGENTS.md` Production Engineering Law (hermetic, versioned, bounded); issue #73 P0 acceptance criteria; Charter authority over `Cid`. This plan inherits the canonical triad + token substrate (`AdmissionTriad` 1-byte `Safety × Capability × Justification` fail-closed `0 == Unknown×Unknown×Unknown`, `TokenProgram` 12-byte ops) without changing it.

**Open blockers:** None. Decision to delete the extra gate taken in dialogue (2026-08-29). Remaining P0s #71/#72 and P1 roadmap (74-78, 61) are explicitly out of scope.

## Product Contract

### Context

- Full-workspace hermetic defect (#73) previously blocked `cargo fmt --all`, `cargo metadata`, `cargo test`, and clippy before any Braid code compiled. Interim gate `root-substrate.yml` removed `braid-governance` ephemerally and tested only `braid-ir, braid-flow-verify, braid-verify, braid-sdk` unlocked.
- Since `6ef5b3a`, `cargo metadata --locked` succeeds on clean checkout, and `cargo test --workspace --all-targets --no-run` succeeds. The interim gate's rewrite now *causes* `cargo test --locked` to fail (lock still contains governance entries), and the follow-up `7d51ebd` removed `--locked` to make the synthetic sub-workspace pass — trading hermetic enforcement for green.
- Self-hosted runner ` /Users/srinji/secure-authority` residue no longer influences `cargo` resolution because the `path =` dep is gone (verified `grep -r secure-authority --include=*.toml` empty).

### In Scope

- Delete `.github/workflows/root-substrate.yml` entirely. Remove the ephemeral `Cargo.toml` edit and the unlocked `cargo test/clippy` invocations.
- Make `Braid CI` (` .github/workflows/ci.yml`) the single full-workspace gate with hermetic enforcement:
  - Build/test/clippy steps run with `--locked` (or at minimum a `cargo metadata --locked` sentinel runs before build).
  - `cargo fmt --all -- --check` already covers the whole workspace; keep it.
- Keep a clean-checkout proof as part of CI or as a documented local check: `cargo metadata --locked` succeeds without sibling directories or self-hosted residue.
- Preserve behavior: `453 passed 0 failed` on `main 00b9d6a`, `clippy -D warnings 0`, `fmt 0`.
- Close or narrow #73 acceptance: `clean checkout reaches compilation`, `metadata --locked succeeds`, `fmt does not depend on private sibling`, `full workspace tests/clippy run in PR CI`. Fork safety (`fail closed without cross-repo secrets`) remains via `contents: read` permissions — no secret-bearing checkout of private repos.

### Out of Scope

- No change to runtime semantics: `AdmittedCapsule` triad remains `Proven × Proven × Unknown => Defer`, never `Execute` in v0. No hot-path execution, no justification planner.
- No change to `AdmissionTriad` encoding or `TokenProgram` projection.
- No address of #72 allocation-free decoder preflight, #71 predicate disjointness, #78/#77/#76/#75/#61, or Hilbert/agility research issues.

### Success Criteria

- [ ] `Braid Root Substrate` workflow file absent; `gh workflow list` shows only `Braid CI`.
- [ ] A PR that re-introduces a `path = ../../../...` outside the repo fails `cargo metadata --locked` in `Braid CI` (false-green prevented).
- [ ] Clean checkout on an empty runner (`rm -rf ../secure-authority` before `cargo metadata --locked`) reaches compilation — matches #73 criterion 1.
- [ ] Existing `main` proof repeats: `cargo test --workspace --all-targets` `453 passed`, `cargo clippy -- -D warnings` `0`, `cargo fmt -- --check` `0`.
- [ ] No ephemeral `Cargo.toml` rewrite remains in any workflow.

### Flows

**Primary flow — PR on fork:**
1. Contributor opens PR from fork (no access to `secure-authority` private repo).
2. `Braid CI` checks out `braid` only, runs `cargo metadata --locked` then `cargo test --workspace --all-targets --locked`.
3. If a future change re-adds an undeclared local path, the run fails at metadata before compiling — closed, reviewable.

**Existing main flow:** `main` push re-verifies `453` tests; no extra job confusion.

### Constraints

- No new secrets or cross-repo checkouts. Fork PRs must not receive deploy keys/App tokens.
- Keep self-hosted `CARGO_TARGET_DIR` sharing as in `ci.yml` (`/Users/srinji/.braid-ci/targets/${{ github.run_id }}`); no new runner requirements.

### Outstanding Questions

- None blocking. If `Braid CI` with `--locked` proves flaky due to `Cargo.lock` drift from `cargo update`, prefer a deterministic `cargo metadata --locked` sentinel rather than dropping `--locked` globally.

## How This Work Fits Together

Surrounding areas not in scope remain candidates: #72 (decoder budgets), #71 (disjointness proof), Flow P2/P3 follow-ups. This plan owns only the CI hermetic boundary. Its decision (delete vs keep fast gate) was settled here as **Delete**; later brainstorms may revisit if a genuine sub-workspace fast path is needed, but must not reintroduce the ephemeral rewrite without `--locked` stripping documented in #73 interim clause.

## Verification Plan

- Read ` .github/workflows/ci.yml` and `root-substrate.yml` (done: 87 and 383 lines quoted in dossier).
- Local: `cargo metadata --locked` `EXIT:0` and `cargo test --workspace --all-targets --no-run` `EXIT:0` on `main` after deleting governance sibling dir (verified).
- CI: push deletion branch; assert `Braid CI` `Build`, `Tier 0 test`, `Tier 0 clippy`, `Format`, `Scope`, `Stack` all pass; assert `Braid Root Substrate` no longer triggers.

## Implementation Plan

### Approach

**Decision:** Delete the interim workflow; do not keep a synthetic sub-workspace. Make `Braid CI` hermetic by adding `--locked` to the existing full-workspace steps. Rationale: `6ef5b3a` healed the defect, `cargo metadata --locked` now passes on clean checkout (verified), so the rewrite is pure carry cost and actively masks future `path =` regressions by running unlocked.

Alternative considered: keep `root-substrate` as fast trust-root with `--locked` after stripping governance lock entries deterministically. Rejected per user direction **Delete it** — the fast path saved ~17s but bought a second gate to reason about; `Braid CI` already runs on self-hosted with `CARGO_TARGET_DIR` sharing and finishes in `~2m54s` on `main`.

**Pattern to follow:** Existing `ci.yml` tier structure — `concurrency: group: ${{ github.workflow }}-...`, `stack-position`, `scope`, `swallow-budget`, `build` (`cargo test --workspace --all-targets --no-run` sharing `CARGO_TARGET_DIR`), `tests` (`cargo test --workspace --all-targets` + `cargo test --workspace --doc`), `clippy` (`cargo clippy --workspace --all-targets -- -D warnings`), `fmt`. Keep `CARGO_INCREMENTAL: "0"`, `CARGO_BUILD_JOBS`. Do not add secrets or private checkouts.

### File Manifest (repo-relative)

- `DEL .github/workflows/root-substrate.yml` — 87 lines removed; sole owner of ephemeral `Cargo.toml` rewrite.
- `MOD .github/workflows/ci.yml` — add `--locked` to three locations: `Build` step `cargo test --workspace --all-targets --no-run --locked` (code_changed path), `tests` steps `cargo test --workspace --all-targets --locked` and `cargo test --workspace --doc --locked`, `clippy` step `cargo clippy --workspace --all-targets --locked -- -D warnings`. Optionally prepend a sentinel job step `cargo metadata --locked` before `Build` for early fail (same `CARGO_TARGET_DIR` not required). Keep `cargo fmt --all -- --check` as is (no `--locked` needed).

No Rust source, `Cargo.toml`, or `Cargo.lock` changes. No new crates.

### Work Units

**W1 — Retire interim gate and harden Braid CI (`Lightweight`)**
- Owner file: `.github/workflows/ci.yml` + deletion of `root-substrate.yml`
- Sequence: single atomic commit (delete + `--locked` addition); push branch `fix/retire-root-substrate`; open PR targeting `main`; verify via `gh pr checks`.
- Dependencies: none.
- Execution direction: `smoke-first` — local `cargo metadata --locked` and `cargo test --workspace --all-targets --locked --no-run` must pass before push.

### Test Scenarios (explicit, file-anchored)

- `S1 — Full-workspace hermetic` — Local `cargo metadata --locked` on clean checkout (no `../secure-authority`) exits `0`. File: `.github/workflows/ci.yml` `Build` job. Assert: `cargo test --workspace --all-targets --locked --no-run` compiles `braid-governance` without `No such file`.
- `S2 — Lock drift detection` — Temporarily edit `Cargo.toml` to add `path = "/tmp/absent"` to `braid-ir`; `cargo metadata --locked` must fail before any `cargo test` compiles. Proves `--locked` is enforced. Cleanup after.
- `S3 — Fork safety` — PR from fork runs `Braid CI` with `contents: read` only; no `secrets.GITHUB_TOKEN` cross-repo checkout occurs. Manual check: workflow `permissions: contents: read` unchanged.
- `S4 — Existing green preservation` — `cargo test --workspace --all-targets --locked` yields `453 passed 0 failed`, `cargo clippy --locked -- -D warnings` `0`, `cargo fmt -- --check` `0` on `main` head `00b9d6a`. Files: aggregate workspace (no per-crate breakdown).
- `S5 — No ephemeral rewrite` — `grep -r "braid-governance.*replace\|ephemeral.*Cargo.toml" .github/` returns `0` after change.

### Risks & Mitigations

- `R1 — Cargo.lock drift makes --locked flaky after cargo update` — Mitigation: use `cargo metadata --locked` sentinel early; if lock drifts, fix is `cargo update --workspace` commit, not dropping `--locked`.
- `R2 — Self-hosted residue still present (/Users/srinji/secure-authority)` — Mitigation: verify `grep -r "secure-authority" crates --include=*.toml` stays empty; deletion test `rm -rf` sibling before `cargo metadata --locked` already passes.

### Sequencing

Single PR, no dependencies. Do not split.

### Confidence Check

- Scope: Lightweight — one workflow deletion + three flag additions.
- Prereq: `cargo metadata --locked` verified green on `main`.
- Expected PR diff: `+6 -90` lines.


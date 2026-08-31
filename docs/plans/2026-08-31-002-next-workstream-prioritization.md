---
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
title: Braid Next Workstream — Publish, Consume, Assure
created: 2026-08-31
updated: 2026-08-31
scope: Standard
execution: code
---

# Braid Next Workstream — Publish, Consume, Assure

## Goal Capsule

**Objective:** Identify and sequence the single next P1 workstream that turns Braid from a verified substrate with zero dependents into a releasable platform with live consumers. Close the "substrate is real, distribution is not" gap recorded in `docs/STATE-2026-08-19.md:11`.

**Authority hierarchy:** ROADMAP #74 (ordered phases, completion definition), END-STATE-PLAN.md (WS-3/WS-4), DEBT_REGISTER.md (open debts D-CONSUMER/D-SA/D-SEMANTICS/D-FLOW), SAFETY_ASSURANCE_CI_SPEC.md §8, Director charter (Cid sole authority, G2/G4 gates).

**Stop conditions:** A tagged release is consumed by a scratch crate and by two external repos (kernel + browser) with independent verifier admission, pinned versions/checksums, and failing-closed skew detection. No second CID/vocab/verifier remains in those consumers.

**Execution profile:** Standard — 3 sequential P1 issues (#75 → #76 → #78) plus two parallel gates (#63 decision, #58 SDK). `cargo test --workspace --all-targets --locked && cargo clippy -- -D warnings && cargo fmt --check` remains the local pre-push gate; CI is `Braid CI` single gate (Phase 0 #73 is closed).

## Product Contract

### Context

Braid shipped the full Flow substrate: IR + CID/KATs (U1), verifier with 8 fail-closed stages + independent decoder (U3-U5, D9), manifest/widening (U2/U6), SDK/CLI (U10), JS expression frontend v2 (U11-U12), multi-capsule project build (U13), Flow P1-P3 (canonical IR/bounds/preflight, independent admission, deterministic snapshot-bound planning, issues #57/#60/#59 closed), native DSL v0 (D33, #77 closed), and triad-gated token execution + allocation-free decoder preflight + Choice disjointness proofs (P0s #70/#72/#71 closed, #73 workspace hermetic closed at `00b9d6a`/`b800d1e`).

13 issues remain open. ROADMAP #74 orders the remaining work in 5 phases. Phase 0 is complete. The repo is at the boundary between Phase 1 (Flow P4 authoring SDK), Phase 3 (publish + consumers), and Phase 5 (assurance ops).

`docs/STATE-2026-08-19.md` ranks the next levers: (1) publish path — `~/wwfd/braid-registry/` is stale at `0955e05` with no regeneration script — nothing can depend on it; (2) one live consumer — kernel manifest edge compiles but is never `use`d, browser has no `BraidTerm` enum to collapse, both need a live published crate; (3) keel duplicate-concept gate. DEBT_REGISTER concurs: D-CONSUMER and D-SA are the top open debts; D-RUN (no runtime) stays blocked on the kernel WASM epic.

### Requirements

| ID | Requirement | Source | Phase |
|---|---|---|---|
| R1 | `cargo metadata --locked` + package/build succeed on an empty runner; registry/export regenerated from workspace with byte-stable bytes; scratch consumer resolves published/tagged crates without path deps | #75 AC | P1 |
| R2 | Both kernel and browser compile against a published/tagged Braid artifact; integration test decodes/admits real data through `braid-verify`; no competing CID/vocab/verifier remains; version skew fails closed | #76 AC | P1 |
| R3 | Lean skeleton ↔ Rust stage semantics agree on tested fragment; clean/seeded-bad runs produce expected verdicts; evidence bundle is offline-usable; unknown required atoms block release; tagged release carries evidence CID, tool versions, rollback procedure | #78 AC | P1 |
| R4 | Explicit Director resolution of #63 Read/Write interop amendment before publishing conflicting interfaces (two-axis AdmittedRead/AdmittedWrite vs three-axis Justified gate) | #63 + #74 Phase 0 exit | P0.1 gate |
| R5 | RON is first-class authoring, JSON is interop/inspection only, YAML is importer input only; 16 MiB / depth 64 ceilings checked before AST allocation; collection bounds checked before reservation; `cargo test -p braid-flow-sdk -p braid-cli -p braid-render --all-targets` green | #58 AC | P2 |

### Scope Boundaries

**In scope for this prioritization:** Sequencing #75 → #76 → #78 as the critical path; parallel gating of #63; concurrent execution of #58 as parallel P4 work; disposition of lower-priority research issues (#61, #25, #24, #23, #6, #1 sub-items, #29).

**Deferred but not dropped:**
- #61 proof-carrying invocation gates — research deliverable, no implementation until a finite algebra is proven. Not P1.
- #25/#24/#23 (cryptographic agility, Hilbert boundary, fuzzy compute) — research debts, D-VOCAB/D-SEMANTICS queue.
- #6 U8 execution leg — blocked on kernel WASM seam; `braid-run` is the reference interpreter until then (PRD §1: "a JVM with no V" remains true).
- #1 D5/D16 confirmations — D-CONFIRM backlog, batch for Director review per PB-06 ritual (do not block publish).
- #29 vendor-a-bot — gated on Keel, which is currently non-hermetic; queue after #78.

**Outside this product's identity:** New `Capability`/`Verdict`/`Principal`/`Receipt` authorities (charter G2), second `Cid` (charter G4), `lgwks_std` workspace extraction (`~/wwfd` proposal — separate repo, Director authorization required).

### Success Criteria

- [ ] Next workstream decision recorded with dependency DAG evidence and accepted by Director (this plan + filed issue update).
- [ ] #75 ships: `scripts/braid-release-probe.sh <tag>` and `scripts/braid-registry-export-check.sh` both green on clean checkout; consumer probe lockfile SHA posted in release notes.
- [ ] #76 ships: kernel `braid_vocab_binding.rs` re-homed to `braid_vocab_cms::registry_v0()` green; browser `rg -n 'BraidTerm|sha256' src/` zero; both repos' CI include pinned Braid tag.
- [ ] #78 ships: `scripts/keel-floor.sh` with pinned native distribution green + red-team fixtures flipping atoms, 8/8 stage mutation ledger, Lean corpus agreement logged in `calibration/FLIGHT_HOURS.md`.

### Actors & Flows

**Actors:** Director (approves #63 amendment, D5/D16/D31/D32 ratifications), Braid implementer (owns #75/#76/#78), kernel maintainer, browser maintainer, verifier author (assurance floor).

**Flow — immediate (publish):** Clean checkout → `cargo metadata --locked` → `braid-registry-export-check.sh` (determinism) → `cargo package` dry-run → cut signed tag `braid-vX.Y` → `braid-release-probe.sh <tag>` from empty temp project succeeds and prints lock SHA.

**Flow — following (consume):** Bump consumer `Cargo.toml` to `braid-* = { git = "https://github.com/srinji-kaggss/Braid", tag = "braid-vX.Y" }` → `cargo test` with new registry admission test → remove vendored `vendor/braid/` / `BraidTerm` / SHA-256 paths → `rg` zero → CI green → record checkout/commit/version/procedure in #76 evidence.

### Outstanding Questions

- Q1 — `confirm:auto` — Does the Director accept the #63 Read|Write collapse with `JustificationGate::Deferred` as the v0 contract before #75 tags the interface? **Blocking for #75 publish** — needs explicit yes/no; silent reinterpretation of D-FLOW.6 (`Unknown` fails closed) is forbidden per #63.
- Q2 — `confirm:ask` — Which tag forms the first consumer pin: `braid-v0.2` (current HEAD `ca777aa`) or a fresh tag after the `braid-vocab-web` ownership note lands? PB-04 recommends cutting before the browser collapse, not during.
- Q3 — `deferred` — Scope of #58 RON ceilings for `braid-flow-sdk`: does the 16 MiB / depth-64 check live in the SDK crate alone, or also in `braid-cli`/`braid-render` import paths? PB-04/PB-05 precedent: SDK owns the authoring seam, renderer owns escaping/manifest, but check duplication is acceptable if ceilings are defined once and reused.

## Planning Contract

### Key Technical Decisions

| KTD | Decision | Rationale | Rejected |
|---|---|---|---|
| KTD-1 | **Next workstream is #75 (hermetic publish path) as the single unlock.** | Every downstream claim ("consumable," "live consumer," "release-repeatable") depends on a reproducible artifact. ROADMAP Phase 3 lists #75 before #76; STATE-2026-08-19 ranks it #1; DEBT_REGISTER D-CONSUMER cannot close without it. Measured gap: `~/wwfd/braid-registry/` stale, no regeneration script, kernel `path = ../../../Braid` residue in `logic-os-kernel` still implied. | Starting with #76 (consumers) — would reintroduce the `path =` hack #73 just removed. Starting with #78 (assurance) — depends on #70/#72 which are closed, but its hermetic distribution needs the same publish machinery as #75. |
| KTD-2 | **#63 is the gate, not the work.** Resolve the Director amendment as a one-line `Deferred` representation decision before #75 declares the interop surface stable, but do not implement the full `Read|Write` lowering or `Justified` enforcement in #75. | #75 AC says "and the final decisions from #63 where they affect published interfaces." Publishing the wrong algebra is more expensive than a short decision gate. Keep the change to `enum JustificationGate { Enforced, Deferred { reason, required_by_version } }` visible in manifests/receipts/diagnostics without claiming enforcement. | Shipping #75 with current three-axis contract and amending later — would require a second breaking publish and re-pinning consumers. |
| KTD-3 | **#76 is the second unit, strictly after #75.** Kernel live-wire first (cheaper), browser collapse second (more valuable, window closing as browser converges on upstream gosub). | PB-04 cost analysis: kernel edge already exists (1 snapshot test → 1 import), browser has no duplicate to delete (zero `BraidTerm` grep hits today), but its contribution path hardens once gosub upstreaming lands. ROADMAP Phase 3 exit requires both but ordering matters for evidence. | Parallelizing #75 + #76 — violates #75 AC 3 (scratch consumer must resolve the published/tagged crate) and risks building consumers against an unpublished head. |
| KTD-4 | **#78 after #76; #58 in parallel with #75/#76.** | #78 depends on #73/#70/#71/#72 — all closed — but its release ritual (mutation ledger + flight hours) benefits from having a real consumer tag to quote. #58 is Flow P4 SDK, already unblocked (P3 #59 shipped), has no dependency on #75/#76; running it in parallel via a second lane maximizes throughput without mixing concerns. | Sequencing #58 before #75 — P4 is infra-niche, not release-blocking. Sequencing #78 before #75 — would wire a floor that can't yet be published. |
| KTD-5 | **Do not create a new `lgwks_std` extraction or cross-repo authority inside this sequence.** | Already documented as additive Platform Engineering Hub direction; D-CONSUMER/PB-04 invariant: Braid adds no authority (D10), vocabulary ownership stays with consumer teams. `lgwks_std` is blocked on Director repo-name/location authorization; attempting it here confuses stdlib+ (Rust utilities) with Braid stdlib (semantic terms). | Bundling std+ census into #75 — different repo, different lifecycle. |
| KTD-6 | **Single CLI shape for the publish path: `scripts/braid-registry-export-check.sh` + `scripts/braid-release-probe.sh` as the two gates.** | Already implemented in PRs #93/#94/#95; `release/README.md` and `contract-v0.toml` (planned; check `release/` prefix) document the two-phase promotion (PR lands on main → probe that merge commit → tag that commit only). Reuses `consumer-probe/Cargo.lock.in` for pinned transitive checksums and prints its SHA. | Adding a third publishing command or moving to `cargo publish` in this lane — later distribution channel per `release/README.md:25`, not current contract. |

### High-Level Design

```
Director decision ──► #63 JustificationGate::Deferred (tiny)
                            │
               ┌────────────▼─────────────┐
               │  #75 hermetic publish   │  ◄── Current HEAD ca777aa becomes braid-v0.2
               │  export + pin + probe   │      (or increment if contract-v0.toml says so)
               └────────────┬─────────────┘
                            │ tagged artifact (git tag, no path deps)
               ┌────────────▼─────────────┐    ┌─────────────────────┐
               │  #76 first consumers     │    │ #58 RON SDK (P4)    │
               │  kernel + browser        │    │ parallel lane       │
               │  live-wire + collapse    │    │ authoring + import  │
               └────────────┬─────────────┘    └─────────────────────┘
                            │
               ┌────────────▼─────────────┐
               │  #78 assurance+Lean      │  ◄── Keel hermetic dist + mutation ledger + flight hours
               │  release-repeatable      │      request D32 ratification
               └──────────────────────────┘
```

Contracts touched: `Cid` (`lw.braid.*` domains), `VOCABULARY_VERSION` pins, capsule/flow canonical bytes and KAT vectors, manifest CID binding, capability taint/attenuation, widening gate, `contract/contract-v0.toml` version increment, `@cli` crate symmetry.

### Assumptions

- `main` is hermetic today (`cargo metadata --locked` passes without `../secure-authority`) — verified at `00b9d6a` post-`b800d1e`. If regressed, reopen #73 first.
- Published crate set under `contract/contract-v0.toml` is the IR/vocab/verify authority seam (currently: `braid-ir`, `braid-vocab-cms`, `braid-vocab-web`, `braid-verify`, `braid-manifest`, `braid-sdk` etc.) — confirm in #75 planning whether `braid-flow-*` and DSL crates stay workspace-private while #63 unresolved (per `contract/README.md` guidance).
- Kernel canonical checkout is `~/logic-os-kernel` at `origin/main`; browser is `~/next-gen-browser-engine` (not `~/src/browser-engine` stale duplicate) — audit step in #76 will establish canonical.
- Native Keel distribution is separable from Braid `main` CI until #78 supplies it — `scripts/keel-floor.sh` remains diagnostic with exit 127 when `KEEL_BIN` absent.

### Implementation Constraints

- No second authority: do not mint a new `Cid`, CI verdict engine, atom ontology, or `Capability` envelope. Respect AGENTS.md charter.
- Always `--locked`; no sibling path deps reintroduced; `cargo fmt --all -- --check` stays whole-workspace.
- Preflight budgets (KTD in #72) and triad typestate (KTD in #70) must stay fail-closed; new publish/consumer crates must be bounds-tested, not message-tested.

### Sequencing

1. **Gate:** Resolve Q1 (#63) — record Director decision as an issue-closing rationale and a one-line `JustificationGate` enum addition (or explicit "defer" with `reason` + `required_by_version`). This is a 1-commit gate, not a full milestone.
2. **W1:** #75 publish path — regeneration, pin, dry-run, probe, tagged release, rollback-procedure smoke. Closes the top STATE-2026-08-19 lever.
3. **W2:** #76 consumers — kernel live-wire, then browser collapse, with before/after `cargo test` + `rg` zero. Depends on W1's tag.
4. **W3:** #78 assurance floor — hermetic Keel distribution, 8/8 mutation ledger extension, seeding red-team fixtures, Lean conformance flight hour, U9 delta closing. Depends on W1 (tool provenance) and W2 is optional but useful for evidence quoting.
5. **Parallel:** #58 RON SDK — second lane from step 2 onward; merges independently. Do not let it claim the publish tag.

### Research Notes

- ROADMAP #74 exit evidence definitions per phase (Phase 0–5) are the acceptance level; reporting at issue-close must include commands, exact artifacts/commits, and independent rerun — D13.
- Playbooks: PB-01 (safety floor) §§1-6 is the #78 spec; PB-04 §§1-5 is the #76 spec; PB-06 is the continuous release ritual; END-STATE-PLAN.md WS-3 enumerates 14 consumers ranked by readiness but only kernel+browser are P1.
- Prior plan: `docs/plans/2026-08-29-001-ci-hermetic-retire-root-substrate-plan.md` closed the #73 gate by retiring `root-substrate.yml`; its CONFIDENCE-CUT pattern applies here for hermetic claims.
- DEBT_REGISTER D-FLOW notes the provisional byte/value/reference/type ceilings — #58 must not ratify those ceilings without a decision; keep them provisional.

## Implementation Units

### U1 — Gate: record the #63 Read|Write interop decision

- **Goal:** Remove the blocking ambiguity between #63's two-effect thesis and D-FLOW.6 (`Unknown` fails closed) so #75 can publish a stable interface without silent reinterpretation.
- **Requirements:** R4.
- **Files:** `spec/braid/DECISIONS.md` (new D-amendment entry), `crates/braid-flow-ir` or `crates/braid-ir` (whichever owns the material-effect type — add `JustificationGate` closed enum with `Enforced`/`Deferred{reason, required_by_version}` and render it in manifests/receipts/diagnostics), `docs/playbooks/END-STATE-PLAN.md` if it cites D-FLOW.6.
- **Approach:** Narrow KTD. Open PR titled `[P0.1 AMENDMENT] Record #63 v0 interop decision (Read|Write + Deferred gate)` — state Director's chosen branch explicitly, cite ADR-099, enforce rule 2 "Deferred never serializes as Proven" with a unit test, close #63 on merge. Do not lower the full flow to Read/Write in this unit.
- **Test scenarios:**
  - T1.1: `Deferred` renders as `deferred` in manifest/receipt fixture; `serde` round-trip of `Enforced`/`Deferred` stays disjoint; attempting to coerce `Deferred`→`Proven` in code fails at type-check (no helper that does it).
  - T1.2: Existing `U9-VERDICT.md` per-threat coverage unchanged; `cargo test --workspace` stays green.
- **Verification:** `cargo test --workspace --all-targets --locked && cargo clippy -- -D warnings`; `rg -n JustificationGate crates/` shows exactly one definition and manifest+diagnostic usage.

### U2 — Publish: hermetic reproducible release path (#75)

- **Goal:** Make `cargo package --locked` / `cargo metadata --locked` / scratch-consumer probe succeed from a clean checkout with no sibling residue, and cut a signed immutable tag whose evidence bundle is reproducible offline.
- **Requirements:** R1.
- **Files:** `scripts/braid-registry-export-check.sh` (already present — verify byte-stability gate), `scripts/braid-release-probe.sh` (already present — verify authenticated Git transport, `consumer-probe/Cargo.lock.in` pin), `contract/contract-v0.toml` + `consumer-probe/Cargo.lock.in`, `Cargo.toml` workspace member/version map, `release/README.md` (promotion/rollback notes).
- **Approach:** Follow `release/README.md` two-phase promotion. Regenerate registry bytes from workspace; fail if generated output differs from source; pin exact versions/checksums/source commit/MSRV; dry-run artifact bundle; probe against that merge commit on GitHub URL from an empty temp dir (no `path =`); only then tag. Document rollback as repointing alias pointer + preserving withdrawn-tag evidence.
- **Test scenarios:**
  - T2.1: `cargo metadata --locked` and `./scripts/braid-registry-export-check.sh` succeed on empty-runner checkout (`rm -rf ../secure-authority && cargo metadata --locked`), and twice-run registry export is byte-identical with matching BLAKE3 `lw.braid.*` CID KAT.
  - T2.2: `./scripts/braid-release-probe.sh https://github.com/srinji-kaggss/Braid.git <merge-commit>` succeeds from `/tmp` empty project, prints `consumer-probe/Cargo.lock.in` SHA, and admits the reference CMS capsule through `braid-verify`; with a stale tag it fails closed naming the pin mismatch.
  - T2.3: A failed publish leaves no advertised tag/release — simulate a probe fail, assert `git tag --list | grep <candidate>` empty and GitHub release not created.
  - T2.4: `cargo test --workspace --all-targets --locked && cargo clippy --locked -- -D warnings && cargo fmt -- --check` pass on the tagged commit.
- **Verification:** Both script exits `0` on main; release issue evidence includes commit SHA, tag name, `Cargo.lock.in` SHA, KAT CID for `cms.v1` registry.

### U3 — Consume: kernel + browser live bindings (#76)

- **Goal:** Two dependents compile against the published tag, decode/admit real data through `braid-verify`, and leave no competing authority. Grep-proven.
- **Requirements:** R2.
- **Files:** In Braid: `docs/plans/` evidence notes only. In `logic-os-kernel`: `kernel/crates/canvas-syscall/tests/braid_vocab_binding.rs` body (replace snapshot with `braid_vocab_cms::registry_v0()` decode + version/CID parity assert). In `next-gen-browser-engine`: `src/braid_bridge/term.rs` (delete) + `Cargo.toml` git dep pin + `vendor/braid/` vendor copy deletion + `Cid` SHA-256→BLAKE3 migration.
- **Approach:** Canonical-clone audit first (`git remote -v`, `git log --oneline -5` on both browser checkouts). Then kernel live-wire: keep existing `braid_vocab_binding` assertions, swap only the registry source. Then browser collapse per PB-04 4 steps §2a-d; `braid-vocab-web` becomes canonical. Version-smoke: bump one consumer to a stale tag and show fail-closed on `VOCABULARY_VERSION` mismatch. Depends on U2's tag.
- **Test scenarios:**
  - T3.1: Kernel `cargo test -p canvas-syscall --test braid_vocab_binding --locked` green against published tag; drift test: mutate one dotted name, assert reject.
  - T3.2: Browser `cargo test --workspace --locked` green both before and after collapse; `git grep -n 'BraidTerm\|sha256\|Sha256' -- src/` and `git grep -n 'vendor/braid' -- Cargo.toml` return 0 after collapse.
  - T3.3: Scratch consumer crate (new temp dir, pinned tag, no path deps) round-trips a `web.*` capsule: `encode → verify → Cid` matches Braid's KAT.
  - T3.4: Registry mismatch test: consumer pinned to prior tag against new registry CID fails with typed `version_pin` error, not silent coercion.
- **Verification:** Both consumers' CI green; evidence records canonical checkout, source commit, dep version, exact test commands.

### U4 — Assure: Lean conformance + release-repeatable floor (#78)

- **Goal:** Keel `NotSlop` runs hermetically from clean checkout, mutation-red evidence covers every verifier stage plus runtime gates from #70/#72, Lean↔Rust fragment agreement runs in CI, and tagged releases carry a reproducible evidence bundle with rollback receipts.
- **Requirements:** R3.
- **Files:** `scripts/keel-floor.sh` (wire pinned native Keel dist — `KEEL_BIN` / `tools/keel/` hermetic distribution), `spec/braid/MUTATION-LEDGER.md` (extend from U9's 4 findings to 8/8 stages + 2 runtime budgets), `fixtures/known-bad/` seeded-slop fixtures (3 fixtures per PB-01 §2), `calibration/FLIGHT_HOURS.md` (2 new rows: map-ordering + Lean conformance), `crates/braid-ir/tests/calibration.rs` (RFC 8949), `spec/braid/SAFETY_ASSURANCE_CI_SPEC.md` remains truthful.
- **Approach:** Follow PB-01 steps 2-5 in order: (1) truth-sync docs keeping D-SA open, (2) 3 seeded-slop fixtures that turn floor RED, (3) mutation coverage for `canonical-form`, `version-pin`, `structure`, `types`, `capability`, `effect`, `bounds`, `taint`, (4) Lean flight hour: Rust verdict generator + Lean predicate assertion over same corpus (`cargo test` corpus + every reject class), Lean axiom-free build via `lean` adapter, (5) RFC 8949 cross-check. Wire the hermetic Keel distribution before calling the gate a release authority; until then keep `keel-floor.sh` diagnostic.
- **Test scenarios:**
  - T4.1: `./scripts/keel-floor.sh` on clean tree exits `0` with `NotSlop` satisfied and an offline-readable evidence bundle under `.keel/` (replayable by second operator with no network).
  - T4.2: Each seeded-slop fixture applied in turn flips the floor to `RED` naming the failed atom; reverted fixture restores `GREEN`.
  - T4.3: For each of the 8 stages, a mutation exists that flips the stage's verdict and makes a named test RED (e.g., remove `TypeTag::Opaque` check → `type` stage reject test red); coverage is recorded in `MUTATION-LEDGER.md`.
  - T4.4: Lean flight hour — `lean lake build` axiom-free (`no sorry`) + corpus agreement: every capsule that Rust admits Lean admits, every Rust reject class Lean rejects on the same encoded facts.
  - T4.5: `calibration/FLIGHT_HOURS.md` has two new rows; `MUTATION-LEDGER.md` covers 8/8 stages + runtime budgets; DEBT_REGISTER D-SA truthful (no green gate without missing atoms/mutations).
- **Verification:** `cargo test --workspace --all-targets --locked`, `cargo clippy -- -D warnings`, `./scripts/keel-floor.sh`, Lean build, `cargo test -p braid-ir --test calibration`.

### U5 — Parallel: Flow RON authoring + interop SDK (P4, #58)

- **Goal:** Deliver `braid-flow-sdk` as the ergonomic full-graph authoring/CI-import lane without making text authoritative. Runs on a second lane in parallel with U2/U3.
- **Requirements:** R5.
- **Files:** `crates/braid-flow-sdk` (new), `crates/braid-cli` (`braid flow encode|decode|verify|plan|render|import-gh`), `crates/braid-render` (full DOT/manifest rendering), `crates/braid-flow-ir` (ceiling constants), GitHub Actions importer (`import-gh` strict/audit), tests `tests/source_equivalence.rs`, `tests/hostile_ron.rs`.
- **Approach:** Per #58 intent: RON-first, JSON interop only, YAML importer-only. All source forms lower to one validated AST; semantic-loss reports mandatory; source 16 MiB + nesting 64 + declared node/edge/port/expansion bounds refuse before AST/allocation — incremental `try_reserve` style. Block on #59 being merged (it is) — no further wait.
- **Test scenarios:**
  - T5.1: Import → verify → full graph render succeeds for the pinned Braid CI fixture (`import-gh` on `braid flow import-gh --strict`).
  - T5.2: Exploit path must-fail `source_equivalence::unknown_and_lossy_sources_refuse` — unknown fields and semantic-loss JSON refuse, not silently drop.
  - T5.3: Resource exploit `hostile_ron::source_envelope_refuses_before_ast_allocation` — 16 MiB+1B and depth 65 both fail at envelope before AST allocation (measure allocation gate, not message string).
  - T5.4: Happy path `cargo test -p braid-flow-sdk -p braid-cli -p braid-render --all-targets` green; rendered DOT escapes labels (injection test).
- **Verification:** `cargo test -p braid-flow-sdk --test source_equivalence -- unknown_and_lossy_sources_refuse` fails as specified; `cargo test -p braid-flow-sdk --test hostile_ron -- source_envelope_refuses_before_ast_allocation` fails as specified; renderer escapes `"><script>` label.

## Verification Contract

- `cargo test --workspace --all-targets --locked` (honest 453+ tests on head `ca777aa`; expect growth after U2/U4 — re-pin counts in plan update).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- `cargo fmt --all -- --check`.
- `./scripts/braid-registry-export-check.sh` and `./scripts/braid-release-probe.sh https://github.com/srinji-kaggss/Braid.git <merge-commit>` — both required green before any tag (U2).
- `./scripts/keel-floor.sh` with `KEEL_BIN` pointing at the hermetic dist (U4) — green on clean, red on seeded fixtures.
- `cargo test -p braid-flow-sdk -p braid-cli -p braid-render --all-targets` plus the two must-fail exploit tests (U5).
- Consumer proofs (U3) run in their repos, not in Braid CI; evidence is the commit SHA + tag + command log in the tracking issue.

## Definition of Done

**Global:** ROADMAP Phase 3 exit evidence (`docs/playbooks/END-STATE-PLAN.md` Phase 3) plus Phase 5 assurance ritual (PB-06) is recordable from a clean checkout by a second operator; DEBT_REGISTER D-CONSUMER and D-SA rows truthful; no `TODO`/`placeholder`/`assume` left in the U2-U4 code paths; `docs/STATE-*.md` reconciled to post-phase truth.

**Per-unit:**
- U1 — #63 closed with Director-credited decision entry + one closed-enum type; no second verdict engine; `cargo test` green.
- U2 — Exactly one reproducible tag exists; scratch consumer probes it without path deps; `consumer-probe/Cargo.lock.in` SHA documented; failed publish produces no tag.
- U3 — Two consumers compile against that tag; `rg` zero for competing authorities; integration tests decode/admit real data; mismatch fails typed.
- U4 — 8/8 stages + 2 runtime budgets have mutation-red tests; 3 seeded fixtures flip `NotSlop` red; Lean corpus agrees axiom-free; flight hours appended; release bundle includes evidence CID + tool versions + rollback procedure.
- U5 — `braid-flow-sdk` ships RON parse/pretty-print + JSON interop + every exploit-path test green; no verifier code duplicated into the SDK.

## Appendix

### Open issue map used to rank

13 open at scan time (2026-08-31). Ranked by ROADMAP DAG + STATE-2026-08-19 priority:

1. **P1 critical path:** #75 (publish) → #76 (consumers) → #78 (assurance+Lean). This is the "from verified substrate to releasable platform" arc in #74 Phase 3-5.
2. **P0.1 gate:** #63 (Read|Write amendment) — must resolve before #75's interface declares stable.
3. **P2 parallel:** #58 (Flow RON SDK) — Phase 1 remainder (P4); not gating release but maximizes flow throughput.
4. **Research backlog (deferred):** #61 (proof-carrying gates) depends on literature survey before any code; #25/#24/#23 are explicit research debts — after D-SA.
5. **Blocked/debt:** #6 U8 execution — blocked on kernel WASM runtime; `braid-run` remains reference semantics. #1 U6-U10 remainder already landed (U6-U7-U8-U9 hardening re-scoped into #70/#72/#78). #29 vendor-a-bot — gates on Keel, queue after #78 + hg sync.

Closed since the last census and taken as precedence: #77 DSL v0, #73 hermetic workspace, #72 preflight budgets, #71 Choice disjointness, #70 triad execution, #59/#60/#57 Flow P1-P3, #56 Flow authority ratification.

### File provenance

All paths repo-relative. Absolute absolute paths below are only for the two canonical consumer checkouts referenced in issues/playbooks:

- Kernel canonical: `~/logic-os-kernel` (`logic-os-kernel/kernel/Cargo.toml:213-214` path dep, snapshot test `kernel/crates/canvas-syscall/tests/braid_vocab_binding.rs`)
- Browser canonicals: `~/next-gen-browser-engine` vs `~/src/browser-engine` (stale duplicate per PB-04; canonical audit required)

### Confidence check

- Scope: Standard — 3 sequential P1 units (U2-U4) + 1 gate (U1) + 1 parallel lane (U5). Throughput is two lanes.
- Load-bearing risks: Q1 Director decision latency (mitigated: tiny enum addition, not full lowering); `~/wwfd/braid-registry/` staleness quarantining (mitigated: existing export+probe scripts); browser gosub upstreaming window (mitigated: start with kernel live-wire first).
- Verification fidelity: U2 and U4 already have runnable script gates; U3 greps are deterministic; U5 hostile tests are allocation-ordering tests (not string asserts).

---

*Plan follows `ce-plan` v3 — repo-relative paths, stable U-IDs, explicit test scenarios per file, KTDs with rejected alternatives. Next step is the Phase 5.4 handoff question.*


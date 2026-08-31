# Excellent Declarative Experience — Ideation

**Date:** 2026-08-31
**Mode:** repo-grounded — about Braid in this codebase
**Focus hint:** excellent declarative experience
**Output:** `docs/ideation/2026-08-31-excellent-declarative-experience.md`

> Braid's thesis is `declarative = correct-by-construction`, not `declarative = YAML with validation`. The IR is the declaration. Every idea below is judged against one test: does it make the declaration *shorter for AI, cheaper for human review, and impossible to misinterpret at admission*? If not, it's rejected.

---

## Grounding Context

### Codebase context — Braid shape today

- **Core:** `braid-ir` (`Capsule { braid, intent, grants, bounds, confirm_policy, versions }` → canonical CBOR → BLAKE3 `lw.braid.*` CID), `braid-verify` 8 fail-closed stages (canonical-form, version-pin, structure, types, capability, effect, bounds, taint), `braid-render` manifest (CID-bound) + DOT, `braid-sdk` Builder (`BuildError` typed refusals), `braid-cli` (`encode|decode|verify|render|diff|project build`), `braid-manifest` repo inventory (closed enums, no UNKNOWN).
- **Flow:** `braid-flow-ir` (bounded `FlowSpec` DAG with `FlowNodeKind::{InvokeCapsule, Choice, JoinAll, Terminal}`, `JustificationDecl`, `UrgencyClass`, `FlowBounds`, preflight budgets, `FlowCid lw.braid.flow.v0`, predicate fragment with verified disjointness), `braid-flow-verify` independent decoder + mutation matrix, `braid-flow-plan` snapshot-bound deterministic satiation (Plan CID `lw.braid.flow.plan.v0`, Snapshot CID `lw.braid.flow.snapshot.v0`).
- **Authoring:** `braid-elaborate-js` (JS expression subset → closed `js.*` vocab, operator precedence, pure-by-default, re-pinned CIDs), `braid-elaborate-dsl` bounded native `cms::v1` DSL (namespaced calls+pipes, authority/effect asserts, JSON-of-IR parity), `braid-project` deterministic multi-capsule build (fail-closed per capsule, project CID), `braid-vocab-{cms,js,web,rust}` closed registries with effect/cost/taint.
- **Lifecycle:** `scripts/cli-loop.sh` / `demo-port.sh` (the daemonized pipeline is the platform), `scripts/braid-registry-export-check.sh` (byte-stable CID check) + `braid-release-probe.sh` (clean scratch consumer, `--locked`), `braid-store`/`catalog`/`summary` (CID-keyed inventory, tap-deny on pin mismatch), `scripts/keel-floor.sh` diagnostic (native Keel hermetic pending #78).
- **Constraints:** charter G2 (no second `Capability`/`Verdict`/`Principal`), G4 (sole `Cid` authority), D6 gated general grammar, D8 (no floats, deterministic CBOR), D9 independent decoder, D10 `grant ⊆ ambient`, D12 manifest binding, D19 JSON-of-IR fence, D21 seam (any surface → core IR → one `verify` path), triad `Safety×Capability×Justification` (`Unknown` fails closed, `Deferred` now loud via `JustificationGate`).

**Pain points relevant to declarative UX:**
- Two lowerings (Builder vs JSON-of-IR) must stay CID-identical; drift is a bug.
- Refusal corpus lives per-frontend; no shared typed `Reject→re-author` shape across DSL/JS/Flow RON.
- Manifest is human-readable but not machine-queryable (grep-only); widening diff is exit-1, not a structured patch.
- Project build is deterministic but its `project CID` has no CLI `diff` or `why` friend.
- Flow authoring still JSON/Rust Builder only; RON (#58) not yet landed, so no single ergonomic surface has both full-graph vision and bounded ceilings (16 MiB / depth 64).
- No `braid test` harness; elaborator tests are golden CID + refusal, not runtime journal replay.

**Leverage points:**
- Verifier already returns `Reject { stage, reason }` typed — the re-author loop is free if surfaced.
- Canonical bytes + CID make every declaration diffable, cacheable, and content-addressable — declaration == identity.
- `TermRegistry` is already the closed alphabet — adding a term is the only way to widen what can be said.

### Past learnings (repo-relevant)

- `docs/solutions/` lesson: stray `cargo fmt` artifacts and `.rlib` ghosts polluted the tree — declarative outputs must have an explicit output dir, never beside source.
- `STATE-2026-08-19.md`: vendored registry at `~/wwfd/braid-registry/` stale at `0955e05` — declarations pinned to stale registries are the top adoption blocker.
- PB-03 invariant: elaborator is outside TCB — richest declarative surface can be arbitrarily clever because the verifier re-checks everything.

### External context — prior art that actually satisfies "excellent declarative"

- **Dhall / Nix / CUE:** total, typed, hermetic, no ambient effects; Dhall's `Type : Kind` and `show` determinism is the closest analogy to `Capsule::cid()` determinism — but none have a verifier that *explains* a rejection as a typed patch.
- **Terraform + Pulumi:** `plan`/`diff` as first-class UX; Pulumi's preview as a manifest-like review object is the model PB-05 copies (immutable deploy + alias); Terraform's state drift is the anti-pattern Braid avoids via `snapshot CID` binding.
- **Kubernetes manifests:** declarative but stringly-typed, `kubectl apply --dry-run=server` closest to `braid verify` before `braid deploy`; admission controllers (OPA/Kyverno) parallel `braid-verify` stages but are bolted-on, not single-path.
- **SQL / Relational algebra:** declarative where optimizer chooses plan; Braid's Flow `Choice` disjointness proof + snapshot-bound satiation is the analogous `EXPLAIN` that can be *proven* disjoint, not heuristically chosen.
- **React / SwiftUI:** declarative view = function of state, diff = patch; Braid's DOT + manifest diff is the same idea for authority/effects rather than pixels.
- **Lean / Coq:** tiny kernel, elaborate surface outside TCB, Mathlib-scale via elaboration — exactly Braid's D21 seam. Lean's error messages as typed goals are the target for `Reject { stage, reason }` ergonomics.
- **Excel / Notion formulas:** end-user declarative that never claims Turing completeness — the "bounded DSL" model for `cms::v1` and `js` expression subsets.

---

## Topic Axes

Derived from grounding, not a template:

1. **Authoring surface** — how a human or LLM writes the wish (text, builder, RON, JSON-of-IR).
2. **Immediate feedback** — what you see when it is wrong (error, hint, counter-example, patch).
3. **Composition & reuse** — how declarations combine (import, project, flow, patterns).
4. **Version & migration** — how declarations survive a registry/vocab bump.
5. **Review & deployment** — how a declaration becomes a running, promotable, rollbackable deployment.

---

## Raw Ideas — 42 across 6 frames

### Frame 1 — Authoring surfaces that stay outside the TCB (lens: elaboration seam)

- **I1 `braid fmt` as canonical authoring form.** `braid dsl check --fix` rewrites any surface (JS subset, DSL, RON, JSON-of-IR) to one pretty-printed RON with deterministic ordering, so the declaration has a single *human* form but text is never authority (bytes are).
- **I2 `braid new` project scaffold with golden pins.** One command scaffolds a `Braid.toml` project, a `fixtures/` dir, and a `justfile` that runs `project build --check-cids` — the declaration starts pinned, not emergent.
- **I3 Closed-snippet palette in `braid integrations`.** Each vocab term ships a one-line `/// doc` snippet; `braid term list --vocab cms --as ron` emits copy-pasteable `call("cms.fetch", ...)` lines — no freeform code to hallucinate.
- **I4 `// braid:ignore` is a hard error.** Any comment pragma that tries to silence a stage is `Reject { stage: Structure }` — prevents Terraform-style `#tfignore` escape.
- **I5 File-as-declaration:** `*.braid` is a directory, not a file — `capsule.braid/strand-*.ron + manifest.toml + evidence/` is the unit, so a large declaration shards naturally and each strand diffs independently.
- **I6 Elaborator as LSP server.** `braid lsp` serves diagnostics from the verifier, not the parser — the LLM and the human see the same `Reject { stage, reason, at: strand }` while typing, with code actions "insert missing `cost_order`" / "attenuate grant".
- **I7 Two-surface parity badge.** Every golden test folder contains `source.dsl` + `source.json` → same `cid.txt`; CI fails if they diverge — the declaration's redundancy is the proof.
- **I8 Sugar that desugars visibly.** `let x = 1 + 2` in DSL lowers to `js.add` but `braid render --explain` shows the desugared strand list — the declaration never hides its expansion.

### Frame 2 — Feedback that is a typed patch, not a message (lens: fail-closed stages)

- **I9 `Reject` as a struct, not a string.** `braid verify --json` emits `{"stage":"Capability","at":"strand 3: js.fetch","expected":"grant ⊆ ambient {read:cms}","got":"{read:cms, egress:web}","fix":"remove grant web.* or attenuate strand"}` — machine-readable re-author payload.
- **I10 `braid why --strand 3`.** Explain *one* strand: its type, effect, grant, taint, cost, and which stage would reject it under a hypothetical wider grant — like `rustc --explain` but for authority.
- **I11 Counter-example for Choice overlap.** When disjointness fails, the verifier prints the minimal `UNSAT` counterexample clause set (`Choice x arm 2 ∧ arm 5` overlap on `fact: index.fragmented = true`) — the declaration tells you why it is not deterministic.
- **I12 Widening as a structured delta.** `braid diff --json` emits `{"widened":["capability web.*","effect egress"],"narrowed":[]} | jq '.widened'` so PR bots can block on exact field rather than grepping manifest text.
- **I13 `braid check --staged` pre-commit hook that prints the *alias* you will promote.** Before commit, shows `preview grant = {read:cms}` vs `prod grant = {read:cms, write:cms}` — the same gate you will hit at deploy, early.
- **I14 `verify --explain taint`.** Taint path-level fold shown as a graph slice (`strand 1 exposure:Public → strand 4 max(Public, Internal)=Internal`) — the declaration's non-interference is visible.
- **I15 Refusal corpus as `cargo test` output.** `cargo test -p braid-elaborate-dsl -- --show-refusals` prints a table `source | stage | fix hint` — the closed alphabet's boundary is a runnable spec.

### Frame 3 — Composition without string imports (lens: content-addressed, closed vocab)

- **I16 `use` is a CID, not a path.** `import { fetch } from "braid:cms@<registry_cid>#fetch"` — the declaration pins the registry CID at the import site; `unknown term ⇒ deny` is the resolver, not a linker error.
- **I17 Flow as a declaration of declarations.** `FlowSpec` declares `roots, nodes { Choice { arms } }, edges` plus `JustificationDecl` — a flow *is* the declarative flow; no hidden `MapStatic` expansion without a CID bump.
- **I18 Project as a declaration of capsules.** `Braid.toml` lists `capsules = ["capsule-a", "capsule-b"]` pinned by `project_cid`; `braid project diff` shows narrower/wider at project granularity — reuse is pinning, not copying.
- **I19 Pattern library, not inheritance.** `braid pattern init cms-crud --as flow` scaffolds a `FlowSpec` template (list→get→mutate→confirm) as copy-paste RON — patterns are starting points per `templates/README.md` pull-not-push contract, not a registry.
- **I20 `braid graph --view compact` for Flows.** DOT export collapses `JoinAll` diamonds and Choice arms into one edge labeled `when:` — the declaration's shape is reviewable in one screen.
- **I21 Capability-aware autocomplete.** `braid complete --grant ambient` only offers terms whose `effect ⊆ grant` — the declaration cannot propose what it cannot admit.
- **I22 `braid check --no-network`.** Composition resolves purely from local `consumer-probe/Cargo.lock.in` + registry bytes — no fetcher, no import side-effect, so a declaration is hermetic.

### Frame 4 — Versioning that is a conscious event (lens: pins, CIDs, migration proofs)

- **I23 `braid pin` is explicit.** `braid pin --vocab cms --version 2` rewrites `vocab_version` + `registry_cid` in all capsules, prints `widened: [cms.paginate]`, and blocks if any golden CID was not re-pinned — a bump is a diff, not an implicit resolve.
- **I24 Migration proofs are declarations.** A version skew produces a `required_by_version: 2` typed `Deferred` gate reason that renders in the receipt — the next version's requirement is visible before it is enforced (D-FLOW.13).
- **I25 Time-travel verify.** `braid verify --at registry@abc123` re-verifies a capsule against a past registry CID without rewriting the file — the declaration's admissibility is a pure function of two CIDs.
- **I26 `braid outdated` as a manifest query.** Lists which capsules are behind the current `registry_v0()` and what new terms they could use — the declaration's upgrade path is enumerated.
- **I27 Rollback is a pointer move.** `braid deploy rollback --to <deployment_cid>` re-verifies the old bytes at load (T9) — the declaration's prior version is safe *because* admission re-runs.

### Frame 5 — Review & deployment as the same primitive (lens: manifests, journals, aliases)

- **I28 Manifest as queryable JSON, not only text.** `braid render --json | jq '.effects'` gives machines the same review object humans read as text — one object, two renderings, one CID binding.
- **I29 Preview is a declaration with attenuated grants.** `braid deploy --preview` builds exactly the prod capsule but with `ambient = {read:cms}` (projection-only) — the declaration is identical, the grant differs, and `promotion` is the only place wider grants attach (PB-05).
- **I30 Promotion is a confirm-gated alias move.** `braid promote --confirm <payload_hash>` journal-entries the alias pointer plus the confirmation hash — the declaration's most dangerous transition is signed.
- **I31 History is a declaration.** `braid log --json` is a CID chain (`deployment_cid → plan_cid → snapshot_cid → capsule_cid`) — the current prod is always re-derivable from the journal file, never daemon memory.
- **I32 `braid summary` as platform dashboard.** Already ships for repo manifests; extend to `braid deploy summary --json` — a fail-closed org map of `deployment × manifest × journal` pins.

### Frame 6 — AI-native declarativeness that embraces redundancy (lens: D20 predictable surface)

- **I33 Declarative task brief as a counted dataset.** The LLM prompt is a `Task { goal, constraints=[...], examples=[...] }` struct with closed vocabulary, not prose — the declaration *of the task* is what the harness (`braid-seam-conformance`) measures `no-learned-gate elaborations / total` against (ADR-100 95% fence).
- **I34 Reject→re-author as typed tool use.** The LLM's tool is `elaborate(source) -> Result<Cid, Reject>` where `Reject` is the JSON from I9 — the loop is deterministic: LLM drafts → verifier rejects with patch → LLM reapplies patch, measured as `rounds-to-admission`.
- **I35 GNN/semantic gate as `Reject { stage: Semantic }`, never a score.** The learned gate's output is a concrete `Capsule` alternative that *must* re-admit — a probabilistic gate never lives inside the TCB, it proposes, the verifier disposes (D32).
- **I36 Vocab stickiness as typed errors.** `regex` where `parse` was required is `Reject { stage: Types, expected: Parse, got: Regex }` — the declaration knows the words, not just the types.
- **I37 Golden corpus as training data generator.** Every `source → cid + manifest` golden is one supervised pair for the emulator; `braid-seam-conformance` is the dataset, not a synthetic benchmark.
- **I38 Red-team as declarative pressure.** `fixtures/known-bad/` are declarations that *should* fail; the floor is measured by `every_invariant_has_a_killing_negative` (mutation matrix) — excellent is proven by the negatives that are refused.
- **I39 Touchless preview metric.** `touches_per_deploy`, `time_to_preview`, `refusal_precision` emitted per AI deploy — the declaration's quality is a number the enterprise pitch can sell ("your AI can deploy unsupervised, and you can prove what it can't do").
- **I40 `braid explain --for llm`.** Same rejection, different rendering: a one-sentence LLM hint ("add `cost_order: cms.ordered` to strand 2") derived from the stage's typed fields — the declaration speaks both languages.
- **I41 Procedural thresholds are declarations.** Bounds `max_nodes 50k`, `capsule preflight` budgets, `wire_depth 64` are not knobs but `INV-FLOW-004` invariants — the declaration's resource ceiling is part of its meaning.
- **I42 Justification as a declarative question.** Every material `InvokeCapsule` declares `needed_when / satisfied_when / guarantees / preserves` — the invocation states *why it exists*, and `Unknown` defers rather than executes (D-FLOW.6).

---

## Critique — all 42 pass the explicit rejection filter first

### Hard rejections (drop, with reason)

- **I4 `// braid:ignore` hard error** — *reject*: requires comment mining in a language-agnostic verifier; capability to ignore is an authority escalation vector. Replace with `allowlist` crate-level `APPROVED.toml` (already exists via `lgwks_std_gate`).
- **I5 File-as-directory for a capsule** — *reject*: breaks `Cid::compute(lw.braid.capsule.v0, bytes)` single-file content addressing; directory hashing introduces filesystem-dependent ordering bugs. Keep capsules as single canonical bytes; use `Braid.toml` project (already exists: `braid-project`) to bundle them.
- **I6 Elaborator as LSP server in scope** — *reject*: scope creep for the next workstream (U1 is a 1-enum gate, U2 is publish). LSP is valuable but belongs after `braid-flow-sdk` RON ships (#58) and after the first consumer tag — not now.

### Survivors after critique: 39 (42 − 3). Rank by leverage × novelty × implementability in this repo.

---

## Survivors Ranked — Top 7 (carry to ce-brainstorm)

### 🥇 S1 — `Reject` as structured JSON that the LLM re-authors from (I9 + I34)

**What:** `braid verify --json`'s `Reject { stage, at, expected, got, fix }` becomes the *only* tool output the AI loop consumes. The loop is `author → elaborate → verify --json → (if Reject) feed Reject json as next prompt → re-elaborate` until `Admit` + CID. Measure `rounds-to-admission` on `braid-seam-conformance` corpus.

**Why it survives:** closes the D24 `reject→re-author` demo, is generic across DSL/JS/Flow RON, makes the verifier the teacher, and is the data that lets a GNN gate be evaluated deterministically (D32: "95% is measured, not vibed"). Highest leverage — turns the weakest declarative point (error messages) into the product's sharpest edge.

**Carries risks:** `fix` must be code-actionable, not prose; start with one code action per stage, not an open-ended hint engine.

### 🥈 S2 — Widening and taint as structured deltas, not text grep (I12 + I14)

**What:** `braid diff --json` and `braid render --json` emit canonical JSON with `widened: [capability, effect, taint]`, `narrowed: []`, `taint_path: ["strand1:Public → strand4:Internal(max)"]`. PR bots block by exact field match; no `grep manifest` drift. Extend `braid-render`'s `Manifest`/`Widening` types to have `to_json()`.

**Why it survives:** PB-05 invariant is "widening vs what is LIVE (aliased), not lineage parent" — unattainable if widening is a text diff. Gives `keel`/`keel-floor.sh` a second consumer for the same struct.

### 🥉 S3 — Import is a CID and composition has no fetcher (I16 + I22)

**What:** The declaration's only import form is `braid:vocab@<registry_cid>#term`; `braid check --no-network` proves hermetic. `Braid.toml` pins one `registry_cid` per project. `unknown term ⇒ deny` is the resolver.

**Why it survives:** kills the entire class of "works on my machine because of a local path" that #73 just closed for `secure-authority`. Makes every declaration reproducible without a fetcher and aligns vocabulary ownership (D15) — `web.*` can change but old pins do not move.

### 4 — Preview is the prod declaration with an attenuated grant (I29 + I31)

**What:** `braid deploy --preview` builds the exact capsule bytes, serves it via `braid-run` with `ambient = {projection reads only}`, and records a `Deployment { capsule_cid, manifest_cid, admission, journal_ref, alias = preview }`. `braid log --json` is a CID chain re-derivable from files.

**Why it survives:** the Vercel niche thesis (`docs/playbooks/PB-05-deploy-platform.md`). The most valuable declarative property is `preview ≠ prod authority` as a structural guarantee, not a policy.

### 5 — Promotion as signed alias move with ConfirmPolicy binding (I30 + I23)

**What:** `braid promote --confirm <payload_hash>` re-verifies, demands a `HumanConfirm(payload-hash-bound)` confirmation iff the manifest contains `Irreversible|Egress`, writes a signed alias move to the journal. `braid pin --vocab` already requires re-pinning golden CIDs — pair the two: a vocab bump that widens to `Irreversible` *cannot* promote without the confirm.

**Why it survives:** threat T10/T4 (payload-hash-bound confirm, re-derive manifest and reject on CID mismatch). Turns the scariest declarative transition into the most auditable one.

### 6 — Authoring palette + `braid check` in the loop, not an IDE first (I3 + I13)

**What:** ship `braid term list --vocab cms --as ron` + `braid new --template cms-crud` immediately; wire `braid check --staged` as the pre-commit hook that prints the alias you will promote. Defer LSP (I6) until after #58. This is the "shortest path that makes declarativeness sticky" — copy-pasteable closed snippets beat an autocomplete that guesses.

**Why it survives:** zero-cost to build (already have `TermRegistry` listing), closes the "human doesn't know what the closed alphabet contains" gap, and is the piece `templates/README.md` already promises under pull-not-push.

### 7 — Time-travel verify + `outdated` as upgrade declaration (I25 + I26)

**What:** `braid verify --at registry@<cid>` and `braid outdated` ("capsule foo is behind registry_v0(), terms you could adopt: cms.paginate") as first-class commands. A version bump is a `braid pin` declaration that prints a widening diff — the migration is reviewable before it is committed.

**Why it survives:** D11 version covenant ("a pin bump is a conscious, reviewed event") has no CLI today. This makes the covenant operational and pairs with D-FLOW.13's `Deferred { required_by_version }` rendering — the *next* version's requirement is visible now.

---

## Near-survivors held for the following workstream

- `braid why --strand 3` (I10), counter-example for disjointness (I11), `braid graph --view compact` (I20), declarative task brief struct (I33), procedural thresholds as declarations (I41), justification as declarative question (I42) — all aligned, but second-order after the 7 above. Carry them as stretch when the top 7 land.

## Rejection Summary Table

| Idea | Verdict | Reason |
|---|---|---|
| I1 braid fmt | survivor (implicit in S6 palette direction) | keep, but not top-7 alone — subsumed by S6 + RON pretty-print in #58 |
| I2 braid new | survivor (in S6) | scaffold already partially exists via `braid-project` |
| I3 palette | 🥇 top-7 (S6) | — |
| I4 ignore pragma | **REJECT** | comment-mining, escape hatch |
| I5 file-as-directory | **REJECT** | breaks single-bytes Cid |
| I6 LSP | **REJECT** (defer) | after #58 + first tag |
| I7 parity badge | survivor | carry as U5 test in #58 (`source_equivalence`) |
| I8 sugar desugar visible | survivor | subsumed by `render --explain` (near) |
| I9 Reject json | 🥇 top-7 (S1) | — |
| I10 why strand | near | — |
| I11 disjointness counterexample | near | — |
| I12 diff json | 🥇 top-7 (S2) | — |
| I13 check staged | survivor (in S6) | — |
| I14 explain taint | near (in S2) | — |
| I15 refusal corpus table | survivor | already in DSL crate |
| I16 CID import | 🥇 top-7 (S3) | — |
| I17 Flow as declaration | survivor | core, already true |
| I18 project as declaration | survivor | already `Braid.toml` |
| I19 pattern library | survivor (near) | post-RON |
| I20 graph compact | near | — |
| I21 capability-complete | survivor | quick follow to S3 |
| I22 no-network check | 🥇 top-7 (S3) | — |
| I23 pin explicit | survivor (in S5/S7) | — |
| I24 migration proofs | survivor (in S7) | — |
| I25 time-travel verify | 🥇 top-7 (S7) | — |
| I26 outdated | 🥇 top-7 (S7) | — |
| I27 rollback pointer | survivor (in S4) | — |
| I28 manifest json | 🥇 top-7 (S2) | — |
| I29 preview attenuated | 🥇 top-7 (S4) | — |
| I30 promotion signed | 🥇 top-7 (S5) | — |
| I31 history CID chain | 🥇 top-7 (S4) | — |
| I32 deploy summary | survivor | post-platform |
| I33 task brief struct | near | — |
| I34 reject re-author loop | 🥇 top-7 (S1) | — |
| I35 GNN as Reject stage | survivor | D32, not now |
| I36 vocab stickiness | survivor | DSL follow-up |
| I37 golden corpus as data | survivor | `braid-seam-conformance` future |
| I38 red-team negatives | survivor | already mutation matrix |
| I39 touchless metric | near | platform demo |
| I40 explain for llm | survivor | hint rendering of S1 |
| I41 thresholds as declarations | near | — |
| I42 justification declarative | near | D-FLOW.13 already |

---

## Handoff

**One-line synthesis for the next `ce-brainstorm`:** *Make Braid's excellent declarative experience concrete as `declaration == bytes == CID`, `rejection == typed patch`, `preview == attenuated prod`, and `promotion == signed alias move` — with imports pinned by CID and every widening as structured JSON, not text grep.*

**Recommended `ce-brainstorm` scope (narrow, excellent-first):** Use the **Top-7** as the candidate feature set. `ce-brainstorm` should decide between shipping S1+S2+S3 first (the *authoring+feedback+pinning* slice that closes the "brittle declarative" risk) vs bundling S4+S5 (the *preview+promotion* slice that proves the Vercel niche), record the thin-slice decision, and write the requirements-only unified plan that `ce-plan` enriches into `U1..U4` with the same CLI surface named above.

**If you proceed directly to `ce-plan`:** add two implementation-ready units — `U_declarative_reject_json` (I9/I34 wire + `--json` flag + `rounds-to-admission` harness) and `U_declarative_diff_json` (I12/I28 `diff --json`/`render --json` + structured `Widening`), both gated on no new authority and `--locked` hermeticity. The remaining top-7 (S3..S7) defer to the deployment platform lane (`docs/playbooks/PB-05`) rather than this repo's next tag if capacity is limited.

**Provenance:** `Braid/spec/braid/{PRD,DECISIONS,DEBT_REGISTER,ADMISSION,SAFETY_ASSURANCE_CI_SPEC}.md` · `docs/{STATE-2026-08-19,STRATEGIC-VISION,playbooks/PB-{01,03,05}}` · `Braid/crates/{braid-flow-ir,braid-verify,braid-render,braid-project}/` · `Braid/docs/adr-099` + `adr-100` · `Braid/braid.profile.json`


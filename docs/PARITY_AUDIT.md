# PARITY_AUDIT — session asks vs braid state

Date: 2026-08-16. Truthful ledger; consumership is gated on "1/1 done" — this file says exactly what is and is not 1/1.

Legend: **DONE** = implemented + tests green · **PARTIAL** = slice landed, rest named · **GAP** = spec-only or absent · **GATED** = decision-locked.

| # | Ask (this session) | State | Evidence |
|---|---|---|---|
| A1 | AppleScript-like DSL, macro similar, on Rust | **GATED** — D6 LOCKED (surface syntax gates on strategy-doc §16). The *substrate* for the ask landed: effect-ordering (this session) + Opaque-dimension typing make command graphs verifiable; the textual/macro surface awaits the D6 re-gate. Similar macros found: `osascript-macros` 1.1.0 (precedent, 0% documented), `osakit-rs` (OSA execution layer). | crates.io + docs.rs fetch |
| A2 | "A database is always 1" | **DONE this session (W5)** — the org database is the braid store + pinned inventory (`inventory.json`: name → manifest CID), tamper-evident by re-hash-on-read; `braid catalog`/`summary` read it. Registry export slice 1 live at `~/wwfd/braid-registry/` (4 crates @ 0955e05) but STALE vs Braid HEAD and has no regeneration script — out of W5 scope, refresh at consumership. | workspace 47 suites green; live journey incl. tamper-denial + recovery |
| A3 | Kill the 9 reimplementations | **GAP** — requires consumership (Director-gated) + keel duplicate-concept gate (keel repo not authorized this session). | cross-repo inventory |
| A4 | Write → accepted → callable crate | **DONE this session (W1)** — `braid-vocab-rust` elaborates an admitted capsule into a dependency-free Rust API surface (Opaque dims → distinct newtypes, terms → trait fn declarations, `CAPSULE_CID` const, re-verify in build.rs); `braid-project build --target rust` emits `<name>-rust/<capsule>/`; emitted crate compiles warning-free (live journey). Trait bodies are declarations only — honest until a runtime (U7). | workspace 47 suites green; `EMITTED-CRATE-COMPILES-WARNING-FREE` |
| A5 | Stickiness — agents use it | **PARTIAL** — mechanism designed (keel/local-ci gate wiring), not built; zero external consumers (D-CONSUMER). | DEBT_REGISTER D-CONSUMER |
| A6 | Time/math type contract; natural order | **DONE this session** — (1) Effect stage 6b: Irreversible/Egress strands must be totally ordered by data flow, else Reject; (2) dimension contract pinned: distinct `Opaque` dimensions Reject at Types; (3) `expires_at` enforced against a clock in braid-governance (previously test-pinned unenforced; strict `YYYY-MM-DDTHH:MM:SSZ` parse, fail-closed). | workspace tests: verify 36+20, governance 36+3, all green |
| A7 | Repo formats + full org summary | **DONE this session (W5)** — `braid store put` / `catalog` / `summary` ship: 8-field manifests (closed enums, no UNKNOWN representable), pinned-inventory completeness check, fail-closed reads (tampered/stale pin denies the whole map), golden-pinned deterministic output, 10-repo fixtures + negative matrix. **Sibling-format decision (Director 2026-08-16):** manifests are NOT capsules yet — strand payloads are valueless (D8-locked); they graduate to capsule form when the Strand-literal unit lands. Fixture manifests stand in for real per-repo data entry (`seed.sh`). | braid-cli 11/11 catalog tests; 47 suites green; journey exit codes 0/1/2 verified |
| A8 | Semantic-contract enforcer completeness | **PARTIAL** — 8 stages mutation-verified; remaining debt: runtime (U7, kernel WASM-blocked), Lean semantic half (U15), D24 error-edge, D25 intent-coherence, literal payloads. | DEBT_REGISTER, MUTATION-LEDGER |
| A9 | Language on top of a language | **DONE at substrate level** (D5/D31) — IR is the day-0 language, Rust day-1; frontend pattern operational (vocab-js U11–U12). | DECISIONS.md D5/D31 |

## SOTA gaps, answered truthfully

**What the field has that braid lacks:**
- A runtime. DEBT_REGISTER's own bar: "a JVM with no V" — a verified capsule does not execute (U7, blocked on the kernel WASM epic).
- Compile-time units: `uom`/`dimensioned`/`typenum` check dimensions at Rust compile time; braid checks them at *admission* of the composition (strictly stronger per-capsule, but no codegen target carries them yet).
- Package registries, IDP catalogs, and consumers: braid has zero external dependents.

**What braid has that the field lacks:**
- Fail-closed deterministic admission of the *composition* — types, effects, ordering, taint, budgets — not just type checking.
- Content-addressed registries where the pin IS the content (version skew is a Reject, not a resolver conflict).
- The org catalog (W5): a fail-closed org map whose every row is content-pinned and tamper-evident — Backstage-class IDPs (plan non-goal) catalog metadata with no integrity binding at all; `braid catalog` denies rather than render a map it cannot prove complete.
- Mutation-verified stages: every stage in the pipeline has a recorded mutation test proving it load-bearing.

## 1/1 ledger

A2 DONE, A4 DONE, A6 DONE, A7 DONE, A9 DONE · A5/A8 PARTIAL · A1 GATED · A3 GAP. **Not 1/1 yet** — consumership authorization correctly deferred.

Closure order: W1 ✅ (A4) → W5 ✅ (A2/A7) → W4 (A3/A5, gates — needs keel authorization) → D6 re-gate (A1) → U15/U7 stay on their own debt-register track.

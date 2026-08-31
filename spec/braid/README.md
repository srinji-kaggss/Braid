# Braid — machine-first framework spec (extraction-ready home)

> ⚠️ **NAME PROVISIONAL — context-blur landmine (D15).** "Braid" is a working
> codename until the Director finalizes naming. Identity = this directory path
> + `DECISIONS.md`, never the word. If you encounter ANY other artifact named
> braid/axiom/similar, treat it as a collision: report it, do not assume it is
> this framework.

> **Braid** (working name, revisable): Logic OS's machine-first application
> framework. Programs are content-addressed graphs of typed terms drawn from
> the closed kernel vocabulary; a deterministic compiler/verifier pipeline —
> not an AI, not a reviewer — owns compliance; humans audit rendered manifests.
> Ratified by `../../docs/adr-088-braid-machine-first-framework-foundations.md`.

## Why this directory exists

The Director's requirement (2026-06-12 session): one comprehensive, locked
document set from which agents can plan and execute without re-deriving
strategy. This directory is **self-contained by covenant** (ADR-088 D5): it was
extracted to its own repository (`srinji-kaggss/Braid`, 2026-06-13), so nothing
in here depends on kernel-internal paths beyond the declared contracts — the one
crossing type, `Capability`, is vendored as the `braid-capability` crate. See
the ADR's extraction addendum for what changed in the move.

## Reading order

1. `../../docs/adr-088-braid-machine-first-framework-foundations.md` — the ratified decisions.
2. `DECISIONS.md` — the numbered decision register with lock status and verbatim Director provenance. **Check the lock legend before proposing any change.**
3. `PRD.md` — product requirements: identity, users, goals/non-goals, the IR model, phases, acceptance scenarios.
4. `threat-model.md` — the failure-mode and abuse catalogue (T1–T16). Every unit's acceptance criteria cite the threats it closes.
5. `units.md` — the issue-ready execution plan (U0–U10) with acceptance criteria and verification commands.
6. `../../docs/adr-099-braid-frontier-flow.md` — ratified inter-capsule authority, wire/source forms, and implementation boundaries.
7. `BRAID_FRONTIER_FLOW.okf.md` — accepted inter-capsule Flow IR, justified frontier planner, falsification matrix, and Codex continuation contract.

## The one-paragraph mental model

A **strand** is one typed term: a registered operator with a declared
signature — input types, output type, required capability, effect class, cost
bound. A **braid** is a DAG of strands: composition is legal only when types
unify, authority only attenuates, effects compose within the registered
postures, and exposure taint stays monotone along every path. A **capsule**
wraps a braid with its declared intent, grants, bounds, confirmation policy,
and evidence policy, canonically encoded and content-addressed. The
**verifier** admits or rejects capsules deterministically; the **manifest** is
the human-readable rendering bound to the capsule's CID — the review object is
the manifest diff. Execution happens only through the kernel's three-syscall
surface; Braid adds zero authority.

## Status

- **Frontier Flow P0 ratified** (ADR-099 / Issue #56): RON is the first-class
  textual authoring form, normalized JSON is interoperability/inspection,
  Braid owns semantic Flow and deterministic planning, and forge-harness owns
  durable execution. P1 is [#57](https://github.com/srinji-kaggss/Braid/issues/57).

- Phase: **P1–P2 core landed on branch** (2026-06-12, Director-directed):
  `crates/braid-ir` (U1 #558 — types, canonical encoding + bijection
  guard, CID, KAT vectors in `vectors/`, boundary conformance; the kernel
  vocab-binding test re-homed to the kernel on extraction — see ADR addendum),
  `crates/braid-verify` (U3–U5 #560 — independent decoder + the full
  fail-closed stage pipeline incl. path-level taint), `crates/braid-render`
  (U2 #559 — CID-bound manifest, widening diff, DOT export). All acceptance
  scenarios implemented as tests; mutation evidence on the issues. **Merged to
  `main`; U9 hacker pass landed.** (D5/D16/D31 remain INTERPRETED pending
  Director veto — see `DEBT_REGISTER.md` D-CONFIRM.)
- U6 `braid-cli` + CI widening gate **landed** (#2, 2026-06-14):
  `encode|decode|verify|render|diff`; `encode` reads JSON-of-IR (D19) through
  the SDK so the CLI path reproduces the pinned reference CIDs; scenario #12
  runs in CI (`scripts/cli-loop.sh`); the T12 widening gate fires on a seeded
  widening (mutation-proven). U10 SDK polish also landed.
- U8 **author→admit→render slice landed** (demo-port, D16; modeled on the
  kernel `blueprints/afternow-port/` surface): three CMS reference actions
  (`edit-home-hero`, `publish-services`, `render-work-listing`) authored via the
  SDK/JSON-of-IR path, admitted, and rendered, with a regenerable evidence
  bundle in `spec/braid/vectors/demo-port/` and pinned-CID tests
  (`crates/braid-cli/tests/demo_port.rs`). The no-confirm publish is the
  fail-closed escalation probe. The **execution leg** (run on kernel WASM +
  on-tape fact journal) is deferred behind the U7 seam — see the bundle README
  and the U7 follow-up issue (#6).
- **U9 adversarial pass landed** — 4 findings closed (T3, T4, T12, R3); verdict
  at `U9-VERDICT.md`. **D31 global-IR refactor landed** — substrate/vocabulary
  split, string-tagged capabilities, `TypeTag::Opaque`; vocabularies CMS + JS +
  web. **U-SA's historical Keel profile is not a current CI gate** — the Node
  adapter was retired, native Keel reports blocking debt, and #78 owns the
  hermetic migration. **v0 substrate behavior exists; assurance release
  repeatability remains open.**
- **Post-v0 frontier (this session, all merged to `main`):**
  - **U11** — `braid-elaborate-js`, the first real language frontend: JS *text*
    compiles into an admitted capsule via the one verifier ("renders JS useless"
    made operational, D31).
  - **U12** — JS vocabulary v1→v2 (8→16 pure terms) + an operator-precedence
    expression language (`+ - * < == && || !`, booleans, parens).
  - **U13** — `braid-project`, the first multi-capsule build: manifest →
    elaborate + admit all (fail-closed) → deterministic project CID.
  - All three carry **mutation-proven anti-dredging guards** (no
    composition/aggregation exfil, no silent CID drift, no test-hollowing).
    See `units.md` U11–U15 and `DEBT_REGISTER.md`.
- Not yet built: **U7** WASM codegen + the U8 execution leg (kernel-runtime-
  blocked); **U14** first live consumer collapse (cross-repo); **U15**
  Lean⇄verifier conformance.
- Work tracker: GitHub Issues (authoritative). No issue, no work.
- This workstream is parallel to — never ahead of — the A-series
  trust-boundary queue (`docs/logic-os-build-state.md`).

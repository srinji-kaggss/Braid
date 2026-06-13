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

- Phase: **P1–P2 core landed on branch** (2026-06-12, Director-directed):
  `crates/braid-ir` (U1 #558 — types, canonical encoding + bijection
  guard, CID, KAT vectors in `vectors/`, boundary conformance; the kernel
  vocab-binding test re-homed to the kernel on extraction — see ADR addendum),
  `crates/braid-verify` (U3–U5 #560 — independent decoder + the full
  fail-closed stage pipeline incl. path-level taint), `crates/braid-render`
  (U2 #559 — CID-bound manifest, widening diff, DOT export). All acceptance
  scenarios implemented as tests; mutation evidence on the issues. Pending:
  Director review of D5/D16 interpretations, U9 hacker pass, merge.
- Not yet built: U6 CLI + CI gate, U7 WASM codegen (kernel-runtime-blocked),
  U8 full CMS reference execution, U10 SDK polish.
- Work tracker: GitHub Issues (authoritative). No issue, no work.
- This workstream is parallel to — never ahead of — the A-series
  trust-boundary queue (`docs/logic-os-build-state.md`).

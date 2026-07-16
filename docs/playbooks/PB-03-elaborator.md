# PB-03 — A real elaborator: one language actually compiles to Braid (D-ELAB)

**Objective**: turn the global-IR thesis (D31) from architectural to operational —
ship the first elaborator that takes a real authoring surface and emits admitted
capsules. This is the `javac` of the Java analogy and the **build step** of the
Vercel analogy: without it, the platform has deploys but nothing to build them from.

**What "elaborator" means here (D21, the Lean shape)**: an untrusted frontend that
lowers a familiar, redundant authoring surface into core IR, where the independent
verifier re-checks everything. The elaborator can be arbitrarily rich and even
LLM-assisted, because it is **outside the TCB** — admission never trusts it (D9).
This is exactly how Lean gets Mathlib-scale reach from a tiny kernel.

**The scope fence (do not violate D6)**: D6 still gates *human-writable surface
syntax*. The first elaborator therefore targets an **existing language subset**, not
a new grammar. Candidate fork for the Director (surface, don't default — D28):
- **(A) JS/TS subset → `braid-vocab-js`** — the vocab already exists (8 terms,
  `js.*` capability space); "renders JS useless" is the stated ambition; enormous
  AI training-corpus familiarity (D20: predictable surface).
- **(B) JSX/TSX component subset → `braid-vocab-web`** — sharper for the Vercel
  niche (PB-05 deploys frontend components; D16: render output is typed directives,
  never DOM strings); the browser engine is the natural consumer (PB-04).
Recommendation to put in the issue: **B for the niche, A for the thesis — start with
B**, because it feeds PB-04/PB-05 directly and its vocabulary (pure render terms +
projection reads, near-zero irreversible effects) is the safest first alphabet
(D16's own argument). Record the pick as a D-entry.

## Deep-learning corpus (read → probe)

1. `DECISIONS.md` D20 (predictable-surface thesis — the authoring surface must be
   familiar/redundant/shallow; the canonical IR is an anchor, NOT what the model
   writes), D21 (elaboration seam), D19 (the JSON-of-IR fence you are superseding
   for one language), D24 (registry = ground truth; unknown term ⇒ deny), D31.
2. `crates/braid-sdk/src/lib.rs` — the Builder the elaborator emits through (author-
   time refusals are your first error channel).
3. `crates/braid-vocab-web/src/lib.rs` + `braid-vocab-js/src/lib.rs` — the closed
   target alphabets; every effect class and egress ceiling you must map onto.
4. `crates/braid-cli/src/main.rs` `encode` path — the JSON-of-IR transport the
   elaborator can emit as its portable output format (keeps the CLI loop, T13).
5. `docs/BRAID_STEER.md` §"what stays yours" — vocabulary ownership rules.
6. Threat model T1 (escape hatches — the elaborator is the #1 place a freeform-code
   smuggle re-enters), T13 (the CLI path must stay equivalent).

Probe: hand-write the smallest web-vocab capsule via JSON-of-IR, admit and render
it. Then write the source-language snippet that *should* elaborate to exactly that
capsule — byte-identical CID is your elaborator's first golden test.

## Invariants

- **The elaborator is untrusted**: nothing it emits is admitted without the full
  `braid-verify` pipeline; no shared serialization code with the verifier (D9).
- **Closed-alphabet totality**: source constructs with no vocabulary mapping are a
  typed elaboration ERROR (with the D24 machine-readable reject→re-author shape),
  never a "best effort" lowering. No `eval`, no string-to-code, no freeform URL —
  T1 lives or dies here.
- **Determinism**: same source + same vocabulary version ⇒ same capsule bytes ⇒ same
  CID. The elaborator pins `vocab_version`/`registry_cid` from the bound registry,
  never hand-typed (the D19 rule carries over).
- **Grants are derived** from terms used, never author-declared-wider (restraint,
  D25; the T12 widening gate stays meaningful).
- **Error-edge completeness** (D24): the lowering must discharge source-level error
  paths into typed error edges, or refuse — happy-path-only elaboration is the slop
  vector.

## Execution steps

1. File the fork issue (A vs B); Director picks; D-entry appended.
2. Define the **source subset spec** (versioned doc in `spec/braid/elaboration/`):
   exactly which constructs are legal, their term mappings, and the refusal catalogue
   for everything else. Small: v0 can be ~10 constructs (component, props of core
   types, list render, conditional, event→capability-dispatch, projection read).
3. New crate `braid-elab-web` (outside the substrate boundary allowlist — it is a
   consumer, like the vocab crates): parse (use an existing vendorable parser per
   the zero-new-deps rule — get Director approval for the dep, or write a minimal
   recursive-descent for the subset), lower to SDK Builder calls, emit JSON-of-IR
   AND direct capsule bytes (both must produce identical CIDs — parity test).
4. Golden corpus: ≥10 source files → pinned capsule CIDs + manifests; plus a
   refusal corpus: ≥10 illegal sources → pinned typed errors (the anti-T1 suite:
   eval attempt, DOM-string emission, unknown import, float literal, unbounded
   loop without cost annotation).
5. The **reject→re-author loop demo** (D24's grounding mechanism): a script that
   feeds an LLM lane (via the blackbox2 executor playbook) a task, lets it author
   source, elaborate, admit-or-reject, and re-author from the typed reason —
   measure rounds-to-admission. This is both the product demo and D30's data.
6. Port the three demo-port actions from JSON-of-IR to the source subset —
   proving the elaborator covers the reference workflow end-to-end.
7. Adversarial pass focused on T1/T2 (smuggle attempts through the parser; parse
   differentials between the two emission paths).

## Verification

```bash
cargo test -p braid-elab-web          # golden CIDs + refusal corpus + parity
cargo test --workspace                # substrate untouched: boundary test green
# mutation: allow an unmapped construct through ⇒ refusal-corpus test RED
# demo: elaborated demo-port capsules admit with CIDs pinned; widening gate still
#       fires when a source change adds a capability (end-to-end T12 through elab)
```

## Exit criteria

A real source language compiles to admitted, executable (PB-02) capsules; the
refusal catalogue is total over the non-subset; DEBT_REGISTER D-ELAB updated;
"renders JS useless" can be stated as: *this subset authors against the verified
substrate and the runtime never sees JS* — the deflated, honest, operational claim.

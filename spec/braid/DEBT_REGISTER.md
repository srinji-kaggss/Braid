# Braid — Debt Register (2026-06-23; updated 2026-06-30 for U11–U13)

> The gap between the current state and the Java-ecosystem end state (PRD §1:
> "Java/WASM-scale — a self-sufficient runtime ecosystem"). Honest, on record.
> Each debt cites the locked invariant it respects and the unit that closes it.

## Verified-substrate debts (the part that's done)

- ✅ **IR + canonical encoding + CID** (U1, D8) — done.
- ✅ **Deterministic verifier** (U3–U5, D9) — 8-stage fail-closed, independent
  decoder, registry-parametric.
- ✅ **Manifest + widening gate** (U2/U6, D12) — CID-bound, mechanical diff.
- ✅ **SDK + CLI** (U6/U10, D5/D19) — the human-reconstructable loop.
- ✅ **U9 adversarial pass** — 4 findings closed (T3, T4, T12, R3); verdict
  at `spec/braid/U9-VERDICT.md`.
- ✅ **D31 global-IR refactor** — substrate/vocabulary split, string-tagged
  capabilities, `TypeTag::Opaque`, 2 vocabularies (CMS + JS proof-of-concept).
- ✅ **Publishability** — `braid-v0.1` git tag cut; `braid-capability`
  crates.io-ready; the rest `cargo add --git`.
- ✅ **Tier-2 safety-assurance CI** (U-SA, D32) — `braid.profile.json` binds
  Keel's 20 atoms (gate `NotSlop`); `keel/` vendored; the gate runs in CI.
- ✅ **First real language frontend** (U11, D31) — `braid-elaborate-js`
  compiles a JS expression subset (literals + `+`) into admitted capsules via
  the one verifier; "renders JS useless" is operational for that subset.
- ✅ **JS expression language + vocab v2** (U12, D-VOCAB.1) — operator-
  precedence frontend (`+ - * < == && || !`, booleans) over 16 pure JS terms,
  with a mutation-proven anti-escape-hatch guard and re-pinned CIDs.
- ✅ **Multi-capsule project build** (U13, D-TOOLCHAIN.1) — `braid-project`:
  manifest → elaborate + admit all (fail-closed) → deterministic project CID;
  cross-capsule anti-dredging guards (no rewrite/aggregation, no shadowing).
- ✅ **Frontier Flow P0 authority and wire contract** (ADR-099,
  D-FLOW.1–D-FLOW.9) — Braid/Forge/experience-as-code authority split,
  RON-first authoring with JSON interoperability, v0 graph/wire decisions,
  crate ownership, and the dependency-ordered P1–P6 issue DAG are ratified.

## Open debts (the Java-ecosystem gap)

### D-FLOW — Inter-capsule Flow is ratified but not implemented

P0 is complete; the implementation is intentionally empty until the following
dependency DAG lands:

1. [Braid #57](https://github.com/srinji-kaggss/Braid/issues/57) — canonical
   Flow AST/IR, encoding, bounds, domains, and KATs.
2. [Braid #60](https://github.com/srinji-kaggss/Braid/issues/60) — independent
   admission and the invariant/falsification matrix.
3. [Braid #59](https://github.com/srinji-kaggss/Braid/issues/59) — deterministic
   snapshot-bound satiation and frontier planning.
4. [Braid #58](https://github.com/srinji-kaggss/Braid/issues/58) — Rust/RON SDK,
   normalized JSON interoperability, full graph rendering, and CI import.
5. [forge-harness #123](https://github.com/srinji-kaggss/forge-harness/issues/123)
   — durable instances and execution.
6. [experience-as-code #66](https://github.com/srinji-kaggss/experience-as-code/issues/66)
   — domain compilation/reconciliation and fair performance proof.

The edges are P1 -> P2 -> P3; P3 forks to the sibling P4 and P5 workstreams,
and P6 depends on both. Forge P5 does not depend on the authoring SDK in P4.

The unresolved technical debts are explicit: bounded proof completeness for
choice predicates; canonical materialization bindings when satiation supplies
demanded outputs; safe secret-version cache identity; commutativity/resource
proofs before parallel frontier execution; a bounded patch protocol before
runtime expansion; and fixture/host hashes before performance thresholds become
normative. None may be guessed by an implementation phase.

### D-FLOW-REGISTRATION — ADR-099 is not yet in the constellation index

ADR numbers are constellation-wide. A filesystem scan found ADR-096 through
ADR-098 already allocated even though the canonical governance `ARCHIVE.md`
index stops at ADR-095, so this record correctly uses ADR-099. The
logic-os-kernel-owned governance map and archive must add ADR-096 through
ADR-099 in an authorized cross-repo governance change. Braid records the debt
but does not mutate another repository's authority from this P0 change.

### D-RUN — No runtime / no VM (the single biggest gap)
A verified capsule does not *run*. U7 (WASM codegen + runtime admission) is
blocked on the kernel Day-0 WASM runtime that does not exist yet. This is a JVM
with no V. A Java-ecosystem substrate without execution is a spec, not a
product.
**Closes**: U7 (blocked); PRD §1 "self-sufficient runtime ecosystem."

### D-ELAB — No real language frontend / elaborator *(first slice LANDED — U11)*
🟡 **Partially closed.** `braid-elaborate-js` (U11) is the first real frontend:
it lexes/parses a JS *expression* subset (string/number literals + `+`,
parenthesizable) and **compiles JS text into an admitted capsule** via the one
`braid-verify` — the "renders JS useless" claim (D31) is now *operational* for
that subset, not just architectural. Remaining gap: it is an *expression* slice
(no identifiers/statements/calls; valueless `js.lit.*`), there is no Java→Braid,
and no D6-gated surface syntax for the human authoring direction. A Java
ecosystem still needs a full `javac`-class frontend.
**Closes**: U11 (the expression slice — done); U12 (JS at scale + literal
payloads); D6-gated surface syntax (later); eventually a real multi-language
frontend.

### D-CONSUMER — Zero real consumers
The browser engine has a parallel `BraidTerm` enum (steer note delivered at
`next-gen-browser-engine/docs/BRAID_STEER.md`, not acted on). The kernel has a
pinned snapshot (`braid_vocab_binding.rs`) not live-wired. "Become a
dependency" is now *possible* (tag cut) but has *zero actual dependents*.
**Closes**: the browser collapse; the kernel binding live-wire (#565).

### D-VOCAB — Vocabulary libraries are seeds, not a stdlib *(first slice LANDED — U12)*
🟡 **Partially closed.** `braid-vocab-js` grew v1→**v2** (8→16 terms: the
pure-operator expansion — arithmetic, comparison, boolean logic) and now backs a
real operator-precedence expression language in `braid-elaborate-js`. A
**vocabulary-extension governance flow** exists (module doc + the
`expansion_added_no_escape_hatch` / pinned-CID guards mechanically enforcing
bump-and-re-pin + pure-by-default). Remaining: still far from a stdlib (no
statements/identifiers, no string library, no `cms`-scale breadth); no package
registry (PRD §49 non-goal); **literal payloads remain deferred to a substrate
unit** (Strand carries no operand — D8-locked work, not a vocab change).
**Closes**: U12 (expansion + governance — done); a stdlib-scale JS vocabulary
(later); the substrate-level literal-payload unit.

### D-TOOLCHAIN — Partial toolchain *(first slice LANDED — U13)*
🟡 **Partially closed.** `braid-cli` (encode/decode/verify/render/diff) plus the
new `braid-project` crate: a multi-capsule project manifest + `braid-project
build` that elaborates + admits every capsule fail-closed and emits a
deterministic project CID. Remaining: no package manager/registry (PRD §49
non-goal), no docs generator, no `braid test` harness for capsules yet, and the
manifest is JS-expression-only (no intents/anchors).
**Closes**: U13 (project build — done); PRD §5 P4+ (the rest).

### D-SA — Tier-2 safety-assurance floor *(BUILT — U-SA landed)*
✅ **Closed.** The "stop slop" semantic floor is no longer just a spec: `U-SA`
landed (commit `6f74c82` + the Keel-vendoring `ae83171`). `braid.profile.json`
binds Keel's 20 evidence atoms to Braid's Tier-1 tools (gate concept `NotSlop`),
`keel/` is vendored (schema + engine), and `.github/workflows/ci.yml` runs the
Keel gate in CI. The design rationale stays in `SAFETY_ASSURANCE_CI_SPEC.md`.
**Remaining**: the Lean⇄verifier conformance check (tracked separately as
D-SEMANTICS / U15), not the Keel wiring itself.

### D-SEMANTICS — The verifier's stage semantics are not machine-checked against the Lean predicates
D22 says Lean is the unforked proof oracle; the Rust verifier implements
proven-sound rules. The Lean skeleton (`excellent_not_hallucinated`) is
axiom-free, but the *mapping* between `braid-verify`'s 8 stages and the Lean
predicates is not machine-checked (D-SA5 in the spec).
**Closes**: a conformance check (part of `U-SA`).

### D-CONFIRM — INTERPRETED decisions awaiting Director confirm/veto
- **D5** (day-0 = IR, rust day-1, compiler owns compliance) — INTERPRETED since
  2026-06-12.
- **D16** (v0 = frontend component, landing-surface port) — INTERPRETED.
- **D31** (global translator IR) — INTERPRETED this session.
- **D32** (adopt Keel) — INTERPRETED this session, awaiting ratification.
**Closes**: Director review on the foundations PR.

## What "done for v0" would mean (the honest bar)

The PRD's v0 goals (G1–G6) are met. The PRD's *end-state ambition* (Java/WASM-
scale) is not v0 and was never claimed to be. The honest claim **as of the
post-v0 frontier (U11–U13)**: **a verified IR substrate with a built
safety-assurance floor, a real JS expression frontend (U11–U12) compiling text
into admitted capsules, a first multi-capsule build tool (U13), and still zero
*external* consumers.** That is a little past where Java was in 1991 — the
`.class` + verifier layer now has a working front-end compiler and a project
builder, but still no VM, no stdlib at scale, and no outside dependents.

## Priority order (my recommendation)

1. ~~**`U-SA`** — safety-assurance floor.~~ ✅ **Done.**
2. ~~**A real JS→Braid elaborator.**~~ ✅ **Done (U11–U12).** "Renders JS
   useless" is operational for the expression subset.
3. **One real consumer live-wired (U14)** — the browser collapse or the kernel
   binding. Zero external dependents = not yet a dependency. **Now the top
   open lever.**
4. **U15 — Lean⇄verifier conformance** — closes D-SEMANTICS; the last piece of
   the Tier-2 floor's rigour.
5. **U7 / a runtime** — the biggest gap but blocked on the kernel WASM epic.
   Either unblock that or build a minimal Braid-direct interpreter.

# Braid — Debt Register (2026-06-23)

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

## Open debts (the Java-ecosystem gap)

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

### D-TOOLCHAIN — Partial toolchain
`braid-cli` (encode/decode/verify/render/diff) exists. No package manager, no
build tool for multi-capsule projects, no docs generator, no `braid test`
harness for capsules (vs. tests *of* the substrate).
**Closes**: PRD §5 P4+.

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
scale) is not v0 and was never claimed to be. The honest v0 claim is:
**a verified IR substrate with two vocabulary seeds, a safety-assurance floor
(specified), and zero consumers.** That is roughly where Java was in 1991
before the JVM ran anything — the `.class` + verifier layer, no VM, no stdlib,
no users.

## Priority order (my recommendation)

1. **`U-SA`** — build the safety-assurance floor (this spec). Unblocks
   trustworthy work on everything else; a Java-ecosystem substrate without it
   ships slop. Low effort (wiring, not reinvention) once D32 is ratified.
2. **A real JS→Braid elaborator** — turns "renders JS useless" from
   architectural to operational. The proof of the whole global-IR thesis.
3. **One real consumer live-wired** — the browser collapse or the kernel
   binding. Zero dependents = not yet a dependency.
4. **U7 / a runtime** — the biggest gap but blocked on the kernel WASM epic.
   Either unblock that or build a minimal Braid-direct interpreter.
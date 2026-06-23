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

## Open debts (the Java-ecosystem gap)

### D-RUN — No runtime / no VM (the single biggest gap)
A verified capsule does not *run*. U7 (WASM codegen + runtime admission) is
blocked on the kernel Day-0 WASM runtime that does not exist yet. This is a JVM
with no V. A Java-ecosystem substrate without execution is a spec, not a
product.
**Closes**: U7 (blocked); PRD §1 "self-sufficient runtime ecosystem."

### D-ELAB — No real language frontend / elaborator
No JS→Braid parser exists. No Java→Braid. No surface syntax (D6-gated). The
"renders JS useless" claim is *architectural* (the IR shape can accept any
language — D31), not *operational* (no tool actually compiles a language to it).
A Java ecosystem needs a `javac`.
**Closes**: a real JS→Braid elaborator (proposed next unit); D6-gated surface
syntax for the human authoring direction.

### D-CONSUMER — Zero real consumers
The browser engine has a parallel `BraidTerm` enum (steer note delivered at
`next-gen-browser-engine/docs/BRAID_STEER.md`, not acted on). The kernel has a
pinned snapshot (`braid_vocab_binding.rs`) not live-wired. "Become a
dependency" is now *possible* (tag cut) but has *zero actual dependents*.
**Closes**: the browser collapse; the kernel binding live-wire (#565).

### D-VOCAB — Vocabulary libraries are seeds, not a stdlib
2 vocabs: `braid-vocab-cms` (12 terms, demo), `braid-vocab-js` (8 terms,
proof-of-concept). Java's stdlib is thousands of packages. No package registry
(D-non-goal, PRD §49). No governance flow for vocabulary extension (PRD §5 P5).
**Closes**: a real JS vocabulary at scale; a vocabulary governance flow.

### D-TOOLCHAIN — Partial toolchain
`braid-cli` (encode/decode/verify/render/diff) exists. No package manager, no
build tool for multi-capsule projects, no docs generator, no `braid test`
harness for capsules (vs. tests *of* the substrate).
**Closes**: PRD §5 P4+.

### D-SA — No Tier-2 safety-assurance floor (the spec this session produced)
Braid's CI is Tier-1 only (fmt/clippy/test). The "stop slop" semantic floor
(Keel + Excellent Code Framework) is *specified* (`SAFETY_ASSURANCE_CI_SPEC.md`)
but not built. Without it, a PR can compile, pass tests, and still be slop.
**Closes**: `U-SA` (the unit filed from the spec).

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
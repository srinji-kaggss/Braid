# ADR-088: Braid — Machine-First Framework Foundations (doctrine + locked invariants)

**Status**: PROPOSED — drafted from Director decisions given live in the 2026-06-12 session; ratifies on Director merge of the foundations PR
**Date**: 2026-06-12
**Issues**: foundations epic (filed with this ADR); relates #555 (§16 trigger status), #556 (cross-repo single-sourcing), #554 (graph hygiene)
**Authors**: Director (decisions) + Claude (synthesis), 2026-06-12 session
**Builds on**: ADR-066 (four-layer substrate / closed vocabulary), ADR-068 (Causal Tape), ADR-069 (capability-gated guest apps, "next JavaScript" open surface), ADR-070 (sequencing — Day-0 CMS demo), ADR-071 (AI as bounded grantable principal), ADR-073 (capability-port), `LOGIC_OS_AI_FIRST_LANGUAGE_STATE_FABRIC_SECURITY_RESEARCH_BASELINE.md` (§8 Axiom doctrine — prose input only), `laws/governance/logic-os-vision-and-strategy.md` §16
**Spec**: `spec/braid/` (PRD, decision register, threat model, unit plan — the extraction-ready home)

---

## Extraction addendum (2026-06-13) — D5 covenant executed

The D5 covenant ("I will move it to its own repo after") is now done: Braid was
extracted from `logic-os-kernel` (PR #564, branch
`claude/machine-first-language-foundations-xv81mq`) into its **own repository,
`srinji-kaggss/Braid`** (private). What changed in the move — and ONLY this:

- **Layout flattened.** Kernel-workspace crates `kernel/crates/braid-*` became
  top-level `crates/braid-*`; `spec/braid/` is preserved verbatim; this ADR now
  lives at `docs/adr-088-…md`. A standalone root `Cargo.toml` workspace replaces
  the kernel one (dep versions mirror the kernel pins so the two stay
  byte-compatible — CID/KAT vectors are unchanged and still green).
- **D3 boundary satisfied by vendoring, not a path dep.** The only kernel type
  that crossed the boundary was `canvas_protocol::Capability`. It is now
  **vendored verbatim** as the `braid-capability` crate
  (`crates/braid-capability`). SOURCE OF TRUTH remains the kernel's
  `canvas-protocol::Capability` on `origin/main`; the vendored copy is a faithful
  mirror (variants + `serde`/`strum` attributes byte-identical, so the
  Display-string that feeds capsule CIDs is preserved). Re-sync = a trivial diff
  against `origin/main`. `boundary_conformance.rs` was updated to pin
  `braid-capability` instead of `canvas-protocol`.
- **D11 vocabulary-binding moved to the consumer side.** `vocab_binding.rs`
  bound the Braid registry to the live kernel vocabulary (`canvas-syscall`
  `VOCABULARY_VERSION` / `action_spec`). By D11's own logic ("never a parallel
  authority"), vendoring the kernel vocabulary into a standalone Braid would
  create exactly the parallel authority it forbids — so that test does NOT belong
  in this repo. It is dropped here and **re-homed as a kernel-side integration
  test** (the kernel, as the consumer, asserts that the Braid registry it pins
  matches its own vocabulary). Tracked as a follow-up against the kernel.

Everything below this addendum is the original ratified ADR text, unchanged.

---

## Doctrine addendum (2026-06-15) — Reddit critique, Lean, DiD/anti-malware, the judgment problems

Director session 2026-06-15, working from the r/ClaudeAI "AI-only language" thread +
its 166-comment expert review (incl. Lucian Wischik, C# async/await), Lean 4 as a
reference architecture, and Director directives on defense-in-depth, real-world
grounding, and "soulless code". Full provenance + lock status in
`spec/braid/DECISIONS.md` D20–D27. The headline shifts (INTERPRETED — veto on review;
none unlock D6 grammar work):

- **"Machine-first" is reframed (D20).** The expert consensus is that there is no special
  machine representation of code and that stripping human-affordances makes the model
  *dumber* — the genre's founding move is wrong. The surviving half is compiler-enforced
  correctness. Braid's true thesis: **predictable-surface + confinement + amortized human
  judgment.** The canonical IR is a *verification + anchor* form; the authoring surface
  must be *familiar, redundant, shallow per-unit, and constrained to the closed
  vocabulary*. **Humans and AI share an anchor (the verified core), never a
  representation** — generalizing D17.
- **Lean is the reference architecture, not a fork (D21/D22).** Trusted-core +
  extensible-elaboration is Lean's proven shape and is Braid's. Custom syntax helps *only*
  at the elaboration layer; design the **elaboration seam** (surface → core IR →
  independent re-check) as a basement interface now. Do **not** fork Lean (wrong runtime,
  wrong TCB, D9 violation, LLM-hostile surface); use Lean *unforked* as an off-line proof
  oracle to prove the verifier RULES sound once (seL4/CompCert verified-checker pattern).
- **Anti-malware = confinement, not detection (D23).** Detection is undecidable
  (Rice/Cohen) and the binary erases the distinguishing information; a closed typed
  vocabulary makes dangerous capability *unrepresentable*, turning classes of malice into
  confinement theorems in a six-layer DiD stack (capability / non-interference /
  effect-typing / totality-bounds / confirm / manifest-recheck). The irreducible residue
  (true intent) escalates to the human — never claimed as decided.
- **Grounding & anti-happy-path (D24).** Error-edge completeness is a fail-closed admission
  obligation (happy-path-only becomes unrepresentable); the registry is real-world ground
  truth with metadata *extracted from the system graph, not idealized*; bounds compose to
  force resource-reality. The verifier reminds the model; the reject→re-author loop is the
  grounding mechanism.
- **Anti-soulless / Sr-SWE judgment (D25).** Intent becomes a first-class typed term;
  admission scores intent-coherence, grain-conformance, and restraint. Senior judgment is
  compiled once into the vocabulary/ontology/coherence-rules and amortized to every
  junior/AI author (silent for seniors when satisfied); residue → human.
- **Research directions (D26/D27, not locked):** a multi-layer ("3D") co-registered
  projection IR over a shared content-addressed anchor (reusing lgwks JEPA multi-view); and
  a "good-code" judgment benchmark (intent-coherence, edit-locality, round-trippability,
  grain, restraint) as a third rung beyond correctness/safety in PRD §8.
- **Human-as-system-designer forcing function (D28).** Architecture is a first-class typed
  node (an ADR-as-runtime-artifact) that is **AI-un-mintable** (AI proposes, only a human
  ratifies), a **required anchor** for every capsule, and **decision-complete** as a
  fail-closed admission obligation (sibling of D24: "un-decided architecture is
  unrepresentable" — the AI must surface a fork, never default it). The chart is coarse and
  amortized, so it stays high-leverage, not human-in-the-loop-constantly.
- **The up→down / down→up sandwich (D29).** The AI authors the middle band, forced to align
  UP to the human-ratified architecture and DOWN to the verifier floor, owning neither end.
  Reference for the top-down half: **Power Apps vibe** (Plan-first; plan/data/app
  co-registered layers ≈ D26; aligns the AI to a systems-design framework) — Braid goes
  further by forcing alignment to a *human-authored* per-system architecture, not a vendor's.
- **Adversarial-loop deflation (D30, finding).** "Autonomous good code" survives only
  deflated: Braid autonomously raises the **floor** (structural correctness + safety), not
  the ceiling. v0 only binary-gates structure+safety; altitude/scope/grain are advisory or
  research, not blocking. Intent-**coherence** ≠ intent-**correctness**. Judgment splits 3
  ways (structural-blocking / advisory-audit-only / irreducible-human). Decisive open test: a
  live A/B on a real port — does gate-admission predict blind senior-approval? — stratified
  by domain. Doc debt: state the advisory-vs-blocking line; correct "intent-grounded" →
  "intent-coherent".

---

## Context

The kernel has, without naming it, already built the semantic core of a
machine-first language: a closed versioned Capability Vocabulary with all three
ADR-066 L2 legs machine-enforced (`canvas-syscall/src/vocabulary.rs`,
`VOCABULARY_VERSION = 1`), a three-syscall surface where actions are typed enum
values never strings, content-addressed sealed facts with domain-separated
BLAKE3 hashing and KAT pins (`state-fabric`), ed25519 `IntentToken` authorship,
attenuating-only capability ports (ADR-073), per-hop + path-level cross-trust
rules with monotone exposure taint (#361/#431), and a single audited egress
door. What does NOT exist is the layer that lets an AI *author programs* against
those primitives and lets a human *audit* them: an intermediate representation,
a compiler/verifier pipeline that owns compliance, a manifest that is the human
review object, and a runtime admission contract.

The strategy doc (§16) deferred "the language" behind four triggers; trigger 1
(A7 vocabulary) has since half-fired (implemented via #461/PR #480; not yet
proven across ≥2 workflows). The research baseline (§8) proposed the shape under
the working name "Axiom" — a name that now collides with an unrelated `axiom/`
daemon package in `logicalworks-` (see #556). The Director resolved the open
forks in-session on 2026-06-12 (verbatim provenance in
`spec/braid/DECISIONS.md`).

## Decision

### D1 — The framework exists, named **Braid** (working name), and it is an IR + verifier, not a grammar
Braid is Logic OS's machine-first application framework: programs are
**content-addressed graphs of typed terms** ("strands" composed into "braids")
drawn from the closed kernel vocabulary, authored as *data* (no textual surface
syntax in v0), admitted by a **deterministic compiler/verifier pipeline that
owns compliance** — capability, effect, taint, bounds, budget — so that neither
an AI nor a tired human is the enforcement mechanism. The name is REVISABLE;
the spec identity is the repo path `spec/braid/` + decision register, not the
word. "Custom language replaces all languages" is ratified as **blue-sky
end-state, explicitly out of v0–v2 scope** (consistent with research baseline
§22.7 and §16's engine-first stance).

### D2 — Day-0 the language IS the IR; Rust is the day-1 SDK; syntax is later and separately gated
The day-0 authoring form is the typed IR itself — "everything is a little
api-type math the AI can chain" (Director). A Rust SDK for constructing IR is
the day-1 host path. A human-writable textual syntax is a later phase, gated on
§16 triggers 2/3 (repetition pain / second surface), never part of the
foundations critical path. Day-1 human interaction is **audit-first**: humans
review the deterministically rendered manifest; the review object is the
manifest diff, never raw IR bytes.

### D3 — Compiler-as-compliance: the locked stage order
Every artifact passes, fail-closed, in order:

```text
term graph (IR)
→ canonical-form check (bijective decode; re-encode == input bytes)
→ vocabulary membership + version pin (closed registry; unknown ⇒ deny)
→ type check (term signatures)
→ capability check (request ⊆ grant; attenuation-only, ADR-073 lattice)
→ effect calculus (composition postures; irreversibility; confirm policy)
→ data-flow / taint check (monotone exposure fold, path-level not per-hop)
→ resource & numeric bounds (declared budgets; fixed-point only, no IEEE floats)
→ proof obligations (crash/replay/revocation hard cases discharged)
→ manifest emission (bound to artifact CID)
→ deterministic codegen (WASM component first target)
→ signing
→ runtime admission (independent re-verification at load)
→ capability-gated execution via the three-syscall surface
→ journal + audit evidence
```

Stage *order* and fail-closed posture are LOCKED; stage internals are
revisable. The verifier MUST be implementation-independent of any generator
(anti-trusting-trust; see threat model T2).

### D4 — Braid adds NO authority and rides the kernel
Braid is a projection/compiler over existing kernel primitives. Execution
reaches the world only through the sealed three-syscall surface
(`Capability`/`Projection`/`Sync`); hashing reuses the kernel's
domain-separated BLAKE3 discipline (new domains under `lw.braid.*`, KAT-pinned
from day 0); capability semantics are the kernel's, not a parallel system. A
Braid artifact can never do what a hand-written guest could not.

### D5 — Home and extraction covenant
Braid lives in `logic-os-kernel` now (Director: "put it wherever you have
access to; I will move it to its own repo after"): spec under `spec/braid/`,
code as kernel-workspace crates (`braid-ir`, `braid-verify`, `braid-render`,
`braid-cli` as units land). The **boundary is locked**: Braid crates may depend
only on the declared kernel contracts (canvas-protocol `Capability`,
`VOCABULARY_VERSION`, state-fabric hash/envelope types, the syscall API) — no
reach into kernel internals — so extraction to its own repo stays mechanical.

### D6 — Reference workflow = the Day-0 CMS demo
v0 acceptance is expressing the `blueprints/afternow-port/` CMS reference
actions as Braid capsules end-to-end (IR → verifier → manifest → admitted
execution with evidence), aligning the framework's proof with the
already-ratified ADR-070 Day-0 sequence. Abstract acceptance criteria alone are
insufficient (anti-slop).

### D7 — Fresh start: the `logicalworks-` `axiom/` code is NOT an input
Director: the lgwks Python work is a daemon; "best to avoid any blur and just
start fresh." The research baseline's *prose doctrine* is an input; no bytes,
encodings, or CID semantics are inherited from the `axiom/` package. This
resolves the #556 fork direction: kernel-side fresh design is canonical.

## Consequences

- §16's trigger-status line is corrected in the same PR (closes the
  documentation half of #555); the doctrine half of #555 is satisfied by this
  ADR. Building the IR-as-data + verifier does NOT violate §16's deferral —
  what §16 defers (grammar/syntax/editor) stays deferred.
- The Braid workstream runs **parallel to, never ahead of**, the A-series
  trust-boundary queue (#424 next); it consumes no override row because it is
  not an A–F node. GitHub Issues remain the work tracker; the unit plan in
  `spec/braid/units.md` is issue-ready with acceptance criteria.
- Locked vs revisable is explicit: see the lock legend and register in
  `spec/braid/DECISIONS.md`. LOCKED items change only by Director-approved
  amendment to this ADR.
- Risk acceptance: building on `VOCABULARY_VERSION = 1` before it has carried
  ≥2 real workflows accepts vocabulary-churn risk; mitigated by version-pinning
  every capsule and the migration-proof rule (threat model T6).

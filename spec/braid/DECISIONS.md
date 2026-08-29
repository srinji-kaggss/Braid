# Braid — Decision Register

**Provenance**: Director decisions given live in the 2026-06-12 remote session
(machine-first-language foundations). Verbatim quotes are preserved where the
decision required interpretation, with the interpretation stated explicitly so
the Director can veto it on review. Register is append-only; decisions are
amended by new entries, never edited.

## Lock legend

| Status | Meaning | Change process |
|---|---|---|
| **LOCKED** | Day-1-or-never for the framework. Building against it is safe. | Director-approved amendment to ADR-088 + new register entry. |
| **REVISABLE** | Settled enough to build on, expected to evolve. | Normal issue + review; update register. |
| **INTERPRETED** | Derived from an ambiguous Director statement; verbatim quote attached. | Director confirm/veto on the foundations PR converts to LOCKED or reopens. |

---

## The register

### D1 — Framework exists; identity = IR + verifier + manifest + runtime contract — **LOCKED**
Braid is a machine-first application framework: typed term-graph IR,
deterministic compiler/verifier pipeline owning compliance, manifest as the
human review object, capability-bounded runtime contract over the kernel.
*Rationale*: "easy for AI, auditable for the human" with enforcement moved
below both the model and the reviewer.
*Rejected*: framework-as-conventions (style guides + review) — that is the
technical-debt human as compliance engine, explicitly what the Director ruled
out.

### D2 — Name: fresh codename **Braid** — name REVISABLE, fresh-name requirement LOCKED
Director chose "fresh codename" to avoid blur with the unrelated lgwks
`axiom/` daemon. "Braid" selected: zero collisions in-repo (verified
2026-06-12); semantically apt (typed strands composed into tamper-evident
braids over the causal DAG). Spec identity = `spec/braid/` path + this
register, not the word.
*Rejected*: keeping "Axiom" (research-baseline continuity) — permanent name
collision with the daemon package, the exact confusion flagged.

### D3 — Home: `logic-os-kernel` now; extraction-ready boundary — boundary LOCKED, location REVISABLE
> Director, verbatim: "Put it wherever you have access to. I will move it to
> its own repo after. I am envisioning a complete end to end framework like
> Java or wasm, customized for our env and easy for AI use."

Spec under `spec/braid/`; code as kernel-workspace crates (`braid-ir`,
`braid-verify`, `braid-render`, `braid-cli`). Locked boundary: Braid crates
depend ONLY on declared kernel contracts — canvas-protocol `Capability`,
`VOCABULARY_VERSION`, state-fabric hash/envelope types, the syscall API —
never kernel internals. A conformance test pins this (unit U1).
> _Extraction note (2026-06-13):_ on the move to `srinji-kaggss/Braid`, the one
> crossing type `Capability` is now **vendored** as the `braid-capability` crate
> (verbatim mirror of `canvas-protocol::Capability` on the kernel `origin/main`),
> and the `VOCABULARY_VERSION` binding moved to a kernel-side integration test.
> See the ADR-088 extraction addendum.

### D4 — Fresh start; lgwks `axiom/` code is NOT an input — **LOCKED**
> Director, verbatim: "Lgwks python work is a daemon. This is completely
> different. If I'm misinterpreting you, then best to avoid any blur and just
> start fresh so we are not confused."

No bytes, encodings, CID semantics, or APIs inherited from the `axiom/`
package. The research baseline
(`LOGIC_OS_AI_FIRST_LANGUAGE_STATE_FABRIC_SECURITY_RESEARCH_BASELINE.md`) is
an input as *prose doctrine only*. Resolves #556's fork direction:
kernel-side fresh design is canonical.

### D5 — Shape: day-0 language IS the IR; Rust SDK day-1; compiler owns compliance — **INTERPRETED**
> Director, verbatim: "Our lang Day 0 and rust day 1?? When I say Java like I
> also mean how there's Java engine and libs and stuff, it's our future
> framework built ground up for the AI, imagine if everything was basically a
> little api type math the ai can chain? Or something of that nature. How do
> you create robustness and Defense in Depth so our compiler is doing the
> compliance n oversight instead of ai and technical debt human probably.
> Think like that."

**Interpretation adopted** (veto on PR review if wrong):
- "Our lang Day 0" = the typed IR authored as data is the day-0 language —
  the "little api type math the AI can chain" is the strand/braid term
  algebra. No textual grammar in v0.
- "rust day 1" = a Rust SDK for constructing/consuming IR is the first host
  path; Rust is also the implementation language.
- "Java… engine and libs and stuff" = the end-to-end ambition (VM-analog =
  verifier+runtime contract; libs = strand vocabulary packages; toolchain =
  braid-cli) — captured as the PRD's phased arc, not all v0.
- "compiler is doing the compliance n oversight" = ADR-088 D3's locked
  fail-closed stage order; neither AI nor reviewer is the enforcement
  mechanism.

### D6 — Surface syntax deferred and separately gated — **LOCKED**
Textual human-writable syntax (and any editor) gates on strategy-doc §16
triggers 2/3, never on the foundations critical path. Day-1 human surface =
rendered manifest (audit), per Director's explicit option choice.
"Custom lang replaces all languages currently is blue sky" (Director, task
brief) — ratified as a non-goal for v0–v2.

**2026-08-29 amendment:** D33 records the Director's explicit decision to fire
this gate for the bounded native Capsule DSL v0 only. The general grammar,
editor, schema/state, Flow, macro, and language-replacement work remains gated.

### D7 — Reference workflow: Day-0 CMS demo — **LOCKED**
v0 acceptance = `blueprints/afternow-port/` CMS reference actions expressed
as Braid capsules end-to-end. Director chose this over the research baseline's
meeting-follow-up slice. Aligns framework proof with ADR-070's Day-0 sequence.

### D8 — Hash + encoding discipline — **LOCKED**
- Content addressing reuses the kernel's BLAKE3 domain-separated preimage
  discipline; Braid domains namespaced `lw.braid.*`; KAT vectors land with the
  first byte ever encoded (no encoding exists without a pinned known answer).
- Canonical encoding: deterministic CBOR subset (canonical map ordering,
  definite lengths, no floats), with a **bijection guard**: decode must
  re-encode to the identical bytes or reject. (Lesson source: the A4.8
  governance-ledger byte-malleability finding.)
- **No IEEE floats anywhere in the IR** — fixed-point integers only
  (precedent: `intent_projection.rs` u64 fixed-point). Floats are
  determinism/canonicalization poison.

### D9 — Verifier independence (anti-trusting-trust) — **LOCKED**
The verifier shares no code-generation or serialization-normalization path
with any authoring SDK or generator. The admission decision is reproducible
from capsule bytes alone by an independent implementation. (Threat T2.)

### D10 — Braid adds no authority — **LOCKED**
All execution reaches the world through the sealed three-syscall surface; all
capability semantics are the kernel's (`authorize`, ports, taint, egress
door). A Braid artifact can never exceed what a hand-written guest could do.

### D11 — Versioning covenant — **LOCKED** (mechanism REVISABLE)
Every capsule pins `VOCABULARY_VERSION` + Braid IR version. Admission refuses
version mismatch absent an explicit migration proof. Vocabulary/IR version
bumps are conscious, reviewed events (existing pin-test pattern). Risk
accepted (ADR-088 Consequences): building on `VOCABULARY_VERSION = 1` before
≥2 workflows.

### D12 — Manifest is the review object — **LOCKED**
Every artifact carries a deterministic manifest (declared intent, caps,
effects, bounds, confirm + evidence policy) **bound to the capsule CID**.
Human review = manifest diff. CI gates on capability-widening diffs
(research-baseline acceptance scenario 11). The runtime re-derives the
manifest from the admitted artifact and rejects on mismatch (threat T4).

### D13 — Workstream governance — **LOCKED**
GitHub Issues authoritative; no issue, no work; every unit defines
verification; evidence discipline per `docs/logic-os-build-state.md`. Braid
runs parallel to — never ahead of — the A-series trust-boundary queue (#424
next). Adversarial hacker pass (U9) is a blocking gate before any "v0 done"
claim.

### D14 — First codegen target: WASM component — REVISABLE
Matches ADR-069's app model and the Day-0 WASM trial. Native-binary and
policy-automaton targets are later phases. Revisable because the kernel WASM
runtime itself is still future work; U7 coordinates rather than duplicates.

---

## Amendments — 2026-06-12, round 2 (Director follow-up answers on the same session)

### D15 — Name stays PROVISIONAL; the "context-blur landmine" convention — process LOCKED, name open
> Director, verbatim: "Some whimsy? Some logicalworks? Something easy for AI?
> Can we be creative but if not, just mark it as assume codebase name is a
> context blur landmine warning or something until we finalize."

"Braid" is retained as the creative working name (whimsical, short, easy for
AI, zero in-repo collisions) but is explicitly **PROVISIONAL until the
Director finalizes**. Convention adopted: every Braid spec doc carries the
landmine warning (see README banner); agents MUST cite the `spec/braid/` path
+ this register as identity, never the word alone; any observed name
collision is reported, not silently resolved. (Amends D2.)

### D16 — v0 execution target: the frontend component; first full port = the landing surface — **INTERPRETED**
> Director, verbatim: "Just the frontend component. The logic-os-kernel has
> the landing page, which can be our first full port to see our language and
> failures. Rust as backend and then eventually it'll replace? What do you
> suggest?"

**Recommendation adopted (veto on PR review if wrong):**
- v0 admitted artifacts are **frontend components**: capsule logic compiles to
  a WASM component whose *render output is typed directives* —
  `ViewDirective` / `MotionDirective` / `WidgetDescriptor` from
  `primitives/canvas-protocol` — **never DOM/HTML/CSS strings** (CODEBOOK
  rule: machines drive the OS via typed terms, never DOM).
- The **reference workload sharpens to the landing-surface port**: this is the
  same artifact family as D7's Day-0 CMS demo (`blueprints/afternow-port/` IS
  the landing port blueprint), so D7 stands, sharpened: v0 = landing
  pages/sections expressed as capsules, run to "see our language and failures".
- **Frontend-first is also the safest first alphabet**: the v0 vocabulary is
  pure render terms + projection reads — zero irreversible effects, zero
  egress — so the verifier earns trust before any destructive capability is
  expressible. Effectful capsules escalate in later phases.
- **Rust remains the backend + implementation language** indefinitely;
  "eventually it'll replace" stays blue-sky (D6 non-goals unchanged).
(Amends D14; sharpens D7.)

### D17 — The IR is the dimension where AI and human meet; translation is bidirectional by design — **LOCKED**
> Director, verbatim: "Build the IR and the translation/graph stuff. Like the
> layer/intersection/dimension where AI and human meet through the IR. at
> some level, both will have to meet at the IR. Whatever's needed even if
> that means both"

The IR is the single meeting layer: AI authors it natively; humans meet it
through **translations**. v0 ships the IR→human direction: the manifest
(audit) AND a braid-graph rendering (the DAG itself, visualizable — the
"translation/graph stuff"; U2 scope now includes a machine-readable graph
export, e.g. DOT/JSON). The human→IR direction (form/projectional editing,
then text syntax) is licensed "whatever's needed even if that means both" —
built when needed, full syntax still gated per D6. (Enriches D6; does not
unlock grammar work.)

### D18 — Growth model: dogfood-inward, "builds and builds" — **LOCKED**
> Director, verbatim: "Day0 in my head was the graphics engine maybe?
> Something within our app and then it builds and builds??"

The framework grows by **porting our own app, surface by surface**, riding
the graphics/canvas-motion engine (whose `ARCHITECTURAL_MANDATES.md` already
states the machine-first thesis) — landing first (D16), then further
surfaces/features authored as capsules. External authoring stays out until
§16 trigger 4. Each port feeds vocabulary/IR refinements back before the
alphabet widens. (Sharpens D7's trajectory.)

### D19 — `braid encode` author input = JSON-of-IR over the SDK Builder — **LOCKED** (Director-confirmed on merge of PR #3, 2026-06-14)
> Director, live this session (2026-06-14), choosing among three framed
> options for what `braid encode` consumes: **"A"** — JSON-of-IR transcription.
> Confirmed by the Director's direction to merge PR #3 ("merge and run
> termination") — INTERPRETED→LOCKED per the lock legend's conversion process.

The U6 CLI's `encode` reads a **JSON that is a 1:1 data transcription of the
IR structures** (`intent`, optional `budget`/`confirm`/`evidence`, `strands:
[{term, inputs:[idx]}]`, `outputs:[idx]`) and routes it through the existing
`braid-sdk` `Builder` against the pinned `registry_v0`. Grants are *derived*
from the terms used, and `vocab_version`/`registry_cid`/`ir_version` are
filled from the bound registry — never hand-typed (no magic constant; the
`registry_cid` is recomputed from the pinned registry, satisfying the
calculator-reconstructable bar). The CLI path is therefore byte-identical to
the SDK path and reproduces the pinned reference CIDs (T13).

**The fence** (so this stays inside D17/D6, not past them): JSON-of-IR is a
*transport encoding of the IR data model* — the "author capsules as IR data"
path the PRD already licenses (PRD §3 actor row). It is **NOT** the textual
surface syntax / grammar / parser the PRD lists as a non-goal (PRD §non-goals)
and that D17 keeps gated. No sugar, no defaults beyond the SDK's, no semantics
the `Capsule`/registry don't already enforce. If a real surface grammar is
ever wanted, that remains a separate, D6-gated decision.
*Why A over the alternatives*: "bytes-only CLI" leaves scenario #12's "human,
no AI, **CLI only**: author" unmet (it would need Rust); "example emitter" is a
demo that can't author the new/edited capsules the T12 widening gate must
exercise. (Enriches D17; does not unlock grammar work.)

---

## Amendments — 2026-06-15 (Director session: Reddit "AI-only language" critique, Lean, DiD/anti-malware, the soul/judgment problems)

**Provenance**: live Director session 2026-06-15, working from (a) the r/ClaudeAI
"I told Claude to build a programming language for use only by AI" thread and its
166-comment expert review (incl. Lucian Wischik, C# async/await), (b) Lean 4 as a
reference proof-system architecture, and (c) Director directives on defense-in-depth,
real-world grounding, and "soulless code". Synthesis by Claude; INTERPRETED entries
are veto-on-review per the lock legend. These ENRICH the foundations; none unlock
grammar work (D6 still gates surface syntax).

### D20 — "Machine-first" is RENAMED in intent to "predictable-surface + confinement + amortized judgment" — **INTERPRETED** (supersedes the marketing reading of D1/README, not its mechanics)
The Reddit expert consensus (multi-commenter, corroborated): there is **no special
"machine representation" of code** — the model tokenizes Braid-bytes, Python, and prose
as the same kind of sequence; stripping human-affordances (names, redundancy, density)
removes the *contextual signal the model uses to stay coherent*, making it **dumber**.
The genre's founding move ("strip the human") is therefore wrong. What survives review is
the OTHER half: **compiler/grammar-enforced correctness**, because LLMs are weak at
correctness.
**Adopted reframe**: Braid leverages token-prediction by making the AI authoring surface
maximally *predictable* — familiar shape (TS/Ruby/JSON-like), shallow per-unit structural
depth (Wischik: "LLMs are poor at syntactic depth below a handful of levels"), and
*constrained to the closed vocabulary the model cannot predict outside of* — and by
closing the loop with the deterministic verifier as machine-readable feedback. The
canonical IR (CBOR/CID) is a **verification + anchor** form, NOT the authoring surface;
the authoring surface must be familiar and redundant. **Humans and AI never share a
representation; they share an *anchor* — the verified content-addressed core — and meet
through axis-specific projections** (generalizes D17). The README sentence "the form a
model manipulates reliably" is to be corrected to this anchor framing.

### D21 — Lean 4 is the REFERENCE ARCHITECTURE (trusted-core + extensible elaboration); design the elaboration seam now — **INTERPRETED**
Lean is the industrial existence-proof of Braid's thesis: a tiny trusted kernel that
re-checks every term, under an arbitrarily rich, user-extensible surface (parser /
elaborator / macros / notation) that *elaborates down* to the core (Mathlib ≈ 1.5M lines
→ one small kernel). Mapping: Lean kernel ↔ `braid-verify` (D9 independent re-check);
Lean core terms ↔ Braid IR; Lean elaboration/macros ↔ Braid's deferred surface (D6);
pretty-printer ↔ manifest (D12); code generator ↔ WASM codegen (P3).
**Decision**: custom syntax helps **only at the elaboration layer, never the trusted
core**. The *grammar* stays D6-gated, but the **elaboration SEAM** — the locked contract
"any surface → core IR → independent re-check" — is a **basement interface to design now**
(build-the-basement discipline). This resolves the Java-scale-vs-closed-vocabulary tension
(D-non-goals): ecosystem lives in the open notation/elaboration layer, trust lives in the
small closed core — exactly how Lean gets million-line reach from a few-thousand-line TCB.

### D22 — Do NOT fork Lean; use Lean (unforked) as an OFF-LINE PROOF ORACLE for verifier-rule soundness — **INTERPRETED**
Forking Lean to *be* Braid is rejected: (1) tool mismatch — Lean is a theorem prover for
human-guided interactive proof; Braid's checks are *decidable* (types, attenuation, taint
fold, bounds), needing a fast total verifier, not tactic search; (2) runtime mismatch —
Lean→C+GC vs Braid→WASM-over-three-syscalls (D14, "does not build a second runtime");
(3) **D9 violation** — building on Lean's kernel makes Lean's math-tuned TCB (kernel +
elaborator + `native_decide`/`axiom`/`sorry`/unsafe escapes) *our security TCB*, an
adjacent-not-identical threat model; (4) Lean surface is LLM-hostile (sparse corpus,
hidden tactic state = the LSP/hidden-context problem in extremis) — poisons "easy for AI";
(5) canonical-bytes/D8 friction; (6) re-imports the coupling extraction just removed.
**Adopted**: use Lean *unforked* as a design-time oracle to **prove Braid's verifier RULES
sound once** — taint-fold ⟹ non-interference, attenuation-lattice soundness,
effect-composition monotonicity, bounds ⟹ termination — then the independent Rust verifier
*implements* the proven-sound rules on the hot path. This is the seL4/CompCert
"verified checker" pattern (proof assistant proves the checker; the fast version ships).

### D23 — Anti-malware doctrine: CONFINEMENT, not detection — **INTERPRETED** (sharpens D10 + threat model)
Malware is binary-identical to benign code (ransomware ≡ backup: same syscalls); malice is
a relation between operation, data, and *authorization* — not an intrinsic code property,
and the distinguishing information is *erased* by the binary level. Detecting "is this
malware?" on arbitrary code is **undecidable** (Rice 1953; Cohen 1987). Defense therefore
**cannot live at the binary** and cannot be "detect the bad."
**The flip**: a closed, typed capability/effect/flow vocabulary turns "what can this do?"
from an undecidable *inference* into a decidable *read-off-the-type*. Whole *classes* of
malice become **confinement theorems** that hold regardless of intent, layered
defense-in-depth (no layer trusted to be complete):
(0) capability security — no ambient authority; an ungranted capability is *unrepresentable*
(D10); (1) information-flow / **non-interference** (Volpano-Smith-Irvine) — path-level taint
fold (the #361 lesson); (2) effect typing — actual effects ⊆ declared; (3) totality /
resource bounds — non-termination/DoS unrepresentable (scenario #9); (4) confirm policy —
the irreducible *value-judgment* residue, payload-hash-bound (T10); (5) manifest re-check at
load — provenance/swap (D12/T4). The residue proof cannot reach — true intent,
authorized-but-adversarial — **escalates to the human**; it is never claimed as decided
("safe-by-construction ≠ correct-by-construction"). Enforce where the distinguishing
information still exists (typed capability level), never at the binary.

### D24 — Grounding & anti-happy-path: confront-at-admission, not remember-at-authoring — **INTERPRETED**
LLM failure modes "forgets the real-world framework" and "only writes the happy path" are
converted from virtues-the-model-lacks into invariants-the-substrate-enforces:
(a) **Error-edge completeness is a fail-closed admission obligation** — a braid with an
undischarged typed-error edge does NOT admit; happy-path-only is *structurally
unrepresentable* (exhaustive case analysis, à la Rust/Lean).
(b) **The registry is the real-world ground truth**, and each term's `effect_class`,
`cost_bound`, and failure modes MUST be **extracted from the real system graph** (lgwks
ingestion graph / impact / complexity), never idealized — else grounding is fake. Unknown
term ⇒ deny (scenario #14) is the no-hallucinated-API guarantee.
(c) **Bounds compose** to force resource-reality (the 5M-row table surfaces as a budget
overflow ⇒ typed reject ⇒ re-author). The verifier *reminds* the model with
machine-readable reasons; the reject→re-author loop IS the grounding mechanism.

### D25 — Anti-soulless / Sr-SWE-judgment: intent as a typed term + amortized compiled judgment — **INTERPRETED** (goal), mechanism **REVISABLE**
"Soulless code" (technically correct, no why/fit/restraint) is a *judgment* failure
orthogonal to correctness, so tests never catch it. You cannot put taste IN the IR; you
make the IR carry **the dimensions taste operates on**, so judgment is checkable/visible:
- **Intent is a first-class TYPED term** (structured ontology, not freetext); admission
  *scores coherence* between declared intent and realized braid (lgwks `cohere` /
  `comprehend`). No coherent non-trivial intent ⇒ flagged low-coherence = soulless.
- **Grain-conformance**: similarity to the braid's graph-neighbors ("the senior knows the
  codebase," made mechanical); alien structure ⇒ innovation-or-slop ⇒ human.
- **Restraint/minimality**: effect-set == intent, no widening (the T12 gate; over-reach =
  soulless-by-excess).
**Amortized judgment + graduated friction**: the senior authors the vocabulary / intent
ontology / certified strands / coherence rules ONCE — taste *compiled into the substrate* —
and every junior / vibe-coder / AI author gets senior-level review for free,
deterministically; for the senior the checks are silent when satisfied. The IR does not
*think* like a senior — it *replays compiled senior judgment* to everyone downstream
(the "amortized intelligence" pattern). Irreducible taste escalates to the human via the
manifest.

### D26 — Multi-layer ("3D") co-registered projection IR — **RESEARCH DIRECTION** (not locked; gated like D6)
Candidate distinctive contribution. A program is normally crushed to 1-D (token stream) /
2-D (AST). Reframe: a single **content-addressed node-set ("shared anchor")** with
orthogonal first-class layers — dataflow / effect-control / intent / authority-taint /
abstraction-depth ("neuron layers") — every node carrying a coordinate on every axis.
Humans, LLMs, and the verifier each **project onto the layer(s) they need** (manifest is
already a 2-layer projection); optional literal 3-D visualization for human comprehension
(navigate a hard program by moving one axis at a time = how seniors hold a system).
Reuses the lgwks **JEPA multi-view** primitive ("shared anchors, machine packet, human
projection"). **Token-prediction rationale**: the LLM only ever authors/reads a *shallow,
single-axis linearization* — dodging "LLMs are poor at structural depth" by construction.
Generalizes D17 from one IR to a layered projection space. STATUS: research; any surface
realization remains D6-gated.

### D27 — A "good-code" (judgment) benchmark — **OPEN / PRD-§8 metric candidate**
PRD §8 success metrics currently measure only the gate (correctness/safety). Add a third
rung — *good* — measured by checkable PROXIES, not an absolute aesthetic score:
intent-coherence; **edit-locality / intent-stability** (small intent change ⇒ small local
diff — the Reddit [109] "preserve intent through small edits" test); **round-trippability**
(meaning survives re-representation — [109]); grain-conformance; restraint/minimality;
catch-rate-without-human. Braid is uniquely positioned (it has the intent binding + the
graph to measure against, which generic code benchmarks lack); feeds the
frontier-coherence-engine + human-AI-binning research. Honest caveat: proxies are gameable
and incomplete — the irreducible residue stays human.

---

## Amendments — 2026-06-15, round 2 (forcing the human as system designer; the up→down sandwich; adversarial-loop deflation)

### D28 — Human-as-system-designer FORCING FUNCTION: architecture is an AI-un-mintable, human-ratified anchor — **INTERPRETED**
The cardinal AI failure to engineer out: the AI silently collapses an open architectural
fork into ONE default — which is (a) too simplistic AND (b) not the AI's decision to make.
Fix it structurally, not by hope. **System architecture is a first-class typed node**
(options-considered + chosen + rationale — i.e., an **ADR as a runtime artifact**;
self-similar to this very register). Three properties make the human the system designer by
construction:
1. **AI-un-mintable**: the AI may *propose* an architecture node (does the legwork); **only a
   human can ratify/mint it.**
2. **Required anchor**: every capsule binds to a ratified architecture node; an unbound
   capsule does not author ("no ratified system-design anchor").
3. **Architectural-decision-completeness is a fail-closed admission obligation** — the
   sibling of D24's error-edge completeness. A capsule depending on an *un-ratified*
   decision blocks authoring and emits "decision required" to the human; **"un-decided
   architecture is unrepresentable."** The AI must SURFACE the fork, never default it.
This generalizes D25/GAP-B *up* from intent to architecture, and makes the central axis
(D17) literal: **human meets the system at architecture + intent (the chart); AI meets it at
composition (the flights); the verifier is the floor.** Stays high-leverage, not
human-in-the-loop-constantly (GAP E): the chart is **coarse and amortized** — one ratified
architecture serves many capsules; the forcing function fires at *architectural* forks only.

### D29 — The up→down / down→up SANDWICH; Power Apps "vibe" as reference for the top-down half — **INTERPRETED**
Two complementary directions, the AI sandwiched in the middle owning neither end:
- **down→up** (Braid's built strength): the verifier floor — autonomous correctness + safety.
- **up→down** (this addition): human-ratified architecture the AI must *align to*, from the top (D28).
The AI authors the MIDDLE band, **forced to align UP** (to the ratified architecture) **and
DOWN** (to the verifier floor). Reference for the top-down half: **Power Apps vibe**
(`learn.microsoft.com/power-apps/vibe/overview`, read 2026-06-15) — *Plan mode* makes the
plan a first-class human-reviewed artifact *before* code (Braid analog: architecture anchor +
manifest ratified before any capsule admits); *plan/data/app co-registered layers* is
**literally D26** (multi-layer projection — plan=intent, data=schema, app=UI, one shared
model); it *forces the AI to align to a systems-design framework* and serves citizen-devs AND
pro-devs (the juniors-AND-seniors target). **Where Braid goes further**: Power Apps aligns the
AI to *Microsoft's fixed* framework; Braid forces alignment to a **human-AUTHORED, per-system
architecture** — the human is the designer, not a picker of a vendor's bins (the difference is
the AI-un-mintable, ratification-required, decision-complete anchor of D28).

### D30 — Adversarial-loop deflation of "autonomous good code" (3-round Haiku red-team, defended in-thread) — **FINDING / REVISABLE**
Stress-tested the claim "the generate→verify→re-author loop autonomously produces GOOD code."
It survives only **deflated**, and the deflation is the honest thesis:
- **Braid autonomously raises the FLOOR, not the ceiling.** It guarantees structural-
  compositional correctness + confinement-safety (types, exhaustive error paths, capability
  attenuation, taint non-interference, resource bounds) for every author class. It does NOT
  autonomously produce taste-excellent or novel-domain code.
- **Code-grounded correction** (read `braid-verify/src/lib.rs`): v0 only *binary-gates*
  structure + safety. **Altitude / scope-creep / grain are ADVISORY (D25 "scores coherence")
  or research (D26/D27) — NOT blocking.** State the **advisory-vs-blocking** line explicitly.
- **Intent-COHERENCE ≠ intent-CORRECTNESS.** The verifier checks internal consistency with the
  declared intent, never whether the goal is right → a confidently-wrong declared intent yields
  admitted competently-wrong code (the exact junior/vibe-coder/AI failure mode). Correct the
  wording "intent-grounded" → "intent-coherent" wherever it appears.
- **Judgment splits 3 ways** (must be named in the design): (i) structural = blocking in-loop
  gate, Goodhart-safe because binary; (ii) advisory scalar = audit-only, **never the author's
  reward** (RLAIF on a coherence score = guaranteed Goodhart); (iii) irreducible taste = human.
- **Explore-next agenda (the gaps):** A) advisory→blocking frontier (restraint binarizes via
  the widening gate; right-altitude likely cannot); B) intent-correctness via a human-ratified
  `work_object`/architecture anchor (D28); C) "amortized judgment" rests on two unproven human
  acts — ontology-quality + humans-actually-read-the-manifest; D) no online feedback (static
  v0): registry-governance + runtime→registry correction loop; E) fast live chart-extension
  protocol for novel intent; F) **the decisive experiment** — a live A/B on a REAL port (not the
  CMS toy): does gate-admission predict *blind* senior-approval? + false-positive-rate-on-taste
  + post-merge outcome-by-axis, **stratified by domain** (rendering vs mutation; the catch-rate
  is NOT constant). Outcomes, not labels — dodges the circular-labeling/selection-bias traps.

---

## Amendments — 2026-06-23 (Director session: Braid as a global translator IR / brew-scale dependency / "renders JS useless")

**Provenance**: live Director session 2026-06-23, working from (a) the next-gen-browser-engine
ADRs (ADR-001 standalone browser, ADR-005 immutable content-addressed acyclic term graph =
Braid's IR shape, ADR-007 LLM not core runtime) and `BRAID_BRIDGE.md` ("the browser depends on
`braid-ir` and `braid-capability`"), (b) the kernel's `braid_vocab_binding.rs` seam (a pinned
snapshot literally waiting for Braid to be wired in: "When `braid-ir` is wired in: replace ONLY
the body"), (c) the Director's framing: "Braid needs to standalone work for a few langs first as
a global IR and try to replace Java," "braid needs to be good enough to not be a fork and become a
dependency," and the brew-scale bar ("brew as in how ubiquitous it is, think from that scale
(billions)"). Synthesis by Claude; INTERPRETED entries are veto-on-review per the lock legend.
These ENRICH the foundations; D6's surface-syntax gate is unchanged (no grammar authored here).

### D31 — Braid is a GLOBAL TRANSLATOR IR, not a browser-CMS framework — **INTERPRETED**
The Director's reframing: Braid is a machinistic, verifiable term IR (Lean/Julia-flavored: real,
not invented syntax) that source languages compile *into*; "custom lang" means the IR is the
global translator, "renders JS useless" means JS stops being a runtime authority surface and
becomes an authoring frontend over the verified substrate. The bar is Homebrew-ubiquity: a
dependency so fundamental and trivial to pull in that every runtime/language toolchain in the
ecosystem ends up depending on it, not a niche framework consumers must fork to escape.
**Adopted structural consequences (built this session):**
- **Substrate vs vocabulary separation.** `braid-ir` ships ONLY the language-neutral substrate
  (`Value`/`canon`/`Cid`/`TermRegistry`/`Capsule`/`TypeTag` atoms). No domain vocabulary lives in
  the substrate. `registry_v0` and the CMS examples moved to a new `braid-vocab-cms` crate; the
  kernel's 10 motion/browser capability verbs moved there as named consts. A consumer pulls the
  substrate + a vocabulary package, never the substrate alone with a baked-in domain.
- **String-tagged capabilities.** `Capability` is a content-addressed string newtype
  (`Capability::new("js.eval")`), not a fixed enum. Each vocabulary owns its capability space
  (`web.*`, `js.*`, `julia.*`); the verifier's attenuation check (grant ⊆ ambient) works on any
  token set — the lattice order is declared per-vocabulary, not hardcoded in the core. A union
  enum (re-coupling every consumer to every other domain's verbs) is the explicit anti-pattern.
- **`TypeTag::Opaque` for vocabulary-defined types.** The core type universe is the language-
  neutral atoms (`Bool`/`Int`/`Bytes`/`Text`/`Cid`/`List`); every domain type (`cms.entity`,
  `cms.directive`, `js.string`, `js.object`, `js.function`) is a vocabulary-owned
  `Opaque(label, args)`. A new language adds types without a core edit.
- **The verifier is already registry-parametric** (`verify(bytes, &TermRegistry, &[Capability])`);
  the coupling was only at the tooling layer. The CLI/SDK now take a vocabulary's registry rather
  than hardcoding `registry_v0`.
- **`braid-vocab-js` is the second vocabulary**, proving the claim: a JS capsule with `js.*`
  capabilities admits via the ONE `braid-verify`, no fork, no core edit (test:
  `js_capsule_admits_via_the_one_verifier`). This is the seed of "renders JS useless."
**What does NOT change (locked invariants preserved):** D8 canonical encoding, D9 verifier
independence, D10 no authority, D11 versioning, D12 manifest, D6 surface-syntax gate. The KAT
vectors re-pinned once (the `Entity`/`Directive` → `Opaque` encoding move changed registry/capsule
CIDs — a conscious, one-time re-pin recorded in `vectors/capsule_v0.kat`). The kernel's
`braid_vocab_binding.rs` snapshot's dotted names (`compute.remote`, `signal.emit`) are preserved
verbatim in `braid-vocab-cms`, so the kernel consumer binds without re-pinning on the name axis.
**Relationship to D6 (non-goal: "replacing existing languages"):** D31 moves the *replacement*
ambition from blue-sky-non-goal to a phased target, but the *mechanism* (a surface grammar) stays
D6-gated. Braid becomes a dependency by being the verified IR that languages elaborate INTO, not
by authoring a competing textual syntax. "Replace Java" = Java elaborates to Braid IR and the
JVM stops being the authority surface; it does NOT mean Braid ships a Java-syntax grammar in v0.
(Enriches D5/D14/D17; sharpens D6; generalizes D20's "anchor" framing to a multi-language anchor.)

## Amendments — 2026-08-25 (Director ratification of Frontier Flow P0)

Canonical detail and authority diagram:
`../../docs/adr-099-braid-frontier-flow.md`. These entries are **LOCKED** by
the Director's instruction to make RON first-class and finish P0 for Issue #56.

### D-FLOW.1 — Braid owns semantic Flow; Forge owns durable execution — **LOCKED**

Braid owns the inter-capsule Flow AST/IR, deterministic encoding and content
identity, independent static admission, and snapshot-bound next-step
derivation. Forge-harness owns durable instances, leases/fencing, event
history, retries, workers, crash recovery, effects, evidence persistence, and
replay. Experience-as-code owns domain declaration/observation/reconciliation;
`lgwks_bot` remains a leaf adapter. No owner may silently absorb another row.

### D-FLOW.2 — Outer Flow and inner strand DAG are distinct — **LOCKED**

Flow connects already-admitted capsule CIDs. It does not inline or rewrite a
capsule and cannot aggregate capsule authority. The existing `Braid` remains
the topologically ordered computation graph inside one capsule.

### D-FLOW.3 — Flow and Plan identities are distinct — **LOCKED**

`lw.braid.flow.v0` identifies source-order-independent semantic graph meaning.
`lw.braid.flow.plan.v0` additionally binds target profile, cache manifest,
planner version, immutable snapshot, and the selected step. Related v0 domains
are frozen in ADR-099. Every domain uses the existing `braid_ir::Cid`; wrappers
may prevent confusion but may not create a second CID implementation.

### D-FLOW.4 — v0 graph constructs are closed and bounded — **LOCKED**

Normalized nodes are `InvokeCapsule`, `Choice`, `JoinAll`, and `Terminal`;
edges are typed `Data` or explicit `After`. Source `MapStatic` must expand
before canonical encoding. Arbitrary cycles, runtime-unbounded expansion,
`JoinAny`, races, cancellation, and learned trusted scheduling are rejected.

### D-FLOW.5 — RON is first-class Flow authoring; JSON is interop — **LOCKED**

Ordinary Rust builders and RON lower to one typed Flow source AST. Normalized
JSON exists only for interoperability/inspection; YAML exists only behind
strict importers. Source text is never identity-bearing: accepted spelling,
comment, whitespace, and field-order differences that yield the same validated
AST yield the same canonical bytes and Flow CID. The existing capsule JSON
contract in D19 remains unchanged.

### D-FLOW.6 — Satiation precedes invocation and Unknown fails closed — **LOCKED**

Proofs are immutable-snapshot-bound. Cache/retry eligibility is derived from
the admitted capsule and term registry, never a Flow-authored boolean. Braid
selects at most one executable node in sequential v0; it does not execute or
persist the workflow lifecycle.

### D-FLOW.7 — Statecharts are projections, not a second runtime — **LOCKED**

Graph-DSL `statechart`/`orchestration` forms may lower bounded inter-capsule
semantics to Flow. Durable state, journals, receipts, compensation, replay,
and resumption belong to Forge. This entry supersedes conflicting runtime
language in the 2026-08-21 architecture blueprints.

### D-FLOW.8 — Flow crate boundaries follow invariant ownership — **LOCKED**

`braid-flow-ir` owns semantic bytes/CID; `braid-flow-verify` owns independent
admission; `braid-flow-plan` owns deterministic snapshot-bound frontier
derivation; `braid-flow-sdk` owns Rust/RON authoring, JSON interop, and
diagnostics. `braid-render` gains Flow manifest/DOT projection;
`braid-project` may consume admitted Flow after P2; `braid-run` executes one
selected capsule. No Flow crate may mint constellation-owned authority types.

### D-FLOW.9 — P1–P6 form a dependency DAG — **LOCKED**

The dependency edges are Braid #57 (IR) -> Braid #60 (verification) -> Braid
#59 (planning). Planning then forks to sibling successors Braid #58
(SDK/RON/interop/render/import) and forge-harness #123 (durable runtime); both
join at experience-as-code #66 (domain integration and performance proof).
Each boundary lands independently with its own negative evidence.

### D-FLOW.10 — P2 delivered — independent admission of `lw.braid.flow.v0` — **OBSERVED 2026-08-26**

`braid-flow-verify` at `f000551e` / PR #65 (merge `f13c71cc`) closes #60 /
`F-FLOW-01…19` admission surface. Own decoder over `Value` → `FlowSpec`
projection (not `braid-flow-ir::decode`), preflight wire cap, `decode_strict`
+ bijectivity `encode(to_canon)==bytes` (INV-018), acyclic SCC (INV-003),
reverse-BFS terminal co-reachability incl. `Choice` arms as edges (INV-014),
choice totality/disjointness (INV-011), `JoinAll` cardinality (INV-013),
terminal existence/soundness (INV-014), justification completeness per
`InvokeCapsule` (INV-006), fail-closed `FlowVerifyError` per invariant.
Verification lane 14/14 green at PR head `f000551` [ran]: `cargo test -p
braid-flow-verify` KAT `flow_v0.kat` admits + `mutation_matrix`
`every_invariant_has_a_killing_negative` kills per INV (malformed bytes,
truncated, cycle, missing justification via raw `Value` Map, isolated node);
`cargo clippy --workspace --all-targets -- -D warnings` clean. Local-ci
26/26 GREEN receipt `flow-p2-verify:f000551ec61c attested 1787724669` [ran];
PR CI run `32937074111` 14/14 success `f000551` pull_request [ran]. The
`f13c71cc` merge is the delivery commit; seam #64 and amendment #63 remain
open research/governance (no code lane).

### D-FLOW.11 — Elaboration seam and P0.1 amendment status — **OPEN 2026-08-26**

D21 seam (#64) and P0.1 Read|Write collapse (#63) are the governance
residue of Flow P0. D21: the basement contract "any surface → core IR →
independent re-check" (`braid-verify::verify(bytes, &TermRegistry,
&[Capability])` sole admission, precedent `braid-elaborate-js`
`lib.rs:893` → `braid-project lib.rs:33`) is doctrine; the conformance
harness `braid-seam-conformance` (two surfaces → same CID → one verdict,
`cargo test -p braid-seam-conformance`) is not yet built. §16 remains
gated — Trigger 2 ACCUMULATING across `braid-vocab-*` but not declared,
Triggers 1/3/4 PENDING/HALF-FIRED/NOT FIRED per #64 table at `e8fb2f4e`.
P0.1: collapsing durable interop to `Read | Write` with `Justified` as a
loud `Deferred { reason, required_by_version }` (never serialized as
`Proven`) requires an explicit amendment to D-FLOW.6 Unknown-fails-closed;
code MUST NOT silently reinterpret `Unknown` until the amendment lands.
D-FLOW.6 therefore stands unchanged in this edit.

### D-FLOW.12 — Choice disjointness is a bounded proof, never target uniqueness — **IMPLEMENTED**

For every pair of `Choice` arms, independent admission proves
`UNSAT(when_i AND when_j)` in the versioned v0 symbolic fragment before
admitting the Flow. The fragment covers constants, canonical literal/exact
reference equality and inequality, ordered reference/literal constraints,
reflexive reference relations, `And`/`Or`/`Not`, and completion-class atoms.
De Morgan and commutative operand normalization are deterministic. Predicate
depth (32), nodes (16,384), normal-form clauses (4,096), normal-form atom slots
(65,536), arm pairs, and aggregate work units (1,000,000) are preflighted before
solver allocation. Proven overlap returns stable Choice/arm
identities plus a deterministic minimal supported-clause counterexample;
unsupported distinct-reference relations and resource exhaustion return typed
`Unknown`. Both outcomes block admission. Distinct targets provide no proof.

Snapshot evaluation remains owned by `braid-flow-plan`: every arm is evaluated
against the exact immutable snapshot, any `Unknown` defers the entire Choice,
and the selected arm or mandatory `otherwise` target is included in the Plan
CID. This preserves D-FLOW.6 and INV-FLOW-007/008 without minting another proof
or identity authority. Because the canonical Plan input now includes step kind,
capsule CID, and Choice target rather than only the selected node key, the
planner algorithm version advances from 0 to 1. Version 0 contexts are typed
refusals; their plans must be recomputed and are never silently reinterpreted.

### D32 — Elaboration seam — 95% deterministic, 5% semantic gate — **INTERPRETED 2026-08-26** (veto on PR review; see `docs/adr-100-braid-elaboration-seam.md`)

> Director, verbatim compressed: "A DSL which operates like a borrow checker
> on top of the borrow checker + Rust. Super declarative/functional —
> `JSON::Find(x)::DoY`; it hates that none of that is true yet and finds the
> laziest `x→z` in the graph. GNN as gate if determinism fails. Highly
> semantically aware and sticky — knows the words, so `regex` vs `parse`
> interchangeably → panic. Three axes: full Rust (hard mode) for Safety,
> Capability-based + DiD, third axis Justification — why am I running. Make
> it 3D. Safety^Capability^Justification and Braid is the foundation. A lot
> less neural — 95% deterministic; GNN useful to enforce the semantic
> contract (or a compiler-style LLM if GNN isn't there)."

**Interpretation adopted (veto on PR review if wrong):**

* The DSL is a *borrow checker on top of the borrow checker*. Rust owns
  memory/ownership; Braid's 8 stages own meaning (unknown term, arity, type
  mismatch, `grant ⊆ ambient`, effect ordering, taint fold, cost overflow);
  the DSL's third checker — semantic stickiness — owns word meaning via
  the closed vocabulary. A probabilistic gate never lives inside the trust
  base: its output is reified as a concrete `Capsule`/`FlowSpec` choice
  that re-admits deterministically.
* Pedagogy is Meta LLM Compiler + PROGRAML: pretrain on IR, not surface;
  learned gate as emulator with a deterministic oracle (the verifier) to
  check against, not as the verifier. See `docs/adr-100-…` for the
  industrial citations and the `braid-seam-conformance` harness that is
  the training-data generator.
* Cut is 95% deterministic elaborator; 5% learned semantic gate is a
  **typed `Reject { stage: Semantic }` fallback**, not a scalar score
  (D30 Goodhart fence). "95%" is a harness-measured query
  (`no-learned-gate elaborations / total` on the conformance corpus +
  KATs), not a vibes percentage — stated as a **falsifier** until the
  harness exists (V3 crooked-ruler fence). PB-08: deterministic gates
  are the instrument; no 5% reading is trusted until stages 1–8 are
  GREEN.
* 3D `Safety^Capability^Justification` is D26's multi-layer projection IR
  over the shared anchor: Safety → type/bounds, Capability^DiD →
  authority/taint, Justification → intent/architecture (D25/D28) with
  `Deferred { reason, required_by_version }` in v0 until P0.1 flips it
  (D-FLOW.6 unchanged; #63 governs that flip).
* The **seam** (D21) is what this entry locks: the basement contract
  "any surface → core IR → independent re-check" — single admission path
  `braid_verify::verify(bytes,&TermRegistry,&[Capability])`
  (`crates/braid-verify/src/lib.rs:511`), no second wire format, no
  producer-side normalization, no allocation before preflight, no
  authority pooling. The **grammar** (JSON::Find/DoY sugar) stays
  D6-gated on §16 triggers 2/3. The v0 proof pair is JS text vs
  JSON-of-IR — zero parser debt, does not count as a §16 second surface.
  RON Flow (#58) is the first D6-gated surface once it exists.
* §16 triggers at `63ee5785`: T2 ACCUMULATING (3 vocab crates, 1 wired),
  others PENDING/HALF-FIRED/NOT FIRED — **record of observation, not a
  lock change**; D6 stays LOCKED until a future ADR declares T2 fired.
  Full table + falsifiers in `docs/adr-100-…`.

Converts to LOCKED on Director merge of the ADR-100 PR. Awaits the
`braid-seam-conformance` harness (`cargo test -p
braid-seam-conformance`: two surfaces → same bytes/CID → one verdict;
hostile bytes rejected identically; no GNN in the trust base) to close
D-FLOW.11.

### D33 — Bounded native Braid Capsule DSL v0 — **LOCKED 2026-08-29**

The Director explicitly rejected the JavaScript expression subset as the
Braid DSL and instructed implementation against issue #77 and the repository
specifications. ADR-102 therefore fires D6 for exactly one versioned surface:
the bounded `cms::v1` Capsule graph grammar in
`spec/braid/elaboration/braid-dsl-v0.md`.

This surface lowers through `braid_sdk::Builder`, emits the existing canonical
Capsule wire, checks exact declared authority/effects, and re-enters the one
independent `braid-verify` admission path. It owns neither a wire format nor an
admission rule. JSON-of-IR parity, pinned CIDs, typed refusals, proof-gated
reference execution, and the complete repository gate are acceptance
requirements.

This does not unlock schemas, state/statecharts, Flow orchestration, imports,
macros, loops, recurrence, runtime literals, embedded code, raw URL
expressions, arbitrary registries, or replacement-language claims. Those remain
D6-gated and require substrate support plus separate decisions.

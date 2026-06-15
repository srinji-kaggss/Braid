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

### D19 — `braid encode` author input = JSON-of-IR over the SDK Builder — **INTERPRETED** (Director-selected live, 2026-06-14)
> Director, live this session (2026-06-14), choosing among three framed
> options for what `braid encode` consumes: **"A"** — JSON-of-IR transcription.

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

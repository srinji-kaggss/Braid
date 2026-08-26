# ADR-100: Braid elaboration seam — 95% deterministic, 5% semantic gate

**Status:** PROPOSED — drafted from Director decisions given live
2026-08-26 session; ratifies on Director merge

**Date:** 2026-08-26

**Issues:** [#64](https://github.com/srinji-kaggss/Braid/issues/64)
(D21 seam, §16 reassessment) · [#63](https://github.com/srinji-kaggss/Braid/issues/63)
(P0.1 Read|Write collapse)

**Decides:** `spec/braid/DECISIONS.md` D32 (new) + D-FLOW.11 closure note;
`spec/braid/units.md` debt anchors D-ELAB/D-VOCAB/D-CONSUMER/D-SEMANTICS
already landed at U11–U14 — see that file

**Supersedes:** nothing; enriches D1/D5/D6/D9/D17/D20/D21/D24/D25/D26/D30/D31
without unlocking D6 grammar work

**Authors:** Director Srinjon Gupta (decisions: 3D DSL, semantic stickiness,
Safety^Capability^Justification, 95/5 split) + Claude (synthesis)

---

## Context — reading the research without becoming it

The 95/5 split is the whole thesis. The nearest industrial artifact for the
5% half is Meta **LLM Compiler** (Code Llama 7B/13B, 546 B tokens LLVM-IR +
assembly plus 164 B fine-tune, LLVM 17.0.6, 16 k context; 77% of
autotuning's optimizing potential with zero extra compilations, 45%
disassembly round-trip / 14% exact
[arXiv 2407.02524](https://arxiv.org/abs/2407.02524)
[HuggingFace](https://huggingface.co/facebook/llm-compiler-13b),
[CSc 81010 slides](https://www.cl.cam.ac.uk/~ey204/teaching/ACS/R244_2025_2026/papers/LLMCompiler_Meta_2024.pdf)) — which proves the
pedagogy to steal: **pretrain on the IR, not the surface**, then use the
probabilistic model as an *emulator with a deterministic oracle to check
against* (the real compiler). Its complement is **PROGRAML** (ICML'21),
a portable graph over IR that captures control/data/call relations for a
Gated GNN to ≥0.939 F1 on analysis tasks
([PROGRAML](https://htor.ethz.ch/publications/img/programl-icml21.pdf);
also [CPE 6869](https://doi.org/10.1002/cpe.6869),
[PMC13012953](https://pmc.ncbi.nlm.nih.gov/articles/PMC13012953)) — the
deterministic counterpart that says the graph *is* the signal the token
stream erases.

Braid already owns both sides of that cut. D21/D31 locked the shape —
*trusted core + extensible elaboration* — as the Lean pattern:
`braid-verify` is the kernel, `braid-ir` the core terms, elaboration the
open layer, manifest the pretty-printer, WASM codegen the generator.
D31 made it a *global* IR (substrate `braid-ir` vs vocabulary packages
`braid-vocab-*`, string-tagged `Capability`, `TypeTag::Opaque`,
registry-parametric `verify`). D30 split the world into
*blocking* (binary gates) vs *advisory* (scalar scores the author never
optimizes against) — because advisory-as-reward is guaranteed Goodhart.
D6 still gates surface syntax on §16 triggers 2/3.

Director vision for this ADR, verbatim compressed:

> A DSL which operates like a borrow checker on top of the borrow checker +
> Rust (already true to some extent). It is super declarative/functional:
> `JSON::Find(x)::DoY` — it hates that none of that is true yet and finds
> the laziest `x→z` in the graph. GNN as gate if determinism fails. Highly
> semantically aware and sticky — it knows the words, so `regex` vs `parse`
> interchangeably → panic. Three axes: full Rust (hard mode) for Safety,
> Capability-based + DiD deeply engrained, third axis Justification — why
> am I running, what is my purpose. Make it 3D. Safety^Capability^
> Justification come together and Braid is the foundation.

That is D24/D25/D26 already said in compiler terms. The ADR's job is to
turn it from doctrine into a **typed basement contract** the harness can
prove — without unlocking grammar.

---

## Decision — the seam

### The one admission path (D9 preserved)

Every surface lowers to bytes admitted by **one** function:

```rust
braid_verify::verify(bytes, &TermRegistry, &[Capability]) -> Verdict
// crates/braid-verify/src/lib.rs:511 — 8 stages in locked order
```

Bytes in, verdict out. No surface bypasses or duplicates admission. The
precedent this generalizes is JS source → Capsule → admit
(`crates/braid-elaborate-js/src/lib.rs:893` wired at
`crates/braid-project/src/lib.rs:33`). The verifier shares no
serialization/normalization path with any authoring SDK (D9).

A second `verify` exists at the outer-graph level —
`braid_flow_verify::verify(bytes)` over `Value → FlowSpec` for
`lw.braid.flow.v0` (D-FLOW.3). The seam shape is identical there
(surface → `FlowSpec` bytes → independent admission), but the CID domains
are distinct (`lw.braid.capsule.vX` vs `lw.braid.flow.v0`) and the two
verifiers are not conflated. This ADR locks the **capsule seam** for v0;
the Flow outer-graph seam is the same contract applied when RON Flow
(#58) exists.

### The 95/5 cut — what is deterministic and what is the 5%

```
surface text (JS / JSON-of-IR / future RON / future Graph DSL)
        │
        ▼  elaboration seam — this ADR's contract
  deterministic elaborator
  surface → Value → Capsule → canonical bytes + CID
  (lex/parse/type-directed emit via Builder over registry_v0)
        │
        ├──▶ semantic gate — deterministic where possible,
        │     learned where it is not (the 5%)
        │       • vocabulary stickiness: regex ≠ parse, find ≠ scan
        │         — closed vocabulary IS the dictionary; wrong word →
        │         deterministic Stage 3/4 Reject (unknown term / no
        │         typed term for operand types — cf. braid-elaborate-js
        │         resolve_binary at src/lib.rs:743-769)
        │       • intent↔braid coherence (D25) — the only place a GNN /
        │         LLM-compiler gate may fire
        │       • JSON::Find(x)::DoY → laziest x→z is a deterministic
        │         shortest-path over the already-typed capsule/Flow graph;
        │         learned gate fires only if that scan is ambiguous or
        │         non-interference/bounds reasoning needs a heuristic
        │     Verdict: typed Reject { stage: Semantic, reason }, not a
        │     hidden scalar — the agent repairs from it without a human
        │
        ▼  D9 — independent re-check, bytes alone
  braid_verify::verify(bytes, &TermRegistry, &[Capability])
  8 stages: CanonicalForm → VersionPin → Structure → Types
            → Capability → Effect → Taint → Bounds
```

Invariants:

* The DSL **is** the borrow checker on top of the borrow checker.
  Rust's borrow checker owns memory/ownership. Braid's 8 stages own
  meaning: unknown term, arity, type mismatch, `grant ⊆ ambient`,
  effect ordering, taint fold (`Exposure` max), cost overflow. The DSL's
  third checker — semantic stickiness — owns *word* meaning: the closed
  vocabulary is the dictionary it reads; near-synonym with wrong intent
  is the one place the learned gate may fire. All three checkers are
  borrow checkers: they make the wrong program *unspellable*.
* A probabilistic gate **never lives inside the trust base**. Its output
  is reified as a concrete `Capsule`/`FlowSpec` choice that then
  re-admits deterministically. The GNN proposes the path; the verifier
  proves it admissible. Deterministic gate GREEN is the instrument
  (PB-08); no reading from the 5% gate is trusted until stages 1–8 are
  GREEN (V3 crooked-ruler fence).
* "95% deterministic" is a **harness-measured query**, not a vibes
  percentage (V3/G9). Proposed query:

  ```text
  determinism = elaborations that admit without invoking the learned gate
                / total elaborations
  ```

  measured on the `braid-seam-conformance` corpus plus the 14 P0/P1 KAT
  vectors. Until that harness exists, the number is not a number — so
  this ADR states the threshold as a **falsifier**, not as a claim:
  ship with deterministic gate GREEN on the existing corpus; the 5%
  fallback is allowed only as a typed `Reject`, and its false-positive
  rate is measured by the same harness stratified by domain (rendering
  vs mutation — the Day-1 false-positive-rate-on-taste note in D30).

### The 3D — Safety^Capability^Justification over the shared anchor

Braid is the **shared anchor** (D20/D26/D31): one content-addressed
node-set with orthogonal projections. The 3D is not metaphor — it is
D26's multi-layer projection IR:

* **Safety** projects onto the type/bounds layer (stages 1–4 + 8 +
  Rust's borrow checker under them; `TypeTag` atoms + `Opaque` vocab
  types).
* **Capability^DiD** projects onto the authority/taint layer
  (stages 5–7; closed vocabulary makes "what can this do?" decidable;
  the voice that says an ungranted effect "does not exist" — D10/D23).
* **Justification** projects onto the intent/architecture layer
  (D25/D28; intent as a typed term, architecture as an AI-un-mintable
  ratified anchor; the `why` axis).

The DSL is the one language that can address all three at once because
it elaborates down to that anchor. The LLM only ever authors a *shallow,
single-axis* linearization per projection — dodging Wischik's "LLMs are
poor at structural depth" — and the seam joins them at the anchor.

Justification in v0 follows **P0.1** ([#63](https://github.com/srinji-kaggss/Braid/issues/63)): durable interop
collapses to `Read | Write`; `Justified` is `Deferred { reason,
required_by_version }` (never serialized as `Proven`), visible in
manifests/receipts/diagnostics. The current rule `Unknown` fails closed
(D-FLOW.6) is **not** silently reinterpreted until the P0.1 amendment
lands. So the DSL's third axis is **visible but non-blocking** in v0 —
`justification_gate = deferred` in every manifest until
`required_by_version` flips it to `Enforced`. High-risk `Write` classes
may opt into `Enforced` earlier without changing the seam.

### Surfaces in scope (no grammar is authored here)

* **v0 — the harness pair:** JS text (`braid-elaborate-js`) vs
  **JSON-of-IR** through `braid_sdk::Builder` (`braid-cli` D19
  JSON-of-IR transport). Both hit `Builder::new(registry, …)` then
  `to_bytes()` then the one `verify` — different elaborators, same
  anchor. This proves the seam now with zero parser debt and without
  artificially firing §16 trigger 2.
* **Future — D6-gated:** RON Flow authoring ([#58](https://github.com/srinji-kaggss/Braid/issues/58)), Graph DSL inner-DAG text
  lowering (`docs/architecture/BRAID-GRAPH-DSL.md`, inner DAG only),
  Lean offline oracle output (D22). Each future surface implements **this**
  seam; none invents a wire format.

### Prohibitions — what a surface may NOT do

Inherited from Flow P2's three producer-side failures and generalized:

1. **No second wire format.** One `Value → Capsule` decode in
   `braid-verify` (and one `Value → FlowSpec` in `braid-flow-verify`)
   is the only decoder. ADR-099 reconciliation rule preserved.
2. **No producer-side normalization of hostile bytes.** Non-canonical
   bytes are rejected by the bijection guard, not massaged into shape.
3. **No allocation before preflight.** Source-byte/depth ceilings and
   declared collection bounds are checked before semantic AST
   allocation (the `braid-flow-sdk` hostile-RON precedent).
4. **No authority pooling.** Each capsule is verified independently
   under the empty ambient set — `braid-project`'s precedent
   (`crates/braid-project/src/lib.rs:18-24`): a capsule's CID inside a
   project equals its standalone CID; building together pools no
   authority, no shadowing, fail-closed on the first reject.

### Training pedagogy — where "CompilerLlama" fits

Not "fine-tune on Braid DSL text." Fine-tune on
`(surface → admitted capsule CID, manifest, verifier verdict)` triples
— so the model learns to **elaborate correctly, not to hallucinate
bytes**. The `braid-seam-conformance` harness **is the training-data
generator**: every green run (`two surfaces → same CID → one verdict`)
is a labeled triple, exactly the LLM Compiler pedagogy (pretrain on IR,
fine-tune on compiler-emulation with an oracle check). PROGRAML's lesson
— the graph *is* the signal the token stream erases — says the corpus
should include the **graph projection** (DOT/JSON export) alongside the
surface text.

---

## §16 trigger status — assessed 2026-08-26 at `63ee5785` (main)

| Trigger | Status | Evidence |
|---|---|---|
| 1. Vocabulary stability across ≥2 real workflows | HALF-FIRED | A7 vocab implemented (#461/PR #480). One real workflow end-to-end (afternow-port, D7). Flow v0 is infrastructure, not a second workflow — P2–P4 build toward it. |
| 2. Repetition pain across ≥2 surfaces | ACCUMULATING, not declared | Three projection targets exist (`braid-vocab-rust`/`-js`/`-web`); only `--target rust` is wired (`crates/braid-project/src/cli.rs:9`). Each new surface re-implements projection glue — the exact signal §16 names. JSON-of-IR being the v0 harness pair does **not** count as a second surface for trigger purposes. |
| 3. Second independent surface | PENDING | Surface 1 = lgwks-frontend scaffold; kernel canvas + browser engine are candidates not yet building on Braid. |
| 4. External authoring | NOT FIRED | Zero external dependents (`docs/PARITY_AUDIT.md` SOTA section). |

Net: **the grammar stays gated.** No trigger has formally fired. T2
evidence is accumulating exactly where §16 predicted — which is an
argument for building the seam now so the eventual grammar is a thin
frontend, not a fork. This table is a **record of observation at
`63ee5785`**, not a lock change — D6 stays LOCKED until a future ADR
declares T2 fired.

---

## Consequences

* The seam contract is provable *before* any grammar ships — the basement
  is built now, the house later (D21 build-the-basement). A future DSL
  becomes a thin frontend over the seam, not a fork.
* Vocabulary discipline gets teeth: `regex` vs `parse` interchange is a
  deterministic reject today, a learned-gate reject tomorrow — both are
  typed and repairable without a human.
* `Safety^Capability^Justification` is no longer a slogan — it is three
  staged elaborations over one anchor, with Justification `Deferred` but
  visible so the thesis is not faked (P0.1).
* The 5% gate is fenced by PB-08 + D30 + V3: it cannot forge an Admit,
  cannot be the author's reward, and its number is a query or it is not
  a number.

## Falsifiers

Reject this amendment (or flip the 95/5 balance) if:

* Any required consumer needs a third persistence primitive beyond
  `Read | Write` that cannot be represented as pure capsule compute or
  an atomic read/write transaction plus external bounded effect handled
  by the Kernel (the #63 falsifier — network/process/device effects are
  Kernel capabilities, not DB verbs).
* The deterministic shortest-path `x→z` over the typed graph is
  systematically ambiguous across real workflows in a way that the
  typed Reject cannot surface for repair — i.e., the ambiguity is not
  repairable by the agent from the verdict alone.
* The harness-measured `determinism` on the real corpus falls below
  0.95 and the learned gate's stratified false-positive rate exceeds
  the blind senior-approval delta from D30's decisive experiment —
  evidence the seam is carrying taste, not structure.

---

## References

* `spec/braid/DECISIONS.md` D1/D5/D6/D9/D17/D20/D21/D24/D25/D26/D30/D31,
  D-FLOW.6, D-FLOW.11
* `spec/braid/PRD.md` §16 / Phases; `docs/adr-088-…` / `docs/adr-099-…`
* `crates/braid-verify/src/lib.rs:511` (one admission path, 8 stages)
* `crates/braid-ir/src/capsule.rs` + `crates/braid-ir/src/cid.rs`
  (capsule CID); `crates/braid-flow-ir` / `crates/braid-flow-verify`
  (Flow CID domain `lw.braid.flow.v0`)
* `crates/braid-elaborate-js/src/lib.rs:893` → `crates/braid-project/src/lib.rs:33`
  (precedent generalized by this seam)
* `crates/braid-project/src/lib.rs:18-24,48`
  (per-capsule empty-ambient, project CID distinct)
* Meta LLM Compiler — 546 B LLVM-IR/assembly + 164 B fine-tune, Code
  Llama 7B/13B, 16 k context; 77% autotuning / 45% round-trip / 14% exact
  ([arXiv 2407.02524](https://arxiv.org/abs/2407.02524))
  ([HuggingFace](https://huggingface.co/facebook/llm-compiler-13b))
* PROGRAML — portable graph over IR → GGNN ≥0.939 F1
  ([PROGRAML](https://htor.ethz.ch/publications/img/programl-icml21.pdf))
  ([CPE 6869](https://doi.org/10.1002/cpe.6869))
  ([PMC13012953](https://pmc.ncbi.nlm.nih.gov/articles/PMC13012953))

# Braid PRD — machine-first framework for Logic OS

**Status**: P0 foundations — agents plan and execute from this document.
**Authority**: ADR-088 + `DECISIONS.md` (lock register). On conflict, the register wins.
**Threats**: every requirement cites the `threat-model.md` entries it exists to close.

---

## 1. Vision

A complete, ground-up framework — engine, vocabulary libraries, toolchain —
where **AI authors richly and is admitted narrowly, and humans audit cheaply
and confirm rarely**. The compiler/verifier pipeline is the compliance and
oversight mechanism; not a model, not a reviewer. The end-state ambition is
Java/WASM-scale (a self-sufficient runtime ecosystem); the v0 commitment is
deliberately small: a typed IR, a deterministic verifier, a manifest renderer,
and one real workflow proven end-to-end.

What makes it *machine-first*: the canonical form of a program is data —
content-addressed, diffable, closed-vocabulary, free of ambient authority —
which is the form a model manipulates reliably. What makes it *human-trustable*:
every artifact deterministically renders to a manifest a specialist can read,
and nothing the manifest doesn't declare can happen at runtime
(correct-by-construction, not correct-by-review).

## 2. Users

| User | Day-1 capability | Later |
|---|---|---|
| **AI agents** (coder/architect under ADR-071 bounds) | Author capsules as IR data via the Rust SDK or raw canonical bytes; receive typed verifier verdicts (machine-readable reject reasons) | Author against the textual syntax; propose vocabulary extensions through governance |
| **Specialist human / Director** | Audit manifest diffs; confirm irreversible effects; run the full pipeline by CLI without AI (L7 reconstructability) | Author in the surface syntax; certify strand libraries |
| **Kernel/CI** | Admit/reject artifacts deterministically; gate PRs on capability-widening manifest diffs | Runtime admission of third-party capsules (ADR-069 tiers) |

## 3. Goals / Non-goals

**v0 goals**
1. G1 — Typed term-graph IR ("strand/braid/capsule") with canonical encoding, CID, and KAT vectors. *(T3, T8)*
2. G2 — Deterministic verifier implementing the ADR-088 D3 stage order through taint + bounds. *(T1, T2, T5, T6, T7)*
3. G3 — Manifest model + renderer, CID-bound; CI capability-widening gate. *(T4, T10)*
4. G4 — Rust SDK for authoring IR ("rust day 1"). *(T13)*
5. G5 — Day-0 CMS reference: ≥3 afternow-port CMS actions as admitted capsules with evidence. *(T14, anti-slop forcing function)*
6. G6 — `braid-cli`: encode/decode/verify/render/diff — the human-reconstructable path. *(L7)*

**Non-goals for v0–v2 (LOCKED as non-goals — D6)**
- Textual surface syntax, grammar, parser, editor/projectional UI.
- Replacing existing languages ("blue sky" end-state).
- New storage, new crypto, new capability semantics (Braid adds no authority — D10).
- ML-assisted anything in the admission path (advisory lanes only, ADR-071).
- General-purpose-language ecosystem features (package registry, FFI) — research baseline §22.7 dead-end.

## 4. The IR model (normative direction; bytes locked only via U1's KATs)

### 4.1 Strand — the unit of "api-type math"
A **strand** is one application of a registered **term**:

```text
Term (registry entry — closed, versioned):
  term_id            stable, content-derived
  signature          typed inputs → typed output (closed type universe)
  capability         Option<canvas_protocol::Capability>  // None = pure
  effect_class       Pure | Read | ReversibleWrite | Irreversible | Egress
  composition        Local | EgressMediated | TerminalDestructive   // mirrors vocabulary.rs
  cost_bound         declared worst-case cost class
```

The v0 type universe is deliberately tiny: bool, int (fixed-point — D8, no
floats), bytes, string (UTF-8, length-bounded), CID-ref, entity-ref, plus
registered record/enum shapes. Pure terms (the "math") carry no capability and
may be evaluated/simulated freely; effectful terms are exactly the kernel
vocabulary's actions plus registered pure adapters around projections.

### 4.2 Braid — composition is checked algebra
A **braid** is a DAG of strands. Edges are typed value flows. Legality
(verifier stages 3–7):
- types unify;
- authority only attenuates along every path (ADR-073 lattice);
- effect composition respects registered postures; an `Irreversible` or
  `Egress` strand requires the confirm policy of §4.3;
- exposure taint folds monotonically (max of producer + parents) along every
  path — **path-level**, not per-hop *(lesson: kernel #361's per-hop ≠
  path-level laundering finding; trip-wire test required — T5)*;
- declared cost bounds compose to a capsule-level budget.

### 4.3 Capsule — the admitted artifact
```text
Capsule:
  braid               the term DAG
  intent              declared purpose + work_object binding
  grants              the (attenuated) capability set requested
  bounds              outcome envelope + resource budget
  confirm_policy      None | HumanConfirm(payload-hash-bound)   // T10
  evidence_policy     what is journaled/retained
  versions            VOCABULARY_VERSION + braid IR version     // T6
  → canonical bytes → CID (BLAKE3, domain `lw.braid.capsule.v0`)
```

### 4.4 Manifest — what the human sees
Deterministic rendering of the capsule: intent, full capability list, effect
classes present, irreversible/egress strands highlighted, bounds, confirm and
evidence policy, version pins, CID. Bound to the capsule CID; re-derivable by
the runtime *(T4)*. The review object is `braid-cli diff <old> <new>` over
manifests — additions of capabilities/effects render as widenings and gate CI.

## 5. Architecture placement

```text
AI / Rust SDK ──authors──▶ Capsule (IR-as-data, canonical bytes, CID)
                                │
                     braid-verify (D3 stage order, fail-closed)
                                │ admit ⇒ (artifact CID, manifest CID) signed
                                ▼
                 runtime admission (independent re-verify at load)
                                ▼
            WASM component (P3) over the THREE-SYSCALL surface only
                  Capability(...) / Projection(...) / Sync(...)
                                ▼
                Causal Tape facts + journal + audit evidence
```

Kernel contracts consumed (the ONLY allowed dependencies — D3 boundary):
`canvas_protocol::Capability`, `canvas-syscall` `VOCABULARY_VERSION` +
`action_spec` postures, `state-fabric` hash/envelope types, the syscall API.
A conformance test fails the build if a `braid-*` crate imports anything else
from kernel crates *(T15)*.
> _Extraction note (2026-06-13):_ in the standalone `srinji-kaggss/Braid` repo,
> `Capability` is vendored as `braid_capability::Capability` (a verbatim mirror
> of the kernel `origin/main`), and the `VOCABULARY_VERSION` / `action_spec`
> binding is enforced kernel-side rather than in this repo. See the ADR-088
> extraction addendum.

## 6. Phases

| Phase | Content | Gate |
|---|---|---|
| **P0** | This doc set; ADR-088 ratified | Director merge |
| **P1** | `braid-ir`: types, canonical encoding, CID, KATs, bijection guard | U1+U2 ACs; hacker pass on encoding |
| **P2** | `braid-verify` stages through taint+bounds; `braid-render` manifests; CI widening gate (observe-only first) | U3–U6 ACs |
| **P3** | WASM codegen + runtime admission — **coordinates with the kernel Day-0 WASM epic; does not build a second runtime** | U7 |
| **P4** | Day-0 CMS reference workflow end-to-end | U8 + U9 hacker pass = "v0 done" |
| **P5** | Rust SDK polish; vocabulary-extension governance flow | U10 |
| **P6+** | Surface syntax (gated §16 triggers 2/3); more targets; blue sky | new ADR |

## 7. Acceptance scenarios (v0 — all must pass; framework-level analogue of research baseline §24)

| # | Scenario | Expected |
|---|---|---|
| 1 | AI authors a CMS "edit page section" capsule (reversible, local) | Admitted; manifest shows no egress/irreversible |
| 2 | Same capsule + an `Irreversible` publish strand, no confirm policy | **Rejected** at effect stage with typed reason |
| 3 | Publish capsule with confirm policy; payload changes after confirmation | **Rejected** at runtime: confirmation hash mismatch (T10) |
| 4 | Capsule requests capability outside its grant chain | **Rejected** at capability stage (attenuation violation) |
| 5 | Vault-read strand feeds an egress strand through 3 pure hops | **Rejected** at taint stage (path-level fold catches laundering — T5) |
| 6 | Two byte-encodings claimed for one capsule | Impossible: non-canonical bytes rejected by bijection guard (T3) |
| 7 | Capsule pins VOCABULARY_VERSION 1; registry at 2 | **Rejected** absent migration proof (T6) |
| 8 | Float smuggled in a param payload | **Rejected** at canonical-form/type stage (D8) |
| 9 | Budget-exhausting braid | Deterministic kill at declared budget; evidence emitted (T7) |
| 10 | Manifest displayed ≠ admitted artifact | Impossible: runtime re-derives manifest from CID, mismatch ⇒ refuse to load (T4) |
| 11 | PR widens a capsule's capability set | CI manifest-diff gate flags for security review (T12) |
| 12 | Human, no AI, CLI only: author capsule → verify → render → run | Same verifier verdict and artifact as the AI path (L7, T13) |
| 13 | Verifier and SDK disagree on any KAT vector | Build RED — parity is a conformance gate (T2) |
| 14 | Unknown term / unknown version / unverifiable manifest | **Deny**, always (fail-closed — L9) |

## 8. Success metrics (v0)

- 3+ CMS reference actions admitted and executed with journaled evidence.
- 100% of acceptance scenarios green in CI; KAT + bijection fuzz suites green.
- Hacker pass (U9) verdict: no confirmed-real bypass of the admission path.
- A capability-widening PR is demonstrably caught by the CI gate (red-team it).
- Zero Braid-crate imports outside the declared kernel contracts.

## 9. Open questions (tracked, non-blocking for P1)

- OQ1: WASM fuel metering ↔ declared cost-bound mapping (U5/U7; needs the
  kernel runtime's engine choice).
- OQ2: strand-library ("stdlib") packaging + certification flow (P5; touches
  ADR-069 tiers).
- OQ3: where vocabulary-extension governance binds to the A4.8
  GovernanceLedger (post-v0).
- OQ4: D5 interpretation confirm — Director veto window on the foundations PR.

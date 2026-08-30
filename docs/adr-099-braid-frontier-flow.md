# ADR-099: Braid Frontier Flow authority and wire contract

**Status:** ACCEPTED
**Date:** 2026-08-25
**Issue:** [#56](https://github.com/srinji-kaggss/Braid/issues/56)
**PR:** [#55](https://github.com/srinji-kaggss/Braid/pull/55)
**Author:** Director Srinjon Gupta (decisions) + Codex (synthesis)

---

## Context

Braid already owns the canonical inner `Capsule -> Braid -> Strand` DAG, its
encoding and content identity, independent admission, rendering, and
single-capsule execution. It does not have a canonical graph for wiring
multiple admitted capsules into a justified workflow. `braid-project` admits a
set but intentionally does not connect it; `braid-run` executes one admitted
capsule but owns no durable scheduler.

Without an explicit boundary, an outer graph could be independently invented
in Braid, forge-harness, experience-as-code, and `lgwks_bot`. That would create
competing identities, replay rules, and authorities. The pre-P0 architecture
documents also used “stateful orchestration” broadly enough to imply that
Braid owns durable journals and resumption, which conflicts with the
Constellation Charter and forge-harness.

The Director additionally requires RON to be the first-class textual source
form, with JSON retained only for interoperability. The repository already
governs RON through `lgwks_std::ron` and its approved dependency contract.

## Decision

```text
Rust builder / RON source / strict importers
                    |
                    v
         Braid Flow source AST
                    |
         normalize + canonical encode
                    |
                    v
     Braid Flow IR + independent admission
                    |
       deterministic snapshot-bound plan
                    |
                    v
 forge-harness durable instance + effects
                    |
        braid-run executes one capsule

experience-as-code --compiles domain intent--> Braid Flow
lgwks_bot --------optional compatibility-----> Braid Flow
```

### D-FLOW.1 — Authority split — LOCKED

Braid owns the semantic inter-capsule Flow AST/IR, canonical encoding, Flow
identity, independent static admission, and deterministic snapshot-bound
frontier derivation. Forge-harness owns durable instances, leases/fencing,
event history, retries, workers, crash recovery, effect execution, evidence
persistence, and replay. Experience-as-code owns experience meaning,
declaration, observation, and reconciliation and may compile into Flow.
`lgwks_bot` remains a leaf library and optional compiler input.

### D-FLOW.2 — Two graph levels — LOCKED

Outer Flow is separate from the existing inner `Braid` strand DAG. A Flow node
may invoke an already-admitted capsule by its existing `braid_ir::Cid`; it may
not inline, rewrite, or aggregate the capsule's authority.

### D-FLOW.3 — Identity and domains — LOCKED

The v0 domains are:

| Concept | Domain |
| --- | --- |
| Semantic Flow | `lw.braid.flow.v0` |
| Flow node | `lw.braid.flow.node.v0` |
| Plan | `lw.braid.flow.plan.v0` |
| Snapshot projection | `lw.braid.flow.snapshot.v0` |
| Proof bundle | `lw.braid.flow.proof.v0` |
| Static expansion patch | `lw.braid.flow.patch.v0` |
| Cache key | `lw.braid.flow.cache.v0` |

`braid_ir::Cid` remains the sole representation. Newtypes may prevent domain
confusion but may not implement another hash or identity contract. Flow CID is
source-order independent and semantic. Plan CID additionally binds the target
profile, cache manifest, planner version, immutable snapshot, and selected
step.

### D-FLOW.4 — Closed v0 graph — LOCKED

The normalized node kinds are `InvokeCapsule`, `Choice`, `JoinAll`, and
`Terminal`. `MapStatic` exists only in the source AST and must expand to the
closed normalized graph before encoding. Edges are typed `Data` or explicit
`After` edges. v0 rejects arbitrary cycles, runtime-unbounded expansion,
`JoinAny`, races, cancellation, and learned trusted scheduling.

### D-FLOW.5 — Source and wire forms — LOCKED

The ordinary Rust builder and RON are first-class authoring forms. Both lower
to the same typed source AST. Normalized JSON exists only for external
interoperability and inspection. YAML exists only behind strict importers such
as the GitHub Actions adapter. Neither RON, JSON, nor YAML bytes carry Flow
identity; Braid's deterministic binary encoding remains the canonical wire.

RON handling is constrained as follows:

1. The source schema carries an explicit version and rejects unknown or
   duplicate fields, implicit environment interpolation, includes, aliases,
   source above 16 MiB, and nesting deeper than 64 before semantic AST
   allocation. Declared collection bounds are checked incrementally before
   reserving node, edge, port, or expansion storage.
2. Comments, whitespace, field order, optional commas, and other accepted RON
   spelling differences are non-semantic. Equal validated ASTs produce equal
   canonical bytes and Flow CIDs.
3. The SDK may emit one stable pretty-RON projection for review, but those text
   bytes are not canonical and are never hashed.
4. Core IR and verifier crates do not depend on a textual parser. RON/JSON/YAML
   parsing belongs in `braid-flow-sdk`, CLI, or importer boundaries.

This is a Flow-specific amendment to D6/D19: existing capsule CLI JSON input is
unchanged; Flow gains RON because the Director explicitly ratified it here.

### D-FLOW.6 — Readiness and effects — LOCKED

Satiation is evaluated before invocation. `Unknown` fails closed and every
proof is snapshot-bound. Effect cache and retry eligibility are derived from
the existing capsule and term-registry semantics, never from a Flow-authored
boolean. Braid emits at most the trusted next step in sequential v0; it does
not execute effects or persist lifecycle state.

### D-FLOW.7 — Architecture-document reconciliation — LOCKED

The graph DSL's `statechart` and `orchestration` syntax are noncanonical
authoring projections. Inter-capsule constructs must lower to Flow; bounded
intra-capsule state remains capsule behavior. Any durable snapshot, journal,
resumption, compensation, or receipt behavior belongs to forge-harness. The
Frontier Flow specification supersedes conflicting durable-runtime language in
`BRAID-GRAPH-DSL.md` and `BRAID-DSL-STATE-AND-SUBSTRATE.md`.

### D-FLOW.8 — Crate ownership — LOCKED

- `braid-flow-ir`: canonical graph shape, encoding, domains, and semantic CID.
- `braid-flow-verify`: independent decoding and static admission.
- `braid-flow-plan`: deterministic snapshot-bound satiation/frontier result.
- `braid-flow-sdk`: Rust/RON authoring, JSON interoperability, and diagnostics.
- `braid-render`: deterministic Flow manifest and full DOT projection.
- `braid-project`: may consume admitted Flows after P2 but owns neither Flow
  admission nor cross-capsule execution.
- `braid-run`: executes one admitted capsule selected by a plan step.

No public `Receipt`, `Capability`, `Verdict`, `Principal`, `Fact`, or
`WorkObject` authority may be introduced by Flow crates.

### D-FLOW.9 — Implementation order — LOCKED

P1–P6 form a dependency DAG and are independently reviewable:

1. [Braid #57](https://github.com/srinji-kaggss/Braid/issues/57) — Flow IR.
2. [Braid #60](https://github.com/srinji-kaggss/Braid/issues/60) — independent verification.
3. [Braid #59](https://github.com/srinji-kaggss/Braid/issues/59) — frontier planning.
4. [Braid #58](https://github.com/srinji-kaggss/Braid/issues/58) — RON SDK, interoperability, rendering, and CI import.
5. [forge-harness #123](https://github.com/srinji-kaggss/forge-harness/issues/123) — durable runtime adapter.
6. [experience-as-code #66](https://github.com/srinji-kaggss/experience-as-code/issues/66) — domain integration and performance proof.

No implementation issue may absorb a later authority boundary merely because
its dependency is unfinished.

The dependency edges are P1 -> P2 -> P3; P3 then forks to P4 and P5, which
both join at P6. P4 and P5 are siblings: the Forge runtime adapter depends on
the admitted planner contract, not on the authoring SDK.

## Required architecture checks

| P0 check | Resolution |
| --- | --- |
| `DECISIONS.md` locks | D5/D17/D31 already make Braid the typed IR and translation boundary; D8/D9 preserve canonical encoding and independent verification; D10 forbids new authority. D-FLOW.5 is a Director-ratified, Flow-specific amendment to D6/D19 and does not change the existing capsule JSON CLI. |
| Graph DSL statecharts | No second graph authority: inner computation remains `Braid`; bounded inter-capsule statecharts lower to Flow; unsupported recurrence is rejected. |
| Justified invocation | The thesis becomes normative through the closed predicate AST, three-valued evaluation, satiation-first planning, and snapshot binding in the Flow spec. |
| Rendering | `braid-render` gains deterministic Flow manifest and full DOT projection; no sibling renderer is created. |
| Projects | `braid-project` remains capsule-set-only through P1 and may consume independently admitted Flow after P2; it never admits or executes Flow itself. |
| Performance | AST/RON improves typing and normalization, not execution speed. Speed claims require measured skipped work, safe reuse, precise invalidation, early cutoff, locality, or lower control-plane overhead. |

## Rejected alternatives

- **Canonical Flow authority in experience-as-code:** rejected. Domain meaning
  remains there, but a generic Flow wire would duplicate Braid and prevent
  other domains from sharing one verifier.
- **Durable scheduling in Braid or `braid-run`:** rejected. It would combine
  semantic identity with leases, effects, and crash lifecycle in one trust
  surface.
- **RON text as the canonical wire:** rejected. Equivalent legal text must not
  create identity aliases; only normalized deterministic Braid bytes are
  hashed.
- **JSON or YAML as the preferred Flow source:** rejected. JSON remains
  interoperability-only and YAML importer-only.
- **A sibling Flow renderer:** rejected for v0. Projection changes on the same
  lifecycle and failure axis as existing `braid-render`.

## Consequences

- Braid gains a canonical outer graph without becoming a durable scheduler.
- RON is ergonomic and first-class, while equivalent RON spellings cannot
  fork semantic identity.
- JSON remains available to tools that cannot consume RON but is no longer the
  preferred Flow authoring surface.
- `braid-render` owns full graph projection, avoiding a presentation-only
  sibling crate.
- The existing capsule JSON contract remains intact; Flow does not silently
  rewrite D19.
- Parallel execution, fixed-point cycles, runtime expansion, proof solver
  completeness beyond the bounded v0 disjointness fragment, materialization of
  satiated outputs, and performance targets remain explicit debts rather than
  guessed v0 behavior. The bounded fragment returns typed `Unknown` for
  unsupported distinct-reference relations or resource exhaustion, and static
  admission fails closed.
- ADR-099 must be added to the constellation governance index in a
  logic-os-kernel-owned change; this Braid change does not mutate another
  repository's governance authority.

## Registration

This ADR is indexed by `spec/braid/README.md` and its stable decisions are
mirrored in `spec/braid/DECISIONS.md`. Constellation-wide index registration is
tracked in `spec/braid/DEBT_REGISTER.md` until the owning repository accepts
the cross-repo update.

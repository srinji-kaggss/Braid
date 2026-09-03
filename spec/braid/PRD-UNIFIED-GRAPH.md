# PRD — Unified Program Graph

**Status:** candidate
**Owner:** Braid
**Primary objective:** replace competing graph authorities with one canonical program graph that deterministically yields a pristine CFG and execution DAG.

## Product requirement

Braid must represent a program once and derive control flow, data dependencies, resource conflicts, scheduling, rendering, and verification from that same representation.

## Users

- AI author: emits a bounded graph/data artifact and receives typed refusals.
- Human reviewer: audits CFG/DAG/effect/resource views without reading executor code.
- Verifier: independently proves structural, type, capability, effect, taint, and bounds invariants.
- Planner/runtime: consume graph projections; they do not reconstruct hidden topology.

## Functional requirements

### R1 — one canonical node domain
All operations use stable `NodeId`; list position is serialization order only.

### R2 — explicit control graph
Branches, joins, terminals, and completion-conditioned ordering are CFG edges.

### R3 — explicit dependency graph
Typed value edges are first-class. Resource/effect ordering is derived from declared access contracts.

### R4 — deterministic projections
`cfg()` and `dag()` are stable for equal graph content and do not depend on insertion/hash-map order.

### R5 — bounded construction
Node/edge/port/resource counts and identifiers are bounded before expensive allocation or graph traversal.

### R6 — fail-closed validation
Unknown node kinds, edge kinds, ports, resource access kinds, completion classes, and graph versions refuse.

### R7 — legacy compatibility
Current `Braid` and `FlowSpec` remain readable during migration. Their adapters must produce the same graph every time and must not change existing CIDs.

### R8 — one scheduling source
Planner and runtime use the DAG projection. No second predecessor map or hidden dependency reconstruction is allowed after migration.

### R9 — one control source
Choice targets and terminal reachability are represented in CFG edges rather than only embedded node payloads.

### R10 — effect/resource contracts
Term/system metadata gains stable resource-qualified read/write/append declarations before parallel scheduling is enabled.

### R11 — typed receipts
Effectful execution eventually yields receipts bound to node, effect, resource/object, attempt, and result.

### R12 — human renderability
Renderer exports at least:
- canonical node/edge manifest;
- CFG DOT;
- DAG DOT;
- resource/effect matrix;
- widening diff.

## Non-functional requirements

- `no_std` compatible kernel where existing substrate requires it.
- No unsafe code.
- No new third-party graph runtime required.
- No hidden ambient authority.
- O(V+E) projection/validation target for ordinary graph passes.
- Deterministic ordering via stable IDs/BTree collections or explicit sort.

## Acceptance scenarios

1. Legacy three-strand capsule converts to one `ProgramGraph`; DAG edges exactly match strand input dependencies.
2. Flow choice converts to CFG branch edges; branch targets are visible without parsing node internals.
3. Same graph built in different insertion orders renders identical CFG/DAG sequences.
4. Data cycle is rejected.
5. Control cycle is rejected in graph version 0.
6. Missing source/target node is rejected.
7. Duplicate node ID is rejected.
8. Duplicate semantic edge is rejected.
9. A write/read conflict over one resource yields an ordering dependency once resource contracts land.
10. Two read-only systems over the same resource remain independent.
11. Existing capsule and flow CIDs are unchanged by adding the compatibility kernel.
12. Planner no longer contains its own topology reconstruction after migration.
13. Runtime cannot call an effectful host operation without the graph/registry-declared effect contract.
14. Renderer derives CFG/DAG from the same graph object consumed by verification.

## Delivery phases

- **M0:** candidate architecture + PRD + migration plan.
- **M1:** canonical graph kernel and deterministic CFG/DAG projections.
- **M2:** Braid adapter and differential tests.
- **M3:** Flow adapter and differential tests.
- **M4:** verifier convergence.
- **M5:** planner convergence.
- **M6:** runtime/resource contracts + receipts.
- **M7:** versioned canonical ProgramGraph wire and legacy authority deletion.

## Definition of done

There is one graph kernel in Braid and every graph-aware crate either consumes it or is explicitly a temporary compatibility adapter scheduled for deletion. No runtime/verifier/planner independently re-derives topology from a different model.

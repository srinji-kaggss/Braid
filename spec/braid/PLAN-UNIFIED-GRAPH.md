# One Refactor Plan — Unified ProgramGraph

**Branch:** `refactor/unified-program-graph`
**Rule:** one migration spine, no parallel architecture.

## Objective

Collapse Braid's duplicated graph semantics into one kernel that deterministically exposes:

- a **CFG** for legal control transitions;
- a **DAG** for execution dependencies;
- later, resource/effect conflict edges from declared access contracts.

## Current duplication to remove

1. `braid-ir::Braid` stores intra-capsule term DAG by topological index.
2. `braid-flow-ir::FlowSpec` stores inter-capsule nodes, data edges, After edges, and hidden Choice successors.
3. `braid-flow-plan` rebuilds a predecessor graph from Flow edges + Choice arms.
4. `braid-run` separately walks Strand indices.
5. Two verifier families validate sibling graph shapes.

## Refactor sequence

### 1. Introduce kernel in `braid-ir`
Create `graph.rs` with:
- `NodeId`
- `GraphNode`
- `GraphNodeKind`
- `GraphPort`
- `GraphEdge::{Data, Control}`
- `ProgramGraph`
- `ControlFlowGraph`
- `DependencyDag`

M1 deliberately excludes resource edges from canonical state until access vocabulary is ratified. The type layout leaves a single extension point rather than inventing semantics.

### 2. Make graph validation one pass family
Kernel validation checks:
- unique IDs;
- endpoints exist;
- no self edges;
- no duplicate semantic edges;
- v0 CFG acyclic;
- data DAG acyclic;
- stable canonical ordering of projected nodes/edges.

### 3. Add legacy Braid adapter
`impl From<&Braid> for ProgramGraph` or an explicit fallible conversion:
- strand `i` -> `NodeId(i)`;
- each `inputs[slot]` -> Data edge producer → consumer slot;
- outputs retained as graph outputs.

No existing `Braid`, Capsule bytes, CIDs, or verifier behavior changes in this step.

### 4. Add Flow adapter
Map:
- `FlowNode` -> GraphNode;
- Data -> Data edge;
- After -> Control edge;
- Choice arms/otherwise -> explicit Control edges.

This is the key elimination of hidden topology.

### 5. Differential fixtures
For existing fixtures:
- old validation result vs kernel adapter validation result;
- old predecessor relations vs `cfg()`/`dag()` relations;
- insertion-order permutations produce identical views.

### 6. Converge verification
Move graph structural checks behind one kernel verification API. Independent byte decoders remain separate, but once decoded they target the same semantic graph.

### 7. Converge planning
Delete the hand-built predecessor reconstruction in `braid-flow-plan`. Planner consumes `ProgramGraph::cfg()` and `ProgramGraph::dag()`.

### 8. Converge execution
`braid-run` consumes dependency DAG ordering. Sequential execution remains the initial policy. Parallel waves are forbidden until resource access contracts are implemented and exercised.

### 9. Add resource/effect contracts
After kernel convergence, extend registry/system descriptors with stable resource keys and access modes. Derive conflict edges into DAG. Host effects return typed receipts.

### 10. Versioned wire migration
Only after adapters and KATs prove semantics:
- define ProgramGraph canonical bytes;
- bump appropriate graph IR version;
- preserve legacy decoders;
- delete Flow/Braid as authorities after downstream migration.

## Hard stop conditions

Stop/refuse expansion if this branch starts doing any of the following:

- rewriting vocabulary crates;
- replacing the canonical serializer;
- adding a graph database;
- adding an ECS dependency;
- changing existing Capsule/Flow CIDs;
- parallel execution before resource contracts;
- moving unrelated `lgwks_std`/bot code.

## Merge gate for this PR

This PR is mergeable only if it delivers M0+M1 as a coherent architectural seam:

- candidate architecture, PRD, and this single plan;
- graph kernel added to `braid-ir`;
- deterministic CFG/DAG projections;
- unit tests for uniqueness, endpoint validity, duplicate edge rejection, CFG cycle rejection, DAG cycle rejection, and deterministic ordering;
- no existing wire/CID changes;
- follow-up migration work explicitly remains unclaimed.

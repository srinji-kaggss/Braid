# Braid Candidate Architecture — Unified ProgramGraph

**Status:** candidate for ratification in the unified-graph refactor PR.
**Scope:** replace competing graph authorities with one canonical graph kernel while preserving existing capsule, flow, verifier, and vocabulary semantics during migration.

## 1. Problem

Braid currently has two independent graph ontologies:

1. `braid-ir::Braid` / `Strand` — an intra-capsule topologically ordered DAG of term invocations.
2. `braid-flow-ir::FlowSpec` / `FlowNode` / `FlowEdge` — an inter-capsule orchestration graph with control, choice, data, and terminal semantics.

The planner then re-derives predecessor/reachability structure from `FlowEdge` and `Choice` arms, while `braid-run` separately interprets `Strand.inputs`. As a result, “the graph” is not one object with one ownership model. Structure, control flow, dataflow, and effect scheduling are split between crates.

The failure mode is architectural drift: two representations can each be locally valid while disagreeing about the actual program dependency structure.

## 2. Decision

Braid shall have exactly one canonical graph kernel: `ProgramGraph`.

`ProgramGraph` is neutral to authoring syntax and execution host. It contains stable nodes and explicit typed edges. Existing capsule and flow forms become projections/adapters over this kernel during migration.

The kernel exposes two deterministic graph views:

- **CFG** — control-flow graph: ordering, branching, completion-conditioned transitions, terminals.
- **DAG** — dependency graph: typed value flow plus declared resource/effect dependencies.

Neither CFG nor DAG is a second wire format. Both are deterministic views over the same `ProgramGraph` bytes/data.

## 3. Canonical entities

```text
ProgramGraph
  nodes: Node[]
  edges: Edge[]
  roots: Port[]
  outputs: Port[]

Node
  id: NodeId
  op: Operation
  guard: Predicate?
  policy: NodePolicy

Operation
  InvokeTerm(term_id)
  InvokeCapsule(cid)
  Choice
  Join
  Terminal(outcome)

Edge
  Data { from: OutputPort, to: InputPort, type }
  Control { from: NodeId, to: NodeId, on: CompletionSet }
  Resource { from: NodeId, to: NodeId, resource: ResourceKey, access: AccessKind }
```

`Resource` edges are derived from declared system access contracts, not hand-authored scheduling hints.

## 4. Invariants

### G1 — one node identity domain
Every executable/control node has one stable `NodeId` inside one graph. Array position is never semantic identity.

### G2 — CFG is explicit
Control dependencies are represented as control edges. Choice successors are not hidden inside node payloads without corresponding CFG edges.

### G3 — data DAG is explicit
All value dependencies are typed edges. An invocation may not obtain an upstream value not represented by a data edge.

### G4 — resource dependencies are declared
Effectful systems declare resource access. Conflicts are derived mechanically:

- Read + Read: independent.
- Read + Write: ordered/conflicting.
- Write + Write: ordered/conflicting.
- Append semantics are registry-defined and may be commutative only when explicitly declared.

### G5 — graph views are deterministic
For identical canonical graph bytes, CFG and DAG projections are byte/order stable.

### G6 — no hidden scheduler semantics
Planner/runtime may select or execute nodes, but may not invent dependency edges that are absent from the canonical graph/access declarations.

### G7 — one verifier family
Existing independent decoding/admission remains mandatory, but `braid-verify` and `braid-flow-verify` must converge onto verification of the same graph kernel rather than indefinitely validating sibling graph models.

### G8 — effects return receipts
A host call for an effectful operation must eventually return a typed effect receipt that binds operation, resource/object identity, attempt, and result. A returned value alone is insufficient evidence of an external mutation.

## 5. Crate target

Target dependency direction:

```text
braid-capability
      ↓
braid-ir
  └── ProgramGraph + type/value/CID/term registry
      ↓
braid-verify
      ↓
braid-plan
      ↓
braid-run

frontends/vocabularies/render/project
      ↓
  adapters only
```

Temporary migration crates (`braid-flow-ir`, `braid-flow-verify`, `braid-flow-plan`) may remain until their APIs are projected onto the kernel, but they are not long-term authorities.

## 6. CFG projection

`ProgramGraph::cfg()` returns only control edges and node kinds relevant to control.

Properties:

- explicit entry roots;
- explicit terminal nodes;
- branch targets represented as edges;
- completion class on transitions;
- cycle policy is explicit: v0 remains acyclic unless a future version introduces bounded loops as a first-class operation.

## 7. DAG projection

`ProgramGraph::dag()` returns dependencies that constrain execution:

1. data edges;
2. resource-conflict edges derived from declared access;
3. mandatory control-predecessor edges where execution cannot legally occur before control release.

The DAG is the source for deterministic scheduling waves. CFG answers “where may control go?” DAG answers “what must be available/ordered before this node can execute?”

## 8. Candidate system contract

```text
SystemContract
  reads: ResourceKey[]
  writes: ResourceKey[]
  appends: ResourceKey[]
  consumes: EventType[]
  emits: EventType[]
  capability: Capability?
  effect: EffectClass
  idempotency: IdempotencyClass
  receipt: ReceiptType?
```

For pure terms, all resource/effect fields are empty/None.

## 9. Migration

### Phase A — kernel introduced
- Add `ProgramGraph`, `NodeId`, typed `GraphEdge`, CFG/DAG projections.
- No wire change to existing Capsule/Flow yet.
- Add adapters from `Braid` and `FlowSpec` into `ProgramGraph`.

### Phase B — verifier convergence
- `braid-verify` verifies kernel structural/type/effect invariants.
- `braid-flow-verify` becomes compatibility verification over the same kernel projection.

### Phase C — planner/runtime convergence
- Planner consumes CFG/DAG from `ProgramGraph`; no hand-built predecessor reconstruction.
- Runtime executes DAG waves or deterministic sequential fallback from the same projection.

### Phase D — canonical wire migration
- Introduce a versioned ProgramGraph wire only after compatibility KATs and migration vectors exist.
- Old capsule/flow bytes remain decodeable as versioned legacy forms.

### Phase E — delete duplicate authorities
- Remove legacy graph validation/planning code only after exact differential fixtures prove equivalence for supported semantics.

## 10. Non-goals

- No `bevy_ecs` dependency.
- No new database.
- No new capability system.
- No new canonical serializer in this refactor.
- No claim that parallel execution is safe until resource contracts are implemented and exercised.
- No silent reinterpretation of existing CIDs.

## 11. Success criterion

A reviewer can point to one canonical object and answer all four questions without opening a second graph model:

1. What operations exist?
2. Where can control flow?
3. What data/resources does each operation depend on?
4. What effects can occur and under what authority?

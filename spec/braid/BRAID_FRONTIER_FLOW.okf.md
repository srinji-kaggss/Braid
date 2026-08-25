---
type: Implementation Specification
title: Braid Frontier Flow
description: Canonical AST, verifier, and deterministic frontier-planning contract for justified inter-capsule orchestration.
resource: https://github.com/srinji-kaggss/Braid
tags:
  - braid
  - orchestration
  - graph
  - ast
  - rust
  - ci
  - incremental-computation
timestamp: 2026-08-25T00:00:00-04:00
okf_version: "0.1"
status: accepted
concept_id: spec/braid/BRAID_FRONTIER_FLOW
source_commit: 111f9f8abcf43b05d134ccd89f05ae91acc2b4ef
ratification: docs/adr-099-braid-frontier-flow.md
authority:
  canonical_flow_ir: Braid
  static_admission: Braid
  durable_execution: forge-harness
  domain_reconciliation: experience-as-code
codex:
  continuation_contract: true
  implementation_order: [P0, P1, P2, P3, P4, P5, P6]
  default_language: Rust
---

# Braid Frontier Flow

> **Status: accepted by ADR-099; P1 is the next implementation unit.**
>
> P0 was ratified on 2026-08-25 through Issue #56 and ADR-099. Public v0 wire
> domains and authority boundaries are frozen by that ADR.

## 1. Decision

Braid owns a canonical **inter-capsule Flow IR** and its independent
static verifier. The Flow IR sits one level above the existing Braid
`Capsule -> Braid -> Strand` compute graph:

```text
Rust AST / RON source / strict importer
                         |
                         v
             canonical Braid Flow IR
                         |
             independent static verifier
                         |
          deterministic frontier planner
                         |
        admitted flow + content-addressed plan
                         |
          forge-harness durable execution
                         |
      braid-run executes each admitted capsule
```

The placement decision is deliberate:

- **Braid** owns the canonical graph language, encoding, content identity,
  static graph proofs, and deterministic plan derivation.
- **`braid-run`** remains the low-level interpreter for one admitted capsule.
  It MUST NOT become a durable multi-run scheduler.
- **forge-harness** owns durable orchestration: leases, event history, retries,
  crash recovery, worker selection, execution receipts, and resumption.
- **experience-as-code** owns Terraform-like domain declaration,
  plan/apply/reconcile semantics, and direct experience observation. It MAY
  compile a domain plan into Braid Flow; it MUST NOT become the generic Flow
  authority.
- **`lgwks_bot`** remains a small leaf-automation library. A compatibility
  compiler MAY lower `BotSpec` chains into Flow nodes, but `tick()` MUST NOT be
  expanded into a second scheduler.

There are two different meanings of “planner,” and they MUST stay separate:

- a generative planner in Forge or an authoring model decides **which graph to
  propose** and emits a `FlowSpec`;
- Braid's trusted frontier planner decides **which already-declared, admitted
  node is next** under an explicit immutable snapshot.

Braid never decomposes a prose goal into work. Forge never bypasses Braid's
deterministic admission and next-step derivation.

A future ergonomic façade MAY be called `lgwks_flow`, but v0 MUST NOT freeze
that public crate name. The canonical protocol names are `braid.flow.*`.

## 2. Reality check: where the speed can actually come from

Lowering Rust, RON, and strict importers into one typed AST improves typing,
normalization, refusal quality, and machine authoring. It does **not**, by
itself, make CI materially faster.
GitHub Actions already represents dependencies through jobs and `needs`; the
expensive parts are usually runner acquisition, checkout, process/container
startup, dependency transfer, repeated environment construction, and work that
was not skipped.[1][2]

For a strictly sequential executor where every node must run, graph scheduling
cannot beat the sum of all node durations. The graph becomes faster only when it
does at least one of the following:

1. proves a node is already satisfied and performs no work;
2. reuses a content-addressed result safely;
3. invalidates only the reverse dependency cone of a changed input;
4. stops invalidation propagation when a recomputed value is unchanged;
5. shares identical in-flight work;
6. avoids runner/process/container boundaries;
7. moves external waits off the active worker;
8. reaches a meaningful failure earlier without changing semantics.

Therefore the normative performance claim is:

> **Braid Frontier Flow minimizes justified work and control-plane overhead. It
> does not claim to make identical mandatory compute disappear.**

Any comparison with GitHub Actions MUST separate queueing, runner provisioning,
checkout, control-plane overhead, and command execution. A local self-hosted
Flow run MUST NOT be compared with a cold hosted runner and presented as proof
of a superior graph algorithm.

## 3. Existing substrate and the exact missing layer

Braid already has:

- an inner `Braid` whose strands are stored in topological order and may only
  reference earlier strands;
- an independent verifier and content-addressed capsule;
- `braid-run`, which evaluates every strand sequentially and journals the
  result;
- `braid-project`, which admits a named set of capsules independently and
  computes a project CID;
- an approved graph DSL direction with statecharts and hierarchical subgraphs;
- the justified-invocation thesis: an action must be safe, authorized, and
  presently justified, and doing nothing must be able to win.

The missing layer is **wiring among admitted capsules**. `braid-project`
currently aggregates capsules but intentionally creates no dependencies and
pools no authority. `braid-run` executes one capsule but has no durable
cross-capsule frontier. `lgwks_bot` evaluates fixed condition/action chains but
explicitly is not an orchestration engine.

The new layer MUST extend these surfaces rather than duplicate them.

## 4. Non-goals

v0 is not:

- a replacement for `braid-ir::Cid`;
- a new capability envelope, principal, verdict, receipt, fact, work-object,
  or policy authority;
- a general distributed runtime inside Braid;
- a natural-language planner;
- an LLM-selected scheduler;
- a weighted utility maximizer;
- unrestricted Petri nets, arbitrary cancellation, arbitrary recursion, or
  Turing-complete guard expressions;
- a promise to reproduce every GitHub Actions behavior;
- a shell-script packaging format;
- a reason to add Tokio, a background reactor, or a worker pool to Braid core;
- a reason to make `braid-run` aware of repositories, pull requests, CI
  vendors, or durable job state.

## 5. Stable conceptual model

### 5.1 Two graph levels

**Inner graph — capsule compute**

```text
Capsule
  -> Braid
     -> [Strand 0, Strand 1, ...]
```

The inner graph remains optimized for typed, admitted, sequential strand
evaluation.

**Outer graph — Flow**

```text
Flow
  -> Nodes reference admitted capsule CIDs or closed control primitives
  -> Edges carry typed data or explicit precedence
  -> Static verifier proves structural and policy invariants
  -> Planner derives a deterministic frontier schedule
```

The outer graph MUST NOT inline or rewrite an admitted capsule. A capsule CID in
a Flow MUST equal its standalone CID. This preserves non-aggregation of
authority and makes cache identity compositional.

### 5.2 Canonical concept identifiers

The following identifiers are ratified protocol domains, not necessarily Rust
type names:

| Concept | Domain separator |
|---|---|
| Flow semantic graph | `lw.braid.flow.v0` |
| Flow node | `lw.braid.flow.node.v0` |
| Flow plan | `lw.braid.flow.plan.v0` |
| Flow snapshot projection | `lw.braid.flow.snapshot.v0` |
| Flow proof bundle | `lw.braid.flow.proof.v0` |
| Flow static expansion patch | `lw.braid.flow.patch.v0` |
| Flow cache key | `lw.braid.flow.cache.v0` |

All hashes MUST use the existing `braid_ir::Cid` representation with a distinct
domain separator. No `FlowCid` implementation may create a second CID
authority; a Rust newtype MAY wrap `Cid` only to prevent domain confusion.

### 5.3 Semantic graph identity and plan identity are different

`Flow CID` identifies what the workflow means:

```text
nodes + ports + edges + guards + justification declarations
+ terminal semantics + declared bounds
```

`Plan CID` identifies one deterministic **partial** execution plan for that
graph under a specific, explicit planning context:

```text
flow CID + planner version + target profile CID + snapshot CID
+ cache manifest CID + selected step(s)
```

The default v0 plan contains all zero-work satiation transitions reachable under
the snapshot plus at most one executable node. A complete static schedule may be
emitted only for a pure subgraph whose relevant snapshot cannot change.
Material state transitions require a new snapshot and a new Plan CID.

Execution order, worker topology, and cache availability MUST NOT silently alter
the Flow CID. Conversely, a plan MUST NOT be reused under a different snapshot
or target profile merely because the Flow CID is unchanged.

## 6. Source forms and AST-first authoring

v0 MUST support three paths into the same typed source AST:

1. an ordinary Rust builder/AST for trusted code generation and IDE use;
2. RON as the first-class textual authoring and review form;
3. normalized JSON strictly for machine-to-machine interoperability and
   inspection.

The canonical wire form remains Braid's deterministic binary encoding. RON and
JSON are decoded, normalized, and validated before encoding; their source bytes
never carry Flow identity.

The RON schema MUST carry an explicit version and reject unknown or duplicate
fields, implicit environment interpolation, includes, aliases, and inputs that
exceed the v0 source envelope: at most 16 MiB of source bytes and nesting depth
64. These limits are checked before constructing the semantic AST; declared
node, edge, port, and expansion bounds are then enforced incrementally before
reserving their collections. Comments, whitespace, field order, optional
commas, and any other accepted spelling difference are non-semantic: if two
RON documents decode to the same validated AST, they MUST produce identical
canonical bytes and Flow CID. The SDK MAY emit one stable pretty-RON projection
for review, but it MUST NOT hash that text.

A future textual Flow DSL MAY lower to the same AST. YAML MAY be imported by a
strict adapter, but MUST never become the preferred authoring or canonical
representation. Every importer emits a semantic-loss report and refuses
unsupported semantics.

Procedural macros are deferred. Ordinary Rust types, serde-derived source
structures, and builders are easier to audit, cheaper to compile, and less
likely to create a second hidden language. Text parsing belongs in the SDK/CLI
boundary; the core IR and independent verifier do not depend on RON or JSON.

## 7. Normalized graph model

Let a Flow be a typed directed graph:

\[
F = (V, E_d, E_c, R, T)
\]

where:

- \(V\) is the finite node set;
- \(E_d\) is the typed data-edge set;
- \(E_c\) is the explicit control/precedence-edge set;
- \(R\) is the finite set of root inputs;
- \(T\) is the non-empty terminal set.

Source declaration order is not semantic. Canonical encoding MUST sort nodes by
stable key and edges lexicographically. A plan contains the derived topological
order.

### 7.1 Node kinds

v0 canonical normalized IR MUST contain this closed set:

```rust
pub enum FlowNodeKind {
    /// Executes one already-admitted Braid capsule.
    InvokeCapsule {
        capsule: Cid,
    },

    /// Selects exactly one branch using total, verified predicates.
    Choice {
        arms: Vec<ChoiceArm>,
        otherwise: NodeKey,
    },

    /// Waits for every declared predecessor token.
    JoinAll,

    /// Declares successful or failed logical completion.
    Terminal {
        outcome: TerminalOutcome,
    },
}
```

The source AST MAY additionally contain `MapStatic`. The compiler expands it
before canonical Flow encoding, so the identity-bearing IR contains ordinary
nodes and edges only:

```rust
pub struct MapStaticSource {
    pub template: FlowTemplateRef,
    pub items: Vec<CanonicalValue>,
    pub max_items: u32,
}
```

Notes:

- `Observe` is not a privileged node kind. Observation is an admitted capsule
  whose effect and capability requirements are already registered.
- `Gate` is represented by node justification and edge predicates.
- `JoinAny`, race, cancellation, and first-completer semantics are rejected in
  v0. They create schedule-sensitive behavior and sharply complicate soundness.
- `MapStatic` items MUST be present at compile/plan time and sorted by their
  canonical encoded bytes before expansion.
- Runtime-discovered expansion is deferred to v1 and requires a bounded,
  content-addressed patch protocol.

### 7.2 Stable node keys

A source-level `NodeKey` is a validated UTF-8 symbol:

```text
[a-z][a-z0-9_.-]{0,127}
```

The key is unique within a Flow and is used for diagnostics and edges. The
node's semantic CID is derived from its canonical definition. Renaming a node
changes the Flow CID because diagnostics, policy references, and imported CI
identity may depend on the stable key.

### 7.3 Edge kinds

```rust
pub enum ValueSource {
    Root(InputKey),
    Node(OutputPort),
    Literal(CanonicalValue),
}

pub enum FlowEdge {
    Data {
        from: ValueSource,
        to: InputPort,
        value_type: TypeTag,
    },
    After {
        from: NodeKey,
        to: NodeKey,
        on: CompletionClass,
    },
}
```

Data edges are the only input-binding mechanism. Exactly one `Data` edge MUST
bind each required capsule input; a duplicate or missing binding is rejected.
A node-origin data edge implies precedence from its output owner. Root and
literal sources bind values without introducing node precedence.

`CompletionClass` is closed and explicit:

```rust
pub enum CompletionClass {
    ExecutedSuccess,
    SatisfiedWithoutExecution,
    Failure,
}
```

A downstream node MAY accept more than one completion class, but this choice is
part of the semantic Flow. A failure MUST never be silently converted to
success or to a cache hit.

Data edges imply precedence. Control edges add precedence without data. Derived
resource conflicts are planning facts and MUST NOT be encoded as fake semantic
dependencies.

### 7.4 Synthetic start and finish

The verifier normalizes every Flow to one synthetic start and one synthetic
finish for soundness analysis. These synthetic nodes are not user-addressable
and do not change the source-level graph meaning.

Every declared node MUST lie on at least one path from start to a terminal.
Dead declarations are rejections, not warnings.

## 8. Guard and predicate AST

Trusted predicates MUST be finite, total, deterministic, and side-effect free.

```rust
pub enum Predicate {
    Const(bool),
    Eq(ValueExpr, ValueExpr),
    Ne(ValueExpr, ValueExpr),
    Lt(ValueExpr, ValueExpr),
    Le(ValueExpr, ValueExpr),
    Gt(ValueExpr, ValueExpr),
    Ge(ValueExpr, ValueExpr),
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
    HasCompletion {
        node: NodeKey,
        class: CompletionClass,
    },
}
```

```rust
pub enum ValueExpr {
    Literal(CanonicalValue),
    RootInput(InputKey),
    NodeOutput(OutputPort),
    SnapshotFact(FactRef),
}
```

Rules:

- no arbitrary function calls;
- no wall clock, randomness, environment reads, file reads, network reads, or
  mutable globals;
- no regex in v0;
- no unbounded quantifiers;
- maximum AST depth: 32;
- maximum predicate nodes per Flow: 16,384;
- every value expression is typed before canonical encoding;
- comparisons over incompatible types are rejected;
- empty `And` and `Or` are rejected rather than assigned surprising identities;
- branch predicates MUST be proven pairwise disjoint, or the `Choice` is
  rejected;
- `otherwise` is mandatory, making choice total.

If disjointness cannot be established by the bounded verifier, the result is
`Unknown`, which blocks admission. Source order MUST NOT resolve overlapping
branches.

## 9. Justified invocation is part of readiness

A Flow node is not ready merely because its predecessors completed.

Each effectful or materially expensive `InvokeCapsule` MUST declare or inherit:

```rust
pub struct JustificationDecl {
    pub needed_when: Predicate,
    pub satisfied_when: Predicate,
    pub guarantees: Vec<RelationRef>,
    pub preserves: Vec<InvariantRef>,
    pub cost_order: Option<CostOrderRef>,
}
```

These fields mean:

- `needed_when`: exact predicate showing the presently unsatisfied reason to
  consider the action;
- `satisfied_when`: exact predicate defining enough;
- `guarantees`: registered proof obligations for the post-state relation;
- `preserves`: higher-order invariants that may not regress;
- `cost_order`: optional deterministic ordering among actions proven to reach
  an equivalent satisfactory region.

Free-form rationales MAY accompany diagnostics but are never admission
evidence.

### 9.1 Three-valued evaluation

Every trusted predicate or proof obligation returns:

```rust
pub enum ProofState {
    Proven,
    Disproven,
    Unknown(MissingEvidence),
}
```

Only `Proven` may authorize execution. `Unknown` fails closed.

### 9.2 Satiation precedes action

For node \(v\) and immutable snapshot \(S\), planning evaluates:

```text
1. satisfied_when(v, S)
   Proven    -> SatisfiedWithoutExecution
   Unknown   -> BlockedUnknown
   Disproven -> continue

2. needed_when(v, S)
   Proven    -> candidate for Ready frontier
   Unknown   -> BlockedUnknown
   Disproven -> Dormant; wait for a new snapshot
```

This ordering makes **doing nothing a first-class successful result**. A node
that is already satisfied releases downstream control dependencies through the
`SatisfiedWithoutExecution` completion class and emits proof evidence, but it
does not fabricate an execution receipt.

Satiation does not magically invent data outputs. If downstream data edges read
the node's outputs, the satisfaction proof MUST also bind every demanded output
port to a canonical existing value or materialized artifact CID. A cache hit is
valid only when its artifacts are present and verified. Without those output
bindings, the node may satisfy control-only successors, while data-dependent
successors remain blocked.

### 9.3 Snapshot binding and stale-proof rejection

Every readiness proof MUST bind to a canonical snapshot CID or version. Before
dispatch, the runtime MUST prove that the relevant snapshot still matches. A
state change invalidates the plan step and requires re-planning.

```text
proof(snapshot N) + execute(snapshot N+1) = reject
```

No runtime may reuse a justification proof merely because the node and Flow CIDs
are unchanged.

### 9.4 Guarantees are not probabilities

`guarantees` and `preserves` reference registered, bounded proof procedures or
runtime evidence checks. They MUST NOT contain confidence scores, model
probabilities, embeddings, or prose assertions.

An authoring model may propose the declaration. It may not decide whether the
declaration is true.

## 10. Frontier semantics

For a fixed admitted Flow \(F\) and immutable snapshot \(S\), define the ready
frontier:

\[
\mathcal{R}(F,S) =
\{v \in V \mid Pending(v) \land PredsSatisfied(v,S)
\land GuardProven(v,S) \land NeedProven(v,S)\}
\]

Two nodes in the ready set cannot be ancestors of one another under the Flow
reachability order; therefore the ready set forms an antichain. This adapts the
frontier/antichain idea used in timely dataflow progress tracking, where a
frontier is a minimal set of mutually incomparable elements in a partial
order.[8]

The term **frontier** in this specification means the deterministic ready
antichain of Flow nodes. It does not claim wire compatibility with Timely or
Naiad.

### 10.1 Sequential v0

v0 selects at most one node from the ready frontier:

```rust
fn next_step(
    admitted: &AdmittedFlow,
    snapshot: &FlowSnapshot,
    policy: &SchedulePolicy,
) -> Result<PlanStep, PlanError>;
```

Given identical canonical inputs, `next_step` MUST return byte-identical output.

### 10.2 Deterministic ranking

The default ranking is lexicographic, never a weighted score:

```text
1. declared urgency class
2. failure-diagnostic class
3. remaining critical-path upper bound, descending
4. declared execution-cost upper bound, ascending
5. canonical node CID, ascending
```

The initial closed urgency order is:

```rust
pub enum UrgencyClass {
    SafetyRecovery,
    Required,
    Diagnostic,
    Optimization,
    Cleanup,
}
```

Constraints:

- `urgency class` is a closed declaration, not inferred sentiment;
- critical-path and execution-cost values use registered or declared upper
  bounds, not model estimates;
- scheduling cost changes order only; it never permits skipping mandatory work;
- `JustificationDecl::cost_order` is narrower: it may choose among alternative
  actions only after equivalent satisfactory outcomes and preserved invariants
  are proven;
- absent or incomparable cost falls through to the next field;
- the node CID is the mandatory final tie-break;
- iteration order, hash-map order, thread timing, and wall clock may not affect
  selection.

With one worker and all nodes mandatory, this ordering does not reduce total
compute. It exists for repeatability, earlier diagnostics, external-wait
overlap, and a future parallel planner.

### 10.3 Parallel extension

A later planner MAY choose a maximal compatible subset of the ready frontier.
That extension requires:

- explicit resource claims;
- proof that selected pure nodes commute or do not share mutable effects;
- stable maximal-set selection;
- deterministic join semantics;
- confluence tests showing output identity under permitted interleavings.

Parallel execution is not required for v0 correctness.

## 11. Cycles and bounded iteration

### 11.1 v0 rule

The normalized outer Flow MUST be acyclic. Tarjan or Kosaraju SCC analysis MUST
reject every non-trivial strongly connected component and every self-loop.

An outer cycle cannot be disguised through `Choice`, `JoinAll`, or expansion.

### 11.2 Bounded repetition

v0 expresses repetition only by finite plan-time expansion:

```text
MapStatic(items, max_items)
```

The verifier rejects:

- `items.len() > max_items`;
- duplicate expansion keys;
- expansion whose canonical order is unstable;
- templates that recursively contain `MapStatic`;
- total expanded node or edge count above the Flow bounds.

### 11.3 Future fixed-point regions

A future version MAY admit an explicit fixed-point region only when:

- every participating transfer function is deterministic and monotone;
- values form a declared finite-height partial order;
- a bottom value and join operation are registered;
- a static iteration ceiling is derived from the lattice height;
- non-convergence is a typed rejection.

This follows the same mathematical restriction used by Salsa's cycle recovery:
fixed-point iteration is safe only for monotone computations over a partial
order with fixed height.[9] Arbitrary workflow loops remain out of scope.

## 12. Static soundness obligations

Workflow-net research uses soundness to rule out deadlocks, livelocks, improper
completion, and dead transitions; it also shows that richer constructs such as
cancellation and priorities can make verification undecidable.[10] Braid Flow
therefore starts deliberately smaller.

The static verifier MUST prove:

1. at least one root and one terminal;
2. all node keys unique;
3. every edge endpoint and port resolves;
4. data-edge types unify;
5. graph acyclic after static expansion;
6. every node reachable from synthetic start;
7. every node can reach a terminal;
8. every successful path reaches exactly one success terminal;
9. every failure path reaches an explicit failure terminal or typed propagation;
10. no token can remain stranded at logical completion;
11. every `Choice` is total and pairwise disjoint;
12. every `JoinAll` has a statically known predecessor cardinality;
13. static expansion remains within bounds;
14. no capsule authority is widened or pooled by the Flow;
15. no node relies on an implicit input;
16. every effectful action has a justification declaration;
17. every retry policy is compatible with the referenced capsule's registered
    effect semantics;
18. every cache policy is compatible with effect semantics;
19. every terminal is reachable under at least one satisfiable bounded path, or
    the verifier returns `Unknown` rather than guessing.

## 13. Required invariant registry

Codex MUST preserve these IDs in code, tests, and diagnostics.

| ID | Invariant |
|---|---|
| `INV-FLOW-001` | Node keys are unique and canonical. |
| `INV-FLOW-002` | Every edge, port, capsule, predicate reference, and invariant reference resolves. |
| `INV-FLOW-003` | The normalized v0 Flow is acyclic. |
| `INV-FLOW-004` | Every expansion is finite, canonical, and within declared bounds. |
| `INV-FLOW-005` | Guard evaluation is total and side-effect free. |
| `INV-FLOW-006` | Every material action has complete justification fields. |
| `INV-FLOW-007` | `Unknown` never dispatches work. |
| `INV-FLOW-008` | Proofs cannot be consumed against a different relevant snapshot. |
| `INV-FLOW-009` | Effectful nodes are not result-cached by default. |
| `INV-FLOW-010` | Retry cannot exceed the referenced effect contract. |
| `INV-FLOW-011` | Frontier selection has a canonical final tie-break. |
| `INV-FLOW-012` | Hidden inputs are rejected. |
| `INV-FLOW-013` | Join cardinality and completion classes are explicit. |
| `INV-FLOW-014` | Every declared node is start-reachable and terminal-co-reachable. |
| `INV-FLOW-015` | Failure has an explicit terminal path or typed propagation. |
| `INV-FLOW-016` | A Flow never aggregates or widens capsule authority. |
| `INV-FLOW-017` | Secret/exposure taint is monotone across data edges. |
| `INV-FLOW-018` | Canonical bytes round-trip bijectively and produce stable CIDs. |
| `INV-FLOW-019` | Proven satiation completes a node without invoking it. |
| `INV-FLOW-020` | Flow identity and target-specific plan identity remain separate. |
| `INV-FLOW-021` | Source declaration order cannot alter Flow identity or plan choice. |
| `INV-FLOW-022` | Raw shell text is never treated as a semantically verified action. |
| `INV-FLOW-023` | Planner output depends only on explicit canonical inputs. |
| `INV-FLOW-024` | No partial admission: one rejected node rejects the Flow. |
| `INV-FLOW-025` | Satiated nodes provide canonical bindings for every demanded data output. |

## 14. Cache and incrementality contract

The fastest node is the node truthfully proven unnecessary.

### 14.1 Cache key

A cacheable node key MUST cover:

```text
domain separator
+ node semantic CID
+ referenced capsule CID
+ canonical input value CIDs
+ relevant snapshot fact CIDs
+ toolchain/profile CID
+ platform/target CID
+ explicitly declared environment inputs
+ cache-policy version
```

Any undeclared file read, environment read, network read, clock read, random
value, mutable global, or toolchain dependency is a correctness defect, not a
cache miss.

Skyframe's model similarly depends on all reads being registered dependencies;
unregistered reads can produce incorrect incremental builds.[3] DICE records
requested keys as dependencies and can resurrect dependents when a recomputed
value is equal to its previous value.[4]

### 14.2 Cache classes

The verifier derives cache eligibility from the referenced capsule and term
registry. It MUST NOT trust a Flow-authored boolean such as `cache: true`.

Default rules:

| Derived semantics | Result reuse |
|---|---|
| Pure, deterministic, all inputs explicit | Allowed |
| Read-only but non-hermetic | Rejected until observation is explicit |
| Idempotent effect | Not result-cached merely because it is idempotent |
| Egress / irreversible / state commit | Never result-cached |
| Unknown effect | Never result-cached |

An effectful action may still be **skipped through satiation** if
`satisfied_when` is proven. That is not cache replay; it is a new proof that the
effect is unnecessary.

### 14.3 Reverse invalidation and early cutoff

The planner/store SHOULD maintain:

- forward dependencies;
- reverse dependencies;
- prior node output CIDs;
- snapshot dependencies;
- proof dependencies.

On an input change:

1. mark its reverse transitive dependents suspect;
2. recompute only when demanded by a terminal or requested output;
3. if the recomputed output CID equals the previous CID, stop invalidation
   propagation at that node;
4. share identical in-flight node keys.

This is the same broad family of incremental techniques used by Skyframe, DICE,
and content-addressed build graphs.[3][4][5] The implementation MUST be
independently specified and tested; no dependency on those systems is implied.

### 14.4 Secret handling

Raw secrets MUST NOT be embedded in Flow bytes, plan bytes, logs, public cache
keys, or CIDs. A node that consumes a secret uses an opaque, versioned
`SecretRef` supplied by the external authority. Unless a safe secret-version
identity exists, result caching is disabled for that node.

## 15. Rust API strawman

Names and shape are ratified by ADR-099.

```rust
use braid_ir::{Cid, TypeTag, Value};

pub struct FlowSpec {
    pub version: u16,
    pub name: String,
    pub roots: Vec<FlowInput>,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub terminals: Vec<NodeKey>,
    pub bounds: FlowBounds,
}

pub struct FlowNode {
    pub key: NodeKey,
    pub kind: FlowNodeKind,
    pub guard: Predicate,
    pub justification: Option<JustificationDecl>,
    pub urgency: UrgencyClass,
}

pub struct FlowBounds {
    pub max_nodes: u32,
    pub max_edges: u32,
    pub max_predicate_depth: u16,
    pub max_expanded_nodes: u32,
}

pub struct AdmittedFlow {
    pub flow: FlowSpec,
    pub flow_cid: Cid,
    pub verification: VerificationSummary,
}

pub struct PlanningContext {
    pub snapshot_cid: Cid,
    pub target_profile_cid: Cid,
    pub cache_manifest_cid: Cid,
    pub planner_version: u16,
}

pub struct FlowPlan {
    pub flow_cid: Cid,
    pub plan_cid: Cid,
    pub satiated: Vec<SatiatedTransition>,
    pub next_step: Option<PlanStep>,
}
```

The core IR SHOULD support `no_std + alloc` if the existing Braid encoding
substrate permits it. The SDK and importers MAY require `std`.

No public type named `Receipt`, `Capability`, `Verdict`, `Principal`,
`WorkObject`, or `Fact` may be introduced by these crates.

## 16. Example: Braid CI as Flow

Illustrative Rust source:

```rust
let ci = FlowBuilder::new("braid-ci")
    .input("change_set", TypeTag::Opaque("lw.change_set".into()))
    .invoke("scope", capsules::scope())
        .with_input("change_set")
        .needed_when(pred::always())
        .satisfied_when(pred::never())
    .invoke("fmt", capsules::cargo_fmt())
        .after_success("scope")
        .needed_when(pred::scope_matches("rust-or-docs"))
        .satisfied_when(pred::evidence_exists("fmt", "same-inputs"))
    .invoke("build", capsules::cargo_build())
        .after_success("scope")
        .needed_when(pred::scope_matches("rust"))
        .satisfied_when(pred::cache_hit("build"))
    .invoke("tests", capsules::cargo_test())
        .after_success("build")
        .needed_when(pred::scope_matches("rust"))
        .satisfied_when(pred::cache_hit("tests"))
    .invoke("clippy", capsules::cargo_clippy())
        .after_success("build")
        .needed_when(pred::scope_matches("rust"))
        .satisfied_when(pred::cache_hit("clippy"))
    .join_all("quality", ["fmt", "tests", "clippy"])
    .success("accepted").after_success("quality")
    .failure("rejected").from_any_failure()
    .build()?;
```

The example is not valid merely because it compiles. Each referenced helper must
lower to the closed predicate AST, each capsule must already be admitted, and
the verifier must prove the full Flow.

## 17. Normalized inspection JSON

A machine-readable inspection projection MAY look like:

```json
{
  "version": 0,
  "name": "braid-ci",
  "roots": [
    {"key": "change_set", "type": "opaque:lw.change_set"}
  ],
  "nodes": [
    {
      "key": "build",
      "kind": {
        "invoke_capsule": {
          "capsule_cid": "b3:..."
        }
      },
      "guard": {"const": true},
      "justification": {
        "needed_when": {"fact_eq": ["scope.code_changed", true]},
        "satisfied_when": {"fact_eq": ["cache.build.valid", true]},
        "guarantees": ["inv.build.outputs_declared"],
        "preserves": ["inv.repo.source_unchanged"]
      },
      "urgency": "required"
    }
  ],
  "edges": [
    {
      "data": {
        "from": {"root": "change_set"},
        "to": {"node": "build", "port": "change_set"},
        "value_type": "opaque:lw.change_set"
      }
    },
    {
      "after": {
        "from": "scope",
        "to": "build",
        "on": ["executed_success", "satisfied_without_execution"]
      }
    }
  ],
  "terminals": ["accepted", "rejected"],
  "bounds": {
    "max_nodes": 1024,
    "max_edges": 4096,
    "max_predicate_depth": 32,
    "max_expanded_nodes": 4096
  }
}
```

This projection is intentionally verbose. Compact canonical bytes are a compiler
output, not the authoring burden.

## 18. Compiler and verification pipeline

```text
source AST / importer
    |
    v
parse or construct typed source nodes
    |
    v
resolve symbols, ports, capsule CIDs, predicates
    |
    v
normalize static expansion and synthetic start/finish
    |
    v
canonical sort nodes and edges
    |
    v
type/effect/taint/capability non-widening checks
    |
    v
guard totality + choice disjointness
    |
    v
SCC + reachability + co-reachability + soundness checks
    |
    v
justification completeness and proof-mode checks
    |
    v
canonical encode -> Flow CID
    |
    v
plan(snapshot, profile, cache) -> frontier schedule -> Plan CID
    |
    v
manifest + DOT + machine verification report
```

The independent verifier MUST decode canonical bytes independently of the SDK.
A builder may refuse early, but it may not admit its own output.

## 19. Execution boundary and replay

### 19.1 What Braid emits

Braid may emit:

- admitted Flow bytes;
- Flow CID;
- verification summary;
- deterministic plan bytes;
- Plan CID;
- one-step frontier decisions;
- human manifest and DOT projection;
- proof requirements and typed refusals.

### 19.2 What forge-harness owns

Forge owns:

- durable Flow instance identity;
- leases and fencing;
- worker lifecycle;
- event history;
- retries and backoff;
- crash recovery;
- external observations;
- state snapshot creation;
- effect execution;
- receipts and evidence persistence;
- re-planning when a snapshot changes.

Temporal demonstrates the relevant runtime separation: deterministic workflow
logic is replayed from history, while non-deterministic I/O belongs in recorded
activities.[11] Braid SHOULD borrow the separation of deterministic decision
from effect execution, not Temporal's API or storage model.

### 19.3 What `braid-run` owns

`braid-run` continues to execute one admitted capsule against a `Host`. A Forge
worker invokes `braid-run` for a planned node. `braid-run` does not decide the
next Flow node and does not persist durable Flow state.

### 19.4 Reference simulator

Braid MAY add a pure `braid-flow-sim` test utility that:

- executes only pure/mock capsules;
- consumes an explicit snapshot;
- produces a deterministic trace;
- never claims crash durability;
- never mints production receipts;
- is excluded from runtime authority.

## 20. Retry, failure, and compensation

Flow does not invent effect semantics. Retry eligibility is derived from the
existing capsule and term registry.

Minimum rules:

- unknown effect semantics -> no automatic retry;
- pure/read-only deterministic operation -> retry permitted within explicit
  bounds;
- irreversible or egress effect -> no automatic retry without an externally
  owned idempotency/fencing proof;
- a failed attempt does not become `SatisfiedWithoutExecution`;
- retry counters and backoff are explicit plan/runtime inputs;
- wall-clock time is never read by the planner;
- compensation is an explicit admitted node, not an implicit rollback hook;
- compensation does not erase the original failure evidence;
- every failure and compensation route must terminate soundly.

v0 SHOULD avoid automatic compensation synthesis. Machine authors may propose
the graph; the verifier checks only explicit registered behavior.

## 21. GitHub Actions importer and exporter

### 21.1 Purpose

The first real fixture SHOULD be Braid's `.github/workflows/ci.yml`. The importer
is a migration and differential-testing tool; GitHub Actions YAML does not
become a Braid authoring or wire language.

### 21.2 Supported v0 subset

The importer SHOULD support:

- workflow triggers as external root events;
- `jobs`;
- `needs`;
- job-level `if` expressions that can be lowered to the closed predicate AST;
- static `strategy.matrix`;
- `timeout-minutes`;
- declared `env`;
- `concurrency` as a runtime lane hint;
- ordered steps inside one job capsule.

### 21.3 Typed refusals

Strict mode MUST refuse:

- floating action references that are not pinned to immutable commits;
- unsupported GitHub expression functions;
- dynamically generated matrices not bounded at plan time;
- raw secret interpolation into command text;
- hidden environment dependencies;
- shell steps that cannot be lowered to an admitted process capsule;
- service containers whose lifecycle contract is not represented;
- implicit platform/toolchain dependencies;
- cancellation/race semantics unsupported by v0.

### 21.4 Legacy audit mode

An audit-only mode MAY preserve raw shell as an opaque legacy node for topology
comparison. Such a node:

- is marked unverified;
- is never admitted in strict mode;
- cannot produce a proof that the workflow is semantically safe;
- exists only to quantify migration gaps.

### 21.5 Export

An exporter MAY project an admitted Flow back to GitHub Actions YAML for
portability. It MUST emit a semantic-loss report. Unsupported Flow semantics
must be refused, never silently weakened.

## 22. experience-as-code integration

experience-as-code may compile:

```text
experience declaration
-> signed domain plan
-> Braid Flow reference or generated Flow
-> forge execution
-> direct experience observation
-> domain reconcile decision
```

The domain repository retains ownership of what an experience means and whether
it drifted. Braid only verifies the generated Flow structure. Forge executes it.

The integration MUST NOT:

- move the generic Flow wire format into experience-as-code;
- give Braid ownership of experience credentials or receipts;
- let a domain `apply` bypass Braid admission;
- let Braid infer domain satisfaction from prose.

## 23. `lgwks_bot` compatibility

A future adapter may provide:

```rust
fn bot_spec_to_flow(spec: &lgwks_bot::BotSpec) -> Result<FlowSpec, BotFlowError>;
```

Mapping:

```text
BotSpec
  chain condition -> Flow predicate
  action           -> admitted action capsule
  chain order      -> explicit After edges
```

Limits:

- the adapter may only lower supported conditions/actions;
- unsupported closures or host behavior are typed refusals;
- the adapter does not create durable state;
- the adapter does not execute the Flow;
- the same independent Flow verifier admits or rejects the result.

This keeps `lgwks_bot` useful as boilerplate without turning it into a parallel
platform.

## 24. Security and resource bounds

Initial hard limits SHOULD be conservative and configurable downward:

| Bound | Default v0 |
|---|---:|
| Source nodes | 10,000 |
| Source edges | 50,000 |
| Expanded nodes | 50,000 |
| Expanded edges | 250,000 |
| Predicate depth | 32 |
| Total predicate nodes | 16,384 |
| Choice arms per node | 128 |
| Ports per node | 128 |
| Static matrix items | 4,096 |
| Identifier bytes | 128 |
| Diagnostic bytes per refusal | 8,192 |

The `braid-flow-sdk` RON boundary additionally MUST reject input above 16 MiB
or nesting deeper than 64 before semantic AST allocation. It MUST validate
declared collection bounds incrementally before reserving node, edge, port, or
expansion storage. These parser ceilings are fixed for the v0 source schema;
lower deployment limits MAY fail closed.

The verifier MUST:

- use checked arithmetic;
- reject allocation estimates over configured memory budget;
- bound recursion or use iterative graph algorithms;
- reject duplicate keys before expensive analysis;
- avoid quadratic pairwise branch checks when a canonical indexed method is
  available;
- never execute source while parsing or verifying;
- forbid unsafe code in core crates unless a separately ratified exception
  exists;
- preserve exposure/secret taint monotonically;
- render user strings with escaping;
- produce stable typed refusals, not panic, for hostile source.

## 25. Performance acceptance and benchmark design

The first implementation has provisional absolute control-plane targets. They
become normative only after P6 records the benchmark host and fixture hashes.

### 25.1 Provisional targets

On a warm local release build:

| Operation | Graph size | Target |
|---|---:|---:|
| Parse RON source + normalize | 1,000 nodes / 5,000 edges | p50 <= 25 ms; p99 <= 100 ms |
| Import or emit normalized JSON | 1,000 nodes / 5,000 edges | p50 <= 25 ms; p99 <= 100 ms |
| Static verify + Flow CID | 1,000 / 5,000 | p50 <= 25 ms; p99 <= 100 ms |
| Static verify + Flow CID | 10,000 / 50,000 | p50 <= 250 ms; p99 <= 1 s |
| One-node completion frontier update | 10,000 / 50,000 | p50 <= 100 us; p99 <= 1 ms |
| Warm no-op plan | 10,000 / 50,000 | p50 <= 50 ms |
| Peak verifier RSS | 10,000 / 50,000 | <= 128 MiB |
| Determinism under 1,000 source permutations | 1,000 / 5,000 | 100% identical Flow CID and plan |

These targets measure Braid's control plane, not command execution.

### 25.2 Required benchmark decomposition

Every end-to-end CI benchmark records:

```text
event-to-runner queue
runner provisioning
checkout/materialization
Flow compile/verify
initial plan
per-frontier update
process launch
actual command CPU time
network wait
cache transfer
artifact transfer
cleanup
```

### 25.3 Fair comparison rule

For Braid vs GitHub Actions:

- same repository commit;
- same commands;
- same toolchain;
- same self-hosted machine class;
- same cold/warm cache state;
- same network and artifact source;
- at least 30 measured runs per condition;
- report median, p95, p99, and variance;
- report skipped work separately from faster execution;
- publish raw benchmark records and fixture CIDs.

No benchmark may attribute runner queue differences to the Flow planner.

## 26. Falsification matrix

The implementation is not accepted until these attacks have executable tests.

| ID | Attack | Required result |
|---|---|---|
| `F-FLOW-01` | Already-satisfied expensive action | `SatisfiedWithoutExecution`; host never called |
| `F-FLOW-02` | Missing fact in `needed_when` | `BlockedUnknown`; no fallback |
| `F-FLOW-03` | Proof created at snapshot N, dispatch at N+1 | typed stale-plan rejection |
| `F-FLOW-04` | Source node order randomly permuted | identical Flow CID and plan |
| `F-FLOW-05` | Two ready nodes tie on all declared fields | node CID decides identically |
| `F-FLOW-06` | Hidden environment read changes output | hermeticity test fails |
| `F-FLOW-07` | Pure node recomputes to same output CID | downstream invalidation stops |
| `F-FLOW-08` | Effectful node declares cacheable | static rejection |
| `F-FLOW-09` | Irreversible node requests automatic retry | static rejection |
| `F-FLOW-10` | Cycle hidden through expansion | SCC rejection after expansion |
| `F-FLOW-11` | Expansion exceeds `max_items` by one | bounded refusal before allocation |
| `F-FLOW-12` | Overlapping Choice guards | `Unknown`/rejection, never first-match |
| `F-FLOW-13` | Choice has no otherwise | totality rejection |
| `F-FLOW-14` | Node unreachable from start | dead-node rejection |
| `F-FLOW-15` | Success path leaves join token stranded | soundness rejection |
| `F-FLOW-16` | Failure has no terminal route | failure-path rejection |
| `F-FLOW-17` | Flow tries to pool grants from two capsules | authority non-aggregation rejection |
| `F-FLOW-18` | Secret literal appears in RON or inspection JSON | redaction/validation rejection |
| `F-FLOW-19` | Raw shell imported in strict mode | typed importer refusal |
| `F-FLOW-20` | Runner crashes after external effect | Forge replay does not duplicate without proof |
| `F-FLOW-21` | Circular justification | vacuity/cycle rejection |
| `F-FLOW-22` | `satisfied_when` is constant true | warning plus policy rejection for material action unless explicitly permitted |
| `F-FLOW-23` | `needed_when` and `satisfied_when` both proven | satiation wins; action not invoked |
| `F-FLOW-24` | Learned priority model changes output | impossible: model output absent from trusted planner inputs |
| `F-FLOW-25` | Canonical bytes contain unknown field | fail closed at independent decoder for v0 wire |
| `F-FLOW-26` | Decode -> encode round trip | byte-identical canonical form |
| `F-FLOW-27` | One semantic bit changes | Flow CID changes |
| `F-FLOW-28` | Target profile changes only | Flow CID same; Plan CID changes |
| `F-FLOW-29` | Cache manifest changes only | Flow CID same; Plan CID changes |
| `F-FLOW-30` | No ready nodes, no terminal, no Unknown | typed deadlock witness |
| `F-FLOW-31` | Satiated producer has a demanded output but no materialization | successor remains blocked and verification/planning reports the missing binding |
| `F-FLOW-32` | RON exceeds 16 MiB or nesting depth 64 by one | SDK refuses before semantic AST allocation |

## 27. Implementation layout

Ratified crates:

```text
crates/
  braid-flow-ir/       canonical outer graph, encoding, CIDs, predicate AST
  braid-flow-verify/   independent decoder and static admission
  braid-flow-plan/     deterministic snapshot-bound frontier planning
  braid-flow-sdk/      Rust builder and diagnostics
```

Existing crates integrate as follows:

```text
braid-project
  -> may build/admit a Flow project after P2
braid-run
  -> executes one InvokeCapsule plan step
braid-render
  -> gains deterministic Flow manifest and full DOT projection
braid-cli
  -> gains flow encode|decode|verify|plan|render|import-gh commands
lgwks-bot
  -> optional adapter only
```

Do not add all four crates in one unreviewable patch. Each boundary requires an
explicit invariant and mutation-resistant tests.

## 28. Implementation phases

### P0 — Ratify authority and wire decisions — COMPLETE

**Files**

- `docs/adr-099-braid-frontier-flow.md`
- `spec/braid/DECISIONS.md`
- `spec/braid/DEBT_REGISTER.md`
- dependency-ordered GitHub issues for P1-P6

**Decisions to freeze**

1. Flow belongs in Braid; durable runtime belongs in Forge.
2. outer Flow is separate from inner `Braid`;
3. Flow CID and Plan CID domains;
4. v0 node and edge closed sets;
5. v0 rejects arbitrary cycles, race, cancellation, and `JoinAny`;
6. source order is non-semantic;
7. effect cache/retry rules derive from existing registry authority;
8. no new Receipt/Capability/Verdict/etc. types.

**Evidence**

- ADR-099 and `D-FLOW.1` through `D-FLOW.9` freeze the decisions;
- `docs/CRATE-OWNERSHIP.md` states each proposed invariant;
- graph/statechart/durable-runtime conflicts are reconciled in the architecture
  documents;
- issue DAG: [P1 #57](https://github.com/srinji-kaggss/Braid/issues/57) ->
  [P2 #60](https://github.com/srinji-kaggss/Braid/issues/60) ->
  [P3 #59](https://github.com/srinji-kaggss/Braid/issues/59); P3 then forks to
  [P4 #58](https://github.com/srinji-kaggss/Braid/issues/58) and
  [P5 forge-harness #123](https://github.com/srinji-kaggss/forge-harness/issues/123);
  both join at
  [P6 experience-as-code #66](https://github.com/srinji-kaggss/experience-as-code/issues/66).

### P1 — `braid-flow-ir`

**Issue:** [#57](https://github.com/srinji-kaggss/Braid/issues/57)

**Deliver**

- no-unsafe core types;
- canonical encoding and independent decode fixtures;
- domain-separated CIDs using `braid_ir::Cid`;
- source-order-independent canonicalization;
- bounds validation before allocation;
- KAT vectors under `spec/braid/vectors/frontier-flow/`.

**Required tests**

- unit tests for every node/edge/predicate variant;
- proptests for encode/decode bijection;
- random declaration-order permutation;
- one-bit CID sensitivity;
- unknown-field refusal;
- hostile size bounds.

**Exit command**

```bash
cargo test -p braid-flow-ir --all-targets
cargo clippy -p braid-flow-ir --all-targets -- -D warnings
```

### P2 — `braid-flow-verify`

**Issue:** [#60](https://github.com/srinji-kaggss/Braid/issues/60), blocked by #57.

**Deliver**

- independent canonical decoder;
- symbol/port/type resolution;
- static expansion;
- iterative SCC and reachability analyses;
- choice disjointness/totality;
- join and terminal soundness;
- justification completeness;
- authority non-aggregation;
- typed invariant IDs in refusals.

**Required tests**

- one positive and one mutation-negative test per `INV-FLOW-*`;
- full falsification cases `F-FLOW-01` through `F-FLOW-19`;
- verifier must reject malformed bytes the SDK cannot construct.

**Exit command**

```bash
cargo test -p braid-flow-verify --all-targets
cargo clippy -p braid-flow-verify --all-targets -- -D warnings
```

### P3 — `braid-flow-plan`

**Issue:** [#59](https://github.com/srinji-kaggss/Braid/issues/59), blocked by #60.

**Deliver**

- immutable snapshot projection;
- satiation-first evaluation;
- ready antichain computation;
- stable lexicographic selection;
- Flow/Plan identity separation;
- reverse dependency index;
- early cutoff on unchanged output CID;
- deterministic trace renderer.

**Required tests**

- `F-FLOW-01` through `F-FLOW-08`;
- `F-FLOW-23`, `F-FLOW-24`, `F-FLOW-28`, `F-FLOW-29`;
- 1,000 randomized map/set insertion orders;
- benchmark harness with synthetic 1k and 10k graphs.

**Exit command**

```bash
cargo test -p braid-flow-plan --all-targets
cargo bench -p braid-flow-plan
```

### P4 — SDK, CLI, and GitHub Actions importer

**Issue:** [#58](https://github.com/srinji-kaggss/Braid/issues/58), blocked by #59.

**Deliver**

- Rust builder;
- first-class RON parsing and stable pretty-RON output;
- normalized JSON interoperability and inspection I/O;
- `braid flow encode|decode|verify|plan|render`;
- strict and audit GitHub Actions import modes;
- Braid `ci.yml` topology fixture;
- semantic-loss report;
- exporter only for the provably representable subset.

**Required tests**

- golden imported graph;
- raw-shell strict refusal;
- floating action reference refusal;
- matrix bound refusal;
- import -> verify -> render loop;
- source permutation does not change Flow CID;
- equivalent RON spellings do not change canonical bytes or Flow CID;
- unknown RON/JSON fields and lossy YAML constructs fail closed;
- RON above 16 MiB or nesting depth 64 fails before semantic AST allocation;
- declared node, edge, port, and expansion bounds fail before collection
  reservation.

### P5 — Forge runtime adapter

**Repository: forge-harness**
**Issue:** [#123](https://github.com/srinji-kaggss/forge-harness/issues/123), blocked by Braid #59.

**Deliver**

- durable Flow instance state;
- snapshot and lease binding;
- one-step plan consumption;
- invocation through `braid-run`;
- append-only event history;
- crash/replay tests;
- idempotency/fencing enforcement;
- evidence and receipt integration through existing owners.

**Required tests**

- worker crash before dispatch;
- crash after dispatch before acknowledgment;
- stale lease;
- stale snapshot;
- duplicate completion event;
- irreversible effect without retry proof;
- deterministic replay against recorded history.

Braid MUST remain buildable and testable without Forge.

### P6 — experience-as-code integration and performance proof

**Issue:** [#66](https://github.com/srinji-kaggss/experience-as-code/issues/66), blocked by Braid #58 and forge-harness #123.

**Deliver**

- compile one real experience plan to Flow;
- execute through Forge;
- observe and reconcile through experience-as-code;
- benchmark Braid CI fixture against its current Actions workflow on the same
  self-hosted machine;
- publish cold/warm raw measurements and fixture CIDs.

**Success condition**

The end-to-end demonstration proves:

```text
declaration
-> canonical Flow
-> independent admission
-> deterministic snapshot-bound plan
-> durable execution
-> direct observation
-> domain reconciliation
```

No mock may stand in for the final observation path.

## 29. Codex continuation contract

Codex MUST read, in order:

1. `AGENTS.md`
2. `spec/braid/README.md`
3. `spec/braid/DECISIONS.md`
4. `docs/adr-099-braid-frontier-flow.md`
5. `docs/architecture/BRAID-GRAPH-DSL.md`
6. `docs/architecture/BRAID-DSL-STATE-AND-SUBSTRATE.md`
7. `docs/architecture/BRAID-JUSTIFIED-INVOCATION.md`
8. `docs/CRATE-OWNERSHIP.md`
9. `crates/braid-ir/src/braid.rs`
10. `crates/braid-run/src/lib.rs`
11. `crates/braid-project/src/lib.rs`
12. this document

Codex MUST NOT:

- implement a durable scheduler in Braid;
- add dependencies before checking `lgwks-std` and the approved dependency
  contract;
- create duplicate authority types;
- weaken an invariant to make a test pass;
- add `todo!`, `unimplemented!`, placeholder success, ignored errors, or panic
  for hostile input;
- claim GitHub Actions parity from topology import alone;
- use an LLM or learned score in the admission or scheduling decision;
- silently support unsupported GitHub expressions;
- merge phases into one giant PR.

For each phase, Codex MUST produce:

```text
1. exact changed paths
2. invariants closed
3. tests added
4. mutation/adversarial evidence
5. commands run and exact outcomes
6. remaining typed gaps
7. next smallest dependency-unblocking unit
```

A failure to prove an assumption becomes a typed gap, not a guessed
implementation.

## 30. Cheap cross-verification

### 30.1 This specification

```bash
python3 - <<'PY'
from pathlib import Path

p = Path("spec/braid/BRAID_FRONTIER_FLOW.okf.md")
text = p.read_text(encoding="utf-8")
assert text.startswith("---\n")
parts = text.split("---", 2)
assert len(parts) == 3
frontmatter = parts[1]
body = parts[2]
assert "\ntype: Implementation Specification\n" in "\n" + frontmatter
assert "okf_version: \"0.1\"" in frontmatter
assert "# Citations" in body
assert "INV-FLOW-025" in body
assert "P0 — Ratify authority" in body
assert "docs/adr-099-braid-frontier-flow.md" in body
assert "first-class textual authoring" in body
assert "Codex continuation contract" in body
print("OK: OKF envelope and continuation anchors present")
PY

git diff --check
```

### 30.2 After P1/P2 exist

```bash
cargo fmt --all -- --check
cargo test -p braid-flow-ir -p braid-flow-verify --all-targets
cargo clippy -p braid-flow-ir -p braid-flow-verify --all-targets -- -D warnings
rg -n 'todo!\(|unimplemented!\(|panic!\(' crates/braid-flow-*
```

Any `panic!` match requires a documented internal-invariant justification;
hostile or ordinary invalid input must return a typed error.

### 30.3 Before each merge

```bash
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
./scripts/lgwks-std-package-smoke.sh
git diff --check
```

### 30.4 Determinism vectors

```bash
# Proposed CLI after P4:
braid flow verify spec/braid/vectors/frontier-flow/valid.cbor
braid flow decode spec/braid/vectors/frontier-flow/valid.cbor \
  --format ron > /tmp/flow.ron
braid flow encode /tmp/flow.ron \
  > /tmp/flow.roundtrip.cbor
cmp spec/braid/vectors/frontier-flow/valid.cbor /tmp/flow.roundtrip.cbor
```

## 31. Ratified P0 decisions

| Decision | Ratified value | Reason |
|---|---|---|
| Public product name | Braid Frontier Flow | Describes the outer graph and ready antichain without reusing `bot`. |
| Crate names | `braid-flow-ir`, `braid-flow-verify`, `braid-flow-plan`, `braid-flow-sdk` | Matches existing authority separation. |
| Authoring source | Rust AST + first-class RON | Typed, enum-friendly authoring over one AST. |
| Interoperability | normalized JSON | External tools can exchange/inspect Flow without owning identity. |
| Wire form | existing Braid canonical encoding | Preserves CID and verifier discipline. |
| Source order | non-semantic | Machine generation and diffs should not alter identity. |
| v0 executor | sequential | Smallest deterministic kernel; speed initially comes from less work. |
| Parallelism | later compatible-frontier extension | Requires effect/resource confluence proof. |
| Loops | reject; static finite expansion only | Preserves bounded soundness. |
| `JoinAny` / race | reject v0 | Avoids cancellation and schedule-sensitive semantics. |
| Durable runtime | Forge | Prevents Braid from becoming a platform monolith. |
| Domain reconcile | experience-as-code | Keeps experience meaning near its owner. |
| `lgwks_bot` | adapter/leaf library | Avoids a second scheduler. |
| Raw shell | strict refusal; audit-only legacy wrapper | AST shape cannot make shell semantics provable. |
| Learned planner | advisory outside trust boundary only | Trusted plan remains exact and replayable. |

## 32. Research synthesis

Your core thought has real ancestors, but the synthesis is useful:

| This proposal | Existing theory/system |
|---|---|
| Separate semantic graph, scheduler, cache, and runtime | Build Systems à la Carte decomposes and recombines build-system components.[6] |
| Content-addressed dependency IR | BuildKit LLB is a content-addressable dependency graph IR.[5] |
| Explicit dependency tracking and precise invalidation | Skyframe and DICE record dependency graphs and prune unchanged recomputation.[3][4] |
| Ready frontier as incomparable work | Timely progress represents frontiers with antichains in a partial order.[8] |
| Reject arbitrary cycles; admit only constrained convergence later | Salsa restricts fixed-point cycles to monotone finite-height domains.[9] |
| Prove workflow completion structure | Workflow-net soundness checks deadlock, livelock, completion, and dead transitions.[10] |
| Deterministic decision separate from effects and replay | Temporal replays deterministic workflow logic while recording effect results.[11] |
| Action only when an unsatisfied condition exists | Braid's justified-invocation thesis synthesizes Hoare logic, contracts, typestate, proof-carrying code, planning, and homeostasis.[12] |

The distinctive Braid move is not “a DAG in Rust.” It is:

> **a content-addressed outer graph whose next invocation is selected
> deterministically from a proven ready frontier, where already-satisfied work
> completes without execution and every effect remains inside an admitted
> capsule.**

# Citations

[1] [GitHub Actions: workflows and actions reference](https://docs.github.com/en/actions/reference/workflows-and-actions) — workflows are YAML-defined processes composed of jobs.

[2] [GitHub Actions workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax) — `needs`, job conditions, matrices, runners, and concurrency semantics.

[3] [Bazel Skyframe](https://bazel.build/reference/skyframe) — immutable dependency nodes, parallel evaluation, precise invalidation, hermeticity, and change pruning.

[4] [Buck2 DICE: Writing a Computation](https://github.com/facebook/buck2/blob/main/dice/dice/docs/writing_computations.md) — dependency recording, memoized computation, equality, and dependent resurrection.

[5] [Docker BuildKit](https://docs.docker.com/build/buildkit/) — concurrent graph solver and content-addressable Low-Level Build dependency graph.

[6] [Mokhov, Mitchell, and Peyton Jones, “Build systems à la carte: theory and practice”](https://www.microsoft.com/en-us/research/publication/build-systems-a-la-carte/) — systematic executable decomposition and recombination of build-system components.

[7] [Salsa](https://salsa-rs.github.io/salsa/) — on-demand incremental computation in Rust. Used as design context, not a required dependency.

[8] [Timely dataflow progress](https://docs.rs/timely/latest/timely/progress/) — progress tracking and `Antichain`, a minimal set of mutually incomparable elements in a partial order.

[9] [Salsa cycle handling](https://salsa-rs.github.io/salsa/cycles.html) — fixed-point iteration requires deterministic monotone queries over a fixed-height partial order.

[10] [van der Aalst et al., “Soundness of workflow nets: classification, decidability, and analysis”](https://link.springer.com/article/10.1007/s00165-010-0161-4) — workflow soundness and the decidability limits introduced by richer workflow extensions.

[11] [Temporal Platform FAQ: deterministic Workflow replay and Activities](https://go.temporal.io/platform-hub/faqs) — deterministic workflow logic is replayed from history; non-deterministic I/O belongs in recorded activities.

[12] [Braid Justified Invocation](../../docs/architecture/BRAID-JUSTIFIED-INVOCATION.md) — local thesis requiring safe, authorized, and justified invocation, snapshot binding, exact predicates, and satiation.

[13] [Braid Graph DSL](../../docs/architecture/BRAID-GRAPH-DSL.md) — approved inner graph, statechart, lowering, and static-budget direction.

[14] [Current Braid IR graph](../../crates/braid-ir/src/braid.rs) — existing topologically ordered strand DAG.

[15] [`braid-run`](../../crates/braid-run/src/lib.rs) — existing deterministic single-capsule sequential interpreter.

[16] [`lgwks_bot`](../../crates/lgwks-bot/README.md) — existing condition/action boilerplate library, explicitly not a scheduler or orchestration runtime.

[17] [`braid-project`](../../crates/braid-project/src/lib.rs) — existing independent multi-capsule admission and project CID without cross-capsule wiring.

[18] [`lgwks_std::ron`](../../crates/lgwks-std/src/ron.rs) — repository-approved serde-based RON boundary; RON is preferred for human-facing configuration while JSON remains the external-interoperability default.

[19] [ADR-099](../../docs/adr-099-braid-frontier-flow.md) — ratified authority, RON/JSON source boundary, v0 wire domains, crate ownership, and P1-P6 issue graph.

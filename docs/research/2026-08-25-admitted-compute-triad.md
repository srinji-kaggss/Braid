# Admitted compute triad — focused implementation research

**Date:** 2026-08-25  
**Scope:** Braid Flow IR boilerplate for `(safe, authorized, justified)`.

## Result

The triad should not become three author-controlled booleans. In Braid Flow:

- **Safety** is the language/structural boundary already established by an
  admitted capsule plus independent Flow verification.
- **Capability** remains external kernel authority derived from the referenced
  capsule. Flow must not pool, mint, or widen it.
- **Justification** is new identity-bearing data: exact `needed_when` and
  `satisfied_when` predicates plus registered guarantees, invariant references,
  and an optional deterministic cost order.

This preserves the decisive property: a safe and authorized action still does
not run when the relevant state is already satisfactory, when its need is
unknown, or when its proof is circular.

## Primary-source findings

1. Braid's ratified [Frontier Flow specification](../../spec/braid/BRAID_FRONTIER_FLOW.okf.md)
   already fixes the authority split, closed predicate AST, snapshot-bound
   justification, satiation-first behavior, canonical ordering, and hard
   bounds. The implementation should execute that contract, not invent a
   sibling triad protocol.
2. [Salsa's official overview](https://github.com/salsa-rs/salsa/blob/546bc5d3ae8d348d6e068a6c455d35a213c790f9/book/src/overview.md)
   makes incremental reuse conditional on computations being deterministic
   functions of explicit inputs; input mutation occurs outside the tracked
   computation. Braid should copy that explicit-input discipline, but not add
   Salsa as a dependency or conflate memoization with permission to act.
3. [Buck2 DICE's official computation contract](https://github.com/facebook/buck2/blob/0e3887c5073dcf700c8472b33a604cdd0b68fe66/dice/dice/docs/writing_computations.md)
   records requested keys as dependencies and uses equality to stop invalidation
   when recomputation yields the same value. Braid should later use the same
   class of dependency/provenance facts in P3, while keeping P1 limited to the
   canonical declarations those facts will satisfy.
4. [Bazel Skyframe](https://bazel.build/reference/skyframe) likewise requires
   dependency reads to be registered for correct invalidation. The lesson is
   negative and important: an ambient read is not merely a cache miss; it makes
   the justification proof unsound.
5. Braid's [Justified Invocation thesis](../architecture/BRAID-JUSTIFIED-INVOCATION.md)
   separates proof-carrying need from purpose prose and requires `Unknown` to
   fail closed. This is the proof-carrying-code move applied to an invocation's
   right to exist, not only to the safety of its implementation.

## What this P1 slice deliberately does not copy

- No weighted utility, probability, embedding, model confidence, or fuzzy gate.
- No general-purpose logic language or arbitrary predicate callbacks.
- No cache/retry boolean authored by Flow.
- No new capability, verdict, fact, principal, receipt, or work-object type.
- No scheduler, durable state, execution, or proof evaluator.
- No Salsa, DICE, or workflow-engine dependency.

The minimality rule is concrete: P1 records immutable meaning and canonical
identity. P2 independently verifies it. P3 evaluates snapshot-bound need and
satiation. Forge alone executes the selected capsule.

## Protocol decisions still requiring ratification

This implementation is a foundation, not closure of Braid #57. Four fail-closed
resource envelopes are intentionally marked provisional in code:

- canonical bytes across literal values are capped at 16 MiB, conservatively
  mirroring the ratified textual source envelope so Rust builders cannot bypass
  all payload bounds;
- literal structure is additionally capped at 262,144 `Value` nodes because a
  flat one-byte list can otherwise turn a modest wire into millions of heap
  objects while remaining inside the byte envelope;
- each guarantee/preservation reference collection is capped at 128 so an
  author cannot force an unbounded canonical sort, with 16,384 references
  across one Flow so individually valid nodes cannot multiply that work;
- aggregate type-tag structure is capped at 262,144 nodes per Flow, in
  addition to Braid IR's per-tag depth, arity, label, and node limits, so many
  individually legal declarations cannot multiply canonical work without a
  ceiling;
- canonical byte admission performs an allocation-free first pass with a
  128 MiB wire ceiling and 3,000,000 projected values, validates declared outer
  collection bounds, and debits aggregate literal, predicate, type-tag, and
  justification-reference budgets before it permits the generic decoder to
  allocate.

The Frontier Flow limits table does not yet name these semantic collections.
Their exact v0 values must be ratified or replaced before Issue #57 closes.
Strict Flow decoding/bijection and complete closed-variant/property coverage
now exist in P1. The decoder rejects unknown fields at every nested map,
unknown variants, non-minimal CBOR, noncanonical semantic declaration order,
and duplicate semantic entries that construction would otherwise normalize.
It also exposed and fixed a real AST normalization defect: `And`/`Or` operands
are now recursively sorted and deduplicated during construction, so a canonical
decode/encode round trip preserves both bytes and normalized structure.
Builder construction now mirrors the exact depth of the fully wrapped wire,
not merely the semantic literal/predicate/type depth. Boundary tests prove that
choice-arm literals, recursive predicates, and opaque types accepted at wire
depth 64 decode back identically and fail at the first value at depth 65.
Typed decode collections check their protocol limit before reserving, and the
property suite generates bounded recursive values, predicates, and type tags in
addition to declaration permutations and arbitrary hostile bytes.

The decoder choices follow [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949.html):
deterministic encodings use shortest forms and definite lengths, while protocol
decoders remain responsible for application-level expected-shape checks and
duplicate-key refusal. The future graph verifier should use an iterative
linear-space SCC implementation; [petgraph's pinned Kosaraju implementation](https://github.com/petgraph/petgraph/blob/037d9ddc2abb4f8690af941179baf315fb8c940b/crates/petgraph/src/algo/scc/kosaraju_scc.rs)
is a concrete Rust reference, not a proposed dependency.

Source expansion and independent admission remain later work in the existing
issue DAG. P2 preserves a newly explicit blocker rather than guessing through
it: Flow data edges carry named typed capsule ports, but the current Capsule
wire carries no named external port interface. P2 can resolve a capsule CID and
its internal result indices, but cannot prove that `node.port` exists or has the
edge-declared type. That requires a ratified choice between extending Capsule
identity and binding a separate interface registry into verification evidence.
Until then, data-bearing Flow admission must fail closed. Predicate fact,
relation, and invariant references likewise remain identity-bearing syntax for
P2 to resolve against an explicit immutable registry before anything is
runnable.

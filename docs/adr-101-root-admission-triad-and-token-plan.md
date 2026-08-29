# ADR-101 — Root Admission Triad and Registry-Scoped Token Plan

**Status:** Accepted for the substrate; runtime integration remains blocked on the justification planner and authority-safe execution boundary.  
**Date:** 2026-08-26  
**Supersedes:** no canonical wire decision. Elaborates ADR-100 and issues #59, #61, #63.

## Context

Braid has two representations with different jobs:

1. The **canonical graph** is the stable, content-addressed meaning. Dotted term names remain readable, globally exchangeable identities inside a registry pinned by CID.
2. The **hot execution projection** should be dense, bounded, and allocation-light. Repeating a heap `String` and a heap `Vec<u32>` for every strand is unnecessary after independent verification.

The AppleScript progression contains one useful idea and one failure to avoid. Its application dictionaries gave commands stable, discoverable identities. Its English-like syntax, implicit coercion, and context-sensitive name resolution made those identities ambiguous in use. Braid keeps the dictionary lesson—closed named vocabularies—but lowers admitted names into typed numeric tokens instead of executing prose.

Admission also had a semantic gap. Safety, authority, and need were described separately, but no root value forced their results to travel together. A safe and authorized operation is still wasteful or harmful when it is not justified under the current snapshot.

## Decision

### 1. The root admission value is one fail-closed triad

Braid defines:

```text
Admission = Safety × Capability × Justification
```

Each axis has exactly three states:

```text
Disproven < Unknown < Proven
```

Reduction is deterministic:

```text
any Disproven -> Reject
else any Unknown -> Defer
else             Execute
```

The representation is one byte: two bits per axis, with `00 = Unknown`, `01 = Proven`, `10 = Disproven`, `11 = reserved`; the high two bits are reserved. Therefore an all-zero value is fail-closed and cannot accidentally execute.

The byte is a compact **result carrier**, not evidence. No producer may obtain authority by serializing `Proven`. The subsystems that own the checks must construct the states from their proof artifacts.

### 2. Current independent capsule admission is deliberately `P × P × U`

`braid-verify` can currently establish:

- **Safety = Proven:** canonical form, version pin, structure, types, effects, taint, and bounds pass.
- **Capability = Proven:** capsule grants are attenuated against an externally supplied ambient authority set.
- **Justification = Unknown:** the snapshot-bound Flow planner is not complete.

The compact verifier result therefore deterministically returns `Defer`, never `Execute`. This is an intentional loud gate, not an incomplete boolean default.

### 3. Canonical names remain identity; dense tokens are a derived cache

A term token is:

```text
(registry_cid, dense_u32_ordinal)
```

The integer alone has no meaning. The ordinal is assigned by canonical registry order, and resolution is valid only under the exact pinned registry CID.

The canonical capsule remains the wire, hash preimage, audit artifact, and interoperability boundary. The token plan is never serialized as a replacement wire and never accepted as authority.

### 4. The compact program uses flat arenas

For `N` operations, `E` input references, and `O` outputs, the variable-size hot arenas are:

```text
12N + 4E + 4O bytes
```

plus one native pointer per term in the registry lookup table and allocator/fixed-header overhead. `TokenOp` is exactly 12 bytes:

```text
term_token:  u32
input_start: u32
input_len:   u32
```

All input references live in one contiguous `u32` arena. This removes the per-strand `String` and `Vec` headers and the per-strand input allocation from the hot form. On a typical 64-bit target, the old in-memory `Strand` shape had at least a 24-byte `String` header plus a 24-byte `Vec` header before payload and allocator metadata; that comparison is architecture-specific and is not itself a budget proof.

### 5. Verification must not become the memory hazard

The Flow verifier uses compressed-sparse-row adjacency, `u32` node IDs, iterative Kahn cycle detection, and iterative reachability. It does not allocate a `Vec` for every node and does not recurse to graph depth.

Capsule irreversible/egress ordering is proven as one explicit dependency chain in linear graph work: every dangerous operation must depend on the immediately preceding dangerous operation. Transitivity orders the complete set without pairwise graph searches.

## Safety invariants

1. **Canonical-first:** tokens are derived only from a capsule that passed independent decoding and admission.
2. **Registry scope:** a token is rejected if the capsule registry CID and token table registry CID differ.
3. **No authority from bits:** `AdmissionTriad` and `TokenProgram` are data, not credentials.
4. **Unknown does not execute:** v0 justification remains `Unknown`; the reduction is `Defer`.
5. **No second wire:** capsule CIDs are computed from the exact canonical bytes, not the token plan.
6. **No hidden topology:** input references remain topological `u32` strand indices and are checked while compacting.
7. **No silent overflow:** arena sizes, indices, and static cost composition use checked arithmetic.
8. **No unsafe substrate code:** `braid-ir`, `braid-verify`, and `braid-sdk` forbid `unsafe`.

## Review findings fixed in this change

- The triad existed in prose but not as a shared root algebra.
- Every canonical strand carried a heap term string into prospective execution.
- Every strand owned a separate input vector instead of a flat arena.
- The SDK allocated temporary capability strings despite `Capability: Ord`.
- The SDK retained an unused parallel output-type vector.
- SDK static cost used saturating arithmetic, which could hide overflow.
- SDK handles from another builder could panic type lookup or silently name the wrong strand; handles are now builder-scoped and rejected.
- The Flow verifier repeatedly cloned node keys into strings and maps.
- The Flow verifier allocated one adjacency vector per node and used recursive DFS.
- Capsule dangerous-effect ordering used repeated pairwise graph searches and allocations.

## Deliberately unresolved gates

These are not papered over by this ADR:

1. **Runtime authority bypass:** `braid-run` still accepts a raw `Capsule`; its API must consume a verifier/planner-owned runnable typestate and external authority context, not self-declared grants.
2. **Runtime allocation amplification:** execution still performs string dispatch and clones every input/output/journal value. It has not yet migrated to `TokenProgram` plus a bounded value arena.
3. **Justification proof:** the Flow planner must evaluate `needed_when` and `satisfied_when` against a CID-bound snapshot and return the third proof.
4. **Choice disjointness:** v0 checks choice shape and duplicate targets; it does not prove predicate disjointness. The verifier must not describe that as solved.
5. **Decoder preflight:** the capsule independent decoder still needs explicit wire-byte and total-value ceilings before allocating the complete canonical value tree.
6. **Cross-process proof transport:** a future runnable artifact needs identity-bound evidence/receipt references; the one-byte triad is insufficient.

## Consequences

- Human- and machine-readable canonical identity is preserved.
- Repeated execution can use dense numeric dispatch without pretending numbers create universal meaning.
- Zero-initialized admission state is safe.
- The current system becomes more honest: it can prove safe and authorized admission, but it visibly defers because need is not yet proven.
- Runtime integration is intentionally a separate change because mixing representation compaction with execution authority would enlarge the trusted computing base and make review weaker.

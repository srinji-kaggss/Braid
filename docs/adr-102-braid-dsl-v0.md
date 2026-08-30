# ADR-102 — Bounded native Braid DSL v0

**Status:** Accepted by explicit Director instruction on 2026-08-29  
**Issue:** [#77](https://github.com/srinji-kaggss/Braid/issues/77)  
**Decision entry:** D33  
**Source contract:** `spec/braid/elaboration/braid-dsl-v0.md`

## Context

The JavaScript expression frontend proves that an external language can lower
through the Braid elaboration seam. It is not the native Braid authoring
surface requested by #77 and cannot express the CMS demo graph as a coherent
declarative source document.

D6 originally kept every custom textual grammar behind strategy triggers.
After reviewing that result, the Director explicitly rejected treating the JS
subset as the DSL and ordered implementation against the GitHub issues and
repository specifications. This is the required D6 decision for one bounded
surface, not permission to invent the broader state, schema, orchestration, or
general-purpose language described in research documents.

## Decision

Braid adopts a versioned `version 0` native source grammar for one Capsule graph
over the closed `cms::v1` vocabulary. It supports explicit intent, registry,
capability and effect assertions, budget, confirmation, evidence, bindings,
namespaced calls, pipelines, and outputs.

The frontend:

1. enforces byte, token, identifier, string, binding, and pipeline ceilings
   before unbounded work;
2. lowers only through `braid_sdk::Builder`;
3. emits the existing canonical Capsule bytes, never a new wire format;
4. compares declared authority and effects exactly with the derived graph;
5. asks the independent `braid-verify` implementation to admit the bytes; and
6. exposes `braid dsl check` and `braid dsl compile`, with optional JSON-of-IR
   output for differential parity checks.

The canonical grammar and refusal set live in
`spec/braid/elaboration/braid-dsl-v0.md`. That file is the versioned language
contract; parser behavior may not outrun it.

## Boundaries

This decision does not unlock schemas, state machines, Flow orchestration,
imports, macros, loops, recurrence, runtime literals, embedded host code, raw
URL expressions, arbitrary vocabularies, a second verifier, or a second wire.
Those require their own substrate and decision records.

The capsule source name remains a source-level label because Capsule v0 has no
canonical name field. It is not smuggled into intent or evidence merely to make
renames alter content identity.

## Acceptance

The implementation is acceptable only when all of the following remain green:

- the three Day-0 CMS sources match the existing JSON-of-IR bytes and pinned
  CIDs;
- ten source programs have pinned CID goldens and at least ten hostile or
  invalid programs fail with stable diagnostic codes;
- changed authority is visible as a manifest widening;
- all three demo sources pass independent admission and reach the proof-gated
  reference execution boundary;
- failed compilation leaves no artifact; and
- the complete repository gate passes.

Issue #77 remains the delivery contract. This ADR does not itself claim that
the issue is closed or that a production runtime exists.

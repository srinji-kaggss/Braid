# Braid Graph-Based DSL: Specification & Lowering Architecture

**Date:** 2026-08-21
**Status:** Approved Specification
**Authority:** Braid Architecture Specification

---

## ADR-099 reconciliation

This document predates the ratified outer Flow boundary. Its `graph` form
describes the inner per-capsule strand DAG. Its `statechart` form is an
authoring projection only: bounded inter-capsule transitions lower to Braid
Frontier Flow, while durable state, journals, resumption, retries, and effects
belong to forge-harness. `~>` recurrence must statically expand within declared
bounds or be rejected by Flow v0. Nothing in this document creates a second
Flow wire format or scheduler.

The visual grammar remains illustrative and is not identity-bearing. In
particular, its `f64` examples must lower to registered fixed-point types or be
rejected under D8; they do not amend Braid's no-float IR rule.

## 1. Overview

Braid's internal Intermediate Representation (IR) is an immutable, content-addressed Directed Acyclic Graph (DAG) of compute strands.

The **Braid Graph DSL** exposes this native graph topology directly in the syntax, providing a visually intuitive, hierarchical, and declarative language for both human developers and LLM autoregressive token generation.

---

## 2. Structural Principles

1. **Explicit Graph Topology:** Dataflow (`->`), state transitions (`=>`), fans (`[A, B] -> C`), and recurrent feedback (`~>`) are first-class language operators.
2. **Hierarchical Subgraphs:** Complex multi-step computations are enclosed in `subgraph` blocks that export specific output nodes, preventing namespace pollution.
3. **Rust `::` Namespacing:** All terms, capabilities, and types use strict `::` scoping (`lw::image::resize`, `lw::cap::net::read`) to ensure unambiguous AST parsing and IDE/LLM autocomplete predictability.
4. **Zero-Branch Compute:** The graph elaborates into an unbranched, topologically sorted array of contiguous memory strands for maximum SIMD throughput.
5. **State & Orchestration as Statecharts:** State machine workflows and human-in-the-loop gates are declared as explicit `statechart` graphs.

---

## 3. Visual Grammar Reference

```rust
capsule lw::example::graph_demo version 1.0 {
    require {
        cap: [lw::cap::fs::read, lw::cap::db::write],
        effects: [lw::effect::read, lw::effect::state::commit],
    }

    schema IngestEvent { id: id::Uuid, value: f64 }
    schema ScoredEvent { id: id::Uuid, score: f64, alert: bool }

    // ─────────────────────────────────────────────────────────────
    // DATAFLOW GRAPH DEFINITION
    // ─────────────────────────────────────────────────────────────
    graph ComputePipeline(source: lw::net::Uri) -> ScoredEvent {
        // Node definition
        node Input: IngestEvent = source -> lw::net::read() -> lw::parse::json()

        // Hierarchical Subgraph
        subgraph MathCore(event: Input) {
            node Normalized: f64 = event.value -> lw::math::z_score(mean: 0.0, std: 1.0)
            node Scaled: f64     = Normalized  -> lw::math::scale(factor: 2.5)
            export Scaled
        }

        // Fan-in Fusion Node
        node Result: ScoredEvent = [Input, MathCore.Scaled]
            -> ScoredEvent {
                id: Input.id,
                score: MathCore.Scaled,
                alert: MathCore.Scaled > 3.0,
            }

        export Result
    }

    // ─────────────────────────────────────────────────────────────
    // STATECHART ORCHESTRATION DEFINITION
    // ─────────────────────────────────────────────────────────────
    statechart PipelineLifecycle {
        state Idle
        state Active    { processed_count: u64 }
        state Suspended { reason: compact_str }

        Idle => Active:
            on lw::event::start
            action lw::state::init()

        Active => Suspended:
            when ComputePipeline.Result.alert == true
            action lw::alert::trigger()

        Suspended => Active:
            require_human_confirm: true
            action lw::state::resume()
    }
}
```

---

## 4. Lowering Pipeline: Graph AST to Braid IR Strands

```
┌─────────────────────────────────────────────────────────┐
│                    Braid Graph DSL                      │
│   Node A -> [SubB.Out, SubC.Out] -> Node D              │
└────────────────────────────┬────────────────────────────┘
                             │
                             ▼ (1. Hierarchical Flattening)
┌─────────────────────────────────────────────────────────┐
│                    Flat Graph DAG                       │
│   Node 0 (A), Node 1 (SubB), Node 2 (SubC), Node 3 (D)  │
└────────────────────────────┬────────────────────────────┘
                             │
                             ▼ (2. Tarjan Topological Sort)
┌─────────────────────────────────────────────────────────┐
│                 Linearized Strand Schedule              │
│   Strand 0 -> Strand 1 -> Strand 2 -> Strand 3          │
└────────────────────────────┬────────────────────────────┘
                             │
                             ▼ (3. Static Offset Plan)
┌─────────────────────────────────────────────────────────┐
│               Contiguous Memory Execution               │
│   Buffer[0..32KB] -> Zero heap allocs during evaluation │
└─────────────────────────────────────────────────────────┘
```

---

## 5. Verification & Safety Guarantees

1. **Acyclic Dataflow Proof:** The compiler runs Tarjan's strongly connected components algorithm to prove that all dataflow graphs are strictly acyclic (except explicit, bounded `~>` state buffers).
2. **Deterministic Canonical CID:** The lowered strand list is serialized using RFC 8949 canonical CBOR and hashed with BLAKE3 to produce the immutable $\text{CID}_{\text{capsule}}$.
3. **Halting & Budget Bounds:** Every node has a statically declared unit cost. The total capsule cost $\sum \text{cost}(\text{node}_i)$ is verified against the ambient resource budget at admission.

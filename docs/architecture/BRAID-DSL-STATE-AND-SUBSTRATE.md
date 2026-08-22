# Braid DSL, Stateful Orchestration & Substrate Architecture

**Date:** 2026-08-21  
**Status:** Approved Architectural Blueprint  
**Method:** Deep Cross-Estate Audit, crates.io Dependency Dredge & Adversarial Falsification Analysis  

---

## 1. Executive Summary & Core Thesis

Braid is a deterministic, content-addressed compiler, verifier, and runtime substrate. Its objective is to replace bloated runtime frameworks (JVM, dynamic JS VMs, heavy container chains) with verified, capability-bounded Intermediate Representation (IR) capsules and sequential execution pipelines.

This document establishes the four pillars that govern Braid and its standard library (`lgwks_std`):
1. **The Low-Dependency Crate Substrate:** Curating battle-tested, zero/near-zero-transitive crates instead of naive hand-rolling.
2. **The Braid Declarative DSL:** Combining Rust-style `::` namespacing with natural, left-to-right dataflow pipelines to maximize human readability and AI next-token predictability while avoiding the pitfalls that killed AppleScript.
3. **Stateful Orchestration & AI Memory:** Eliminating LLM statelessness and multi-agent coordination churn by making state machines, cryptographic journals, and deterministic resumption first-class language primitives.
4. **Adversarial Dredge & Falsification Analysis:** Rigorously verifying all structural claims, boundary limits, and edge-case behaviors.

---

## 2. The Low-Dependency Crate Substrate

### 2.1 The Principle: Zero Transitive Bloat vs. Hand-Rolling Traps
Low-dependency architecture does not mean writing naive, unoptimized toy replacements for complex OS primitives. Mature, battle-tested crates written by domain experts are order-of-magnitude faster, SIMD-accelerated, cache-line aware, and audited.

The golden rule for `lgwks_std` inclusion: **Only admit crates with 0 or near-0 non-optional transitive dependencies, zero unsafe code where possible, and strict MSRV stability.**

### 2.2 Curated Substrate Catalog

| Capability Domain | Battle-Tested Crate | Non-Optional Transitive Deps | Architecture & Performance Rationale |
|---|---|---|---|
| **Future Execution** | `pollster` (v1.0) | **0** | ~40 lines of safe Rust. Drives any `Future` to completion synchronously on the calling thread. Replaces Tokio for CLI tools, verifiers, and DAG evaluation. |
| **MPMC Channels** | `flume` (v0.12) | **1** (`spin`) | Lock-free MPMC queue with cache-padded atomic state. Outperforms `std::sync::mpsc` without async runtime bloat. |
| **I/O Polling** | `polling` (v3.11) | **0 direct platform** (`rustix`/`windows-sys`) | Unified OS event reactor (epoll/kqueue/IOCP). Quarantined strictly to edge network adapters; never leaks into the core IR. |
| **Zero-Alloc Strings** | `compact_str` (v0.10) | **0 runtime** (proc-macro helpers only) | 24-byte Small String Optimization (SSO). Strings $\le 24$ bytes allocate **zero heap memory**; crucial for millions of DAG node labels and symbol paths. |
| **Stack Vectors** | `tinyvec` (v1.12) | **0** (100% safe Rust) | Stack-allocated contiguous vectors with 0 heap allocation for small input/output index arrays (`[0, 1]`). |
| **Zero-Copy Layout** | `zerocopy` (v0.8) | **1** (`zerocopy-derive`) | Google-maintained compile-time layout validation and safe casting from raw byte slices to typed structs. |
| **SIMD Search** | `memchr` (v2.8) | **0** | AVX2/NEON hardware-accelerated byte and substring search for lexers, decoders, and parsers. |
| **HTTP Client** | `ureq` (v3.4) | **Minimal** (`rustls` backing) | Synchronous, blocking HTTP client. Eliminates the multi-megabyte `tokio`+`hyper`+`openssl` tree for API integrations. |
| **CLI Parser** | `pico-args` (v0.5) | **0** | Lightweight linear argument parser. Zero heap allocation, zero macro overhead; replaces 5MB `clap` binaries. |
| **Cryptography** | `blake3`, `sha2`, `ed25519-dalek`, `zeroize` | **0** (pinned) | Audited RustCrypto reference implementations; pinned and vendored under `vendor/` with `PROVENANCE.md`. |

---

## 3. The Braid Declarative DSL: Design & Syntax

### 3.1 What Killed AppleScript & Why Rust `::` is the Solution
AppleScript attempted "natural English" syntax (`tell application "Finder" to set bounds of window 1...`) and failed because:
1. **Syntactic Ambiguity:** Natural language grammar has hundreds of edge cases; parsing is non-deterministic.
2. **Zero Autocomplete / Discovery:** Neither humans nor LLMs can predict valid adverbs, prepositions, or target properties.
3. **No Namespacing:** Global namespace collisions made composition impossible across large systems.
4. **Dynamic Coercion Disasters:** Type errors were deferred to runtime exceptions.

**The Braid Solution:** Combine **declarative left-to-right pipelines (`|>`)** with **Rust's unambiguous namespacing (`::`) and strong static typing**.

### 3.2 DSL Syntax Specification

```rust
capsule lw::telemetry::process_metrics version 1.0 {
    // 1. Explicit Capability Boundary
    require {
        cap: [lw::cap::fs::read, lw::cap::db::write],
        effects: [lw::effect::read, lw::effect::state::commit],
    }

    // 2. First-Class Persistent State Model
    state MetricsSession {
        checkpoint_id: id::Uuid,
        baseline_cid: cid::Cid,
        records_processed: u64,
    }

    // 3. Declarative Schema with Auto-Derived Zero-Copy Codecs
    schema EventRecord {
        user_id: id::Uuid,
        timestamp: time::Rfc3339,
        latency_ms: u32,
        status: enum { Active, Degraded, Inactive },
    }

    // 4. Sequential Dataflow Pipeline
    step ingest(source: lw::net::Uri) -> Stream<EventRecord> {
        source
        |> lw::http::fetch_stream()
        |> lw::parse::json::<EventRecord>()
        |> lw::filter(|e| e.status == Status::Active)
    }

    // 5. Monadic State Update & Effect Execution
    step evaluate(stream: Stream<EventRecord>, inout state: MetricsSession) -> CommitReceipt {
        let scores = stream |> lw::math::compute_anomaly(weight: 1.4);
        state.records_processed += scores.len();
        
        scores |> lw::db::atomic_commit(table: "analytics.scores")
    }
}
```

### 3.3 Elimination of Boilerplate
1. **Schema Auto-Derivation:** `schema` declarations automatically derive:
   - Zero-copy byte layout validation (`zerocopy`).
   - Canonical RFC 8949 deterministic CBOR encoding for CID calculation.
   - Field constraint validation (non-empty strings, numeric ranges).
2. **Composable Capsule Units:** Pipelines do not re-implement authentication, rate limiting, or hashing; they compose verified capsules via `use` imports.

---

## 4. Stateful Orchestration: Solving AI Statelessness & Amnesia

### 4.1 The Problem: The Stateless Prompting Churn
Modern AI agent frameworks (LangChain, CrewAI, AutoGen) treat execution as ephemeral string conversations. If step 4 of an agent workflow fails or needs confirmation, the agent:
- Re-prompts from scratch with conversational history.
- Re-writes or re-executes earlier steps.
- Cannot cryptographically guarantee that previous steps were not silently mutated.

### 4.2 The Solution: Content-Addressed State Machines & Journals

```rust
orchestration lw::deploy::safe_rollout {
    state CanaryState {
        phase: Phase { Init, Deployed, Verified, Promoted, RolledBack },
        error_rate: f64,
        evidence_chain: Vec<cid::Cid>,
    }

    transition init -> deployed {
        verify: [lw::auth::verify_deployer, lw::policy::check_budget],
        action: lw::infra::deploy_canary(traffic_percent: 5),
    }

    transition deployed -> verified {
        gate: lw::observe::monitor_error_rate(threshold: 0.001, window: time::minutes(5)),
        on_breach: trigger rollback,
    }

    transition verified -> promoted {
        require_human_confirm: true, // Freezes state CID, pauses runtime
        action: lw::infra::promote_full(),
    }
}
```

### 4.3 Key Orchestration Invariants
1. **State Snapshots as Merkle Nodes:** Every state transition calculates a new anchor:
   $$\text{CID}_{\text{state}_{n}} = \text{BLAKE3}(\text{CID}_{\text{state}_{n-1}} \parallel \text{CID}_{\text{transition}} \parallel \text{CID}_{\text{evidence}})$$
2. **Zero-Amnesia Deterministic Resumption:** An AI agent resuming an orchestration loads the exact `CID` state anchor. It cannot bypass verification gates or hallucinate past steps.
3. **Audit Evidence Log:** All effectful tool calls emit structured receipts into an append-only journal (`EvidenceLog`), creating an immutable proof chain.

---

## 5. Sequential Unfolding for Massive Compute Gains

To maximize compute performance and eliminate runtime branches:

```
[Declarative DSL / Monadic Pipeline]
                │
                ▼ (Elaborator Pass)
[Linearized SSA Strand Schedule]
                │
                ▼ (Static Offset Resolution)
[Contiguous Memory Buffer (L1/L2 Cache Prefetch)]
                │
                ▼ (SIMD Execution)
[Hardware Vectorized Compute Engine (AVX2/NEON)]
```

- **Branchless Linearization:** The DAG elaborator converts nested logic into an unbranched, topologically sorted array of strands.
- **Cache-Locality:** Input and output buffers are indexed by flat offsets (`inputs: [0, 2]`), allowing CPU prefetchers to operate at full memory bandwidth.
- **Zero Runtime Capability Checks:** All capability boundaries are verified at admission. The inner execution loop runs with **zero permission branching**.

---

## 6. The Contract: Why Braid Rejects Protobuf in Favor of Canonical Zero-Copy

| Property | Google Protocol Buffers (Protobuf) | Braid Canonical Schema / WIT |
|---|---|---|
| **Determinism** | **Non-deterministic on wire** (tag ordering, unknown fields, and default value omission yield divergent hashes). | **100% Bit-for-Bit Deterministic** (RFC 8949 canonical sorting rules ensure unique CIDs). |
| **Toolchain Footprint** | Heavy (`protoc` binary, dynamic reflection tables, 30k-line runtime). | **Zero External Tooling** (Self-describing manifests + lightweight zero-copy structs). |
| **Deserialization Speed** | Allocates object trees in heap memory. | **Instantaneous Zero-Copy** (`zerocopy` reads directly from mmap buffers). |
| **AI Predictability** | Complex `.proto` tags (`tag = 1`) with manual numbering errors. | Explicit, strongly-typed semantic fields (`user_id: id::Uuid`). |

---

## 7. Deep Dredge & Adversarial Falsification Audit

A rigorous engineering review of the architectural claims identifies and resolves the following edge cases:

### Falsification 1: "Can pure linear dataflow express loops and pagination without Turing-complete hangs?"
- **Finding:** Unbounded while-loops make halting undecidable and prevent static capability budgeting.
- **Resolution:** Braid explicitly forbids arbitrary `while`/`loop` constructs. Repetition is constrained to:
  1. **Bounded Monadic Collections:** `map`, `fold`, `scan`, `filter`, `chunk`.
  2. **Explicit State Transitions:** Iterations occur across distinct, journaled state transitions with bounded counters.
- **Invariant:** Every capsule execution is mathematically guaranteed to terminate within its declared resource budget.

### Falsification 2: "How does content-addressing hold when external effects are non-deterministic?"
- **Finding:** A network fetch or clock read returns different bytes across runs, which would break static CID reproducibility.
- **Resolution:** Braid enforces a strict **Two-Plane Model**:
  1. **The Capsule Plane (Static Intent):** Content-addressed by structure, terms, and capability requirements ($\text{CID}_{\text{capsule}}$).
  2. **The Evidence Journal Plane (Dynamic Execution):** External effect payloads are recorded with their raw byte hash into an append-only cryptographic journal. Replay verifies the recorded evidence against the static capsule rather than re-querying the non-deterministic outside world.

### Falsification 3: "Does a forgiving elaborator compromise cryptographic security?"
- **Finding:** Allowing an AI authoring compiler to "guess" intent could smuggle unintended permissions or relax validation.
- **Resolution:** The Elaborator's role is strictly limited to **Syntactic Normalization** (e.g. URI normalization, RFC 3339 date standardization, symbol canonicalization).
  - The Elaborator must emit the normalized canonical source back to the author.
  - The independent **Verifier admits ONLY canonical bytes** and rejects any ambiguity with typed refusals.
  - **Rule:** *Forgiving at the authoring frontend, uncompromising at the admission gate.*

---

## 8. Implementation Roadmap & Milestones

1. **`lgwks_std` Substrate Consolidation:**
   - Integrate `compact_str`, `tinyvec`, `pollster`, and `zerocopy`.
   - Complete `lgwks_std::task::block_on` and `lgwks_std::id`.
2. **Braid DSL Parser & Elaborator (`braid-elaborate-dsl`):**
   - Implement the recursive descent parser for `capsule`, `schema`, `state`, and `step` blocks with `::` scoping and `|>` pipelines.
   - Lower DSL AST into canonical `braid-ir::Capsule` representations.
3. **Execution Engine (`braid-run`):**
   - Implement the unbranched, sequentially unfolded DAG interpreter over contiguous flat buffers.
   - Wire state machine transition journaling and deterministic replay.
4. **Cross-Repo Rollout:**
   - Enforce `lgwks_std_gate` across `forge-sdk`, `forge-harness`, and `rust-ai-stack`.

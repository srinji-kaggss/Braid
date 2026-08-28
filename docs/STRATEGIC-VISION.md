# Braid — End-User Strategic Vision

**Status:** DRAFT for Director review — 2026-08-28
**Audience:** human product owner / specialist auditor / AI agent operator
**Authority:** ADR-088 (machine-first framework), ADR-099 (frontier Flow), ADR-100 (elaboration seam), Constellation Charter
**Scope:** what an end user sees, owns, and verifies — not crate internals

---

## 1. One-sentence promise

**AI authors code as data; humans own design as a rendered manifest; a deterministic verifier owns the safety floor. No human review can be bypassed, no AI prompt can mint authority, and every production decision is replayable from its content hash.**

Braid is not a framework you rewrite your app for. It is a compiler and gate you put *between* authoring and execution. If the gate does not admit it, it does not run — on any machine, in any workspace, for any tenant.

---

## 2. Who the end user is

| Actor | Owns | Sees | Cannot do |
|---|---|---|---|
| **Human specialist / Director** | architecture, intent, capability grants, budgets, confirmation policy, vocabulary choice | CID-bound manifest, diff, DOT projection, journal evidence | cannot be bypassed by an AI prompt; confirmation is a hash-bound gate, not a button |
| **AI coder / architect agent** | authoring — Rust SDK, RON source, or raw bytes | typed verifier verdicts (`REJECT` with reason, not "try again") | cannot invent authority, cannot execute, cannot persist state |
| **Verifier (braid-verify / braid-flow-verify)** | admits or rejects — no discretion, no fallback | canonical bytes only | cannot be coerced by author |
| **Runtime (braid-run) + forge-harness** | executes exactly one admitted capsule/plan step under a snapshot | journaled evidence, Plan CID | cannot execute the unadmitted |

The law that governs every change is `AGENTS.md: Production Engineering Law` — multi-tenant, hostile input, retries, restarts, bounded resources, version skew. A feature that works only for one user on one machine is a fixture, not the product.

---

## 3. End-to-end data-flow — the five DAGs

### 3.1 System-level lifecycle (the only path to production)

Every artifact reaches production through the same six gates. There is no second wire format, no side door, no "internal" build.

```mermaid
flowchart TB
    subgraph Authoring["Authoring (any surface)"]
        A1["Rust SDK builder<br/>typed Strand handles"]
        A2["RON source<br/>first-class text (ADR-099 D-FLOW.5)"]
        A3["Raw canonical bytes<br/>interop / importers"]
    end

    A1 --> AST
    A2 -->|"parse & lower<br/>16 MiB / depth 64"| AST
    A3 -->|"strict import"| AST

    AST["Braid / Flow source AST<br/>closed vocab, bounded"]

    AST -->|"normalize +<br/>deterministic encode<br/>RFC 8949 CBOR"| Bytes["Canonical bytes<br/>single wire"]

    Bytes --> CID["BLAKE3 CID<br/>lw.braid.capsule.v0<br/>lw.braid.flow.v0<br/>lw.braid.flow.plan.v0"]

    Bytes --> Verify{"Independent verifier<br/>braid-verify / braid-flow-verify<br/>fail-closed, 6-stage pipeline"}

    Verify -->|"ADMIT + manifest CID"| Manifest["Deterministic manifest<br/>+ DOT projection<br/>CID-bound"]

    Manifest --> Human{"Human audit<br/>braid diff old→new<br/>widening = hard gate"}

    Human -->|"approve"| Plan["Snapshot-bound planner<br/>braid-flow-plan<br/>satiation-first, at most 1 step"]

    Plan -->|"Plan CID covers<br/>snapshot + target + cache + version"| Next["Next trusted step<br/>InvokeCapsule / Choice / JoinAll"]

    Next -->|"forge-harness<br/>owns lease, journal, retry, crash"| Exec["braid-run DAG interpreter<br/>topo order, cap-gated, budgeted"]

    Exec -->|"journal + evidence CID"| Evidence["Append-only evidence<br/>replayable, content-addressed"]

    Evidence --> Render["Re-rendered manifest<br/>runtime re-derives CID<br/>mismatch ⇒ refuse to load"]

    style Verify fill:#fee,stroke:#c00
    style Human fill:#ffe,stroke:#a80
    style Plan fill:#eef,stroke:#00a
    style Exec fill:#efe,stroke:#0a0
```

**Invariants that never break:**
- `only known-good may execute` — `Unknown` never authorizes.
- `no second wire` — RON/JSON/YAML spelling differences never fork identity.
- `snapshot-bound` — stale proof invalidated on state change.
- `separation of authority` — Braid never mints capability; kernel does.

---

### 3.2 Inner capsule DAG — the computation graph (what the AI authors)

One capsule is a DAG of **strands** (one typed term application). Edges are typed value flows. The graph is immutable, content-addressed, and branchless at execution.

```mermaid
flowchart LR
    subgraph Capsule["Capsule CID = BLAKE3(canonical Braid bytes)<br/>grants=[fs.read, db.write]  budget=420  confirm=None"]
        direction TB
        S0["Strand 0<br/>lw.net.read<br/>Pure::Read<br/>cap: net.read"]
        S1["Strand 1<br/>lw.parse.json<br/>Pure"]
        S2["Strand 2<br/>lw.math.z_score<br/>Pure"]
        S3["Strand 3<br/>lw.math.scale<br/>Pure"]
        S4["Strand 4<br/>lw.db.atomic_commit<br/>ReversibleWrite<br/>cap: db.write"]
        S0 --> S1 --> S2 --> S3 --> S4
        S0 -.->|"typed edge<br/>Stream<Bytes>"| S1
        S1 -.->|"EventRecord"| S2
    end

    subgraph Linearize["Execution view (braid-run)"]
        direction LR
        L0["Strand 0"] --> L1["Strand 1"] --> L2["Strand 2"] --> L3["Strand 3"] --> L4["Strand 4"]
        note["topological order<br/>inputs are flat u32 offsets<br/>contiguous arena 12N+4E+4O bytes<br/>zero alloc during eval"]
    end

    Capsule -->|"flatten + topo sort<br/>Tarjan SCC proves acyclic"| Linearize
```

**Lowering pipeline (from `BRAID-GRAPH-DSL.md`):**
`Graph DSL / subgraph / fan-in` → hierarchical flatten → Tarjan SCC (acyclic proof except bounded `~>`) → linear strand schedule → static offset plan → contiguous buffer → SIMD-friendly evaluation.

Each strand declares `inputs: [u32]` as offsets into the flat arena, not per-strand allocations (ADR-101 compact projection).

---

### 3.3 Outer Flow DAG — wiring capsules into justified work (what the human designs)

The outer graph does **not** inline capsule internals. It invokes already-admitted capsules by CID, gated by three-valued predicates.

```mermaid
flowchart TB
    subgraph FlowIR["Flow IR — lw.braid.flow.v0 — 4 node kinds, 2 edge kinds"]
        direction TB
        N0["InvokeCapsule<br/>capsule: CID(edit-home-hero)<br/>needed_when: backlog>10<br/>satisfied_when: backlog≤10"]
        N1["Choice<br/>predicate: latency>10ms vs ≤10ms<br/>disjoint Proven/Disproven"]
        N2["InvokeCapsule<br/>capsule: CID(compact_index)<br/>guarantees: frag' < frag"]
        N3["JoinAll<br/>needs: [N2a, N2b] done"]
        N4["Terminal"]

        N0 -->|"Data: ScoredEvent<br/>or After"| N1
        N1 -->|"True →"| N2
        N1 -->|"False →"| N3
        N2 --> N3
        N3 --> N4
    end

    Snapshot["Immutable FlowSnapshot<br/>CID(lw.braid.flow.snapshot.v0)<br/>fact bindings + proof freshness"]

    PlanStep["Plan step<br/>satiated=[...]<br/>next=Choice(N1)<br/>trace=[satiation checks]<br/>Plan CID = lw.braid.flow.plan.v0"]

    FlowIR -->|"eval predicates<br/>three-valued: Proven/Disproven/Unknown"| Eval{"eval_predicate<br/>Unknown ⇒ fail-closed"}
    Snapshot --> Eval
    Eval -->|"satiation-first §9.2<br/>ready antichain §10<br/>stable urgency→CID ranking"| PlanStep

    style N1 fill:#ffe,stroke:#a80
    style Eval fill:#fee,stroke:#c00
    style PlanStep fill:#eef,stroke:#00a
```

**Trust split (ADR-099 D-FLOW.1):**
- Braid owns: semantic Flow AST/IR, canonical encoding, Flow CID, independent static admission, deterministic snapshot-bound frontier derivation.
- forge-harness owns: durable instance, lease/fencing, event history, workers, retries, crash recovery, evidence persistence, receipts. Braid emits *at most one sequential next step*; it does not schedule, persist, or retry.

Closed v0 kinds: `InvokeCapsule`, `Choice`, `JoinAll`, `Terminal` — no `JoinAny`, no races, no unbounded `MapStatic` (expands before encoding). Edges are `Data` (typed capsule port) or `After`.

---

### 3.4 Verification pipeline — six fail-closed gates (the safety floor)

Every byte passes through independent stages in order. Any stage may REJECT with a typed reason; forgiving at authoring, uncompromising at gate.

```mermaid
flowchart LR
    Bytes["Canonical bytes<br/>≤16 MiB wire<br/>≤128 MiB preflight"] --> S1

    S1["Stage 1<br/>Structure<br/>CBOR bijection guard"] --> S2
    S2["Stage 2<br/>Types<br/>closed TypeTag universe<br/>no floats"] --> S3
    S3["Stage 3<br/>Capabilities<br/>attenuation only<br/>grant envelope check"] --> S4
    S4["Stage 4<br/>Effects<br/>Reversible/Irreversible/Egress<br/>confirm policy required"] --> S5
    S5["Stage 5<br/>Taint<br/>path-level fold<br/>max(parent)+producer"] --> S6
    S6["Stage 6<br/>Bounds<br/>budget & cost<br/>checked accumulation"] --> Verdict

    Verdict{{"Verdict<br/>Admit / Reject(reason)<br/>manifest CID bound"}} -->|"Admit"| Proof["AdmissionProof<br/>opaque, CID triple:<br/>capsule + registry + authority"]
    Verdict -->|"Reject"| Block["Typed refusal<br/>no fallback"]

    style S3 fill:#fee,stroke:#c00
    style S5 fill:#fee,stroke:#c00
    style Verdict fill:#efe,stroke:#0a0
    style Proof fill:#eef,stroke:#00a
```

**Boundary guarantee:** substrate crates `braid-ir / braid-verify / braid-capability` fail the build if an unapproved dep or import appears (`crates/braid-ir/tests/boundary_conformance.rs`, `lgwks-std-gate`).

---

### 3.5 Justified invocation — the triad gate (why a call exists at all)

The most distinctive product property: **a safe, authorized, correctly implemented function may still be the wrong invocation**. Execution requires the conjunction:

```
Run(f,S,G) ⇔ Safe(f,S) ∧ Authorized(f,S) ∧ Justified(f,S,G)
```

where `S` is the immutable snapshot and `G` is the declared `satisfied_when`.

```mermaid
flowchart TB
    S["Snapshot state<br/>versioned, content-addressed"] --> Need{"needed_when(S)?"}

    Need -->|"Proven<br/>unsatisfied"| Guarantees{"Action guarantees<br/>progress toward G?"}
    Need -->|"Disproven<br/>already satisfied"| Satiated["SATISFIATED<br/>do nothing — valid winner"]
    Need -->|"Unknown<br/>insufficient proof"| Defer["DEFER<br/>fail-closed<br/>surface MissingEvidence"]

    Guarantees -->|"Proven<br/>frag' < frag"| Preserves{"preserves invariants<br/>data_integrity?"}
    Guarantees -->|"Unknown "| Defer

    Preserves -->|"Proven"| Cost{"deterministic cost order<br/>equivalent winners ⇒ cheapest"}
    Preserves -->|"Disproven"| Reject["REJECT<br/>global invariants dominate cost"]
    Preserves -->|"Unknown"| Defer

    Cost --> Execute["ADMIT next step<br/>Proof = (AdmissionProof + JustificationProof)<br/>bound to snapshot CID"]
    Execute -->|"snapshot changes<br/>before dispatch"| Invalidate["STALE ⇒ re-evaluate"]

    style Satiated fill:#efe,stroke:#0a0
    style Defer fill:#ffe,stroke:#a80
    style Reject fill:#fee,stroke:#c00
    style Execute fill:#eef,stroke:#00a
    style Invalidate fill:#fee,stroke:#c00
```

**Three-valued logic is the product:** `Proven / Disproven / Unknown`; `Unknown` never executes or satiates. The `satisfied_when` predicate makes *doing nothing* a first-class, auditable winner. The proof binds to the exact snapshot CID and is invalidated on state change — no check/use race.

Evidence: `braid-flow-plan` evaluation is total, stable, and CID-bound; `braid-verify` AdmissionProof binds capsule/registry/authority CIDs; `braid-run` RunnableInvocation consumes both proofs and has no raw-capsule public entry.

---

### 3.6 Durable execution — forge-harness owns the lifecycle (Braid owns the decision)

Braid derives the trusted next step; forge-harness makes it durable.

```mermaid
sequenceDiagram
    participant H as Human
    participant B as Braid (planner)
    participant F as forge-harness (durable)
    participant R as braid-run (interpreter)
    participant J as Evidence journal

    H->>B: admitted Flow + immutable snapshot CID
    B->>B: eval satiation, predicates, ready antichain
    B-->>F: Plan { satiated, next_step, Plan CID }

    F->>F: fence / lease, append history
    F->>R: RunnableInvocation(AdmissionProof + JustificationProof + snapshot)

    R->>R: topo walk, cap-gated dispatch, budget + confirm checks
    R-->>F: Journal { strand outputs, unit costs, cap checks }
    F->>J: append-only, content-addressed, replayable

    F-->>H: receipt + manifest re-render
    H->>H: braid diff — widening?
    Note over B,F: B never retries, persists, or leases.<br/>F never admits or invents a ready node.

    F->>B: on wake / after transition: new snapshot CID?
    B->>F: new Plan (deterministic over new snapshot)
```

If the instance crashes mid-step, forge-harness resumes from the fenced history and asks Braid for a plan under the exact snapshot — it cannot bypass admission or invent readiness.

---

## 4. End-user journeys (the real product paths)

### Journey A — AI authors, human audits, capsule ships

```
AI (SDK/RON) → elaborate → canonical bytes → braid verify
    → REJECT(typed reason: missing cap / effect without confirm / taint) → AI re-authors from reason
    → ADMIT → braid render → manifest + DOT
    → human: braid diff base→new → approve (or reject widening)
    → braid-run (proof-gated) → journal → evidence CID
```

The AI loop is two-state: `AttemptPlan` → `FailureRequiresFix` → `RecoveryImpact`. The typed refusal is the API; there is no "confidence score" — the LLM either addresses the refusal or the artifact never ships.

Real gates exercised: laundering capsule REJECT at taint (hostile case), irreversible publish without confirm REJECT at effect, seeded widening flagged in CI (`scripts/cli-loop.sh`, T12).

### Journey B — Flow orchestrates multiple capsules with justification

```
Human designs Flow (Invoke/Choice/JoinAll) in RON
  → braid-flow-verify admits structure, predicates bounded
  → snapshot: { backlog: 34, latency: 7ms }
  → plan: satiation? no → ready antichain = [compact_index] → next=InvokeCapsule(CID...)
  → RunnableInvocation(AdmissionProof + JustificationProof) → braid-run executes one capsule
  → new snapshot { backlog: 3, latency: 7ms }
  → next plan: satisfied_when(backlog≤10) Proven → satiated → do nothing wins
```

Doing nothing is the product: the same Flow that authorized work now deterministically refuses it.

### Journey C — Hostile / malformed / resource-exhaustion path

Every hostile input is a typed rejection, not a panic, not a silent fallback:

- Unknown capability → `MissingCapability` / `UnregisteredCrate` (boundary gate)
- Float smuggled → `Rejected at canonical-form`
- `let _ =` / `.ok()` swallowed → `swallow-budget ≤5` CI gate
- Massive wire → 16 MiB / 128 MiB preflight, 262k Value node ceiling
- Unknown justification → `UnknownProof` fail-closed; registry transplant → `InvalidRunnableProof`

Evidence: `cargo test --workspace --all-targets` (225+ suites today), `cargo clippy -- -D warnings`, `cargo fmt --check`, `lgwks_std_gate`, plus KAT vectors and mutation-ledger prover for anti-dredging.

### Journey D — Day-0 CMS reference (shipped)

Three real capsules, not fixtures, modeled on `blueprints/afternow-port`:

- `edit-home-hero` (reversible, local) — ADMIT
- `publish-services` (irreversible, HumanConfirm) — ADMIT
- `publish-services-noconfirm` (escalation probe) — AUTHOR-REFUSED at build time (exit 2, `ConfirmRequired`)

Evidence bundle regenerable via `scripts/demo-port.sh`, verified in `scripts/cli-loop.sh`, CIDs pinned and parity-checked.

---

## 5. What the manifest shows an auditor (the review surface)

The manifest is CID-bound and re-derivable by the runtime — mismatch ⇒ refuse to load.

- Intent and work-object binding
- Full capability list (capability names are dotted tokens, e.g., `db.write`)
- Effect classes present, with `Irreversible`/`Egress` highlighted
- Bounds, budget, confirmation and evidence policy, version pins
- Flow nodes, edges, predicates, guarantees, satiation trace
- Widening vs narrowing classification against previous manifest (CI gate)

Review is a diff, not a free-form read. A capability-widening PR is red-teamed in CI to prove the gate fires.

---

## 6. Architecture placement — where Braid sits in the constellation

```
AI / SDK / RON  ──authors──▶  Braid Flow SDK (parse → normalize → canonical)
                                         │
                                         ▼
                              Braid Flow IR  (lw.braid.flow.v0)
                              Braid Flow Verify (independent admit)
                                         │
                                         ▼
                              Braid IR (lw.braid.capsule.v0)
                              Braid Verify (independent admit)
                              Braid Render (manifest + DOT)
                                         │
                              ┌──────────┴──────────┐
                              ▼                     ▼
                    Braid Project        Braid Integrator
                    (multi-capsule,      (repo-graph advisor,
                     deterministic         proposes lgwks_std seams,
                     project CID)          never verifies)
                                         │
                                         ▼
                              Braid Flow Plan (snapshot-bound frontier)
                                         │
                              ┌──────────┴──────────┐
                              ▼                     ▼
                    braid-run (one capsule,          forge-harness
                    topo, cap-gated)                 (durable instance,
                                                     journal, lease, retry)
```

**Authority owners (Charter G2):**
- Braid: `Cid` (BLAKE3, `cid.rs`) + canonical bytes + term vocabularies.
- kernel: capability envelope, assignment record.
- forge-harness: receipt, work-object lifecycle.
- Braid never claims `Capability`, `Verdict`, `Fact`, `Principal`, `Receipt`, `WorkObject`.

Consumers path-dep `braid-ir` + `braid-vocab-cms`; browser vendored snapshot is deprecated on next publish (Charter Step 3).

---

## 7. Strategic roadmap — end-user-visible milestones

| Milestone | End-user outcome | Verifiable gate |
|---|---|---|
| **v0 — verified substrate (DONE)** | typed IR, verifier, render, CLI loop, budget/taint/confirm gates | 225+ tests, KATs, bijection fuzz, seeded widening red-team |
| **v0.1 — language frontends** | `braid-elaborate-js` (expression → statement, identifiers, lexical scope), golden/refusal corpus | pinned CIDs, depth-bound refusal, mutation guards |
| **v0.2 — project build** | `braid-project build` — elaborate + admit all caps, fail-closed, deterministic project CID | `cargo test -p braid-project` |
| **v0.3 — frontier Flow (NOW)** | `braid-flow-ir` + `braid-flow-verify` + `braid-flow-plan` + `braid-run` runnable proof typestate | 8 plan invariants, 9 runnable proofs, snapshot-bound refusal, registry transplant rejection |
| **v1 — first live consumer** | kernel or browser consumes published crate, collapses parallel enum, live API is Braid | `use braid_ir::Capsule` in downstream repo, not just `Cargo.toml` |
| **v1.1 — publish discipline** | crates publishable, semver on `Cid`/encoding breaks (G4 charter gate) | `cargo publish --dry-run` for all `braid-*` |
| **v1.2 — vocabulary scale** | `braid-vocab-*` covers real product surface (not just expressions) | literal payloads (D8), ported vocabularies |
| **v2 — durable orchestration** | Flow steps execute under forge-harness with evidence journals, deterministic replay | forge-harness #123 closed, cross-repo performance proof (10× vs re-execution) |

Non-goals remain locked per D6: textual surface syntax beyond RON/JSON interop, new storage/crypto, ML in admission path, package registry/FFI — until a ratified ADR says otherwise.

---

## 8. Why this vision is falsifiable (not marketing)

Every claim above is tied to a command that either passes or it does not:

```bash
cargo check --workspace
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
scripts/cli-loop.sh target/debug/braid
scripts/demo-port.sh target/debug/braid
cargo test -p braid-flow-plan --test plan_invariants
cargo test -p braid-run --test execution   # RunnableInvocation proofs
cargo test -p braid-ir --test boundary_conformance
```

A new `braid-verify` or `braid-flow-verify` change that breaks a KAT or bijection, a widening that CI does not flag, a swallowed error that pushes `let _ =` past 5, an unapproved dep that slips `lgwks_std_gate`, or a plan that dispatches under `Unknown` — all are build-red, not opinion-red.

---

## 9. Glossary — the ten terms an end user needs

- **Strand** — one typed term application (registered operator + signature + cap + cost).
- **Braid** — DAG of strands; composition legal only if types unify and authority attenuates.
- **Capsule** — braid + intent + grants + bounds + confirm/evidence policy; the admitted artifact.
- **CID** — BLAKE3 hash over canonical bytes; domain `lw.braid.*`; the identity.
- **Manifest** — deterministic rendering of an admitted capsule/Flow; the audit object.
- **Verifier** — independent, fail-closed admission pipeline; never co-located with authoring.
- **Flow** — outer DAG of capsules (`InvokeCapsule`, `Choice`, `JoinAll`, `Terminal`).
- **Snapshot** — immutable, content-addressed fact bindings; proofs are bound to it.
- **Plan** — satiation + at most one ready step; Plan CID covers all inputs.
- **Justification** — `needed_when / satisfied_when / guarantees / preserves / cost_order`; `Unknown` never executes.

---

## 10. How to start (for a team adopting Braid today)

1. **Read the audit surface:** `cargo run -p braid-cli -- render <capsule.braid>` and `diff`.
2. **Wire the advisor:** `cargo run -p braid-integrate -- --json` in your repo; apply proposed `lgwks_std` seams.
3. **Author a capsule:** Rust SDK → `braid-cli encode → verify → render`. Keep the `REJECT` reason — it is the API.
4. **Hold the widening gate:** add `scripts/cli-loop.sh` to your CI; seed a widening and watch it fail.
5. **When ready for orchestration:** author Flow in RON → `braid-flow-verify` → `braid-flow-plan` → `RunnableInvocation` → `braid-run`. Do not persist Flow state in Braid.

---

*This vision is additive to `AGENTS.md` Production Engineering Law and the Constellation Charter. Where a repo-local doc contradicts the charter, the charter wins. License: BSD-3-Clause.*

# Braid End-State Production Plan

**Generated:** 2026-07-16
**Status:** Ratified by Director
**Method:** Adversarial review + playbook analysis + cross-repo consumer audit

---

## Vision

Braid is a compiler. Source languages elaborate into content-addressed, capability-scoped IR. An independent verifier admits only what the manifest declares. A runtime executes admitted capsules under capability gates. The end-state replaces the JVM ecosystem: IR + verifier + vocabulary stdlib + runtime + toolchain, where verified IR is the authority surface instead of any language's runtime.

Current state: "a JVM with no V" — the `.class` + verifier layer works, a front-end compiler exists for a tiny expression subset, a project builder exists, but no VM, no stdlib at scale, and 14 repos waiting to consume.

---

## Decisions Ratified (2026-07-16)

| Decision | Verdict |
|----------|---------|
| D5 (capability model) | **RATIFIED** |
| D16 (vocab versioning) | **RATIFIED** |
| D31 (substrate/vocab split, "replace Java" = IR becomes authority surface) | **RATIFIED** |
| D32 (adopt Keel for safety-assurance floor) | **RATIFIED** |

---

## Workstreams

### WS-1: Execution Leg (PB-02) — CRITICAL PATH

**Goal:** Capsules run. Today they verify but never execute.

**Approach:** Rust-native DAG interpreter with Rust-Abi migration path.

**Rationale:** Rust-Abi (`/Users/srinji/Rust-Abi`) has a `LoadPlan::execute()` pattern (validate metadata at load time → allow calls) that maps directly to Braid's "runtime admission" stage. But Rust-Abi has no compiler yet (design stage, 14 crates synthesized). The practical path:

1. **Phase 1 (now):** Build `braid-run` as a Rust-native DAG interpreter. Walk strands in topological order, dispatch pure terms via a `Host` trait, gate effects via capability checks, journal evidence.
2. **Phase 2 (when Rust-Abi ships a compiler):** Migrate `braid-run` to emit Rust-Abi native components. The `Host` trait becomes the Rust-Abi vtable contract. Load-time re-verification uses Rust-Abi's L0/L1/L2 verifier.

**Deliverables:**
- [x] New `braid-run` crate with `Host` trait + `execute(capsule, host) -> Journal`
- [x] Pure-term evaluation in DAG order (topological sort, strand-by-strand)
- [x] Effect dispatch: capability-gated, confirm-policy enforced, journal entries emitted
- [x] `MockHost` for tests: records all effect calls, enforces capability grants
- [x] Demo-port capsule execution: encode → verify → execute → journal committed
- [x] U9-style adversarial pass on the runner (capability escalation, budget exhaustion, confirm bypass)
- [x] Rust-Abi integration spec: documented in `docs/architecture/BRAID-DSL-STATE-AND-SUBSTRATE.md`

**Blocked by:** Nothing. Start now.

---

### WS-2: Elaborator Chunks (PB-03) — PARALLEL WITH WS-1

**Goal:** Make Java defunct by building the `javac` of the Braid ecosystem. Take in chunks.

**Chunk 1 (now):** Expand `braid-elaborate-js` from expressions → statements + identifiers.

**Deliverables:**
- [x] `let` bindings with type inference from RHS
- [x] Identifier resolution (variable lookup → `js.lit.*` or strand reference)
- [x] `if`/`else` as conditional strands (not statements — DAG nodes)
- [x] Function calls (pure only: `js.add`, `js.concat`, etc.)
- [x] Refusal corpus: 10+ illegal sources with typed errors (eval attempt, DOM-string emission, float literal, unbounded loop, mutation)
- [x] Golden corpus: 10+ source files with pinned capsule CIDs

**Chunk 2 (after chunk 1 lands):** JSX/TSX component subset → `braid-vocab-web`.

**Deliverables:**
- [ ] Component definition (props, state, render function)
- [ ] List render (`map` → `web.navigate` + `web.interact`)
- [ ] Event-to-capability dispatch (onClick → `web.interact.write` grant)
- [ ] Projection read (state access → `web.compute.local` capability)
- [ ] Reject-to-reauthor loop demo: LLM authors source → elaborate → admit-or-reject → re-author from typed reason

**Blocked by:** Nothing. Chunk 1 starts now. Chunk 2 starts when chunk 1 lands.

---

### WS-3: First Consumers (PB-04) — PARALLEL, CHEAP

**Goal:** File GH issues on all 14 consumer repos. Collapse parallel `BraidTerm` enums onto `braid-ir`.

**Consumer repos (ranked by readiness):**

| # | Repo | Status | Issue to file |
|---|------|--------|---------------|
| 1 | `next-gen-browser-engine` | DIRECT CONSUMER (99 files) | Collapse `BraidTerm` → `braid_ir::Capsule`, replace SHA-256 with BLAKE3 |
| 2 | `logic-os-kernel` | DIRECT CONSUMER (60 files) | Live-wire `braid_vocab_binding.rs` to decode `registry_v0()` |
| 3 | `moo` | DIRECT CONSUMER (config-level) | Add `braid-ir` + `braid-verify` as crate deps, wire `Moo.toml` paths |
| 4 | `rust-ai-stack` | DIRECT CONSUMER (vendored) | Update vendored Braid to latest, add integration test |
| 5 | `forge-sdk` | BRAID-AWARE (16 files) | Add `braid-ir` as crate dep, wire `keel_braid.rs` tests |
| 6 | `keel` | BRAID-AWARE (17 files) | Add `braid-ir` as crate dep, formalize anchor/projection co-design |
| 7 | `logicalworks-` | BRAID-AWARE (48 files) | Add `braid-ir` as crate dep, encode obligations as runtime checks |
| 8 | `harness-sdk` | BRAID-AWARE (transitive) | Add `braid-ir` as crate dep via `forge-sdk` |
| 9 | `experience-as-code` | BRAID-AWARE (integration plan) | Add `braid-ir` + `braid-capability` as crate deps |
| 10 | `google-genai-rs` | BRAID-AWARE (pattern-inspired) | Add `braid-ir` as crate dep, wire manifest type |
| 11 | `settling-field-lab` | BRAID-AWARE (research) | Document Braid integration path |
| 12 | `batch-code` | BRAID-AWARE (work-units) | Update work-unit descriptions with Braid status |
| 13 | `rust-abi` | POTENTIAL (high fit) | Evaluate Braid CID integration with `abi-manifest` |
| 14 | `claudex` | POTENTIAL (agent) | Evaluate Braid capability scoping for agent outputs |

**Blocked by:** Nothing. File issues now.

---

### WS-4: Documentation Hygiene — TRIVIAL

**Goal:** Update stale docs to match reality.

**Deliverables:**
- [ ] README: update test count from 135 → 190
- [ ] DEBT_REGISTER: mark D-RUN as "in progress" (WS-1 started), D-CONSUMER as "in progress" (WS-3 started)
- [ ] DECISIONS.md: mark D5, D16, D31, D32 as RATIFIED (was INTERPRETED)

**Blocked by:** Nothing. Do now.

---

## Execution Order

```
WS-4 (docs) ──────────────────────────────────────────────> done (trivial)
WS-3 (issues) ────────────────────────────────────────────> done (file issues)
WS-1 (braid-run) ─────────────────────────────────────────> critical path
WS-2 chunk 1 (statements) ────────────────────────────────> parallel with WS-1
WS-2 chunk 2 (components) ────────────────────────────────> after chunk 1
```

**Parallelism:** WS-1, WS-2-chunk-1, WS-3, WS-4 can all run in parallel. WS-2-chunk-2 waits for chunk-1.

---

## Success Criteria

| Criterion | How to verify |
|-----------|---------------|
| Capsules execute | `cargo test -p braid-run` passes; demo-port capsule runs end-to-end |
| Elaborator covers statements | `cargo test -p braid-elaborate-js` passes; golden corpus CIDs match |
| 14 repos have GH issues | `gh issue list --repo srinji-kaggss/<repo> --label braid-integration` |
| Docs match reality | README says 190 tests; DEBT_REGISTER reflects WS-1/WS-3 status |
| CI green | `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` |

---

## Known Gaps (Not Fixed, Documented)

| Gap | Why not fixed | Owner |
|-----|---------------|-------|
| `expires_at` never enforced against a clock | Needs clock injection abstraction (substrate change) | WS-1 (braid-run) |
| `ConfirmPolicy` payload-hash binding not enforced | Needs verifier changes (substrate change) | WS-1 (braid-run) |
| `validate_commitment` doesn't check capabilities | Needs `DesignCommitment` schema change | Future |
| `braid-capability` has only 4 tests | Architectural (needs capability model expansion) | Incremental |
| Boundary conformance covers 4/12 crates | Needs `ALLOWED_USE_ROOTS` extension per tier | Incremental |

---

## Agent Dispatch Plan

| Agent | Workstream | Scope |
|-------|------------|-------|
| Agent 1 | WS-1 (braid-run) | Build `braid-run` crate: `Host` trait, DAG interpreter, effect dispatch, MockHost, demo-port test |
| Agent 2 | WS-2 chunk 1 (elaborate-js) | Expand parser: `let`, identifiers, `if`/`else`, function calls, golden + refusal corpus |
| Agent 3 | WS-3 (GH issues) | File integration issues on all 14 consumer repos |
| Agent 4 | WS-4 (docs) | Update README, DEBT_REGISTER, DECISIONS.md |

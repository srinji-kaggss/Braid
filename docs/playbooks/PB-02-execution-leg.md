# PB-02 — The execution leg: make admitted capsules actually run (D-RUN)

**Objective**: close the single biggest production gap — "a verified capsule does not
*run*. This is a JVM with no V" (DEBT_REGISTER D-RUN). Ship an execution path that
preserves every runtime-deferred threat closure: load-time re-verification (T4/T9),
payload-hash-bound one-shot confirmation (T10, acceptance #3), enforced budgets
(T7, acceptance #9), and journaled evidence (U8's deferred leg, issue #6).

**The Director fork to surface first (D28 discipline — do not default it):**
U7 as spec'd is WASM codegen over the kernel three-syscall runtime, which **does not
exist yet** (R2). Options:
- **(A) Minimal Braid-direct interpreter** (debt register's own suggested fallback):
  walk the admitted term DAG in Rust, dispatch effectful terms to a host trait. Fast
  to ship, no codegen, keeps D10 by making the host trait's only real implementation
  the kernel syscall client. Risk: becomes a beloved second runtime that U7 then
  can't displace — mitigate by declaring it the *reference interpreter* (like the
  CLI is the reference loop), never the performance target.
- **(B) Unblock the kernel WASM epic first** — architecturally cleanest, but couples
  Braid's whole production arc to another repo's biggest open work.
- **(C) Wasmtime-embedded runner in the Braid repo** — violates the U7 note "must not
  build a second runtime" unless the kernel epic formally adopts it.
Recommendation to put in the issue: **A**, framed as "the executable semantics of the
IR" (every language has one); WASM codegen (U7) later proves *equivalence against it*.
Director picks; record as a new D-entry in `DECISIONS.md`.

## Deep-learning corpus (read → probe)

1. `spec/braid/units.md` U7 + U8 (the deferred execution-leg text and its seam:
   "capsule CID → kernel runtime load → manifest re-derivation (T4) + fact journal").
2. `spec/braid/threat-model.md` T4, T7, T9, T10 — the runtime halves you now own.
3. `spec/braid/PRD.md` §5 architecture placement + acceptance #3, #9, #10.
4. `crates/braid-verify/src/lib.rs` — the verdict you re-run at load; note
   `verify(bytes, &TermRegistry, &[Capability])` is registry-parametric.
5. `crates/braid-ir/src/capsule.rs` (confirm/evidence/budget fields),
   `crates/braid-vocab-cms/src/lib.rs` + `braid-vocab-web/src/lib.rs` (the effect
   classes and `EgressMediated`/`TerminalDestructive` postures you must honor).
6. Kernel context: ADR-069 (guest app model), the three-syscall surface docs, and
   `blueprints/afternow-port/` (what the demo-port capsules would *do* when run).
7. `spec/braid/vectors/demo-port/` — your first three executable programs.

Probe: run `./scripts/demo-port.sh`; decode `publish-services.json`; hand-trace what
executing its strands should produce; that trace is your interpreter's first test.

## Invariants

- **D10 above all**: execution reaches the world ONLY through the host trait mapping
  to Capability/Projection/Sync. The interpreter adds zero authority; pure terms
  evaluate internally, effectful terms dispatch out.
- **Re-verify at load** (D3 final stages): the runner takes canonical bytes, runs the
  full `braid-verify` admission itself, re-derives the manifest, and refuses on any
  mismatch — even for a capsule "already admitted" upstream (T4/T9).
- **Deterministic execution**: no floats, no ambient time/rand/iteration-order —
  nondeterminism enters only as typed effects (T8). Same capsule + same host
  responses ⇒ same journal.
- **Budgets enforce, never narrate** (T7): a step-metered fuel counter; exhaustion =
  deterministic kill + typed evidence, mapped from declared `cost_bound`s.
- **Confirmation is payload-hash-bound and one-shot** (T10): before an
  `Irreversible`/`Egress` strand fires, the runner demands a confirmation token
  binding BLAKE3(exact effect payload); replay or payload drift ⇒ refuse (acceptance
  #3's runtime half).
- **Journal-before-effect**: evidence per `evidence_policy`, appended before the
  effect dispatches (kernel discipline).

## Execution steps

1. File the fork-decision issue (options A/B/C above); get the Director's pick;
   append the D-entry.
2. (Assuming A) New crate `braid-run`: `Host` trait (capability dispatch, projection
   read, sync, clock/entropy as typed effects) + `run(bytes, registry, ambient, host,
   confirmations) -> Result<Journal, RunRefusal>`. Depends on `braid-verify` +
   `braid-ir` only; boundary test extended to pin its allowlist.
3. Implement pure-term evaluation for the closed type universe; effect dispatch in
   DAG topological order; fuel metering per strand `cost_bound`.
4. Implement the confirmation gate + one-shot token store seam (token issuance
   belongs to the platform layer, PB-05; the runner only checks).
5. `MockHost` for tests (records dispatches, scriptable responses) — this is a test
   double for the HOST, not a mock of the verification path (T14: the admission gate
   itself is never mocked).
6. Wire acceptance scenarios: #3 runtime (confirmation hash mismatch ⇒ refuse),
   #9 runtime (budget exhaustion ⇒ deterministic kill + evidence), #10 (manifest
   mismatch at load ⇒ refuse). Add `braid run` to the CLI so scenario #12's
   human-reconstructable loop extends to execution.
7. Execute the three demo-port capsules against a `FileHost` (a trivial
   content-addressed local store standing in for the kernel tape) and commit the
   journals to `spec/braid/vectors/demo-port/` as pinned evidence — closing U8's
   execution leg for the reference workflow.
8. U9-style adversarial pass on the runner (confirmation replay, journal
   truncation, fuel-bypass via pure-term bombs, host-response smuggling) before any
   "runs" claim (D13).

## Verification

```bash
cargo test -p braid-run                       # incl. scenarios 3/9/10 runtime halves
cargo test --workspace && ./scripts/cli-loop.sh
./scripts/demo-port.sh                        # now includes the execution leg
# mutation: disable load-time re-verify ⇒ scenario-10 test RED
# mutation: disable fuel metering ⇒ scenario-9 test RED
# replay a confirmation token ⇒ refusal with typed reason
```

## Exit criteria

Demo-port capsules execute end-to-end with journaled, committed evidence; acceptance
#3/#9/#10 runtime halves green; adversarial pass verdict written; T7/T9/T10/T14 rows
in the threat model updated from "deferred to U7" to closed-or-superseded; U7 remains
open only as "WASM codegen must match `braid-run` semantics" (a parity target, the
same D9 independence pattern).

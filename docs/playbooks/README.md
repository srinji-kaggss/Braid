# Braid Productionization Playbooks — index + core-problem analysis

> ⚠️ Name provisional (D15). Identity = `spec/braid/` + `DECISIONS.md`, never the word.
>
> **Audience**: an AI agent (or human) dropped in cold, tasked with taking Braid from
> "verified substrate, zero consumers" to a production platform. These playbooks are
> written for **deep learning of the system**: each one names the corpus to read, the
> commands to probe understanding against, the invariants that must never break, and
> the exit criteria that make "done" falsifiable. Authored 2026-07-02 from a full read
> of the spec set + blackbox2 research + memory. Facts below were verified against the
> repo at commit `a9c9a0f` (tests/CI/CLI probed live where claimed).

## 1. The core problems Braid solves (the analysis)

Braid exists because five problems compound in the AI-authorship era, and every
mainstream answer to them is a human or a model — i.e., not an enforcement mechanism.

**P-TRUST — AI authors code nobody can afford to verify.** Review does not scale to
machine-speed authorship, and models cannot be the compliance mechanism for their own
output (T2 trusting-trust). Braid's answer: move enforcement *below* both the model and
the reviewer into a deterministic 8-stage fail-closed verifier (ADR-088 D3): canonical
form → version pin → types → capability attenuation → effect calculus → path-level
taint → bounds → manifest. The verifier owns a **floor** (structural correctness +
confinement safety), never the ceiling (D30's honest deflation: taste stays human).

**P-REVIEW — review fatigue is an attack surface.** Hundreds of diffs train the human
to approve; a capability widening slips through (T12). Braid's answer: the review
object is never code — it is the CID-bound **manifest**, and widenings are detected
*mechanically* (`braid diff` exit 1; seeded-widening red-team in CI). Amortized human
judgment: ratify architecture/vocabulary once, every downstream capsule is checked
against it for free (D25/D28).

**P-MALWARE — detection is undecidable.** Malice is a relation between operation,
data, and authorization; the binary erases the distinguishing information (Rice/Cohen,
D23). Braid's answer: **confinement, not detection** — a closed typed vocabulary makes
ungranted capability *unrepresentable*; "what can this do?" becomes a decidable
read-off-the-type. Six-layer DiD: capability / non-interference (path taint, the
kernel #361→#431 lesson) / effect typing / bounds-totality / payload-hash-bound
confirm / manifest re-check.

**P-CANON — content addressing is usually done wrong.** Two byte-strings claiming one
CID (T3 malleability — the A4.8 governance-ledger exploit happened in-house). Braid's
answer: canonical CBOR subset, no floats (D8), a bijection guard at *every* map level
(the U9 High finding, closed), KAT vectors pinned before any consumer, and external
flight hours (RFC 8949 + BLAKE3 team vectors, `calibration/`).

**P-BABEL — every runtime re-invents its own trust substrate.** The kernel, the
browser engine, and every future surface each need typed terms + capability checks +
content addressing. Braid's answer (D31): a **global translator IR** — a
language-neutral `no_std` substrate (`braid-ir` + `braid-capability`) with
per-domain vocabulary packages (`cms.*`, `js.*`, `web.*`) admitted by the ONE
registry-parametric verifier. "Renders JS useless" = JS becomes an authoring frontend;
the verified IR is the authority surface. Bar: Homebrew-ubiquity dependency.

## 2. The niche (the Vercel mapping)

Braid targets the deployment-platform slot **for AI-authored software** — what Vercel
is to human-pushed frontends, Braid is to machine-authored components. The mapping:

| Vercel primitive | Braid primitive | Why Braid's is stronger for AI authors |
|---|---|---|
| `git push` → build | capsule authored (SDK / JSON-of-IR, D19) → elaborated (PB-03) | authoring surface is closed-vocabulary; hallucinated API = deny (D24) |
| immutable deployment | content-addressed capsule CID (BLAKE3, `lw.braid.*`) | bijection-guarded; two bytes ≠ one CID (T3) |
| preview deployment | rendered **manifest**, CID-bound (D12) | review object shows *authority*, not diffs of code; spoof-hardened (R3 closed) |
| PR checks | 8-stage admission + widening gate + Keel `NotSlop` floor | fail-closed and deterministic — no flaky heuristics, no AI in the verdict path |
| promote to production | ratified architecture anchor (D28) + payload-hash-bound confirm (T10) | irreversible effects *cannot ship* without a human confirmation bound to exact bytes |
| instant rollback | repoint to prior CID | artifact store is CID-keyed; rollback is a pointer move |
| edge sandbox | WASM over the three-syscall surface (D10) | authority is attenuation-only; a capsule can never exceed a hand-written guest |

The pitch in one line: **Vercel trusts what you push and sandboxes coarsely; Braid
admits only what the manifest declares — which is the property that makes accepting
*AI-authored* deployments viable at all.**

## 3. Where production actually stands (verified 2026-07-02)

Done and green: IR + canonical encoding + CID (U1); 8-stage verifier (U3–U5);
manifest + widening gate (U2/U6); SDK + CLI (U10/U6); U9 adversarial pass (4 findings
closed, mutation-verified); D31 substrate/vocab split with **three** vocabularies
(cms, js, and `braid-vocab-web` — merged PR #13, the browser's `web.*` moved home);
`no_std+alloc` core (PR #12); `braid-v0.1` tag. **U-SA's historical Node/profile
integration is not a current gate**: native Keel migration and its blocking
finding baseline are tracked by #78. The source orchestration policy is a
separate fail-closed guard, not a second semantic verdict engine.

Open (the production gap, in dependency order):
1. **D-RUN** — nothing executes. A JVM with no V. → PB-02
2. **D-ELAB** — no language actually compiles to Braid; global-IR claim is
   architectural, not operational. → PB-03
3. **D-CONSUMER** — zero live dependents (browser has a parallel `BraidTerm` enum;
   kernel binding is a pinned snapshot). → PB-04
4. **Platform surface** — no deploy/preview/promote/rollback product exists. → PB-05
5. **Assurance ops** — U9/flight-hours/mutation discipline is a one-shot, not a
   release ritual; Lean conformance (D-SA5) unbuilt. → PB-01, PB-06
6. **D-CONFIRM** — D5, D16, D31, D32 are INTERPRETED, awaiting Director confirm/veto.

## 4. The playbooks

| # | Playbook | Closes | Blocked by |
|---|---|---|---|
| PB-01 | `PB-01-safety-floor-hardening.md` — finish the assurance floor | D-SA residue, D-SA5, U-SA AC 2/6 | nothing |
| PB-02 | `PB-02-execution-leg.md` — make capsules run | D-RUN, T7/T9/T10 runtime halves | Director call: interpreter vs kernel-WASM |
| PB-03 | `PB-03-elaborator.md` — a real language → Braid | D-ELAB, D21 seam | PB-01 (floor qualifies the compiler) |
| PB-04 | `PB-04-first-consumer.md` — live-wire browser + kernel | D-CONSUMER | none (tag exists) |
| PB-05 | `PB-05-deploy-platform.md` — the Vercel-niche product | the niche itself | PB-02 (deploys must run) |
| PB-06 | `PB-06-assurance-ops.md` — release + vocabulary governance | D-VOCAB, ops debt | none |

Recommended order: **PB-01 → PB-04 (cheap, unblocked) in parallel; then PB-02; then
PB-03; PB-05 rides on 02/03; PB-06 is continuous.**

## 5. Locked invariants (violating any of these is the bug, whatever the playbook says)

- **D8**: canonical CBOR subset, no IEEE floats, bijection guard, KAT-before-consumer.
- **D9**: verifier shares zero serialization code with any authoring path.
- **D10**: Braid adds no authority — three-syscall surface only, kernel capability semantics.
- **D11**: every capsule pins versions; mismatch = deny absent migration proof.
- **D12**: manifest CID-bound; runtime re-derives and refuses on mismatch.
- **D6**: no surface grammar / editor until §16 triggers fire (JSON-of-IR is transport, not syntax — D19 fence).
- **D13**: no issue, no work; U9-class adversarial pass blocks any "done" claim.
- Boundary: `braid-*` substrate crates import only the allowlist (`boundary_conformance.rs` enforces).

## 6. Research + memory index (where deep context lives)

**In-repo (canonical):** `spec/braid/README.md` (reading order) · `PRD.md` ·
`DECISIONS.md` D1–D32 (lock legend!) · `threat-model.md` T1–T16/R1–R3 · `units.md`
U0–U10 · `U9-VERDICT.md` (per-threat file:line pins) · `SAFETY_ASSURANCE_CI_SPEC.md`
(D32 historical design) · `DEBT_REGISTER.md` · `docs/adr-088-…md` (doctrine + both
addenda) · `docs/authoring-cli.md` · `calibration/FLIGHT_HOURS.md` ·
`braid.profile.json` (historical atom mapping) · issue #78 (native gate migration).

**Kernel repo (`~/logic-os-kernel`):**
`LOGIC_OS_AI_FIRST_LANGUAGE_STATE_FABRIC_SECURITY_RESEARCH_BASELINE.md` (§8 "Axiom"
doctrine — prose input ONLY, D4) · `kernel/.../braid_vocab_binding.rs` (the pinned
consumer seam, PB-04) · `blueprints/afternow-port/` (the D7/D16 reference surface).

**Browser repo (`~/next-gen-browser-engine` + `~/src/browser-engine`):**
`docs/BRAID_BRIDGE.md` (§6: "depends on braid-ir, does not recreate the registry") ·
`docs/BRAID_STEER.md` (the collapse instructions, delivered 2026-06-23, not yet acted
on) · `src/braid_bridge/term.rs` (the parallel `BraidTerm` enum = PB-04's kill target).

**blackbox2 (research home, GDrive `LogicalWorks-Research/blackbox2/`):**
`LOGICAL-WORKS-PLAN.md` goal #8 — Keel+Braid completion is a named top-level goal and
"the dropped facet" · `PLAYBOOK-orchestrator.md` (lane routing: delegate mechanical
work, verify executor output yourself — these playbooks are executed THROUGH that
model) · `PLAYBOOK-forge-fanout.md` (fan-out method + trust tallies for executors) ·
`RESEARCH-enterprise-codebase-protection-patterns.md` ·
`RESEARCH-2026-eng-principles-and-ux.md` · `FRAMEWORK-human-using-ai-2026.md`.

**Memory (`~/.claude/projects/-Users-srinji/memory/`):**
`project_director_mandate_2026_06_26b` — Keel+Braid = PERMANENT Opus+Codex
co-ownership · `compressed_past_codebase_engineering` — cross-repo invariants
(generated-not-handwritten, model_port, one-canonical-implementation) ·
`research_substrate_theses` — "Braid = the ONE no_std core both [kernel + browser]
consume"; browser is standalone, content-addressed immutable DAG ·
`identity_simulator_promotion` — orchestrate execution down to executor lanes.

## 7. How to deep-learn this system (the loop every playbook assumes)

1. Read the corpus for the playbook (each names it).
2. **Probe, don't trust**: `cargo test --workspace` (103 green), `./scripts/cli-loop.sh`,
   `./scripts/demo-port.sh`, `./scripts/keel-floor.sh` — reproduce every claim you read.
3. Falsify your understanding: mutate the thing you believe is enforced and watch the
   gate go RED (the U9 `mutation ×2` discipline). If it stays green, you found either
   your misunderstanding or a real hole — both are the point.
4. Execute via the blackbox2 orchestration model: issue first (D13), delegate
   mechanical work to executor lanes, verify their output against the invariants in §5.
5. Ship nothing without its U9-style adversarial pass and evidence on the issue.

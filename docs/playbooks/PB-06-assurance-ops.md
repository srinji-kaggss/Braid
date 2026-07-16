# PB-06 — Assurance ops + vocabulary governance (the continuous playbook)

**Objective**: convert Braid's one-shot assurance artifacts (U9 pass, flight hours,
mutation evidence, INTERPRETED-decision backlog) into a **release ritual**, and stand
up the vocabulary-extension governance flow (D-VOCAB, PRD P5) that the ecosystem arc
depends on. This playbook never exits; it runs alongside PB-01…05.

## Deep-learning corpus

1. `spec/braid/U9-VERDICT.md` — the verdict format and mutation discipline to repeat.
2. `calibration/FLIGHT_HOURS.md` — the external-standards ledger + its queue (map
   ordering, Lean conformance, known-bad corpus post-elaborator).
3. `spec/braid/DECISIONS.md` lock legend + every INTERPRETED entry (D5, D16, D31,
   D32 — the D-CONFIRM backlog) and the append-only amendment convention.
4. `spec/braid/DEBT_REGISTER.md` — keep it truthful (it already drifted once: D-SA).
5. blackbox2 `PLAYBOOK-orchestrator.md` + `AGENT-RUNWAY.md` — how work on this repo
   is actually dispatched and verified (delegation packets, verify-executor-output).
6. Memory `project_director_mandate_2026_06_26b` — Keel+Braid co-ownership is
   PERMANENT (Opus+Codex); coordination happens over the blackbox2 coord bus.

## The release ritual (every tag)

1. `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
   && cargo fmt --all --check && ./scripts/cli-loop.sh && ./scripts/demo-port.sh
   && ./scripts/keel-floor.sh` — all green, no exceptions, no gate weakening.
2. **U9 delta pass**: adversarial review scoped to what changed since the last tag,
   verdict appended in the U9 format (threat → exploitable? → file:line pin). Any
   new crate gets a full pass. A release with an open confirmed-real finding does
   not tag (D13).
3. **Mutation ledger check**: every new enforcement claim since last tag has a
   recorded RED-under-mutation test.
4. **Flight hours**: run the calibration suite; add at least the queued next vector
   class when the release touches encoding/verifier.
5. **KAT/CID re-pin audit**: any CID re-pin in the diff must be a conscious,
   register-recorded event (the D31 re-pin is the precedent) — an unexplained pin
   change is a release blocker, it is either a canonicality break or tampering (T3).
6. **Debt + decision truth-sync**: DEBT_REGISTER rows updated; INTERPRETED decisions
   presented to the Director for confirm/veto (batch them — the veto window is the
   cheapest correction point we have).
7. Tag (`braid-vX.Y`), then notify consumers (browser, kernel, platform) with the
   manifest-level summary: *what widened, what narrowed, what re-pinned* — the same
   review object humans use, applied to the substrate's own releases.

## Vocabulary governance (the D-VOCAB flow)

The stdlib grows vocabulary-by-vocabulary; each addition widens what programs CAN
express, so vocabulary review is where the platform's security budget is spent:

- **Proposal**: new term/vocab arrives as an issue with the full registry entry
  (signature, capability, effect class, composition posture, cost bound) and the
  D24 grounding evidence — effect/cost extracted from the real system, not idealized.
- **The T1 review**: the single question that matters — *does any param admit
  freeform interpretable content?* A term that smuggles open-ended power re-opens
  everything (`eval`, freeform URL, SQL string). The registry conformance test must
  mechanically reject interpretable-code param shapes; the human review is the
  second layer, not the first.
- **AI proposes, human ratifies** (D28 applied to vocabulary): an AI lane may draft
  the entry and evidence; only the Director (or a named delegate) mints it. A minted
  vocabulary version bump is a conscious event: `VOCABULARY_VERSION`/registry CID
  moves, consumers see it in their pin (D11).
- **Ownership**: each vocabulary has a named owner (web.* = browser team; cms.* =
  kernel; js.* = the elaborator workstream). Cross-vocab capability names are a
  collision to report, not resolve silently (D15 landmine convention generalized).
- **Certification tiering** (PRD OQ2, ADR-069): certified strand libraries are a
  later phase — do not build a package registry yet (locked non-goal); the git-tag +
  vendored-vocab model carries until §16-class triggers fire.

## Operating rules for agents working these playbooks

- **No issue, no work** (D13); every unit ships evidence (command + SHA + output +
  independent re-run).
- Delegate mechanical implementation to executor lanes per the blackbox2
  orchestrator playbook; the orchestrating lane verifies diffs, runs the gates
  itself, and never trusts an executor's SUCCESS claim (proven failure mode:
  false-success prefixes in the forge trust tally).
- Claim the repo on the coord bus before working it; Braid is co-owned — broadcast
  material decisions (`coord.sh state all`).
- Never weaken a gate to go green; retargeting a stale test requires the invariant
  preserved + a logged reason.
- Uncommitted working-tree noise (e.g. stray `cargo fmt` artifacts) is reconciled or
  flagged before branching — never swept into a PR (the lgwks WIP-sweep lesson).

## Exit criteria

None — this is the steady state. Health check: at any moment, the last tag has a U9
delta verdict, a green floor, a truthful debt register, and zero INTERPRETED
decisions older than two Director sessions.

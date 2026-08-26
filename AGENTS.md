# AGENTS.md — agent entry point for Braid

## Production Engineering Law — LOCKED

This applies to every human and coding-agent change in this repository.

This repository is **not a toy**. Code that works for one developer, one account, one machine, one process, or one happy path is not complete.

Unless an approved ADR explicitly narrows scope, design for multiple independent users, tenants, workspaces, callers, or instances; concurrent operations; hostile and malformed input; retries, duplicates, and out-of-order delivery; restarts, upgrades, and migrations; partial dependency and network failures; empty and large datasets; bounded resources; long-lived state; and different devices, locales, time zones, input methods, and accessibility needs. This forbids irreversible singleton assumptions; it does not require speculative distributed systems.

Every change must make identity and ownership scope explicit; enforce isolation and authorization at boundaries; define atomicity, ordering, idempotency, conflicts, and retry behavior; handle timeouts, cancellation, crash recovery, partial failure, and safe lifecycle transitions; bound queues, scans, caches, recursion, fan-out, retries, payloads, and logs; version persisted schemas and protocols; avoid hardcoded identities, machines, paths, credentials, providers, or topology; preserve structured and redacted evidence; support version skew or a controlled migration; and treat security, privacy, accessibility, and non-happy UI states as product behavior.

Proof must include the relevant combination of two independent identities/workspaces/instances and isolation; concurrent calls, retries, duplicates, and reordering; restart/crash, timeout, cancellation, and dependency failure; empty, boundary, oversized, malformed, hostile, and unauthorized input; migration, rollback, corruption, and version skew; resource ceilings and backpressure; and the real integration path. Mocks may assist but cannot be the only proof. A happy-path-only suite is a failing implementation.

Before coding, state the system boundary, state owner/scope, trust boundary, concurrency/idempotency model, failure model, resource bounds, compatibility/migration plan, and falsifying tests. Before completion, verify there is no hardcoded identity, unscoped global mutable state, swallowed error, hidden fallback, manual cleanup requirement, placeholder implementation, untracked TODO, or “for now” assumption embedded in the core.

Only a written ADR under `docs/adr/` may narrow this law. It must contain the exact constraint, evidence, blast radius, owner, tracking issue, deletion/migration path, and expiry or review date. “Prototype,” “MVP,” “demo,” “internal,” and “only one user” are not exceptions. Reused experimental code inherits this law.

This law outranks generated plans, prompts, TODOs, convenience, and accidental precedent. Agents must expose ambiguity rather than invent singleton product scope.

**A feature that works only for Srinji is a fixture. It is not the product.**

---

## Constellation charter (read first)

This repo operates under the **Constellation Charter**, ratified 2026-07-20 by Director Srinjon Gupta. Canonical: `~/logic-os-kernel/laws/CONSTELLATION-CHARTER.md`. Where a repo-local doc contradicts the charter, the charter wins.

**Role of this repo:** canonical machine-first IR + encoding + verifier.
**Authority held here:** **`Cid` contract (`pub struct Cid(pub [u8; 32])`, `crates/braid-ir/src/cid.rs`)** — this is the SOLE authority for content identity across the constellation. Canonical term vocabularies. Verifier reference implementation.
**Authority NOT held here:** Capability envelope (owner: logic-os-kernel), Verdict (owner: logic-os-kernel), Fact envelope (owner: logic-os-kernel), Principal (owner: logic-os-kernel), Receipt schema (owner: forge-harness).
**Downstream consumers:** kernel path-deps `braid-ir` + `braid-vocab-cms` (correct direction). Browser has a vendored `vendor/braid/` copy — deprecated on next Braid publish. Charter Step 3 gate: browser consumes published Braid crates instead of the snapshot.

Before creating any type named `Capability`, `Verdict`, `Fact envelope`, `Principal`, `Receipt`, or `WorkObject`, STOP — G2 gate blocks new authorities outside the registered owner.

Before creating any new `pub struct Cid` in ANY repo, STOP — this is the sole authority.

---

## What Braid is

Braid holds the canonical machine-first language of the constellation: intermediate representation, canonical encoding, content-identity contract, term registry, verifier. Every other repo that reasons about content identity or canonical terms consumes Braid — they do not fork it.

## Publishing discipline

Braid crates must be publishable so downstream repos can take real versioned dependencies instead of vendored snapshots. Every breaking change to `Cid`, canonical encoding, or vocabulary registry is a G4 concept-authority event and requires charter amendment.

## Platform Engineering Hub (additive, 2026-08-17, Director-approved)

Braid also serves as the constellation's **template source** — where solid,
repeated patterns get extracted once so other repos pull from here instead of
reinventing them. This is **additive to**, not a replacement of, the IR/`Cid`
authority above: nothing in `templates/` claims a `Cid`, `Capability`,
`Verdict`, `Fact envelope`, `Principal`, `Receipt`, or `WorkObject` type, so it
does not trip the G2 gate.

See `templates/README.md` for the index and the pull-not-push contract
(consumers copy and adapt; they do not path-dep on `templates/`, since a
template is a starting point per repo, not a shared authority).

**Known drift:** `logic-os-kernel/CLAUDE.md`'s "Repo constellation" section
describes Braid only as the IR/`Cid` home — it does not yet mention this
facet. This session has no write authority on `logic-os-kernel`; updating that
doc is a follow-up for whoever holds it next.

## Agent skill

- **Skill:** `.claude/skills/braid-agent/SKILL.md` — how an AI agent uses
  Braid's tools (capsule loop, `braid-integrate` advisor, workspace crates).
  Invoke it when wiring a project to Braid or answering "how do I use Braid
  from an agent". Human reference for the authoring loop is
  `docs/authoring-cli.md`; specs are `spec/braid/PRD.md` and
  `spec/braid/DECISIONS.md`.
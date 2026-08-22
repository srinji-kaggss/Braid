# AGENTS.md — agent entry point for Braid

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

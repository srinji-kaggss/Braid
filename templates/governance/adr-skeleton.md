<!--
Extracted from: logic-os-kernel/laws/governance/adr-058-oidc-issuer-allowlist.md
and the ADR table shape in logic-os-kernel/laws/governance/README.md.
Date: 2026-08-17
Naming: adr-{{NNN}}-{{short-slug}}.md, sequential across the WHOLE constellation
(not per-repo) — check governance/README.md's ADR table and ARCHIVE.md for the
next free number before assigning one; two repos picking the same number
independently is the exact "second authoritative artifact" bug this format
exists to prevent.
-->
# ADR-{{NNN}}: {{Title}}

**Status:** {{PROPOSED | ACCEPTED | AMENDED | SUPERSEDED-BY-ADR-xxx}}
**Date:** {{YYYY-MM-DD}}
**Issue:** #{{issue_number}}
**Author:** {{name/agent}}

---

## Context

{{What triggered this decision. Cite the specific file/line/PR/review finding
that surfaced the problem — an ADR that states a general concern without a
concrete trigger is a design essay, not a decision record. State the risk in
"concrete inputs -> wrong outcome" terms where the decision is a security or
correctness fix, not just "this seems risky."}}

---

## Decision

{{The actual rule, stated as something checkable — not a goal or intention.
"Add a required X field" not "we should think about X". If the decision has
enforcement rules, number them and make each one independently verifiable.}}

**Enforcement rules:**

1. {{...}}
2. {{...}}

---

## Consequences

<!-- Most real ADRs in this constellation include this even when not shown
in the excerpt above — what breaks, what must migrate, what's now load-bearing.
An ADR with no consequences section either changed nothing (so why write it)
or is hiding the cost. -->
- {{...}}

## Registration

After merge, add one row to `governance/README.md`'s ADR table (Domain +
one-line Decision Summary) and check whether `ARCHIVE.md`'s supersession
register needs an entry (does this ADR replace or amend an earlier one?).
An ADR that exists as a file but isn't indexed is exactly the "declaration
with no reader" class keel's `contract-reach-budget` gate exists to catch —
don't ship one un-indexed even in a repo that has no such gate.

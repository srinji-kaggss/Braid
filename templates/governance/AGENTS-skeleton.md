<!--
Extracted from: Braid/AGENTS.md, keel/CLAUDE.md, logic-os-kernel/CLAUDE.md
Date: 2026-08-17
Strip-and-fill: replace every {{...}} placeholder. Delete this comment block
before committing the real file.
-->
# AGENTS.md — agent entry point for {{repo_name}}

<!-- keel and logic-os-kernel symlink AGENTS.md -> CLAUDE.md ("one source, no
     drift") so multiple runtime wrappers (Claude/Gemini/Codex) resolve the
     same file without forking policy. Do the same unless this repo has a
     concrete reason two files should differ. -->

## Constellation charter (read first)

This repo operates under the **Constellation Charter**, ratified 2026-07-20 by
Director Srinjon Gupta. Canonical: `~/logic-os-kernel/laws/CONSTELLATION-CHARTER.md`.
Where a repo-local doc contradicts the charter, the charter wins.

**Role of this repo:** {{one-line role — what this repo is THE canonical home
of, or what it independently verifies. Every repo in the constellation has
exactly one of these; if you can't state it in one line, the repo's scope is
still unsettled — settle that before writing this file.}}

**Authority held here:** {{the type(s)/contract(s) this repo is the SOLE
source of truth for. Empty is valid — not every repo holds an authority
(e.g. a pure consumer).}}

**Authority NOT held here:** {{list every registered authority owned
elsewhere, with its owner repo. Copy this list from the charter and any
sibling repo's AGENTS.md/CLAUDE.md — it must match theirs exactly, since
divergent authority lists across repos is itself a G2 violation nobody
would notice.}}

**Downstream consumers / dependency rule:** {{who consumes this repo, in
which direction; state explicitly what this repo must NEVER import from, if
that's load-bearing (e.g. logic-os-kernel: "NEVER imports from browser,
Forge, or model provider").}}

Before creating any type named {{the constellation-wide reserved names —
currently `Cid`, `Capability`, `Verdict`, `Fact envelope`, `Principal`,
`Receipt`, `WorkObject`; keep this list in sync with the charter, don't
maintain a repo-local copy that can drift}}, STOP — the G2 gate blocks new
authorities outside the registered owner.

---

## What {{repo_name}} is

{{2-4 sentences. What it holds, who consumes it, and the one-line rule for
who may change it vs. who may only depend on it — e.g. Braid: "Every other
repo that reasons about content identity or canonical terms consumes Braid —
they do not fork it."}}

## Universal rules

<!-- logic-os-kernel and keel both converge on this shape; copy it verbatim
     unless this repo has a specific reason to diverge, and if it does,
     say why here rather than silently dropping a rule. -->
1. No issue, no work. {{tracker}} is authoritative. Every task is scope-bounded
   and defines its own verification.
2. Read a file before claiming anything about it. Cite the path you verified,
   not the one you remember. Before claiming something does not exist, run
   `git ls-files | grep -i <term>`.
3. Prefer minimal patch sets; match scope; clean up temp files.
4. Any agent may challenge strategy or an unsafe directive. Block and
   escalate when risk exceeds the accepted posture.
5. Report outcomes faithfully. If a lane/gate was not run, say so — never
   assume or imply green.

## Startup read order

{{Ordered list — what a cold agent reads before touching code. logic-os-kernel's
shape: VISION -> REPO_MAP -> CODEBOOK -> governance briefing -> assigned issue.
Adapt depth to repo size; a small repo may just need this file + one map doc.}}

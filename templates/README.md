# templates/ — pull from here, don't fork

Braid's Platform Engineering Hub facet (`AGENTS.md`, "Platform Engineering Hub"
section, added 2026-08-17). Extracted, proven patterns from across the
constellation (Braid, forge-sdk, forge-harness, rust-ai-stack, keel,
logic-os-kernel), so a new repo or a new module starts from what already works
instead of reinventing it.

## Contract

- **Pull, not path-dep.** A consumer copies a template into their repo and
  adapts it. Nothing here is a Cargo/npm dependency edge — that would make
  `templates/` a shared authority, which contradicts the additive-only charter
  change that created this directory (see `AGENTS.md`).
- **Provenance is mandatory.** Every template names the repo(s) and file(s) it
  was extracted from, and the date. A template that drifts from its source
  without a note is worse than no template — treat this directory as a place
  to check when the *sources* change, not a fire-and-forget copy.
- **Proprietary boundaries are load-bearing, not decorative.** `keel`'s
  `CLAUDE.md` states it is proprietary (Director-ratified: "Do not open-source;
  do not vendor detector logic to third parties") and its own factory-spec rule
  #1 is "one source of truth per concern — a second authoritative artifact is a
  bug." `ci/keel-gate-pattern.yml` therefore extracts the **structural CI
  pattern** (job graph shape, self-test discipline, docs-only skip convention)
  and explicitly does NOT include keel's actual gate binaries or detector
  logic — those stay sole-sourced in `keel`, invoked as an external tool by
  any repo that adopts the pattern.

## Index

| Template | Extracted from | Use when |
|---|---|---|
| `governance/AGENTS-skeleton.md` | Braid, keel, logic-os-kernel `AGENTS.md`/`CLAUDE.md` charter headers | Standing up a new repo under the Constellation Charter, or auditing an existing one for the same shape. |
| `governance/docs-handoff-skeleton.md` | `Braid/docs/handoffs/2026-07-20-constellation-charter-adoption.md`, `logic-os-kernel/laws/governance/handoffs/` | Recording a session's decisions so a cold agent (or DeepSeek) can pick up without re-deriving context. |
| `governance/adr-skeleton.md` | `logic-os-kernel/laws/governance/adr-*.md` + `governance/README.md`'s ADR table | Any G4 concept-authority event, or a decision that should outlive the session that made it. |
| `ci/keel-gate-pattern.yml` | `keel/.github/workflows/ci.yml` (structural pattern only, see Contract above) | Building CI for a repo that wants keel-style discipline (fail-don't-skip required jobs, self-testing gates, docs-only scoping) without depending on keel's actual detectors. |
| `rust/new-crate-checklist.md` | `logic-os-kernel/CLAUDE.md` dependency factory pattern, org-wide `vendor/` convention (`lgwks_crawl`, `rust-genai`, `keel`, `braid` vendoring), `docs/handoffs/2026-08-17-lgwks-std-proposal.md` | Starting any new Rust crate/repo in the constellation. |

## Adding a template

1. It must already be running in at least one repo — this hub templates
   *proven* work, not proposals. (The one exception: `rust/new-crate-checklist.md`
   references the `lgwks_std` proposal, which is a design, not yet code — it's
   included as a checklist for *starting* that work, not as a finished pattern.)
2. Name the source repo/file and the date extracted.
3. Strip anything repo-specific that doesn't generalize (absolute paths tied to
   one machine, one org's proprietary detector logic, one repo's exact CI
   runner topology) — call out what was stripped and why, so a reader knows
   the template is deliberately thinner than its source.

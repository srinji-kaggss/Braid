---
name: braid-agent
description: >
  AI-agent guide to Braid's tool surface — the `braid` binary,
  `braid-integrate` advisor, the `braid-ir`/`braid-verify`/`braid-sdk`
  libraries, and the authoring loop. Use when an AI agent needs to author,
  verify, render, or diff capsules; graph a foreign repo for seams; or
  integrate `lgwks_std` / `lgwks_bot` into another codebase. Also use when
  wiring a new project to Braid or answering "how do I use Braid from code".
allowed-tools: []
---

# Braid for AI Agents

You are an AI agent operating inside (or against) the **Braid** repo.
Braid is a machine-first capsule framework — typed term graphs, a closed
capability vocabulary, a deterministic compiler/verifier. Humans and AIs meet
at a shared verified anchor through separate projections.

## Quick start (CLI)

```bash
cargo build --release -p braid-cli -p braid-integrate   # or: cargo install --path crates/braid-cli
braid --help
braid-integrate --help
```

### `braid` — the capsule loop

```
braid encode <input.json> [-o <out.braid>]    JSON-of-IR -> canonical bytes (+ CID on stderr)
braid decode <capsule.braid>                  bytes -> JSON-of-IR (inverse)
braid verify <capsule.braid> [--grant CAP]    admission pipeline (exit 1 on Reject)
braid render <capsule.braid>                  human-review manifest (exit 1 if rejected)
braid diff <old.braid> <new.braid>            manifest delta; exit 1 on Widening
braid store put <repo-manifest.json> [--store DIR] [--replace]
braid catalog [--store DIR]
braid summary <repo> [--store DIR]
```

Exit codes: `0` ok, `1` policy-negative (Reject/Widening), `2` operator error.

### `braid-integrate` — the advisor (foreign repos)

```
braid-integrate <repo-path> [--json] [--apply] [--verbose]
```

- Foregrounds `--json` — single JSON object: `{repo, mode, languages, graph, findings, proposals, next_steps}`.
- **Read-only by default** — never mutates without `--apply`. Paste the `--json` output into your next turn and act without re-scanning.

Example:

```bash
braid-integrate /path/to/foreign-repo --json | jq .findings
braid-integrate /path/to/foreign-repo --apply   # only after approval
```

## Authoring a capsule (minimal)

Capsules are DAGs of typed term applications from the closed v0 registry:

```json
{
  "intent": "Edit landing section (reversible)",
  "budget": 20,
  "confirm": "none",
  "evidence": [],
  "strands": [
    {"term": "lit.entity",       "inputs": []},
    {"term": "lit.text",         "inputs": []},
    {"term": "cms.edit_section", "inputs": [0, 1]},
    {"term": "view.section",     "inputs": [1]}
  ],
  "outputs": [2, 3]
}
```

Do **not** author `grants`, `vocab_version`, `registry_cid`, or `ir_version` — they are derived/filled from the pinned `registry_v0`. A capsule with irreversible/egress strands must set `confirm: "human-confirm"` or `encode` refuses (exit 2). Then:

```bash
braid encode input.json -o out.braid    # prints CID on stderr
braid verify out.braid                  # ADMIT or REJECT
braid render out.braid                  # manifest for human audit
```

Authoring notes: `docs/authoring-cli.md` · Decision provenance: `spec/braid/DECISIONS.md` (D19).

## Libraries (from code)

| Crate | Use when | Entry |
|-------|----------|-------|
| `braid-ir` | Encoding/decoding, CIDs | `braid_ir::Capsule`, `braid_ir::cid::Cid` — SOLE authority for content identity |
| `braid-verify` | Running the admission pipeline | `braid_verify::verify(bytes)` — sole admission authority |
| `braid-sdk` | Typed builder over IR | `braid_sdk::Builder` |
| `braid-render` | Manifest + diff rendering | `braid_render::{render, diff}` |
| `braid-project` | Multi-capsule project build | `braid_project::build(manifest)` |
| `lgwks_std` | Zero-config primitives (hex, base64, uuid, hash, glob, pattern, json, …) | `lgwks_std::{hex, encoding, id, hash, glob, pattern, json}` |
| `lgwks_bot` | Capability-gated bots (Observe→Evaluate→Execute→Query) | `lgwks_bot::{Bot, GrantSet}`; domains `flow`, `net`, … |
| `lgwks-std-gate` | Gate drift checks | `lgwks_std_gate::check_dependencies(root)` |

Workspace layout and ownership: `docs/CRATE-OWNERSHIP.md`. Build/test: `cargo check --workspace` / `cargo test --workspace` (see `README.md`, `AGENTS.md`).

## When to reach for what

- "I need to add/change a Braid feature" → `braid-sdk` or hand-authored `JSON-of-IR` + `braid encode/verify`.
- "I need to integrate another repo with Braid's primitives" → `braid-integrate --json` first, then follow `findings[*].maps_to` and `proposals[*].patch`.
- "I need to audit deps or the migration posture" → `lgwks_std_gate` + `docs/LGWKS-STD-MIGRATION.md` + `contract/APPROVED.toml`.
- "I need to understand what changed between two capsules" → `braid diff old.braid new.braid`.

## Non-goals for this skill

This is an orientation layer, not a re-specification. The specs live in `spec/braid/PRD.md`, `spec/braid/DECISIONS.md`, and the per-crate docs — read them for normative detail. If this file contradicts a spec, the spec wins.

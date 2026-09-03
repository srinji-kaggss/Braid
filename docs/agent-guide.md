# Agent Guide — Using Braid's Tools

This is the repo-stable counterpart to `.claude/skills/braid-agent/SKILL.md`
(Claude Code's skill loader). Either is canonical — keep them in sync when you
change the tool surface.

## Which tool when

| Goal | Start here | Then |
|------|-----------|------|
| Author/change a capsule feature | `braid-sdk` or hand-authored JSON-of-IR | `braid encode` → `braid verify` → `braid render` |
| Audit a foreign repo for seams | `braid-integrate --json` | Follow `findings[*].maps_to` + `proposals[*].patch` |
| Understand a diff between capsules | `braid diff old.braid new.braid` | Read the widening/narrowing report |
| Check dep posture / migration | `lgwks_deps::check_dependencies` | `docs/LGWKS-STD-MIGRATION.md`, `contract/APPROVED.toml` |
| Multi-capsule project build | `braid-project` | `docs/authoring-cli.md` |

Rule: `braid-integrate` is **read-only by default** — never mutates without
`--apply`. An agent that receives `--json` output can act without re-scanning.

## Capsule loop (CLI)

```bash
cargo build --release -p braid-cli -p braid-integrate   # or cargo install --path crates/braid-cli
braid encode input.json -o out.braid   # JSON-of-IR -> canonical bytes (+ CID on stderr)
braid decode out.braid                 # inverse
braid verify out.braid [--grant CAP]   # ADMIT / REJECT (exit 1 on Reject)
braid render out.braid                 # human audit manifest
braid diff old.braid new.braid          # delta; exit 1 on Widening

# Org map (W5)
braid store put repo-manifest.json [--store DIR] [--replace]
braid catalog [--store DIR]
braid summary <repo> [--store DIR]
```

Exit codes: `0` ok · `1` policy-negative (Reject/Widening) · `2` operator error.

Capsules are DAGs of typed terms from the closed v0 registry — see
`docs/authoring-cli.md` for the minimal JSON shape and worked examples.
`grants` / `vocab_version` / `registry_cid` / `ir_version` are derived, never
hand-authored. An irreversible/egress capsule must set `confirm: "human-confirm"`
or `encode` exits 2. Normative specs: `spec/braid/PRD.md`, `spec/braid/DECISIONS.md`.

## Advisor (`braid-integrate`)

```
braid-integrate <repo-path> [--json] [--apply] [--verbose]
```

Produces a single JSON object for agents:

```json
{"repo":"...","mode":"rust|polyglot","languages":[...],
 "graph":{"files":N,"by_ext":{},"manifests":[],"imports":[]},
 "findings":[{"id":"STD-...","maps_to":"lgwks_std::..."}], 
 "proposals":[{"id":"STD-...","patch":"diff --git ...","caps":[]}],
 "next_steps":["..."]}
```

## Libraries

| Crate | Entry |
|-------|-------|
| `braid-ir` | `braid_ir::Capsule`, `braid_ir::cid::Cid` (SOLE `Cid` authority) |
| `braid-verify` | `braid_verify::verify(bytes)` |
| `braid-sdk` | `braid_sdk::Builder` |
| `braid-render` | `braid_render::{render, diff}` |
| `lgwks_std` | `lgwks_std::{hex, encoding, id, hash, glob, pattern, json}` |
| `lgwks_bot` | `lgwks_bot::{Bot, GrantSet}` — verbs Observe/Evaluate/Execute/Query; domains `flow`, `net`, … |
| `lgwks-deps` | `lgwks_deps::check_dependencies(root)` |

Workspace index: `docs/CRATE-OWNERSHIP.md`. Build/test: `cargo check --workspace` / `cargo test --workspace`.

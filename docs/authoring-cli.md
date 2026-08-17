# Authoring a Braid capsule by hand (no AI, CLI only)

This is the human-reconstructable path (ADR-088 L7, threat T13, unit U6). With
only the `braid` binary you can author, admit, review, and diff a capsule —
the same loop and the same artifacts an AI/SDK author produces. The verifier is
the sole authority; the CLI adds zero permissions.

> **Decision provenance:** the `encode` input format is **JSON-of-IR**, fixed
> by **D19** (`spec/braid/DECISIONS.md`). It is a 1:1 data transcription of the
> IR — *not* a surface language (that remains a D6-gated non-goal). No grammar,
> no sugar, no defaults beyond the SDK's.

## Build the binary once

```bash
cargo build --release -p braid-cli      # produces target/release/braid
```

## The loop

```bash
braid encode <input.json> [-o out.braid]   # author -> canonical bytes (+ CID on stderr)
braid decode <capsule.braid>               # bytes -> JSON-of-IR (the inverse of encode)
braid verify <capsule.braid> [--grant CAP] # run the admission pipeline
braid render <capsule.braid>               # the human-review manifest
braid diff   <old.braid> <new.braid>       # manifest delta; fails on any Widening
braid store put <repo-manifest.json> [--store DIR] [--replace]   # validate + install
braid catalog [--store DIR]                # the org map (inventory == store)
braid summary <repo> [--store DIR]         # one repo's full summary
```

`render` and `diff` are human-review projections of **admitted** artifacts.
They first run the verifier using the capsule's declared grants as ambient
authority, matching `verify`'s default happy-path check. A canonical but
verifier-rejected capsule (for example, stale `vocab_version` or
`registry_cid`) is refused before manifest text is emitted.

**Exit codes:** `0` ok · `1` policy-negative (a clean run that ends in
`REJECT` or `WIDENING`) · `2` operator error (bad usage / IO / malformed input
/ author-time reject). The split lets CI and `&&` chains distinguish "the tool
broke" from "the tool correctly said no".

## Write a capsule

A capsule is a DAG of typed term applications drawn from the closed v0
registry. Each strand names a `term` and lists the indices of the earlier
strands feeding its inputs (inputs must reference **strictly earlier** strands
— acyclicity is structural, not checked-after). `outputs` lists the strand
indices the capsule yields.

```json
{
  "intent": "Edit landing section and render preview (reversible)",
  "budget": 20,
  "confirm": "none",
  "evidence": ["fact.cid"],
  "strands": [
    {"term": "lit.entity",       "inputs": []},
    {"term": "lit.text",         "inputs": []},
    {"term": "cms.edit_section", "inputs": [0, 1]},
    {"term": "view.section",     "inputs": [1]}
  ],
  "outputs": [2, 3]
}
```

What you do **not** write — and cannot, by design:

- **`grants`** — derived from the terms used. You can't request a capability
  you don't use, or use one you didn't request (that omission is unauthorable).
- **`vocab_version` / `registry_cid` / `ir_version`** — filled from the pinned
  `registry_v0`. The `registry_cid` is recomputed from the registry bytes, so
  there is no magic constant to copy.

Optional fields: `budget` (defaults to the composed cost of the strands),
`confirm` (`none` | `human-confirm`), `evidence` (keys journaled on execution).
A capsule containing an irreversible or egress strand **must** set
`confirm: "human-confirm"`, or `encode` refuses it at author time (exit 2).

## The v0 term registry

| term | inputs → output | effect | capability |
|---|---|---|---|
| `lit.text` | → text | pure | — |
| `lit.bytes` | → bytes | pure | — |
| `lit.entity` | → entity | pure | — |
| `text.concat` | text, text → text | pure | — |
| `bytes.id` | bytes → bytes | pure | — |
| `view.section` | text → directive | pure | — |
| `view.page` | list&lt;directive&gt; → directive | pure | — |
| `proj.listing` | entity → list&lt;text&gt; | read | `tape.read` |
| `vault.read` | entity → bytes | read | `tape.read` |
| `cms.edit_section` | entity, text → cid | reversible-write | `signal.emit` |
| `cms.publish` | cid → cid | irreversible | `intent.emit` |
| `net.egress` | bytes → cid | egress | `compute.remote` |

(Render output is always a typed `directive`, never HTML/DOM — D16. `vault.read`
+ `net.egress` exist so the taint trip-wire has real teeth: vault-exposed data
reaching egress is rejected at the taint stage, even through pure hops.)

## Worked example

```bash
braid encode edit_section.json -o edit.braid
#   stderr: wrote edit.braid (332 bytes)
#           cid 8221c8b58bceea6a4f9129260fa1eb8c6179c7206f37a769d45a542a4f1fe130
braid verify edit.braid        #   ADMIT  cid 8221...
braid render edit.braid        #   the manifest a reviewer reads
```

Add a `proj.listing` strand and diff against the original:

```bash
braid diff edit.braid edit_widened.braid
#   WIDENING  capabilities: +tape.read
#   WIDENING  effects: +read
#   (exit 1 — the gate fires)
```

Non-widening artifact changes are still shown. `no change` is reserved for the
same admitted capsule/manifest, not merely "same authority." For example, an
evidence-policy-only change exits `0` but prints neutral `capsule` and
`evidence` deltas.

`--grant` on `verify` models the authority the *principal* holds (default: the
capsule's own declared grants). Hand it a narrower set to see attenuation
enforced:

```bash
braid verify publish.braid --grant signal.emit
#   REJECT [Capability] grant `intent.emit` exceeds ambient authority   (exit 1)
```

## Repo manifests & the org map (W5)

`braid store put` / `braid catalog` / `braid summary` manage the org map: one
repo-manifest artifact per repo plus the declared inventory.

> **Sibling-format decision (Director, 2026-08-16):** the plan says
> "repo-manifest capsule", but strand payloads are valueless and literal
> payloads are D8-locked substrate work, so manifests ship as a sibling
> validated format with capsule-grade discipline (canonical CBOR, BLAKE3 CID
> under `lw.braid.repo-manifest.v1`, strict decode, validate at every
> boundary). They graduate to capsule form when the Strand-literal unit
> lands; `braid-manifest::validate` is the migration surface. braid-verify
> remains the sole admission authority for capsules — manifests carry no
> capabilities, nothing is verified twice, and no second verifier exists.

### The manifest (8 fields, all required — no UNKNOWN is representable)

| field | type | contract |
|---|---|---|
| `name` | string | safe storage key: `[A-Za-z0-9._-]+`, no leading `.` |
| `archetype` | enum | `workspace-crate` \| `single-crate-app` \| `infra-gate` \| `docs` |
| `owner` | string | non-empty; no tab/newline/comma |
| `gate_version` | string | non-empty; no tab/newline/comma (`none` when no gate) |
| `ci_status` | enum | `green` \| `red` \| `none` |
| `entry_docs` | list | non-empty; entries non-empty; no TSV separators |
| `canonical_commands` | list | same contract |
| `local_ci` | bool | `.wwfd/local-ci.sh` present? |

```json
{ "name": "braid", "archetype": "workspace-crate", "owner": "Director",
  "gate_version": "none", "ci_status": "green",
  "entry_docs": ["AGENTS.md", "README.md", ".wwfd/local-ci.sh"],
  "canonical_commands": ["cargo test --workspace"], "local_ci": false }
```

### The store and the inventory (the org database)

- Store root: `~/.local/share/braid/store` (override with `--store`; tests use
  temp stores). `braid store put <manifest.json>` validates, installs
  `<name>.manifest` (canonical bytes), prints the CID, and creates the store
  on first run.
- The **inventory** (`<store>/inventory.json`) is the ONE database: a JSON
  object mapping repo name → pinned manifest CID (`null` = declared, not yet
  admitted). The pins make the store tamper-evident: `catalog`/`summary`
  re-hash every artifact on read and deny on any pin mismatch, missing repo,
  undeclared repo, or unpinned declaration. No inventory ⇒ `catalog` fails
  closed (completeness is unprovable without the declared set).
- Changing a repo's metadata: `braid store put … --replace` (denies without
  `--replace`, naming the existing CID), then record the new CID in the
  inventory. Until re-pinned, `catalog` denies — an org-metadata change is a
  recorded event in the org database, never silent drift.

### Reading the map

```bash
braid catalog                 # every repo: human blocks + `---` + machine lines
braid summary <repo>          # one repo, full detail + the SAME machine line
```

The machine line is a stable 9-field TSV contract (name, archetype, owner,
gate_version, ci_status, entry_docs, canonical_commands, local_ci, cid8) —
comma-joined lists round-trip losslessly because commas are banned in
authored strings; `cid8` is the first 8 hex chars of the BLAKE3 CID.
`summary`'s line is byte-identical to `catalog`'s line for that repo — one
parser for both commands. Output is deterministic and golden-pinned
(`spec/braid/vectors/w5/catalog.golden`). Fixture manifests in
`spec/braid/vectors/w5/` stand in for real per-repo data entry
(`sh spec/braid/vectors/w5/seed.sh` seeds the default store).

**Exit codes (whole binary):** `0` ok · `1` policy-negative / fail-closed
denial (Reject, Widening, unknown repo, duplicate, tampered/stale pin,
inventory mismatch, unpinned declaration, out-of-contract manifest) · `2`
operator error (usage, missing/unreadable path, malformed input).

## Reference fixtures & the CI loop

Worked `.json` capsules live in `crates/braid-cli/tests/fixtures/`. The whole
loop, with every exit code asserted (including the widening gate firing on a
seeded widening and the laundering capsule rejected at taint), runs as
`scripts/cli-loop.sh` — executed in CI on every PR (`.github/workflows/ci.yml`,
job `scenario #12 + T12 widening gate`).

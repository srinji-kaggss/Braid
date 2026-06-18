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

`--grant` on `verify` models the authority the *principal* holds (default: the
capsule's own declared grants). Hand it a narrower set to see attenuation
enforced:

```bash
braid verify publish.braid --grant signal.emit
#   REJECT [Capability] grant `intent.emit` exceeds ambient authority   (exit 1)
```

## Reference fixtures & the CI loop

Worked `.json` capsules live in `crates/braid-cli/tests/fixtures/`. The whole
loop, with every exit code asserted (including the widening gate firing on a
seeded widening and the laundering capsule rejected at taint), runs as
`scripts/cli-loop.sh` — executed in CI on every PR (`.github/workflows/ci.yml`,
job `scenario #12 + T12 widening gate`).

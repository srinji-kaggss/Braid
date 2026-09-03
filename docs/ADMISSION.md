# Admission — the process for adding something new

INV-DEP-EDGE-OWNED says every external edge authored in a workspace manifest
must name its semantic owner, capability, source, requirement, allowed
consumers, and dependency kinds. Transitive packages remain lockfile provenance;
they are not 100 accidental authorities. This file is how an edge legitimately
gets in.

The short version: **most of this process ends before a dependency exists.**
Rungs 0 through 4 produce no register entry at all, and they are where the
honest answer usually is.

## ── The ladder ──────────────────────────────────────────────────────

Climb it in order. Stop at the first rung that works. Record which rung you
stopped at and why the ones above it failed — that sentence is the `reason`
field later.

| rung | move | produces |
|---|---|---|
| 0 | **Drop the feature.** Is it worth having? | nothing |
| 1 | **Use `std`.** It grew while you were not looking. | nothing |
| 2 | **Use `lgwks_std`.** Already approved, zero new edges. | nothing |
| 3 | **Add a module to `lgwks_std`** — ELIMINATE tier. | a PR here |
| 4 | **Consolidate onto one existing choice** — CONSOLIDATE tier. | a PR here |
| 5 | **Vendor the audited source** — VENDOR tier. | `vendor/` + an entry |
| 6 | **Approve it as a boundary** — BOUNDARY tier. | an entry |

`lgwks-deps tiers` prints this at the terminal.

### Which rung is which

**Rung 3, ELIMINATE.** Small, well-understood, no correctness-critical security
property: `hex`, `base64`, `glob`, `uuid`, `chrono`'s stamping, `walkdir`,
`fs2`, `percent-encoding`. The test is whether you can state the whole algorithm
in a paragraph and test it against published vectors. If yes, a reimplementation
is *less* risk than a supply-chain edge — `event-stream` was four lines.

**Rung 4, CONSOLIDATE.** The estate already has two crates doing this job and
the fix is to pick one and wrap it once. `reqwest` versus `ureq` is the live
example: `lgwks_crawl` already runs `ureq`, so a second HTTP client is a second
source of truth, which Law 3 forbids.

**Rung 5, VENDOR.** Cryptography, and anything where a hand-rolled version is a
security regression rather than a simplification: `sha2`, `blake3`, `hmac`,
`ed25519-dalek`, `aes-gcm`, `zeroize`, `ring`. Pull the pinned upstream source
into `vendor/<crate>/` with a `PROVENANCE.md` naming the exact version and
commit, matching the `lgwks_crawl`/`vendor/spider` precedent. **Vendoring removes
the dependency edge without removing the audit.** Reimplementing these is the
canonical way to introduce a timing side channel.

**Rung 6, BOUNDARY.** Reimplementing it is a multi-year project of its own:
`tokio` (no async runtime in `std`), `serde` (this is *why* serde exists),
`regex` (a correctness-critical parser and VM, maintained by the rustc team),
`syn`/`quote`/`proc-macro2` (the compiler as a library), `rusqlite` (a database
engine), `cap-std` (capability-secure OS sandboxing, upstream-audited), and the
ML frameworks in `rust-ai-stack`.

Only rungs 5 and 6 produce a register entry. That is why `tier` admits only
`vendor` and `boundary`; an entry claiming `eliminate` is refused as a category
error.

## ── Writing the approval ────────────────────────────────────────────

There is **no command that approves anything.** `lgwks-deps request` prints a
block; a human fills it in and commits it. The commit is the signature, which is
what makes the approval reviewable and attributable.

```
lgwks-deps request tokio 1.40
```

```toml
[[approved]]
crate = "tokio"
tier = "boundary"
version = "1.40"
owner = "lgwks_std"
capability = "task.async-runtime"
source = "registry"
allowed_consumers = "lgwks_std"
allowed_kinds = "normal"
reason = "An async runtime is not in std and reimplementing one is a multi-year project."
approved_by = "Director"
approved_on = "2026-08-19"
review = "docs/ADMISSION.md#rung-6"
```

Every field is required and every field is checked:

| field | rule |
|---|---|
| `crate` | upstream Cargo package name; `-`/`_` drift is tolerated |
| `tier` | `boundary` or `vendor` only |
| `version` | Cargo manifest requirement exactly as metadata reports it |
| `owner` | workspace crate responsible for the semantic capability |
| `capability` | stable semantic name for what the package supplies |
| `source` | admitted origin class: `registry`, `git`, or `path` |
| `allowed_consumers` | comma-separated workspace crates permitted to declare the edge |
| `allowed_kinds` | comma-separated `normal`, `build`, and/or `dev` |
| `reason` | a sentence — 4+ words, 24+ characters, ends in a full stop, and not a restatement of the crate name |
| `approved_by` | the human who decided |
| `approved_on` | `YYYY-MM-DD` |
| `review` | path or URL to the evidence |

The `reason` rule is the one that matters. A name on a list is a whitelist; a
name with a reason, a pin, an approver, a date, and a link to the evidence is a
contract. `reason = "needed"` fails the build with the same force as no entry at
all — verified, and it is the point of the exercise.

## ── Updating an approved requirement ────────────────────────────────

A changed Cargo manifest requirement is refused even though the crate is
approved. That is deliberate:

> "Show me the commit we need. Don't update for the sake of it."
> — Mitchell Hashimoto

To change a requirement, read the upstream diff, then edit `version` and
`approved_on` in the same commit that changes Cargo.toml. The contract spelling
must match Cargo metadata exactly (`^1.0`, `=1.0.219`, and `~1.0` are distinct
policies). Cargo.lock remains the exact resolved-byte provenance.

## ── Bringing a repo onto the gate ───────────────────────────────────

Adoption is not "approve a thousand crates."

1. `lgwks-deps init` — writes a fail-closed starting register.
2. Set `[policy] enforce = false`. Refusals now report as warnings. This is the
   only sanctioned use of that flag, it belongs in a diff a human signed, and it
   carries an expiry the repo's own issue tracker owns.
3. Wire `lgwks-deps check` as the first local and remote CI build lane.
4. Land the ELIMINATE-tier swaps. The count falls without anyone approving
   anything — that is the whole design.
5. Approve what genuinely remains, rung by rung.
6. Flip `enforce = true`. From here the gate is a ratchet.

Step 4 before step 5 is not an optimisation. Approving first and migrating later
means the register fills with entries nobody would write today, and a register
full of unexamined approvals is a whitelist again.

## ── Removing an approval ────────────────────────────────────────────

Delete the block. If the direct edge still exists, the next gate run fails and
names its consumer. If the edge no longer exists, leaving the block also fails
as stale authority; approvals cannot outlive the code that justified them.

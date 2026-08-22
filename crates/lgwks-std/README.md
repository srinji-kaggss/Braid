# lgwks-std — the `+` in `std+`

**The rule, in the Director's words:** if a library is not in `std` or `std+`,
it is not an approved dependency, and the code does not compile until a human
has registered it in the semantic contract.

This repo is both halves of that sentence. `lgwks_std` is the `+`. `lgwks-gate`
is the "does not compile."

It lives in Braid, alongside the IR and the verifier, because Braid is where the
estate's contracts live — but it is deliberately not a `braid-*` crate. The IR
and the verifier answer to Braid's `AGENTS.md` charter; `lgwks_std` answers to a
different authority, and neither owns the other.

```
Braid/
  crates/lgwks-std/        the library — zero dependencies, std only
  crates/lgwks-std-gate/   the gate — refuses a build whose deps are not approved
  contract/APPROVED.toml   Braid's register (adoption mode, see below)
  docs/ADMISSION.md        the process for adding something new
  docs/LGWKS-STD-MIGRATION.md   per-crate swap notes for the four repos
```

## INV-STDPLUS-ZERO-DEPS

Neither crate declares a dependency. That is not an aspiration; it is checked by
`admits_no_dependency_of_its_own` in `crates/lgwks-std-gate/src/lib.rs`, which
reads both manifests at test time. A gate that took a dependency in order to
police dependencies would be self-refuting, so the lock reader and the register
reader are both line-oriented rather than built on a TOML crate.

## Phase 1 — what exists now

The ELIMINATE tier: crates small enough that reimplementing them is less risk
than a supply-chain edge. Each module states the crate it retires and the
measured call surface it has to cover, so every swap is a substitution rather
than a redesign.

Counts re-derived against the tree on 2026-08-19. Manifest declarations and
actual call sites differ a lot, and only the second number tells you what a
migration costs.

| module | retires | manifests | call sites | surface |
|---|---|---|---|---|
| `hex` | `hex` | 11 | 21 | `encode` ×22, `decode` ×8 — the whole surface |
| `id` | `uuid` | 7 | 12 | `new_v4` then `to_string`, every site |
| `glob` | `glob` | 2 | 3 | matching, not traversal |
| `time` | `chrono` | 2 | 2 | `Utc::now().to_rfc3339()`, both sites |
| `encoding::percent` | `percent-encoding` | 1 | 1 | component encode/decode |
| `encoding::base64` | `base64` | 0 | 0 | present so rung 2 already answers "yes" |
| `random` | — | — | — | the OS-entropy owner `id` is built on |

Two of these are worth reading twice. **`chrono` costs two lines to remove** —
it is declared in two manifests and called from two sites. And **`rand` is not a
phase-1 target**: 4 manifests but **43** call sites, including distributions and
seeded generators, which makes it a phase-3 CONSOLIDATE item. `random` here is
the single entropy owner the rest will be built on, not a `rand` replacement.

`base64` has no caller today and is here anyway. That is the one place where
building before the third caller is right: the purpose of a stdlib+ is that the
answer at rung 2 of the ladder is already "yes", so the question never reaches
rung 6. It is stated rather than dressed up as demand.

`hex` is hoisted from the estate's existing hand-rolled copy at
`Braid/crates/braid-ir/src/cid.rs:30-48` rather than written fresh, and
`matches_the_braid_cid_encoding` is the differential proof — `Cid` byte output
is a G4 charter authority in Braid's `AGENTS.md` and must not drift.

**Not here, deliberately.** No cryptography. Hand-rolling a hash, a MAC, or a
signature is a security regression dressed as a simplification, so `sha2`,
`blake3`, `hmac`, `ed25519-dalek`, `aes-gcm`, and `zeroize` arrive in phase 2 as
*pinned upstream source* under `vendor/` — the audited bytes, re-exported behind
a narrow API, matching the `lgwks_crawl`/`vendor/spider` precedent. Vendoring
removes the dependency *edge* without removing the audit.

## The gate

Three lines in a consumer's `build.rs` turn INV-DEP-REGISTERED into a compile
error:

```rust
// build.rs
fn main() {
    lgwks_std_gate::enforce();
}
```

```toml
[build-dependencies]
lgwks_std_gate = { path = "..." }
```

Cargo runs `build.rs` after resolution and before compiling, with the
*consumer's* `CARGO_MANIFEST_DIR`, so the gate reads the consumer's own
`Cargo.lock` — the resolved graph, not the declared one. A crate that arrives
only as somebody else's transitive dependency is audited exactly like a direct
one, which is the point: `event-stream` was a transitive.

Everything else is diagnosis:

```
lgwks-gate check [PATH] [--contract FILE]   audit; exit 2 on refusal
lgwks-gate request <CRATE> <VERSION>        print the block to fill in
lgwks-gate init [PATH]                      write a fail-closed register
lgwks-gate tiers                            print the admission ladder
```

### Fail-closed

A missing register, an unparseable register, and an unreadable lock file are all
refusals. A gate that passes when it cannot find its own contract reports
success for the one condition it exists to catch. The only way to stand
enforcement down is `enforce = false` under `[policy]` **in the register**, which
is a reviewable diff carrying a human's name — never an environment variable a
build can set for itself.

### Approval is semantic, not a whitelist

An entry is refused unless it says what the standard library cannot do. The
register demands `crate`, `tier`, `version`, `reason`, `approved_by`,
`approved_on`, and `review`, and a `reason` must be a sentence — four or more
words, ending in a full stop, not a restatement of the crate's name.
`reason = "needed"` fails the build with the same force as no entry at all.

`tier` admits only `boundary` and `vendor`, because ELIMINATE and CONSOLIDATE
crates do not get entries — they get an `lgwks_std` module. An entry claiming
`eliminate` is a category error and is refused as one.

An approved version is matched as a prefix on a dot boundary: `1.0` admits
`1.0.219`, `1.0.219` admits only itself, `*` admits anything and says so in the
diff. A drifted version is a *new* approval, not an automatic one — "show me the
commit we need."

## Measured today

`lgwks-gate check` against the four repos' unmodified lock files, audited
against an empty register:

| repo | resolved | unapproved |
|---|---|---|
| Braid | 119 | 105 |
| forge-sdk | 331 | 324 |
| forge-harness | 554 | 546 |
| rust-ai-stack | 1025 | 1019 |

That is the work-list, and it is also the argument. Adoption is not "approve
1,019 crates"; it is `[policy] enforce = false` on day one, then the ELIMINATE
modules land and the number falls without anyone approving anything.

## Adoption state

`contract/APPROVED.toml` carries `[policy] enforce = false`. That is the
sanctioned adoption setting and it is the only one: refusals report as
`cargo::warning` and the build passes, so the register can be filled honestly
instead of in a single rubber-stamping commit. No consumer has wired `build.rs`
yet, so nothing in this workspace is gated today.

The order out of adoption is in `docs/LGWKS-STD-MIGRATION.md` — land the
ELIMINATE swaps, approve what genuinely remains rung by rung, then flip
`enforce` to `true`. From that point the gate is a ratchet.

Braid's own first swap is visible in its root `Cargo.toml`: `hex = "0.4.3"` is
declared as a test-only workspace dependency, and `lgwks_std::hex` was hoisted
from `crates/braid-ir/src/cid.rs:30-48` specifically to retire it.

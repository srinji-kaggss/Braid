# Migration — swapping the four repos onto `lgwks_std`

Every count here was re-derived against the tree on 2026-08-19, not carried
forward from the 2026-08-17 proposal. Two of the proposal's numbers did not
survive that re-derivation and are corrected below.

## ── Order ───────────────────────────────────────────────────────────

Smallest blast radius first, and ELIMINATE before anything else — the point of
the ordering is that the unapproved-crate count falls *before* anyone writes a
register entry.

1. `Braid` — 119 resolved, 105 unapproved. `hex` only.
2. `forge-sdk` — 331 resolved, 324 unapproved. `hex`, `uuid`, `chrono`, `util.rs`.
3. `forge-harness` — 554 resolved, 546 unapproved. `hex`, `uuid`, `glob`.
4. `rust-ai-stack` — 1025 resolved, 1019 unapproved. `hex`, `uuid`.

## ── Per crate ───────────────────────────────────────────────────────

### `hex` → `lgwks_std::hex` — 11 manifests, 21 lines

Mechanical. `hex::encode` is signature-compatible. `hex::decode` returns
`Result<Vec<u8>, lgwks_std::hex::DecodeError>` instead of the upstream
`FromHexError`; call sites that only `?` or `.ok()` need no change, and sites
that match on the error variant need reading.

**One site is not mechanical.** `Braid/crates/braid-ir/src/cid.rs:30-48` is the
implementation this module was hoisted from. `Cid` byte output is a G4 charter
authority in Braid's `AGENTS.md`, so routing it through `lgwks_std::hex` is a
charter event and needs the differential proof, not a compile check.
`matches_the_braid_cid_encoding` in `crates/lgwks-std/src/hex.rs` is that proof
already written; extend it over a fixed corpus of existing `Cid` values before
the swap lands.

### `uuid` → `lgwks_std::id` — 7 manifests, 12 sites

Every site is `Uuid::new_v4()` followed by `to_string()`. The one difference
that matters: `lgwks_std::id::Uuid::new_v4()` returns
`Result<Uuid, EntropyError>` where the upstream panics or silently degrades.
That is deliberate — a v4 identifier built from anything weaker than the OS
CSPRNG is predictable — and it means each site chooses between `?` and an
explicit `expect` naming the invariant.

`rust-ai-stack/rust-torch/src/keel_tensor_engine.rs:38` stores `uuid::Uuid` in a
public struct field, so that one is a type change in a public surface, not a
call-site change. Take it last.

### `chrono` → `lgwks_std::time` — 2 manifests, 2 sites

The cheapest item on the list, and the proposal overstated it.

- `forge-sdk/forge-core/src/agent.rs:1708` —
  `chrono::Utc::now().to_rfc3339()` becomes `lgwks_std::time::now_rfc3339()`.
- One further site in `rust-ai-stack`, same shape.

`NaiveDateTime::parse_from_str` does **not** appear anywhere in the four repos.
The proposal said it appeared at two sites and that claim did not survive
re-derivation, so no `strptime` engine is needed and none was built.

**The real work here is a duplicate, not a dependency.**
`forge-sdk/forge-core/src/util.rs:71-118` already hand-rolls `now_iso()` and
`days_to_date()` specifically to avoid `chrono`, in a repo that then declares
`chrono` anyway and calls it once. That is two implementations of one concept
plus the dependency both were meant to avoid. Delete `now_iso`, `days_to_date`,
and `is_leap` from `util.rs` and route callers to `lgwks_std::time`.

Two behavioural differences to check when you do:

| | `util.rs::now_iso` | `lgwks_std::time` |
|---|---|---|
| pre-epoch clock | `unwrap_or_default()` → `1970-01-01` | preserved as a negative instant |
| year lookup | loops year by year from 1970 | closed-form, valid across the proleptic range |
| fractional seconds | never emitted | emitted when non-zero |

The first row is a swallowed fault, which SPINE rejects on sight — a
misconfigured host currently writes plausible 1970 stamps into audit logs
instead of an obviously wrong one.

### `glob` → `lgwks_std::glob` — 2 manifests, 3 sites

`lgwks_std::glob::matches(pattern, path)` is pure matching. The upstream crate
also walks the filesystem; no site in the estate uses that, but confirm per site
before deleting the dependency.

### `percent-encoding` → `lgwks_std::encoding::percent` — 1 manifest, 1 site

One site. `encode_component` escapes everything outside the RFC 3986 unreserved
set, which is stricter than upstream's default set — confirm the single site
wants component-safe escaping, which it almost certainly does.

### `base64` — nothing to migrate

0 manifests, 0 call sites. The module exists so the answer at rung 2 of the
admission ladder is already "yes".

### `rand` — **not** phase 1

The proposal put `rand` in phase 1 at 5 manifests. Re-derived: 4 manifests but
**43 call sites**, including distributions and seeded generators that
`lgwks_std::random` does not provide and should not grow casually. This is a
phase-3 CONSOLIDATE item with its own design pass. `lgwks_std::random` today is
the single OS-entropy owner that `id` is built on, and that is all it claims.

## ── Wiring the gate ─────────────────────────────────────────────────

Per repo, in this order — see `docs/ADMISSION.md` for the reasoning:

```
lgwks-gate init .                      # fail-closed starting register
# set [policy] enforce = false          # adoption only, in a signed diff
# add build.rs + [build-dependencies]
lgwks-gate check .                     # the work-list
# land the ELIMINATE swaps above
# approve what genuinely remains, rung by rung
# flip enforce = true                   # from here it is a ratchet
```

To see a repo's work-list *before* committing a register to it:

```
lgwks-gate check ~/Braid --contract /tmp/empty-register.toml
```

## ── Exit criterion ──────────────────────────────────────────────────

Per repo, `lgwks-gate check .` exits 0 with `[policy] enforce = true`, and:

```
cargo tree -e normal --prefix none | awk '{print $1}' | sort -u
```

shows only workspace members, `lgwks_std`, and crates carrying a register entry.
Run it before and after; the diff is the receipt.

<!--
Extracted from: logic-os-kernel/CLAUDE.md dependency factory pattern, the
org-wide vendor/ convention observed in forge-harness/lgwks_crawl (vendors
spider), forge-harness (vendors rust-genai), rust-ai-stack (vendors keel,
braid), and docs/handoffs/2026-08-17-lgwks-std-proposal.md.
Date: 2026-08-17. The lgwks_std proposal is a DESIGN, not shipped code — this
checklist references it as the first thing to check, not as a finished
dependency you can `cargo add` yet.
-->
# New Rust crate/repo checklist

## Before adding any dependency

1. **Check `lgwks_std` first** (once it exists — see
   `docs/handoffs/2026-08-17-lgwks-std-proposal.md` in this repo for the
   current design/status). If the need is hashing, hex/base64, a UUID, a
   RFC3339 timestamp, glob matching, or the async-trait dyn-dispatch pattern,
   it belongs there, not as a fresh crates.io dependency.
2. If it's genuinely a new class of need (an async runtime, a serialization
   framework, a regex engine, a proc-macro toolchain, a database engine, a
   capability-sandboxing library, an ML framework) — that's a BOUNDARY
   dependency per the lgwks_std proposal's tiering, not something to
   reimplement. Take the dependency directly.
3. If it's a **security-critical primitive** (crypto: hashing, signing,
   AEAD, key derivation) — never hand-roll it. Either it's already vendored
   under `lgwks_std/vendor/`, or it should be added there (pinned, audited
   upstream, with a `PROVENANCE.md` noting exact version/commit), not pulled
   as a loose crates.io dependency with an unpinned version range.

## Workspace dependency factory pattern

Per `logic-os-kernel/CLAUDE.md`: "Shared versions are declared once in
`<workspace>/Cargo.toml` `[workspace.dependencies]` ... and inherited via
`{ workspace = true }`. Never inline a version in a crate/package." Apply
this in any multi-crate workspace — a version pinned in N places is N places
that can silently drift, and nobody notices until a build breaks on exactly
one crate.

## Pin what needs pinning

- `rust-version` in the root `Cargo.toml` — checked 2026-08-17: **none** of
  Braid, forge-sdk, forge-harness, or rust-ai-stack currently pin this
  (`grep rust-version` across all four root manifests, zero matches). Don't
  repeat that gap in a new crate — state the MSRV explicitly, especially if
  it depends on a specific stdlib stabilization (e.g. `OnceLock`, native
  `async fn` in traits).
- The toolchain channel, if the repo runs its own CI gates — keel's
  `rust-toolchain.toml` + `scripts/pinned-channel.sh` pattern exists because
  a bare `cargo` picks the runner's default, which silently drifted from the
  pin at least once in that repo's history.

## Vendoring convention

When a dependency is kept but its *supply-chain edge* should be removed
(the reasoning behind vendoring `spider`, `rust-genai`, `keel`, `braid`
elsewhere in the constellation):

```
<crate>/vendor/<dep-name>/          # pinned source tree
<crate>/vendor/<dep-name>/PROVENANCE.md   # upstream repo, exact commit/tag,
                                            # license, date vendored, why
```

Reference the vendored path in `Cargo.toml` (`{ path = "vendor/<dep-name>" }`)
rather than a crates.io version range. Do not let cargo resolve a version for
anything meant to be pinned this way — that defeats the point.

## Exit check

```
cargo tree -e normal --prefix none | awk '{print $1}' | sort -u
```

Review the output against the crate's stated dependency policy (BOUNDARY
list, or "lgwks_std + stdlib only" once that exists) before merging. This is
the same command the lgwks_std proposal uses as its proof command — reuse it
rather than inventing a repo-local variant.

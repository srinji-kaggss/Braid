# lgwks_std proposal — one canonical dependency behind the constellation

## Status
ISSUE for DeepSeek execution. Claude lane (thinking/review) authored this; no crate code
written. This session's write authority is Braid-only (`wwfd-guard authority` denies writes
to `/Users/srinji`-rooted paths outside it `[proof]`), so the target repo below does not
exist yet and cannot be created from this session.

## Literal outcome (Director's words)
"End state will be no default deps outside of rust stdlib and lgwks_std." Scoped to the
four authorized Rust repos: `Braid`, `forge-sdk`, `forge-harness`, `rust-ai-stack`.

## Governance flag (read before executing)
Braid's `AGENTS.md` charter holds sole authority for `Cid`/IR/verifier here — a shared
utility library is a *different* authority and must not be nested inside Braid, forge-sdk,
forge-harness, or rust-ai-stack (any one of those becoming "host" would make the other three
depend on a product repo, the same anti-pattern the 2026-08-15 forge-sdk/forge-harness split
was fixing per `forge-sdk/Cargo.toml:17-23` `[read]`). This is a new repo: **`lgwks-std`**,
alongside `lgwks_crawl`/`lgwks_hashing` in the existing org naming family
(`forge-harness/lgwks_crawl/Cargo.toml` `[read]`; `forge-harness/forge-harness/src/referee/hashing.rs:1-8` `[read]`).
Crate name: `lgwks_std` (underscore, matching `lgwks_crawl`/`lgwks_hashing`, not `lgwks-std`
hyphenated — org convention uses underscore in the package name itself).
Director action needed before execution: create+authorize the `lgwks-std` repo, and widen
this session's (or a fresh session's) `authorized-repos` to include it plus write access to
`forge-sdk`, `forge-harness`, `rust-ai-stack` for the migration commits. **QUESTION** for the
Director: confirm repo name/location, or redirect.

## Dependency census (evidence, not guesswork)
Parsed every `[dependencies]`/`[workspace.dependencies]` block in all 24 `Cargo.toml` files
under Braid, forge-sdk, forge-harness, rust-ai-stack (excluding internal `braid-*`/`forge-*`
path crates) `[ran: python3 census.py over a full cat of all Cargo.toml files]`. Baseline
resolved-crate counts from each `Cargo.lock` `[ran: grep -c '^name = ']`: Braid 119,
forge-sdk 331, forge-harness 560, rust-ai-stack 1025.

External deps declared 2+ times:

| dep | files | class |
|---|---|---|
| serde, serde_json | 31, 31 | BOUNDARY |
| thiserror | 16 | ELIMINATE |
| tokio | 13 | BOUNDARY |
| hex | 11 | ELIMINATE |
| tracing | 11 | CONSOLIDATE |
| uuid | 9 | ELIMINATE |
| blake3, sha2 | 8, 8 | VENDOR |
| async-trait | 7 | CONSOLIDATE (own macro) |
| rand | 4 | CONSOLIDATE |
| anyhow | 4 | ELIMINATE |
| chrono | 4 | ELIMINATE (usage is UTC-stamp only, see below) |
| regex | 3 | BOUNDARY |
| ed25519-dalek | 3 | VENDOR |
| syn, quote, proc-macro2 | 3, 2, 2 | BOUNDARY |
| glob | 3 | ELIMINATE |
| rusqlite | 3 | BOUNDARY |
| proptest | 2 | BOUNDARY (dev-only) |
| base64, hmac | 2, 2 | ELIMINATE / VENDOR |
| cap-std | 2 | BOUNDARY |
| reqwest | 2 | CONSOLIDATE onto `ureq` (already the org's HTTP client in `lgwks_crawl/Cargo.toml` `[read]`) |
| walkdir, fs2, notify, libloading, rayon, aes-gcm, machine-uid, percent-encoding, zeroize | 1 each | ELIMINATE / VENDOR, see table below |
| burn-*, candle-*, tract, ort, tokenizers, polars, image, dasp | 1-2 each | BOUNDARY — ML/media frameworks, out of scope |

Full per-dep file lists: `census.py` + `all_cargo_tomls.txt` in
`/private/tmp/claude-501/-Users-srinji/91847e70-463f-4b73-a99f-9a7097b4759b/scratchpad/`
(session scratch, not durable — re-run census.py against fresh Cargo.toml files before
executing if this doc is used more than a few days after 2026-08-17).

## Why three classes, not one (research-backed)
The literal ask ("no default deps outside stdlib and lgwks_std") is satisfiable **without**
reimplementing cryptography or an async runtime, using a pattern this org already runs:
vendoring. `lgwks_crawl` already vendors `spider` at `../vendor/spider` instead of taking a
crates.io dep (`lgwks_crawl/Cargo.toml` comment `[read]`); `forge-harness` vendors
`rust-genai`; `rust-ai-stack` vendors `keel` and `braid`. **lgwks_std generalizes this**: for
anything unsafe or absurd to hand-roll, lgwks_std vendors the audited source internally and
re-exports a narrow API, so every consumer `Cargo.toml` lists exactly one non-stdlib line —
`lgwks_std` — even though crypto/regex/tokio bytes still exist in the tree.

- **ELIMINATE** (reimplement in lgwks_std, delete the crates.io dep entirely): the
  functionality is small enough that reimplementation risk is lower than a supply-chain
  dependency's. Confirmed by codegraph usage queries, not assumption:
  - `chrono` — actual usage is `DateTime<Utc>` stamping + RFC3339 parse/format only
    (`forge-harness/src/referee/clock.rs:7`, `daemon_store/store.rs:847,875` `[read]`
    `[ran: codegraph query chrono --path forge-harness]`). No timezone/calendar arithmetic.
    `std::time::SystemTime` + a ~40-line RFC3339 formatter covers 100% of observed call sites.
  - `hex`, `base64`, `glob`, `percent-encoding` — well-known ~30-80 line algorithms, no
    correctness-critical security property.
  - `uuid` — v4 generation needs only OS randomness (`getrandom(2)`/`/dev/urandom` via
    `std::fs::File::open("/dev/urandom")` or the `getrandom` syscall wrapper already implied
    by `rand`'s OS source) + RFC 4122 byte layout.
  - `thiserror`, `anyhow` — boilerplate-generation macros; a ~150-line proc-macro in
    lgwks_std covering the observed `#[error("...")]`/`#[from]` usage shapes replaces both.
  - `walkdir`, `fs2` — thin wrappers over `std::fs::read_dir` recursion / `flock(2)`.
- **CONSOLIDATE** (one lgwks_std module, one implementation choice, still may vendor
  internally): multiple crates doing the same job today.
  - `async-trait` → lgwks_std ships its own `#[lgwks_async_trait]` attribute macro
    reproducing the documented expansion (`Pin<Box<dyn Future + Send + 'a>>` per method,
    exact mechanism confirmed current as of Rust 1.97.1/2026-07-16 — dyn async traits are
    **still not stabilized**: `dyn Trait` with an `async fn` member still hits `E0038`
    dyn-compatibility, per the async-trait README's own current documented error message
    `[read: https://github.com/dtolnay/async-trait fetched 2026-08-17 via 9router]`
    `[read: https://doc.rust-lang.org/releases.html, latest stable 1.97.1, 2026-07-16,
    no dyn-async-trait stabilization in Language/Stabilized-APIs sections, via 9router]`).
    This is a reimplementation of dtolnay's own documented macro, not a guess.
  - `tracing` → keep the facade (structured logging is not worth hand-rolling), but
    lgwks_std owns the one `tracing-subscriber` init + format, so no repo picks its own.
  - `rand` → lgwks_std wraps one CSPRNG source (OS randomness) for both `uuid` generation
    and any other randomness need; stops the current split between `rand` crate use and
    ad hoc approaches.
  - `reqwest` → drop; standardize every repo on `ureq` (already in `lgwks_crawl`), wrapped
    by an `lgwks_std::http` module so call sites don't touch `ureq` directly.
- **VENDOR** (keep the real, audited implementation; delete the crates.io *dependency edge*
  by pulling the source into `lgwks-std/vendor/<crate>`, pinned, per the existing org
  pattern): cryptography and anything where a hand-rolled reimplementation is a security
  regression, not a simplification.
  - `sha2`, `blake3`, `hmac`, `ed25519-dalek`, `aes-gcm`, `zeroize` — hand-rolling
    cryptographic primitives is the canonical way to introduce timing/side-channel bugs;
    RustCrypto's crates are the audited reference. Vendor, don't reimplement.
  - `ring` — same reasoning.
- **BOUNDARY** (out of scope for lgwks_std, keep as direct deps): `tokio` (no async
  runtime in std, and reimplementing one is a multi-year undertaking, not a "stdlib+"
  module), `serde`/`serde_json` (reimplementing a derive-based serialization framework
  correctly is its own multi-year project — precedent: this is *why* serde exists at all),
  `regex` (a regex engine is a correctness-critical parser+VM, not stdlib+ material —
  rust-lang/regex is maintained by the same team as rustc itself), `syn`/`quote`/
  `proc-macro2` (this **is** "the compiler as a library" for macros; no stdlib
  equivalent exists or is planned), `rusqlite` (SQLite bindings — reimplementing a
  database engine is out of scope), `cap-std` (capability-secure OS sandboxing —
  security-critical, upstream-audited), `burn`/`candle`/`tract`/`ort`/ML frameworks
  (domain-specific, not general-purpose stdlib+ material), `proptest` (dev-dependency
  only, doesn't ship).

## lgwks_std module layout (spec for the new repo)
```
lgwks-std/
  Cargo.toml            # package lgwks_std, edition pinned, rust-version set explicitly
                         # (none of the 4 repos currently pin rust-version — [ran: grep
                         # rust-version across all 4 root Cargo.toml, zero matches — this
                         # is a real gap: lgwks_std depends on stable APIs current as of
                         # 1.97, so it must declare rust-version = "1.75" at minimum, since
                         # its own async-trait macro depends on async fn in traits stabilized
                         # in 1.75, and should be re-verified against each consumer's actual
                         # toolchain before merge)
  vendor/
    sha2/ blake3/ hmac/ ed25519-dalek/ aes-gcm/ zeroize/ ring/   # pinned source, RustCrypto
                                                                  # upstream, same convention
                                                                  # as lgwks_crawl's vendor/spider
  src/
    lib.rs
    hash.rs        # hex encode/decode, re-exports vendored sha2/blake3/hmac API
    crypto.rs       # re-exports vendored ed25519-dalek/aes-gcm/zeroize/ring API
    id.rs           # uuid v4 (OS randomness + RFC 4122 layout)
    time.rs         # RFC3339 stamp/parse over std::time::SystemTime (replaces chrono usage)
    error.rs        # #[derive(Error)]-equivalent + anyhow-equivalent proc-macro
    async_trait.rs  # #[lgwks_async_trait] attribute macro (replaces async-trait crate)
    http.rs         # thin wrapper over vendored/pinned ureq, one client config
    fs.rs           # walkdir-equivalent recursive walk, fs2-equivalent advisory lock
    encoding.rs     # base64, percent-encoding
    glob.rs         # glob pattern matching
    rand.rs         # OS-randomness CSPRNG wrapper, backs id.rs and any other caller
  README.md         # states the ELIMINATE/CONSOLIDATE/VENDOR tiers above as the contract
```

## Migration order (per repo, smallest blast radius first)
1. **lgwks-std** — stand up the repo, implement `hash.rs`/`encoding.rs`/`glob.rs`/`time.rs`
   (pure reimplementation, zero vendoring, fastest to land and prove).
2. **lgwks-std** — vendor tier (`crypto.rs`, `hash.rs`'s sha2/blake3 backing), pin exact
   upstream commit/version per vendored crate, record provenance in `vendor/<crate>/PROVENANCE.md`
   (matches the `lgwks_crawl` spider-vendoring precedent).
3. **lgwks-std** — `async_trait.rs`, `error.rs`, `rand.rs`, `http.rs` (the CONSOLIDATE tier;
   each needs a focused regression test reproducing the current call-site shape before
   swap-in, since these change generated code, not just an import path).
4. **Braid** (smallest repo, 119 resolved crates) — migrate `hex`, `blake3`, `sha2`,
   `ed25519-dalek` call sites in `braid-governance`, `braid-ir`, `braid-verify` onto
   `lgwks_std::hash`/`crypto`. Braid's `Cid` contract (`crates/braid-ir/src/cid.rs`) is a
   G4 authority per `AGENTS.md` — any change to its byte-level output is a charter event,
   so this migration must be **byte-identical**, proven with a differential test over a
   fixed corpus of existing `Cid` values before/after the swap, not just "it compiles."
5. **forge-sdk** → **forge-harness** → **rust-ai-stack**, same per-crate substitution order
   (ELIMINATE tier first, VENDOR tier second, CONSOLIDATE tier last since it touches the
   most call sites: `async-trait` alone spans `forge-core/{agent,auth,doctor,metering,pool}.rs`
   and classifier modules — 7+ files, needs a mechanical per-file diff review, not a
   sed pass).

## Exit criterion
Per repo, after migration:
```
cargo tree -e normal --prefix none | awk '{print $1}' | sort -u
```
must show only: workspace member crates, `lgwks_std`, and the BOUNDARY-tier crates named
above (tokio, serde, serde_json, regex, syn, quote, proc-macro2, rusqlite, cap-std, and the
ML frameworks in rust-ai-stack). Any other external name in that output is a regression.

## Proof command
```
for repo in Braid forge-sdk forge-harness rust-ai-stack; do
  echo "=== $repo ==="
  cargo tree --manifest-path ~/$repo/Cargo.toml -e normal --prefix none \
    | awk '{print $1}' | sort -u
done
```
Run before and after migration; diff the two outputs per repo as the completion receipt.

## Open items for the Director (non-derivable)
- Confirm `lgwks-std` as repo name/location (or redirect) and authorize it +
  forge-sdk/forge-harness/rust-ai-stack write access for the execution session.
- Confirm the VENDOR-tier crate pins (exact versions to freeze) — this proposal names the
  crates, not exact commit hashes, since that's a decision to make at execution time against
  whatever RustSec-clean version is current then.
- `keel` (the excellence gate) is explicitly withheld from this session's authority — if
  `lgwks-std` should register with keel/CI the way Braid and forge-harness do, that's a
  separate grant.

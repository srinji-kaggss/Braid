# PB-04 — First live consumers: the browser collapse + the kernel live-wire (D-CONSUMER)

**Objective**: go from zero dependents to two. "Become a dependency" is possible
(tag `braid-v0.1` is cut, `braid-capability` is crates.io-ready) but not actual.
A platform with no consumers is a spec. This playbook executes the two consumer
integrations that are already designed and waiting.

**Why this is cheap and unblocked**: both seams were prepared from both sides —
the browser repo's own `BRAID_BRIDGE.md §6` already commits to depending on
`braid-ir`/`braid-capability`; the kernel's `braid_vocab_binding.rs` is a pinned
snapshot with a literal "when braid-ir is wired in: replace ONLY the body" note.
And since the steer was written, `braid-vocab-web` (PR #13) moved the browser's
closed `web.*` vocabulary INTO this repo as its canonical home — the collapse
target is now even smaller than the steer describes.

## Deep-learning corpus (read → probe)

1. `docs/BRAID_STEER.md` (this repo's own delivered steer — the 4-step collapse) and
   `~/next-gen-browser-engine/docs/BRAID_BRIDGE.md` (the browser's contract, esp. §5
   SHA-256→BLAKE3 and §6 no-parallel-registry).
2. `~/src/browser-engine/src/braid_bridge/term.rs` — the parallel `BraidTerm` enum:
   the kill target. Note (memory `feedback_wrapup_boilerplate_verify` class of
   lesson): there were historically TWO browser-engine checkouts
   (`~/next-gen-browser-engine` and `~/src/browser-engine`, one 9 commits stale) —
   **establish the canonical clone first** (Prime Directive: never operate on a
   divergent duplicate; reconcile or flag).
3. `crates/braid-vocab-web/src/lib.rs` — the web vocabulary as it now exists here
   (effect classes, egress ceilings, `COMPUTE_LOCAL_NAME` etc. — the tests in that
   file encode the browser's own security assertions).
4. Kernel side: `~/logic-os-kernel/kernel/.../braid_vocab_binding.rs` (grep for it;
   it has a compiled binding artifact trail in `target/`), plus ADR-088 extraction
   addendum ("D11 vocabulary-binding moved to the consumer side").
5. `crates/braid-ir/tests/boundary_conformance.rs` — what consumers may import.

Probe: in a scratch crate, `cargo add braid-ir braid-capability braid-vocab-web
--git https://github.com/srinji-kaggss/Braid --tag braid-v0.1` and round-trip a
`web.*` capsule through `braid-verify` — prove the consumer path works before
touching either consumer repo.

## Invariants

- **One hash discipline**: the browser's `Cid = String` / SHA-256 interface must
  become `braid_ir::Cid` (BLAKE3, `lw.braid.*`). A parallel hash is the "second
  authority system" D11 forbids — the steer's one explicit pushback. No transition
  period where both hashes are minted.
- **Vocabulary ownership**: the `web.*` terms' semantics belong to the browser team
  even though the crate now lives here — changes to `braid-vocab-web` require
  browser-side review (record this in the crate README).
- **The kernel binds, Braid does not re-vendor**: the kernel-side integration test
  asserts the registry it pins matches its own vocabulary (extraction addendum);
  Braid never imports kernel internals back (T15).
- **No behavior change during collapse**: the browser's existing tests must pass
  before/after with the enum swapped for the real IR — this is a pure
  duplicate-collapse (Prime Directive 3), not a feature.

## Execution steps

1. **Canonical-clone audit**: `git remote -v` + `git log` on both browser checkouts;
   pick/flag the live one; record in the issue.
2. **Browser collapse** (execute the steer's 4 steps, adjusted for PR #13):
   a. Add the git deps pinned to `braid-v0.1` (or cut `braid-v0.2` first if PB-01
      landed changes worth having — tag before, not during).
   b. Delete the `BraidTerm` enum; route `WebAnchor` ↔ Braid through
      `braid_ir::Capsule`/`Cid`/`Value`; the browser's registry constructor now
      calls `braid_vocab_web::registry()` instead of defining terms.
   c. Replace the SHA-256 CID path with `braid_ir::Cid` end-to-end.
   d. Delete any browser-side term/capability definitions now duplicated by
      `braid-vocab-web` — the crate here is canonical.
3. **Kernel live-wire** (issue #565 per the debt register): replace the
   `braid_vocab_binding.rs` snapshot body with a decode of
   `braid_vocab_cms::registry_v0()`; the dotted names were preserved verbatim so
   its assertions stay green; the test now fails if kernel vocabulary and Braid
   registry drift — which is the point.
4. **Version covenant in practice**: document in both consumer repos how a Braid
   version bump reaches them (pin bump PR + registry_cid change surfaces in the
   manifest diff — the D11 conscious-event rule).
5. Adversarial check: attempt to construct a browser-side capsule that admits
   against a stale registry pin (T6 across the repo boundary).

## Verification

```bash
# in Braid: cargo test --workspace (boundary + vocab-web tests green)
# in browser repo: full test suite green with BraidTerm deleted; grep proves no
#   parallel enum/SHA-256 CID remains:  git grep -n 'BraidTerm\|sha256' -- src/
# in kernel: the re-homed binding test green against braid-vocab-cms
# consumer smoke: the scratch-crate probe compiles against the published tag
```

## Exit criteria

Two live dependents building against a pinned Braid tag; zero parallel
term/hash/capability implementations left in either consumer (grep-proven);
DEBT_REGISTER D-CONSUMER closed; the dependency direction and re-sync covenant
documented. This is the "brew-ubiquity" arc's first two data points (D31) — and the
existence proof PB-05 cites when it claims the platform has real workloads.

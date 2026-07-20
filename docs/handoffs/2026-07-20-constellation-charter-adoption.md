# Constellation Charter adoption — 2026-07-20

## What changed
- Adopted the Constellation Charter (canonical at `~/logic-os-kernel/laws/CONSTELLATION-CHARTER.md`, ratified 2026-07-20 by Director).
- New `AGENTS.md` (didn't exist before) now opens with a charter pointer + this repo's role line.

## This repo's role under the charter
**Role:** canonical machine-first IR + encoding + verifier.
**Authority held (SOLE across the constellation):** `Cid` contract (`pub struct Cid(pub [u8; 32])`, `crates/braid-ir/src/cid.rs`), canonical term vocabularies, verifier reference.
**Authority NOT held:** Capability envelope (kernel), Verdict (kernel), Fact envelope (kernel), Principal (kernel), Receipt schema (forge-harness).

## Local implications
- **Publishing discipline is now load-bearing.** Downstream repos (browser `vendor/braid/`, kernel path-deps) need Braid crates published so they can consume versioned deps instead of snapshots. Charter Step 3 gate blocks convergence until this is done.
- Every breaking change to `Cid`, canonical encoding, or vocabulary registry is a G4 concept-authority event — requires charter amendment (`~/.claude/state/charter-unlock` sentinel + Director ADR).
- Kernel already path-deps `braid-ir` + `braid-vocab-cms` — that edge is CORRECT direction and confirmed working.

## Follow-ups outstanding
- Publish Braid crates (versioned) so browser can drop `vendor/braid/` and consume the real crate.
- Any Cid or vocabulary change → surface as an ADR under `logic-os-kernel/laws/governance/` before merge.

## Provenance
- Commit: `8174963e05ac` (this docs file will be a subsequent commit).
- Branch: `fix/cc-1.3.0-toolchain-drift`.
- Not yet pushed.
- Related memory: `~/.claude/projects/-Users-srinji/memory/project_constellation_charter_2026_07_20.md`.

//! `lgwks_std` owns the estate's only approved non-`std` surface and enforces
//! INV-STDPLUS-ZERO-DEPS: every module here is built against Rust's `std`
//! alone, so a consumer that takes this crate adds exactly one line to its
//! manifest and zero transitive crates to its lock file.
//!
//! Phase 1 covers the ELIMINATE tier named in
//! `Braid/docs/handoffs/2026-08-17-lgwks-std-proposal.md` — the crates small
//! enough that reimplementation carries less risk than a supply-chain edge.
//! Each module records the upstream crate it retires and the measured call
//! surface it has to cover, so the swap is a mechanical substitution rather
//! than a redesign.
//!
//! The VENDOR tier is deliberately absent. Hand-rolling a hash, a MAC, or a
//! signature is a security regression dressed as a simplification, so `sha2`,
//! `blake3`, `hmac`, `ed25519-dalek`, `aes-gcm`, and `zeroize` arrive in a
//! later phase as pinned upstream source — the audited bytes, re-exported
//! behind a narrow API, matching the `lgwks_crawl`/`vendor/spider` precedent.
//!
//! What is *not* here is as much of the contract as what is. `tokio`, `serde`,
//! `regex`, `syn`, `rusqlite`, and `cap-std` are BOUNDARY tier: they stay
//! direct dependencies, declared once each in
//! `contract/APPROVED.toml` with a human's name against them.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod encoding;
pub mod fs;
pub mod glob;
pub mod hex;
pub mod id;
pub mod leb128;
pub mod random;
pub mod task;
pub mod time;

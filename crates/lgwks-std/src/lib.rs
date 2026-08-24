//! `lgwks_std` owns the estate's one approved non-`std` surface.
//!
//! Every module is a declarative primitive — identity, hashing, encoding,
//! pattern matching, time, filesystem, regex — built on a vetted dependency
//! stack that bottoms out at zero external deps. A consumer that takes this
//! crate gets one import path for the operations every repo needs, instead of
//! seven different crates with seven different APIs.
//!
//! ## Dependency contract
//!
//! INV-STDPLUS-APPROVED-ONLY: every dependency is a vetted leaf or
//! single-purpose stack with zero further external deps:
//!
//! - `blake3` (arrayvec, cfg-if, constant_time_eq — all zero-dep leaves)
//! - `regex` (memchr, regex-syntax, aho-corasick, regex-automata — all
//!   BurntSushi, all internal to the regex stack, no external deps)
//! - `rkyv` (rend, ptr_meta, rancor, munge + derive — all djkoloski, zero
//!   external deps; proc-macro stack shared with serde)
//! - `serde` + `serde_json` (itoa, ryu — zero-dep leaves; proc-macro stack
//!   is proc-macro2, quote, syn)
//!
//! What is *not* here is as much of the contract as what is. `tokio`,
//! `rusqlite`, and `ed25519-dalek` are BOUNDARY tier: they stay direct
//! dependencies in consumer crates, declared in `contract/APPROVED.toml`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
pub mod encoding;
pub mod fs;
pub mod glob;
#[cfg(feature = "hash")]
pub mod hash;
pub mod hex;
pub mod id;
#[cfg(feature = "json")]
pub mod json;
pub mod leb128;
#[cfg(feature = "pattern")]
pub mod pattern;
pub mod random;
pub mod task;
pub mod time;
#[cfg(feature = "wire")]
pub mod wire;

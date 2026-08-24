//! Nightly-only experiment surface for Braid.
//!
//! INV-NIGHTLY-ISOLATION — this crate builds only inside the nightly
//! toolchain pinned by this directory's `rust-toolchain.toml`. It is not a
//! workspace member and never ships; promotion into `crates/` requires the
//! feature to exist on stable 1.98+ or an approved contract change.

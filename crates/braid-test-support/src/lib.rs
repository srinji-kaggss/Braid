//! Shared test-only dependencies for the Braid workspace.
//!
//! Production crates depend only on this local boundary in dev scope; the
//! external property engine is registered and owned once here.

#![forbid(unsafe_code)]

/// Property-test engine used by Braid's falsification suites.
pub use proptest;

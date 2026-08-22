//! INV-DEP-REGISTERED — nothing outside `std` and `std+` reaches this
//! workspace's lock file without a human's approval in `contract/APPROVED.toml`.
//!
//! `lgwks_std_gate::enforce` reads the *resolved* graph from the workspace
//! `Cargo.lock`, not this crate's declared dependencies, so a single attachment
//! point audits every crate Braid resolves — including the ones that arrive
//! only as somebody else's transitive. `braid-cli` is that point because it is
//! the operator surface and sits outside the verified core, so wiring it costs
//! the trust base nothing.
//!
//! Braid's register is in adoption mode (`[policy] enforce = false`), so
//! refusals arrive as `cargo::warning` and this build passes. The order out of
//! adoption is `docs/LGWKS-STD-MIGRATION.md`.

fn main() {
    lgwks_std_gate::enforce();
}

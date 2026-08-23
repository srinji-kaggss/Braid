//! Library surface for the Braid operator executable.
//!
//! Keeping command dispatch outside `main.rs` makes the process entrypoint a
//! thin shell and lets tests exercise the same dispatch path that the binary
//! uses. The CLI remains the sole owner of its operator output contracts.

mod cli;
mod store;

pub use cli::run;

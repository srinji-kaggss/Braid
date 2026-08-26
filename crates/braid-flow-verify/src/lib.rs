//! Independent Flow graph admission verifier — P2 #60.
//!
//! Own decoder (preflight + projection) so producer ≠ admission.
//! Fail-closed typed refusals per INV-FLOW-*.

#![forbid(unsafe_code)]
#![deny(warnings)]

extern crate alloc;

pub mod decode;
pub mod error;
pub mod verify;

pub use error::{FlowVerifyError, VerifyResult};
pub use verify::{AdmittedFlow, verify};

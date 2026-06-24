//! Capability token — the closed permission vocabulary boundary (ADR-088 D3).
//!
//! //why a string newtype, not a fixed enum: Braid is a *global* IR (D31). A
//! fixed enum of verbs ties every consumer to one domain's capability space —
//! the kernel's `signal.emit`/`motion.schedule`, the browser's `web.dom.read`,
//! a JS frontend's `js.eval`, a Julia frontend's `julia.io` are mutually
//! foreign. A union enum re-couples every consumer to every other domain's
//! verbs (the "good enough to not be a fork" failure). A content-addressed
//! string token lets each vocabulary package declare its own capability space,
//! while the verifier's attenuation check (grant ⊆ ambient) works on any token
//! set — the lattice order is declared per-vocabulary, not hardcoded here.
//!
//! The dotted name (`web.dom.read`, `compute.remote`) is the protocol-stable
//! identity: it is what canonical encoding serializes, what the manifest
//! renders, and what feeds capsule CIDs. Drift in a name silently changes
//! content addresses — vocabulary packages own their names the way the kernel
//! enum owned its `#[strum(serialize)]` attributes.
//!
//! The token is canonicalized at construction (the dotted name is stored
//! verbatim; no normalization) so `Capability::new("web.dom.read")` and
//! `Capability::from_str("web.dom.read")` produce the same bytes.

#![no_std]
extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt;
use core::str::FromStr;
use serde::{Deserialize, Serialize};

/// A capability permission token — a dotted string name drawn from a
/// vocabulary's declared capability space. Capabilities are compared by name;
/// the verifier checks `grant ⊆ ambient` by name equality.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Capability(pub String);

impl Capability {
    /// Construct a capability from its dotted name. The name is stored
    /// verbatim (no normalization) so it round-trips through canonical
    /// encoding byte-for-byte.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The dotted name — the protocol-stable identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Capability {
    type Err = core::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_the_identity() {
        let cap = Capability::new("web.dom.read");
        assert_eq!(cap.as_str(), "web.dom.read");
        assert_eq!(cap.to_string(), "web.dom.read");
        assert_eq!(Capability::from_str("web.dom.read").unwrap(), cap);
    }

    #[test]
    fn equality_is_by_name() {
        assert_eq!(Capability::new("a.b"), Capability::new("a.b"));
        assert_ne!(Capability::new("a.b"), Capability::new("a.c"));
    }

    #[test]
    fn arbitrary_dotted_names_are_representable() {
        // A global IR must accept any vocabulary's capability space — kernel,
        // browser, JS, Julia — without a core edit.
        for name in [
            "signal.emit",
            "web.dom.read",
            "js.eval",
            "julia.io",
            "compute.remote",
        ] {
            let cap = Capability::new(name);
            assert_eq!(cap.as_str(), name);
        }
    }

    #[test]
    fn serde_round_trips_the_name() {
        let cap = Capability::new("motion.schedule");
        let j = serde_json::to_string(&cap).unwrap();
        assert_eq!(j, "\"motion.schedule\"");
        let back: Capability = serde_json::from_str(&j).unwrap();
        assert_eq!(back, cap);
    }
}

//! Content addressing (D8): BLAKE3 with explicit domain separation and a
//! length-framed payload — the same preimage discipline as
//! `state-fabric::compute_content_hash` (domain ‖ framed fields), under Braid's
//! own `lw.braid.*` namespace so a Braid hash can never collide a fact hash.

use alloc::format;
use alloc::string::String;

/// Domain for a capsule's content address.
pub const CAPSULE_DOMAIN: &[u8] = b"lw.braid.capsule.v0";
/// Domain for a term-registry's content address (pinned inside every capsule —
/// threat T6: a capsule commits to the EXACT alphabet it was authored against).
pub const REGISTRY_DOMAIN: &[u8] = b"lw.braid.registry.v0";

/// A Braid content identifier: 32 BLAKE3 bytes under a stated domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cid(pub [u8; 32]);

impl Cid {
    /// `BLAKE3(domain ‖ len(payload) ‖ payload)` — the length frame keeps a
    /// future multi-field preimage from being ambiguous with this one.
    pub fn compute(domain: &[u8], payload: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        hasher.update(&(payload.len() as u64).to_be_bytes());
        hasher.update(payload);
        Cid(*hasher.finalize().as_bytes())
    }

    pub fn to_hex(&self) -> String {
        let mut hex_string = String::with_capacity(64);
        for byte_val in self.0 {
            hex_string.push_str(&format!("{byte_val:02x}"));
        }
        hex_string
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64 || !hex.is_ascii() {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            let hex_pair = core::str::from_utf8(chunk).ok()?;
            out[i] = u8::from_str_radix(hex_pair, 16).ok()?;
        }
        Some(Cid(out))
    }
}

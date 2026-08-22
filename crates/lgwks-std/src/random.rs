//! `random` owns every source of randomness in the estate and enforces
//! INV-RANDOM-ONE-SOURCE: all randomness comes from the operating system's
//! CSPRNG, and a failure to read it is an error the caller must handle — never
//! a silent fall back to a clock, a counter, or a userspace PRNG.
//!
//! Backs `uuid` v4 generation and is the intended home for the `rand` crate's
//! work — but `rand` is declared in 4 manifests and reached from **43** call
//! sites, by far the widest of the phase-1 targets, and those sites include
//! distributions and seeded generators this module does not provide. `rand` is
//! therefore a phase-3 CONSOLIDATE item, not a phase-1 ELIMINATE one; what is
//! here now is the single OS-entropy owner the rest will be built on. A second
//! source of randomness would be a second source of truth for one concept,
//! which Law 3 forbids.
//!
//! The entropy device is `/dev/urandom`, which is non-blocking and
//! cryptographically seeded on both platforms the estate targets. A fallback
//! path is deliberately absent: a hidden downgrade from CSPRNG bytes to
//! timestamp bytes is the exact failure mode that makes generated identifiers
//! predictable, so this module surfaces the fault instead.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;

const ENTROPY_DEVICE: &str = "/dev/urandom";

/// The OS entropy source could not be read. There is no second source; a caller
/// that cannot proceed without randomness must fail, not substitute.
#[derive(Debug)]
pub struct EntropyError {
    device: &'static str,
    cause: std::io::Error,
}

impl EntropyError {
    /// The device path that could not be read.
    pub fn device(&self) -> &'static str {
        self.device
    }
}

impl fmt::Display for EntropyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not read OS entropy from {}: {}", self.device, self.cause)
    }
}

impl Error for EntropyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

/// Fills `buf` with cryptographically secure random bytes from the OS.
pub fn fill_bytes(buf: &mut [u8]) -> Result<(), EntropyError> {
    if buf.is_empty() {
        return Ok(());
    }
    let mut device = File::open(ENTROPY_DEVICE)
        .map_err(|cause| EntropyError { device: ENTROPY_DEVICE, cause })?;
    device
        .read_exact(buf)
        .map_err(|cause| EntropyError { device: ENTROPY_DEVICE, cause })
}

/// Returns `N` cryptographically secure random bytes from the OS.
pub fn bytes<const N: usize>() -> Result<[u8; N], EntropyError> {
    let mut out = [0u8; N];
    fill_bytes(&mut out)?;
    Ok(out)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_the_whole_buffer() {
        // A 64-byte draw leaving the sentinel untouched everywhere would be a
        // short read presented as success.
        let mut buf = [0xAAu8; 64];
        fill_bytes(&mut buf).expect("OS entropy unavailable");
        assert!(buf.iter().any(|&b| b != 0xAA), "buffer looks unwritten");
    }

    #[test]
    fn successive_draws_differ() {
        let a = bytes::<32>().expect("OS entropy unavailable");
        let b = bytes::<32>().expect("OS entropy unavailable");
        assert_ne!(a, b);
    }

    #[test]
    fn empty_buffer_is_a_no_op() {
        assert!(fill_bytes(&mut []).is_ok());
    }
}

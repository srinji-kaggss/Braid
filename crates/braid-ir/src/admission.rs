//! Compact, deterministic invocation admission algebra.
//!
//! Braid's root admission question is three-dimensional:
//!
//! - **Safety** — is the computation structurally and semantically admitted?
//! - **Capability** — is the caller authorized by the external authority boundary?
//! - **Justification** — is there a snapshot-bound reason to run now?
//!
//! These are not author-controlled booleans. A verifier, authority adapter, or
//! planner produces evidence and projects its result into [`ProofState`].
//! [`AdmissionTriad`] only composes those results. It cannot create evidence,
//! mint authority, or turn an unknown proof into success.
//!
//! The packed representation is deliberately one byte. Zero means
//! `Unknown × Unknown × Unknown`, so zero-initialization fails closed by
//! deferring rather than executing.

/// Result of checking one admission axis.
///
/// The declaration order is the conservative lattice order:
/// `Disproven < Unknown < Proven`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProofState {
    /// A counterexample or failed obligation exists.
    Disproven,
    /// The obligation was not established under the declared proof envelope.
    Unknown,
    /// The obligation was established by the owning verifier.
    Proven,
}

impl ProofState {
    /// Conservative conjunction. Only two proven inputs remain proven.
    pub const fn meet(self, other: Self) -> Self {
        match (self, other) {
            (Self::Disproven, _) | (_, Self::Disproven) => Self::Disproven,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Proven, Self::Proven) => Self::Proven,
        }
    }
}

/// One coordinate of Braid's admission triad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdmissionAxis {
    /// Structural, type, effect, taint, and resource safety.
    Safety,
    /// Externally supplied capability/authority.
    Capability,
    /// Snapshot-bound need and purpose.
    Justification,
}

/// Deterministic reduction of the complete triad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationDecision {
    /// At least one required obligation was disproven.
    Reject {
        /// First disproven axis in stable triad order.
        axis: AdmissionAxis,
    },
    /// Nothing was disproven, but at least one obligation is unknown.
    Defer {
        /// First unknown axis in stable triad order.
        axis: AdmissionAxis,
    },
    /// Every axis is proven; execution may be considered by the runtime.
    Execute,
}

/// Invalid compact admission representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionEncodingError {
    /// Bits outside the three two-bit coordinates were set.
    ReservedHighBits {
        /// Rejected packed byte.
        packed: u8,
    },
    /// A coordinate used the reserved `0b11` state.
    ReservedState {
        /// Coordinate containing the reserved state.
        axis: AdmissionAxis,
    },
}

impl core::fmt::Display for AdmissionEncodingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ReservedHighBits { packed } => {
                write!(f, "admission byte {packed:#010b} sets reserved high bits")
            }
            Self::ReservedState { axis } => {
                write!(f, "admission axis {axis:?} uses reserved state 0b11")
            }
        }
    }
}

impl core::error::Error for AdmissionEncodingError {}

const STATE_MASK: u8 = 0b11;
const SAFETY_SHIFT: u8 = 0;
const CAPABILITY_SHIFT: u8 = 2;
const JUSTIFICATION_SHIFT: u8 = 4;
const RESERVED_HIGH_BITS: u8 = 0b1100_0000;

const fn encode_state(state: ProofState) -> u8 {
    match state {
        ProofState::Unknown => 0b00,
        ProofState::Proven => 0b01,
        ProofState::Disproven => 0b10,
    }
}

const fn decode_known_state(bits: u8) -> ProofState {
    match bits {
        0b01 => ProofState::Proven,
        0b10 => ProofState::Disproven,
        _ => ProofState::Unknown,
    }
}

const fn axis_shift(axis: AdmissionAxis) -> u8 {
    match axis {
        AdmissionAxis::Safety => SAFETY_SHIFT,
        AdmissionAxis::Capability => CAPABILITY_SHIFT,
        AdmissionAxis::Justification => JUSTIFICATION_SHIFT,
    }
}

/// One-byte projection of Safety × Capability × Justification.
///
/// This type is a compact *result carrier*, not a proof token. It is not part
/// of the canonical capsule wire and must never be accepted as authority merely
/// because its bits say [`ProofState::Proven`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AdmissionTriad(u8);

impl AdmissionTriad {
    /// Construct a triad from independently established axis states.
    pub const fn new(
        safety: ProofState,
        capability: ProofState,
        justification: ProofState,
    ) -> Self {
        Self(
            (encode_state(safety) << SAFETY_SHIFT)
                | (encode_state(capability) << CAPABILITY_SHIFT)
                | (encode_state(justification) << JUSTIFICATION_SHIFT),
        )
    }

    /// Decode a packed byte, refusing reserved states and future-version bits.
    pub fn from_packed(packed: u8) -> Result<Self, AdmissionEncodingError> {
        if packed & RESERVED_HIGH_BITS != 0 {
            return Err(AdmissionEncodingError::ReservedHighBits { packed });
        }
        for axis in [
            AdmissionAxis::Safety,
            AdmissionAxis::Capability,
            AdmissionAxis::Justification,
        ] {
            let bits = (packed >> axis_shift(axis)) & STATE_MASK;
            if bits == STATE_MASK {
                return Err(AdmissionEncodingError::ReservedState { axis });
            }
        }
        Ok(Self(packed))
    }

    /// Return the compact byte.
    pub const fn packed(self) -> u8 {
        self.0
    }

    /// Read one axis.
    pub const fn state(self, axis: AdmissionAxis) -> ProofState {
        decode_known_state((self.0 >> axis_shift(axis)) & STATE_MASK)
    }

    /// Return a copy with one coordinate replaced.
    ///
    /// This is composition machinery only; callers still need evidence from
    /// the subsystem that owns the axis.
    pub const fn with_state(self, axis: AdmissionAxis, state: ProofState) -> Self {
        let shift = axis_shift(axis);
        let cleared = self.0 & !(STATE_MASK << shift);
        Self(cleared | (encode_state(state) << shift))
    }

    /// Pointwise conservative composition.
    pub const fn meet(self, other: Self) -> Self {
        Self::new(
            self.state(AdmissionAxis::Safety)
                .meet(other.state(AdmissionAxis::Safety)),
            self.state(AdmissionAxis::Capability)
                .meet(other.state(AdmissionAxis::Capability)),
            self.state(AdmissionAxis::Justification)
                .meet(other.state(AdmissionAxis::Justification)),
        )
    }

    /// Reduce the triad with fail-closed precedence:
    /// disproven → reject, unknown → defer, all proven → execute.
    pub const fn decision(self) -> InvocationDecision {
        let safety = self.state(AdmissionAxis::Safety);
        let capability = self.state(AdmissionAxis::Capability);
        let justification = self.state(AdmissionAxis::Justification);

        if matches!(safety, ProofState::Disproven) {
            InvocationDecision::Reject {
                axis: AdmissionAxis::Safety,
            }
        } else if matches!(capability, ProofState::Disproven) {
            InvocationDecision::Reject {
                axis: AdmissionAxis::Capability,
            }
        } else if matches!(justification, ProofState::Disproven) {
            InvocationDecision::Reject {
                axis: AdmissionAxis::Justification,
            }
        } else if matches!(safety, ProofState::Unknown) {
            InvocationDecision::Defer {
                axis: AdmissionAxis::Safety,
            }
        } else if matches!(capability, ProofState::Unknown) {
            InvocationDecision::Defer {
                axis: AdmissionAxis::Capability,
            }
        } else if matches!(justification, ProofState::Unknown) {
            InvocationDecision::Defer {
                axis: AdmissionAxis::Justification,
            }
        } else {
            InvocationDecision::Execute
        }
    }
}

impl Default for AdmissionTriad {
    fn default() -> Self {
        Self::new(
            ProofState::Unknown,
            ProofState::Unknown,
            ProofState::Unknown,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    const STATES: [ProofState; 3] = [
        ProofState::Disproven,
        ProofState::Unknown,
        ProofState::Proven,
    ];

    #[test]
    fn triad_is_exactly_one_byte() {
        assert_eq!(size_of::<AdmissionTriad>(), 1);
    }

    #[test]
    fn zero_is_fail_closed_unknown() {
        let triad = AdmissionTriad::from_packed(0).unwrap();
        assert_eq!(triad, AdmissionTriad::default());
        assert_eq!(
            triad.decision(),
            InvocationDecision::Defer {
                axis: AdmissionAxis::Safety
            }
        );
    }

    #[test]
    fn all_twenty_seven_states_round_trip() {
        for &safety in &STATES {
            for &capability in &STATES {
                for &justification in &STATES {
                    let triad = AdmissionTriad::new(safety, capability, justification);
                    assert_eq!(AdmissionTriad::from_packed(triad.packed()).unwrap(), triad);
                    assert_eq!(triad.state(AdmissionAxis::Safety), safety);
                    assert_eq!(triad.state(AdmissionAxis::Capability), capability);
                    assert_eq!(triad.state(AdmissionAxis::Justification), justification);
                }
            }
        }
    }

    #[test]
    fn decision_never_executes_with_unknown_or_disproven_axis() {
        for &safety in &STATES {
            for &capability in &STATES {
                for &justification in &STATES {
                    let triad = AdmissionTriad::new(safety, capability, justification);
                    let all_proven = safety == ProofState::Proven
                        && capability == ProofState::Proven
                        && justification == ProofState::Proven;
                    assert_eq!(
                        matches!(triad.decision(), InvocationDecision::Execute),
                        all_proven
                    );
                }
            }
        }
    }

    #[test]
    fn reserved_bits_are_rejected() {
        assert!(matches!(
            AdmissionTriad::from_packed(0b0000_0011),
            Err(AdmissionEncodingError::ReservedState {
                axis: AdmissionAxis::Safety
            })
        ));
        assert!(matches!(
            AdmissionTriad::from_packed(0b1000_0000),
            Err(AdmissionEncodingError::ReservedHighBits { .. })
        ));
    }

    #[test]
    fn meet_is_conservative() {
        let proven =
            AdmissionTriad::new(ProofState::Proven, ProofState::Proven, ProofState::Proven);
        let partial = AdmissionTriad::new(
            ProofState::Proven,
            ProofState::Unknown,
            ProofState::Disproven,
        );
        assert_eq!(proven.meet(partial), partial);
    }
}

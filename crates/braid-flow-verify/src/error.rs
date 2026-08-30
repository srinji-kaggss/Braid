//! Typed admission refusals — one invariant per variant.

use alloc::boxed::Box;
use alloc::string::String;

use crate::disjoint::{DisjointnessUnknown, PredicateCounterexample};

pub type VerifyResult<T> = Result<T, FlowVerifyError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOverlap {
    pub choice: String,
    pub left_arm: usize,
    pub right_arm: usize,
    pub left_target: String,
    pub right_target: String,
    pub counterexample: PredicateCounterexample,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LimitKind {
    SourceNodes,
    SourceEdges,
    ExpandedNodes,
    ExpandedEdges,
    PredicateDepth,
    PredicateNodes,
    ChoiceArms,
    Ports,
    Roots,
    Terminals,
    References,
    CompletionClasses,
    LiteralBytes,
    LiteralNodes,
    LiteralDepth,
    TypeTagNodes,
    CanonicalDepth,
    CanonicalValues,
    WireBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowVerifyError {
    Canon {
        reason: String,
        invariant: &'static str,
    },
    Identifier {
        kind: &'static str,
        length: usize,
        invariant: &'static str,
    },
    Malformed {
        field: &'static str,
        invariant: &'static str,
    },
    UnsupportedVersion {
        found: u16,
        expected: u16,
        invariant: &'static str,
    },
    NonBijective {
        invariant: &'static str,
    },
    LimitExceeded {
        kind: LimitKind,
        actual: usize,
        limit: usize,
        invariant: &'static str,
    },
    InvalidBound {
        kind: LimitKind,
        requested: u32,
        hard_limit: u32,
        invariant: &'static str,
    },
    EmptyCollection {
        field: &'static str,
        invariant: &'static str,
    },
    Duplicate {
        field: &'static str,
        key: String,
        invariant: &'static str,
    },
    Unresolved {
        field: &'static str,
        key: String,
        invariant: &'static str,
    },
    Cycle {
        invariant: &'static str,
    },
    ChoiceNotTotal {
        invariant: &'static str,
    },
    DuplicateChoiceTarget {
        choice: String,
        left_arm: usize,
        right_arm: usize,
        target: String,
        invariant: &'static str,
    },
    ChoiceNotDisjoint {
        overlap: Box<ChoiceOverlap>,
        invariant: &'static str,
    },
    ChoiceDisjointnessUnknown {
        choice: String,
        left_arm: usize,
        right_arm: usize,
        reason: DisjointnessUnknown,
        invariant: &'static str,
    },
    JoinCardinality {
        invariant: &'static str,
    },
    TerminalUnreachable {
        key: String,
        invariant: &'static str,
    },
    TerminalSoundness {
        invariant: &'static str,
    },
    JustificationIncomplete {
        field: &'static str,
        invariant: &'static str,
    },
    AuthorityWidened {
        invariant: &'static str,
    },
    CacheIncompatible {
        invariant: &'static str,
    },
    HiddenInput {
        field: &'static str,
        invariant: &'static str,
    },
    ArithmeticOverflow {
        field: &'static str,
        invariant: &'static str,
    },
}

impl core::fmt::Display for FlowVerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Canon { reason, invariant } => {
                write!(f, "{invariant}: canonical decode failed: {reason}")
            }
            Self::Identifier {
                kind,
                length,
                invariant,
            } => write!(f, "{invariant}: invalid {kind} ({length} bytes)"),
            Self::Malformed { field, invariant } => write!(f, "{invariant}: malformed `{field}`"),
            Self::UnsupportedVersion {
                found,
                expected,
                invariant,
            } => write!(
                f,
                "{invariant}: unsupported version {found} expected {expected}"
            ),
            Self::NonBijective { invariant } => write!(f, "{invariant}: non-bijective wire"),
            Self::LimitExceeded {
                kind,
                actual,
                limit,
                invariant,
            } => write!(f, "{invariant}: {kind:?} exceeded {actual} > {limit}"),
            Self::InvalidBound {
                kind,
                requested,
                hard_limit,
                invariant,
            } => write!(
                f,
                "{invariant}: invalid bound {kind:?} {requested} > {hard_limit}"
            ),
            Self::EmptyCollection { field, invariant } => write!(f, "{invariant}: empty `{field}`"),
            Self::Duplicate {
                field,
                key,
                invariant,
            } => write!(f, "{invariant}: duplicate {field} `{key}`"),
            Self::Unresolved {
                field,
                key,
                invariant,
            } => write!(f, "{invariant}: unresolved {field} `{key}`"),
            Self::Cycle { invariant } => write!(f, "{invariant}: cycle"),
            Self::ChoiceNotTotal { invariant } => write!(f, "{invariant}: choice not total"),
            Self::DuplicateChoiceTarget {
                choice,
                left_arm,
                right_arm,
                target,
                invariant,
            } => write!(
                f,
                "{invariant}: choice `{choice}` arms {left_arm} and {right_arm} duplicate target `{target}`"
            ),
            Self::ChoiceNotDisjoint { overlap, invariant } => write!(
                f,
                "{invariant}: choice `{choice}` arms {left_arm} (`{left_target}`) and {right_arm} (`{right_target}`) overlap ({} value bindings, {} completion bindings)",
                overlap.counterexample.values.len(),
                overlap.counterexample.completions.len(),
                choice = overlap.choice,
                left_arm = overlap.left_arm,
                right_arm = overlap.right_arm,
                left_target = overlap.left_target,
                right_target = overlap.right_target,
            ),
            Self::ChoiceDisjointnessUnknown {
                choice,
                left_arm,
                right_arm,
                reason,
                invariant,
            } => write!(
                f,
                "{invariant}: choice `{choice}` arms {left_arm} and {right_arm} disjointness unknown: {reason:?}"
            ),
            Self::JoinCardinality { invariant } => {
                write!(f, "{invariant}: join cardinality not explicit")
            }
            Self::TerminalUnreachable { key, invariant } => {
                write!(f, "{invariant}: terminal `{key}` unreachable")
            }
            Self::TerminalSoundness { invariant } => {
                write!(f, "{invariant}: terminal soundness violated")
            }
            Self::JustificationIncomplete { field, invariant } => {
                write!(f, "{invariant}: justification incomplete `{field}`")
            }
            Self::AuthorityWidened { invariant } => write!(f, "{invariant}: authority widened"),
            Self::CacheIncompatible { invariant } => write!(f, "{invariant}: cache incompatible"),
            Self::HiddenInput { field, invariant } => {
                write!(f, "{invariant}: hidden input `{field}`")
            }
            Self::ArithmeticOverflow { field, invariant } => {
                write!(f, "{invariant}: arithmetic overflow `{field}`")
            }
        }
    }
}
impl core::error::Error for FlowVerifyError {}

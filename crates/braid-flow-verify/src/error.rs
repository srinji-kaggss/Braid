//! Typed admission refusals — one invariant per variant.

use alloc::string::String;

pub type VerifyResult<T> = Result<T, FlowVerifyError>;

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
    ChoiceNotDisjoint {
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
            Self::ChoiceNotDisjoint { invariant } => write!(f, "{invariant}: choice not disjoint"),
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

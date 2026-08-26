//! Typed construction failures. Untrusted declarations never panic or fall
//! back to a guessed interpretation.

use alloc::string::String;

pub type FlowResult<T> = Result<T, FlowError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct IdentifierError {
    pub kind: &'static str,
    pub length: usize,
}

impl core::fmt::Display for IdentifierError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid {} ({} bytes)", self.kind, self.length)
    }
}

impl core::error::Error for IdentifierError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowError {
    Canon(braid_ir::CanonError),
    Identifier(IdentifierError),
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
    InvalidTypeTag {
        field: &'static str,
        error: braid_ir::TypeTagError,
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
    ArithmeticOverflow {
        field: &'static str,
        invariant: &'static str,
    },
}

impl From<braid_ir::CanonError> for FlowError {
    fn from(value: braid_ir::CanonError) -> Self {
        Self::Canon(value)
    }
}

impl From<IdentifierError> for FlowError {
    fn from(value: IdentifierError) -> Self {
        Self::Identifier(value)
    }
}

impl core::fmt::Display for FlowError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Canon(error) => write!(f, "INV-FLOW-018: canonical decode failed: {error:?}"),
            Self::Identifier(error) => error.fmt(f),
            Self::Malformed { field, invariant } => {
                write!(f, "{invariant}: malformed `{field}`")
            }
            Self::UnsupportedVersion {
                found,
                expected,
                invariant,
            } => write!(
                f,
                "{invariant}: unsupported Flow version {found}; expected {expected}"
            ),
            Self::NonBijective { invariant } => {
                write!(
                    f,
                    "{invariant}: decoded Flow does not re-encode byte-identically"
                )
            }
            Self::LimitExceeded {
                kind,
                actual,
                limit,
                invariant,
            } => write!(
                f,
                "{invariant}: {kind:?} limit exceeded ({actual} > {limit})"
            ),
            Self::InvalidBound {
                kind,
                requested,
                hard_limit,
                invariant,
            } => write!(
                f,
                "{invariant}: {kind:?} bound {requested} exceeds hard limit {hard_limit}"
            ),
            Self::EmptyCollection { field, invariant } => {
                write!(f, "{invariant}: `{field}` must not be empty")
            }
            Self::InvalidTypeTag {
                field,
                error,
                invariant,
            } => write!(f, "{invariant}: invalid `{field}`: {error}"),
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
            Self::Cycle { invariant } => write!(f, "{invariant}: Flow contains a cycle"),
            Self::ArithmeticOverflow { field, invariant } => {
                write!(f, "{invariant}: arithmetic overflow in `{field}`")
            }
        }
    }
}

impl core::error::Error for FlowError {}

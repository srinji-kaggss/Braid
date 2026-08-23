//! Canonical encoding errors and invariants.

/// Maximum nesting depth (fail-closed resource bound, not a tunable).
pub const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonError {
    /// Input ended early / declared length exceeds remaining bytes.
    Truncated { at: &'static str },
    /// A head/feature outside the subset (float, tag, indefinite, null, …).
    ForbiddenForm {
        form: &'static str,
        at: &'static str,
    },
    /// An integer head that is not the minimal-length encoding.
    NonMinimalInt { at: &'static str },
    /// Map keys out of canonical order or duplicated.
    KeyOrder { at: &'static str },
    /// Text bytes are not valid UTF-8.
    Utf8 { at: &'static str },
    /// Bytes remain after the single top-level value.
    TrailingBytes { at: &'static str },
    /// Nesting exceeds [`MAX_DEPTH`].
    DepthExceeded { at: &'static str },
    /// Decoded value re-encoded to different bytes (bijection guard).
    NotBijective { at: &'static str },
    /// Integer outside i64 range.
    IntRange { at: &'static str },
}

impl CanonError {
    pub fn location(&self) -> &'static str {
        match self {
            Self::Truncated { at }
            | Self::ForbiddenForm { at, .. }
            | Self::NonMinimalInt { at }
            | Self::KeyOrder { at }
            | Self::Utf8 { at }
            | Self::TrailingBytes { at }
            | Self::DepthExceeded { at }
            | Self::NotBijective { at }
            | Self::IntRange { at } => at,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Truncated { .. } => "truncated input",
            Self::ForbiddenForm { .. } => "forbidden form",
            Self::NonMinimalInt { .. } => "non-minimal integer",
            Self::KeyOrder { .. } => "key order violation",
            Self::Utf8 { .. } => "invalid utf-8",
            Self::TrailingBytes { .. } => "trailing bytes",
            Self::DepthExceeded { .. } => "depth exceeded",
            Self::NotBijective { .. } => "not bijective",
            Self::IntRange { .. } => "integer out of range",
        }
    }
}

impl core::fmt::Display for CanonError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Self::ForbiddenForm { form, at } = self {
            write!(f, "forbidden form {form} at {at}")
        } else {
            write!(f, "{} at {}", self.label(), self.location())
        }
    }
}

impl core::error::Error for CanonError {}

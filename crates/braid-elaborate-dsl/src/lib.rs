//! Bounded Braid DSL frontend.
//!
//! This crate is deliberately outside the verifier trust base. It parses the
//! versioned Braid DSL v0 subset, lowers through [`braid_sdk::Builder`], emits
//! the existing canonical capsule bytes, and asks the independent
//! `braid-verify` implementation to admit those bytes. It owns no wire format
//! and no admission rule.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use braid_ir::{Capsule, ConfirmPolicy, EffectClass};
use braid_render::{manifest, render_text};
use braid_sdk::{Builder, Strand};
use braid_verify::{Verdict, verify};
use braid_vocab_cms::registry_v0;
use lgwks_std::json::Serialize;

/// Source-language version accepted by this implementation.
pub const DSL_VERSION: u64 = 0;
/// Whole-source ceiling, enforced before tokens or AST nodes are allocated.
pub const MAX_SOURCE_BYTES: usize = 65_536;
/// Maximum token count.
pub const MAX_TOKENS: usize = 4_096;
/// Maximum bindings in one step.
pub const MAX_BINDINGS: usize = 1_024;
/// Maximum calls in one pipeline expression.
pub const MAX_PIPELINE_CALLS: usize = 64;
/// Maximum identifier length in bytes.
pub const MAX_IDENTIFIER_BYTES: usize = 128;
/// Maximum decoded string length in bytes.
pub const MAX_STRING_BYTES: usize = 4_096;

/// Stable machine-readable DSL failure category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    SourceTooLarge,
    TooManyTokens,
    InvalidToken,
    UnexpectedToken,
    MissingField,
    DuplicateField,
    UnsupportedConstruct,
    UnsupportedVersion,
    UnsupportedRegistry,
    LimitExceeded,
    DuplicateBinding,
    UnknownBinding,
    UnknownTerm,
    BuildRejected,
    CapabilityMismatch,
    EffectMismatch,
    VerificationRejected,
    RenderFailed,
    SerializationFailed,
}

impl ErrorCode {
    /// Stable diagnostic identifier suitable for scripts and refusal corpora.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceTooLarge => "BRD001_SOURCE_TOO_LARGE",
            Self::TooManyTokens => "BRD002_TOO_MANY_TOKENS",
            Self::InvalidToken => "BRD003_INVALID_TOKEN",
            Self::UnexpectedToken => "BRD004_UNEXPECTED_TOKEN",
            Self::MissingField => "BRD005_MISSING_FIELD",
            Self::DuplicateField => "BRD006_DUPLICATE_FIELD",
            Self::UnsupportedConstruct => "BRD007_UNSUPPORTED_CONSTRUCT",
            Self::UnsupportedVersion => "BRD008_UNSUPPORTED_VERSION",
            Self::UnsupportedRegistry => "BRD009_UNSUPPORTED_REGISTRY",
            Self::LimitExceeded => "BRD010_LIMIT_EXCEEDED",
            Self::DuplicateBinding => "BRD011_DUPLICATE_BINDING",
            Self::UnknownBinding => "BRD012_UNKNOWN_BINDING",
            Self::UnknownTerm => "BRD013_UNKNOWN_TERM",
            Self::BuildRejected => "BRD014_BUILD_REJECTED",
            Self::CapabilityMismatch => "BRD015_CAPABILITY_MISMATCH",
            Self::EffectMismatch => "BRD016_EFFECT_MISMATCH",
            Self::VerificationRejected => "BRD017_VERIFICATION_REJECTED",
            Self::RenderFailed => "BRD018_RENDER_FAILED",
            Self::SerializationFailed => "BRD019_SERIALIZATION_FAILED",
        }
    }
}

/// Typed source or lowering diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslError {
    /// Stable category.
    pub code: ErrorCode,
    /// UTF-8 byte offset in source, or zero for post-parse failures.
    pub offset: usize,
    /// Human-readable cause and correction.
    pub message: String,
}

impl DslError {
    fn new(code: ErrorCode, offset: usize, message: impl Into<String>) -> Self {
        Self {
            code,
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for DslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte {}: {}",
            self.code.as_str(),
            self.offset,
            self.message
        )
    }
}

impl std::error::Error for DslError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Ident(String),
    String(String),
    Integer(u64),
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
    Comma,
    Semicolon,
    Equals,
    PathSep,
    Pipe,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    offset: usize,
}

fn push_token(tokens: &mut Vec<Token>, kind: TokenKind, offset: usize) -> Result<(), DslError> {
    if tokens.len() >= MAX_TOKENS {
        return Err(DslError::new(
            ErrorCode::TooManyTokens,
            offset,
            format!("source exceeds the {MAX_TOKENS}-token ceiling"),
        ));
    }
    tokens.push(Token { kind, offset });
    Ok(())
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

fn lex_identifier(source: &str, start: usize, end: usize) -> Result<String, DslError> {
    let len = end - start;
    if len > MAX_IDENTIFIER_BYTES {
        return Err(DslError::new(
            ErrorCode::LimitExceeded,
            start,
            format!("identifier is {len} bytes; maximum is {MAX_IDENTIFIER_BYTES}"),
        ));
    }
    Ok(source[start..end].to_owned())
}

fn lex_string(source: &str, start: usize) -> Result<(String, usize), DslError> {
    let bytes = source.as_bytes();
    let mut cursor = start + 1;
    let mut value = String::new();
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => return Ok((value, cursor + 1)),
            b'\\' => {
                let escape_offset = cursor;
                cursor += 1;
                let escaped = *bytes.get(cursor).ok_or_else(|| {
                    DslError::new(
                        ErrorCode::InvalidToken,
                        escape_offset,
                        "unterminated string escape; close the quoted string",
                    )
                })?;
                let decoded = match escaped {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    _ => {
                        return Err(DslError::new(
                            ErrorCode::InvalidToken,
                            escape_offset,
                            r#"unsupported escape; use \" \\ \n \r or \t"#,
                        ));
                    }
                };
                value.push(decoded);
                cursor += 1;
            }
            byte if byte.is_ascii_control() => {
                return Err(DslError::new(
                    ErrorCode::InvalidToken,
                    cursor,
                    "raw control characters are forbidden in strings; use an explicit escape",
                ));
            }
            _ => {
                let ch = source[cursor..].chars().next().ok_or_else(|| {
                    DslError::new(ErrorCode::InvalidToken, cursor, "invalid UTF-8 boundary")
                })?;
                value.push(ch);
                cursor += ch.len_utf8();
            }
        }
        if value.len() > MAX_STRING_BYTES {
            return Err(DslError::new(
                ErrorCode::LimitExceeded,
                start,
                format!("string exceeds the {MAX_STRING_BYTES}-byte ceiling"),
            ));
        }
    }
    Err(DslError::new(
        ErrorCode::InvalidToken,
        start,
        "unterminated string; add a closing quote",
    ))
}

// A single exhaustive byte dispatch keeps the accepted character surface
// reviewable beside the grammar; splitting it would hide that security boundary.
#[allow(clippy::too_many_lines)]
fn lex(source: &str) -> Result<Vec<Token>, DslError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(DslError::new(
            ErrorCode::SourceTooLarge,
            0,
            format!(
                "source is {} bytes; maximum is {MAX_SOURCE_BYTES}",
                source.len()
            ),
        ));
    }

    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let start = cursor;
        match bytes[cursor] {
            byte if byte.is_ascii_whitespace() => cursor += 1,
            b'/' if bytes.get(cursor + 1) == Some(&b'/') => {
                cursor += 2;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            byte if is_ident_start(byte) => {
                cursor += 1;
                while cursor < bytes.len() && is_ident_continue(bytes[cursor]) {
                    cursor += 1;
                }
                let ident = lex_identifier(source, start, cursor)?;
                push_token(&mut tokens, TokenKind::Ident(ident), start)?;
            }
            byte if byte.is_ascii_digit() => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                    cursor += 1;
                }
                let value = source[start..cursor].parse::<u64>().map_err(|_| {
                    DslError::new(
                        ErrorCode::LimitExceeded,
                        start,
                        "integer exceeds the unsigned 64-bit source limit",
                    )
                })?;
                push_token(&mut tokens, TokenKind::Integer(value), start)?;
            }
            b'"' => {
                let (value, end) = lex_string(source, start)?;
                cursor = end;
                push_token(&mut tokens, TokenKind::String(value), start)?;
            }
            b'{' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::LeftBrace, start)?;
            }
            b'}' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::RightBrace, start)?;
            }
            b'[' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::LeftBracket, start)?;
            }
            b']' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::RightBracket, start)?;
            }
            b'(' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::LeftParen, start)?;
            }
            b')' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::RightParen, start)?;
            }
            b',' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::Comma, start)?;
            }
            b';' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::Semicolon, start)?;
            }
            b'=' => {
                cursor += 1;
                push_token(&mut tokens, TokenKind::Equals, start)?;
            }
            b':' if bytes.get(cursor + 1) == Some(&b':') => {
                cursor += 2;
                push_token(&mut tokens, TokenKind::PathSep, start)?;
            }
            b'|' if bytes.get(cursor + 1) == Some(&b'>') => {
                cursor += 2;
                push_token(&mut tokens, TokenKind::Pipe, start)?;
            }
            other => {
                return Err(DslError::new(
                    ErrorCode::InvalidToken,
                    start,
                    format!(
                        "unsupported byte `{}`; v0 accepts only the closed grammar",
                        char::from(other)
                    ),
                ));
            }
        }
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        offset: source.len(),
    });
    Ok(tokens)
}

/// Parsed Braid DSL document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    name: String,
    version: u64,
    intent: String,
    registry: String,
    capabilities: Vec<String>,
    effects: Vec<String>,
    budget: Option<u64>,
    confirm: ConfirmPolicy,
    evidence: Vec<String>,
    step: Step,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    name: String,
    bindings: Vec<Binding>,
    outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Binding {
    name: String,
    expression: Expression,
    offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Expression {
    seed: Option<String>,
    calls: Vec<Call>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    path: String,
    args: Vec<String>,
    offset: usize,
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn advance(&mut self) -> Token {
        let token = self.current().clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.cursor += 1;
        }
        token
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(value) if value == keyword)
    }

    fn next_is_pipe(&self) -> bool {
        matches!(self.current().kind, TokenKind::Ident(_))
            && matches!(
                self.tokens.get(self.cursor + 1).map(|token| &token.kind),
                Some(TokenKind::Pipe)
            )
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<usize, DslError> {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(value) if value == keyword => Ok(token.offset),
            other => Err(DslError::new(
                ErrorCode::UnexpectedToken,
                token.offset,
                format!("expected keyword `{keyword}`, found {other:?}"),
            )),
        }
    }

    // Punctuation tokens are tiny. Passing the expected value makes each call
    // site read like the grammar and avoids reference noise.
    #[allow(clippy::needless_pass_by_value)]
    fn expect_simple(&mut self, expected: TokenKind, label: &str) -> Result<usize, DslError> {
        let token = self.advance();
        if token.kind == expected {
            Ok(token.offset)
        } else {
            Err(DslError::new(
                ErrorCode::UnexpectedToken,
                token.offset,
                format!("expected {label}, found {:?}", token.kind),
            ))
        }
    }

    fn parse_ident(&mut self) -> Result<(String, usize), DslError> {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(value) => Ok((value, token.offset)),
            other => Err(DslError::new(
                ErrorCode::UnexpectedToken,
                token.offset,
                format!("expected identifier, found {other:?}"),
            )),
        }
    }

    fn parse_integer(&mut self) -> Result<u64, DslError> {
        let token = self.advance();
        match token.kind {
            TokenKind::Integer(value) => Ok(value),
            other => Err(DslError::new(
                ErrorCode::UnexpectedToken,
                token.offset,
                format!("expected integer, found {other:?}"),
            )),
        }
    }

    fn parse_string(&mut self) -> Result<String, DslError> {
        let token = self.advance();
        match token.kind {
            TokenKind::String(value) => Ok(value),
            other => Err(DslError::new(
                ErrorCode::UnexpectedToken,
                token.offset,
                format!("expected quoted string, found {other:?}"),
            )),
        }
    }

    fn parse_path(&mut self) -> Result<(String, usize), DslError> {
        let (first, offset) = self.parse_ident()?;
        self.expect_simple(TokenKind::PathSep, "`::` in a namespaced path")?;
        let (second, _) = self.parse_ident()?;
        let mut path = format!("{first}::{second}");
        while matches!(self.current().kind, TokenKind::PathSep) {
            self.advance();
            let (part, _) = self.parse_ident()?;
            path.push_str("::");
            path.push_str(&part);
        }
        Ok((path, offset))
    }

    fn parse_path_list(&mut self) -> Result<Vec<String>, DslError> {
        self.expect_simple(TokenKind::LeftBracket, "`[`")?;
        let mut values = Vec::new();
        if !matches!(self.current().kind, TokenKind::RightBracket) {
            loop {
                values.push(self.parse_path()?.0);
                if matches!(self.current().kind, TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_simple(TokenKind::RightBracket, "`]`")?;
        Ok(values)
    }

    fn parse_ident_list(&mut self) -> Result<Vec<String>, DslError> {
        self.expect_simple(TokenKind::LeftBracket, "`[`")?;
        let mut values = Vec::new();
        if !matches!(self.current().kind, TokenKind::RightBracket) {
            loop {
                values.push(self.parse_ident()?.0);
                if matches!(self.current().kind, TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_simple(TokenKind::RightBracket, "`]`")?;
        Ok(values)
    }

    fn parse_string_list(&mut self) -> Result<Vec<String>, DslError> {
        self.expect_simple(TokenKind::LeftBracket, "`[`")?;
        let mut values = Vec::new();
        if !matches!(self.current().kind, TokenKind::RightBracket) {
            loop {
                values.push(self.parse_string()?);
                if matches!(self.current().kind, TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_simple(TokenKind::RightBracket, "`]`")?;
        Ok(values)
    }

    fn parse_require(&mut self) -> Result<(Vec<String>, Vec<String>), DslError> {
        self.expect_keyword("require")?;
        self.expect_simple(TokenKind::LeftBrace, "`{`")?;
        let mut capabilities = None;
        let mut effects = None;
        while !matches!(self.current().kind, TokenKind::RightBrace) {
            if self.peek_keyword("capabilities") {
                let offset = self.expect_keyword("capabilities")?;
                if capabilities.is_some() {
                    return Err(DslError::new(
                        ErrorCode::DuplicateField,
                        offset,
                        "`capabilities` appears more than once",
                    ));
                }
                capabilities = Some(self.parse_path_list()?);
            } else if self.peek_keyword("effects") {
                let offset = self.expect_keyword("effects")?;
                if effects.is_some() {
                    return Err(DslError::new(
                        ErrorCode::DuplicateField,
                        offset,
                        "`effects` appears more than once",
                    ));
                }
                effects = Some(self.parse_ident_list()?);
            } else {
                return Err(DslError::new(
                    ErrorCode::UnexpectedToken,
                    self.current().offset,
                    "require block accepts only `capabilities` and `effects`",
                ));
            }
            self.expect_simple(TokenKind::Semicolon, "`;`")?;
        }
        self.expect_simple(TokenKind::RightBrace, "`}`")?;
        Ok((
            capabilities.ok_or_else(|| {
                DslError::new(
                    ErrorCode::MissingField,
                    0,
                    "require block needs `capabilities [...]`",
                )
            })?,
            effects.ok_or_else(|| {
                DslError::new(
                    ErrorCode::MissingField,
                    0,
                    "require block needs `effects [...]`",
                )
            })?,
        ))
    }

    fn parse_call(&mut self) -> Result<Call, DslError> {
        let (path, offset) = self.parse_path()?;
        self.expect_simple(TokenKind::LeftParen, "`(`")?;
        let mut args = Vec::new();
        if !matches!(self.current().kind, TokenKind::RightParen) {
            loop {
                args.push(self.parse_ident()?.0);
                if matches!(self.current().kind, TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect_simple(TokenKind::RightParen, "`)`")?;
        Ok(Call { path, args, offset })
    }

    fn parse_expression(&mut self) -> Result<Expression, DslError> {
        let seed = if self.next_is_pipe() {
            let (name, _) = self.parse_ident()?;
            self.expect_simple(TokenKind::Pipe, "`|>`")?;
            Some(name)
        } else {
            None
        };
        let mut calls = vec![self.parse_call()?];
        while matches!(self.current().kind, TokenKind::Pipe) {
            if calls.len() >= MAX_PIPELINE_CALLS {
                return Err(DslError::new(
                    ErrorCode::LimitExceeded,
                    self.current().offset,
                    format!("pipeline exceeds {MAX_PIPELINE_CALLS} calls"),
                ));
            }
            self.advance();
            calls.push(self.parse_call()?);
        }
        Ok(Expression { seed, calls })
    }

    fn parse_step(&mut self) -> Result<Step, DslError> {
        self.expect_keyword("step")?;
        let (name, _) = self.parse_ident()?;
        self.expect_simple(TokenKind::LeftBrace, "`{`")?;
        let mut bindings = Vec::new();
        let mut outputs = None;
        while !matches!(self.current().kind, TokenKind::RightBrace) {
            if self.peek_keyword("output") {
                self.expect_keyword("output")?;
                outputs = Some(self.parse_ident_list()?);
                self.expect_simple(TokenKind::Semicolon, "`;`")?;
                if !matches!(self.current().kind, TokenKind::RightBrace) {
                    return Err(DslError::new(
                        ErrorCode::UnexpectedToken,
                        self.current().offset,
                        "`output` must be the final statement in a step",
                    ));
                }
                continue;
            }
            if bindings.len() >= MAX_BINDINGS {
                return Err(DslError::new(
                    ErrorCode::LimitExceeded,
                    self.current().offset,
                    format!("step exceeds {MAX_BINDINGS} bindings"),
                ));
            }
            let (binding_name, offset) = self.parse_ident()?;
            self.expect_simple(TokenKind::Equals, "`=`")?;
            let expression = self.parse_expression()?;
            self.expect_simple(TokenKind::Semicolon, "`;`")?;
            bindings.push(Binding {
                name: binding_name,
                expression,
                offset,
            });
        }
        self.expect_simple(TokenKind::RightBrace, "`}`")?;
        Ok(Step {
            name,
            bindings,
            outputs: outputs.ok_or_else(|| {
                DslError::new(
                    ErrorCode::MissingField,
                    0,
                    "step needs a final `output [...]` declaration",
                )
            })?,
        })
    }

    fn duplicate_header(offset: usize, name: &str) -> DslError {
        DslError::new(
            ErrorCode::DuplicateField,
            offset,
            format!("header `{name}` appears more than once"),
        )
    }

    // Keeping the closed top-level dispatch together makes unsupported
    // constructs and duplicate-header behavior auditable as one table.
    #[allow(clippy::too_many_lines)]
    fn parse_document(&mut self) -> Result<Document, DslError> {
        self.expect_keyword("capsule")?;
        let (name, _) = self.parse_path()?;
        self.expect_keyword("version")?;
        let version = self.parse_integer()?;
        self.expect_simple(TokenKind::LeftBrace, "`{`")?;

        let mut intent = None;
        let mut registry = None;
        let mut requirements = None;
        let mut budget = None;
        let mut confirm = None;
        let mut evidence = None;
        let mut step = None;

        while !matches!(self.current().kind, TokenKind::RightBrace) {
            if self.peek_keyword("intent") {
                let offset = self.expect_keyword("intent")?;
                if intent.is_some() {
                    return Err(Self::duplicate_header(offset, "intent"));
                }
                intent = Some(self.parse_string()?);
                self.expect_simple(TokenKind::Semicolon, "`;`")?;
            } else if self.peek_keyword("registry") {
                let offset = self.expect_keyword("registry")?;
                if registry.is_some() {
                    return Err(Self::duplicate_header(offset, "registry"));
                }
                registry = Some(self.parse_path()?.0);
                self.expect_simple(TokenKind::Semicolon, "`;`")?;
            } else if self.peek_keyword("require") {
                let offset = self.current().offset;
                if requirements.is_some() {
                    return Err(Self::duplicate_header(offset, "require"));
                }
                requirements = Some(self.parse_require()?);
            } else if self.peek_keyword("budget") {
                let offset = self.expect_keyword("budget")?;
                if budget.is_some() {
                    return Err(Self::duplicate_header(offset, "budget"));
                }
                budget = Some(self.parse_integer()?);
                self.expect_simple(TokenKind::Semicolon, "`;`")?;
            } else if self.peek_keyword("confirm") {
                let offset = self.expect_keyword("confirm")?;
                if confirm.is_some() {
                    return Err(Self::duplicate_header(offset, "confirm"));
                }
                let (value, value_offset) = self.parse_ident()?;
                confirm = Some(match value.as_str() {
                    "none" => ConfirmPolicy::None,
                    "human" => ConfirmPolicy::HumanConfirm,
                    _ => {
                        return Err(DslError::new(
                            ErrorCode::UnexpectedToken,
                            value_offset,
                            "confirm must be `none` or `human`",
                        ));
                    }
                });
                self.expect_simple(TokenKind::Semicolon, "`;`")?;
            } else if self.peek_keyword("evidence") {
                let offset = self.expect_keyword("evidence")?;
                if evidence.is_some() {
                    return Err(Self::duplicate_header(offset, "evidence"));
                }
                evidence = Some(self.parse_string_list()?);
                self.expect_simple(TokenKind::Semicolon, "`;`")?;
            } else if self.peek_keyword("step") {
                let offset = self.current().offset;
                if step.is_some() {
                    return Err(Self::duplicate_header(offset, "step"));
                }
                step = Some(self.parse_step()?);
            } else if matches!(
                &self.current().kind,
                TokenKind::Ident(value)
                    if matches!(
                        value.as_str(),
                        "schema" | "state" | "statechart" | "orchestration" | "import" | "macro"
                    )
            ) {
                return Err(DslError::new(
                    ErrorCode::UnsupportedConstruct,
                    self.current().offset,
                    "construct is reserved but not representable by the v0 Capsule contract",
                ));
            } else {
                return Err(DslError::new(
                    ErrorCode::UnexpectedToken,
                    self.current().offset,
                    "expected intent, registry, require, budget, confirm, evidence, or step",
                ));
            }
        }
        self.expect_simple(TokenKind::RightBrace, "`}`")?;
        self.expect_simple(TokenKind::Eof, "end of source")?;

        let (capabilities, effects) = requirements.ok_or_else(|| {
            DslError::new(ErrorCode::MissingField, 0, "capsule needs a require block")
        })?;
        Ok(Document {
            name,
            version,
            intent: intent.ok_or_else(|| {
                DslError::new(ErrorCode::MissingField, 0, "capsule needs `intent`")
            })?,
            registry: registry.ok_or_else(|| {
                DslError::new(
                    ErrorCode::MissingField,
                    0,
                    "capsule needs `registry cms::v1`",
                )
            })?,
            capabilities,
            effects,
            budget,
            confirm: confirm.unwrap_or(ConfirmPolicy::None),
            evidence: evidence.unwrap_or_default(),
            step: step.ok_or_else(|| {
                DslError::new(ErrorCode::MissingField, 0, "capsule needs exactly one step")
            })?,
        })
    }
}

/// Parse Braid DSL source without lowering it.
///
/// # Errors
///
/// Returns a typed [`DslError`] when source violates the grammar or a
/// pre-allocation bound.
pub fn parse(source: &str) -> Result<Document, DslError> {
    Parser::new(lex(source)?).parse_document()
}

fn dotted(path: &str) -> String {
    path.replace("::", ".")
}

fn lookup_binding(
    bindings: &BTreeMap<String, Strand>,
    name: &str,
    offset: usize,
) -> Result<Strand, DslError> {
    bindings.get(name).copied().ok_or_else(|| {
        DslError::new(
            ErrorCode::UnknownBinding,
            offset,
            format!("binding `{name}` is not defined before this use"),
        )
    })
}

fn effect_name(effect: EffectClass) -> &'static str {
    match effect {
        EffectClass::Pure => "pure",
        EffectClass::Read => "read",
        EffectClass::ReversibleWrite => "reversible_write",
        EffectClass::Irreversible => "irreversible",
        EffectClass::Egress => "egress",
    }
}

fn canonical_set(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    values.into_iter().collect()
}

fn reject_duplicates(values: &[String], label: &str) -> Result<(), DslError> {
    let unique = canonical_set(values.iter().cloned());
    if unique.len() == values.len() {
        Ok(())
    } else {
        Err(DslError::new(
            ErrorCode::DuplicateField,
            0,
            format!("{label} contains a duplicate entry"),
        ))
    }
}

fn compare_declared_set(
    declared: &BTreeSet<String>,
    derived: &BTreeSet<String>,
    code: ErrorCode,
    label: &str,
) -> Result<(), DslError> {
    if declared == derived {
        return Ok(());
    }
    let missing: Vec<_> = derived.difference(declared).cloned().collect();
    let extra: Vec<_> = declared.difference(derived).cloned().collect();
    Err(DslError::new(
        code,
        0,
        format!(
            "declared {label} do not exactly match derived {label}; missing={missing:?}, extra={extra:?}"
        ),
    ))
}

// This is intentionally one visible lowering pipeline: validate source
// declarations, build the graph, and compare derived authority before return.
#[allow(clippy::too_many_lines)]
fn lower_document(document: &Document) -> Result<Capsule, DslError> {
    if document.version != DSL_VERSION {
        return Err(DslError::new(
            ErrorCode::UnsupportedVersion,
            0,
            format!(
                "source version {} is unsupported; use version {DSL_VERSION}",
                document.version
            ),
        ));
    }
    if document.registry != "cms::v1" {
        return Err(DslError::new(
            ErrorCode::UnsupportedRegistry,
            0,
            format!(
                "registry `{}` is unsupported; v0 requires `cms::v1`",
                document.registry
            ),
        ));
    }
    if document.step.name != "main" {
        return Err(DslError::new(
            ErrorCode::UnsupportedConstruct,
            0,
            "v0 requires the single step to be named `main`",
        ));
    }

    reject_duplicates(&document.capabilities, "require.capabilities")?;
    reject_duplicates(&document.effects, "require.effects")?;
    reject_duplicates(&document.step.outputs, "step.output")?;

    let registry = registry_v0();
    let mut builder = Builder::new(&registry, document.intent.clone());
    let mut bindings = BTreeMap::new();

    for binding in &document.step.bindings {
        if bindings.contains_key(&binding.name) {
            return Err(DslError::new(
                ErrorCode::DuplicateBinding,
                binding.offset,
                format!("binding `{}` is declared more than once", binding.name),
            ));
        }
        let mut previous = binding
            .expression
            .seed
            .as_deref()
            .map(|name| lookup_binding(&bindings, name, binding.offset))
            .transpose()?;
        for call in &binding.expression.calls {
            let term = dotted(&call.path);
            if registry.get(&term).is_none() {
                return Err(DslError::new(
                    ErrorCode::UnknownTerm,
                    call.offset,
                    format!("term `{}` is not in registry cms::v1", call.path),
                ));
            }
            let mut inputs = Vec::with_capacity(call.args.len() + usize::from(previous.is_some()));
            if let Some(handle) = previous {
                inputs.push(handle);
            }
            for arg in &call.args {
                inputs.push(lookup_binding(&bindings, arg, call.offset)?);
            }
            previous = Some(builder.strand(&term, &inputs).map_err(|error| {
                DslError::new(
                    ErrorCode::BuildRejected,
                    call.offset,
                    format!("term `{}` failed typed lowering: {error}", call.path),
                )
            })?);
        }
        let result = previous.ok_or_else(|| {
            DslError::new(
                ErrorCode::MissingField,
                binding.offset,
                "binding expression contains no term call",
            )
        })?;
        bindings.insert(binding.name.clone(), result);
    }

    for output in &document.step.outputs {
        builder.output(lookup_binding(&bindings, output, 0)?);
    }
    if let Some(budget) = document.budget {
        builder.budget(budget);
    } else {
        builder.budget_tight();
    }
    builder.confirm(document.confirm);
    for evidence in &document.evidence {
        builder.evidence(evidence.clone());
    }
    let capsule = builder.build().map_err(|error| {
        DslError::new(
            ErrorCode::BuildRejected,
            0,
            format!("capsule failed typed lowering: {error}"),
        )
    })?;

    let declared_caps = canonical_set(document.capabilities.iter().map(|value| dotted(value)));
    let derived_caps = canonical_set(
        capsule
            .grants
            .iter()
            .map(|capability| capability.as_str().to_owned()),
    );
    compare_declared_set(
        &declared_caps,
        &derived_caps,
        ErrorCode::CapabilityMismatch,
        "capabilities",
    )?;

    let declared_effects = canonical_set(document.effects.iter().cloned());
    let derived_effects = canonical_set(capsule.braid.strands.iter().map(|strand| {
        let spec = registry
            .get(&strand.term)
            .expect("builder already resolved every term");
        effect_name(spec.effect).to_owned()
    }));
    compare_declared_set(
        &declared_effects,
        &derived_effects,
        ErrorCode::EffectMismatch,
        "effects",
    )?;

    Ok(capsule)
}

#[derive(Serialize)]
#[serde(crate = "lgwks_std::json::serde")]
struct JsonStrand<'a> {
    term: &'a str,
    inputs: &'a [u32],
}

#[derive(Serialize)]
#[serde(crate = "lgwks_std::json::serde")]
struct JsonCapsule<'a> {
    intent: &'a str,
    budget: u64,
    confirm: &'static str,
    evidence: &'a [String],
    strands: Vec<JsonStrand<'a>>,
    outputs: &'a [u32],
}

fn json_transport(capsule: &Capsule) -> Result<String, DslError> {
    let value = JsonCapsule {
        intent: &capsule.intent,
        budget: capsule.budget,
        confirm: match capsule.confirm {
            ConfirmPolicy::None => "none",
            ConfirmPolicy::HumanConfirm => "human-confirm",
        },
        evidence: &capsule.evidence,
        strands: capsule
            .braid
            .strands
            .iter()
            .map(|strand| JsonStrand {
                term: &strand.term,
                inputs: &strand.inputs,
            })
            .collect(),
        outputs: &capsule.braid.outputs,
    };
    lgwks_std::json::to_string_pretty(&value).map_err(|error| {
        DslError::new(
            ErrorCode::SerializationFailed,
            0,
            format!("failed to serialize JSON-of-IR transport: {error}"),
        )
    })
}

/// Fully checked result of source elaboration.
#[derive(Debug, Clone)]
pub struct Elaboration {
    /// Parsed source name, used for receipts only and not as a second identity.
    pub source_name: String,
    /// Canonical admitted capsule.
    pub capsule: Capsule,
    /// Canonical wire bytes used for independent admission and CID identity.
    pub bytes: Vec<u8>,
    /// JSON-of-IR transport that must reproduce `bytes` through `braid encode`.
    pub json_ir: String,
    /// Deterministic human-review manifest.
    pub manifest_text: String,
}

/// Parse, lower, canonically encode, independently admit, and render source.
///
/// # Errors
///
/// Returns a typed [`DslError`] for syntax, bounds, lowering, declaration
/// mismatch, serialization, independent admission, or rendering failure.
pub fn elaborate(source: &str) -> Result<Elaboration, DslError> {
    let document = parse(source)?;
    let capsule = lower_document(&document)?;
    let bytes = capsule.to_bytes();
    let registry = registry_v0();
    match verify(&bytes, &registry, &capsule.grants) {
        Verdict::Admit { capsule_cid } if capsule_cid == capsule.cid() => {}
        Verdict::Admit { capsule_cid } => {
            return Err(DslError::new(
                ErrorCode::VerificationRejected,
                0,
                format!(
                    "independent verifier returned mismatched CID {}",
                    capsule_cid.to_hex()
                ),
            ));
        }
        Verdict::Reject { stage, reason } => {
            return Err(DslError::new(
                ErrorCode::VerificationRejected,
                0,
                format!("independent verifier rejected at {stage:?}: {reason}"),
            ));
        }
    }
    let manifest_value = manifest(&capsule, &registry).map_err(|error| {
        DslError::new(
            ErrorCode::RenderFailed,
            0,
            format!("manifest rendering failed: {error:?}"),
        )
    })?;
    Ok(Elaboration {
        source_name: document.name,
        json_ir: json_transport(&capsule)?,
        manifest_text: render_text(&manifest_value),
        capsule,
        bytes,
    })
}

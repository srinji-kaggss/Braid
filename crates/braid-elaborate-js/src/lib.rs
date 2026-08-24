//! # braid-elaborate-js — JavaScript statement and expression frontend (WS-2, U11–U13, D31)
//!
//! The **first real frontend over the global IR.** `braid-vocab-js` is the
//! elaboration target; this crate compiles JS *text* (statements, let-bindings,
//! identifier resolution, and expressions) into an admitted [`braid_ir::Capsule`]
//! via the **one** `braid-verify`. It operationalizes D31's "renders JS useless" —
//! JS becomes an authoring frontend over the verified substrate, not a runtime
//! authority. Zero AI in the path.
//!
//! ## Scope
//! - **Literals**: strings, integers, booleans (`true`/`false`). Floats are rejected (D8).
//! - **Statements**: `let <ident> = <expr>;`, `const <ident> = <expr>;`, `return <expr>;`.
//! - **Scoping & Identifiers**: immutable bindings, duplicate detection, unresolved identifier checks.
//! - **Operators**: arithmetic `+` `-` `*`, comparison `<` `==`, logic `&&` `||`, prefix `!`, parentheses.
//! - **Pure Function Calls**: `add(a, b)`, `concat(a, b)`, `mul(a, b)`, `sub(a, b)`.
//! - **Fail-Closed Guards**: bans loops (`while`, `for`), mutation (`x = 2`), eval (`eval(...)`),
//!   DOM access (`document`, `window`), globals (`process`, `globalThis`), and implicit coercions.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::BTreeMap;

use braid_ir::Capsule;
use braid_render::{manifest, render_text};
use braid_sdk::{Builder, Strand};
use braid_verify::{Verdict, verify};
use braid_vocab_js::registry_v0;

pub mod cli;

/// Maximum source length accepted by the frontend.
///
/// This is a whole-pipeline work ceiling: lexing, parsing, and emission are
/// linear in the token stream, so refusing oversized input before the lexer
/// bounds every later stage as well.
pub const MAX_SOURCE_CHARS: usize = 8192;

/// Maximum recursion depth allowed during parsing and elaboration to prevent stack exhaustion.
pub const MAX_DEPTH: usize = 128;

// ─────────────────────────────────────────────────────────────────────────────
// Value types + errors
// ─────────────────────────────────────────────────────────────────────────────

/// The value types this frontend's type-directed elaboration distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
    /// String type (`js.string`).
    Str,
    /// Integer number type (`js.number`).
    Num,
    /// Boolean type (`js.boolean`).
    Bool,
}

impl core::fmt::Display for ValType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            ValType::Str => "string",
            ValType::Num => "number",
            ValType::Bool => "boolean",
        })
    }
}

/// Every way the frontend fails closed. None of these produce a capsule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElabError {
    /// No tokens — empty or whitespace-only source.
    Empty,
    /// Source exceeded the frontend's bounded-work ceiling.
    SourceTooLong {
        /// Number of Unicode scalar values supplied.
        len: usize,
        /// The [`MAX_SOURCE_CHARS`] limit.
        limit: usize,
    },
    /// A character or token the lexer cannot form.
    Lex(String),
    /// A token stream the grammar does not accept.
    Parse(String),
    /// An operator applied to operand type(s) it has no typed term for (no
    /// implicit coercion). `operands` is the operand type list in source order.
    TypeError {
        /// The operator symbol.
        op: String,
        /// The observed operand types.
        operands: Vec<ValType>,
    },
    /// An identifier was referenced without prior declaration in scope.
    UnresolvedIdentifier(String),
    /// An identifier was declared multiple times in the same scope.
    DuplicateBinding(String),
    /// A banned keyword, global, or destructive function was encountered.
    BannedIdentifier(String),
    /// The SDK refused to author the capsule (carries the `BuildError` debug).
    Build(String),
    /// Manifest rendering failed (carries the `RenderError` debug).
    Render(String),
}

impl ElabError {
    fn build(e: braid_sdk::BuildError) -> Self {
        ElabError::Build(format!("{e:?}"))
    }
}

impl core::fmt::Display for ElabError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ElabError::Empty => f.write_str("empty source: nothing to elaborate"),
            ElabError::SourceTooLong { len, limit } => {
                write!(f, "source has {len} characters; maximum is {limit}")
            }
            ElabError::Lex(m) => write!(f, "lex error: {m}"),
            ElabError::Parse(m) => write!(f, "parse error: {m}"),
            ElabError::TypeError { op, operands } => {
                let types: Vec<String> = operands.iter().map(|t| t.to_string()).collect();
                write!(
                    f,
                    "type error: operator `{op}` has no typed term for operand \
                     type(s) [{}] (no implicit coercion)",
                    types.join(", ")
                )
            }
            ElabError::UnresolvedIdentifier(name) => {
                write!(
                    f,
                    "unresolved identifier `{name}` (referenced before declaration)"
                )
            }
            ElabError::DuplicateBinding(name) => {
                write!(
                    f,
                    "duplicate binding `{name}` (variable already declared in scope)"
                )
            }
            ElabError::BannedIdentifier(name) => {
                write!(
                    f,
                    "banned identifier/keyword `{name}` is forbidden in pure Braid dataflow"
                )
            }
            ElabError::Build(m) => write!(f, "build error: {m}"),
            ElabError::Render(m) => write!(f, "render error: {m}"),
        }
    }
}

impl std::error::Error for ElabError {}

// ─────────────────────────────────────────────────────────────────────────────
// Lexer
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Str(String),
    Num(i64),
    Bool(bool),
    Ident(String),
    Let,
    Const,
    Return,
    Plus,
    Minus,
    Star,
    Lt,
    EqEq,
    Assign,
    AndAnd,
    OrOr,
    Bang,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semi,
    Comma,
}

fn is_banned_ident(name: &str) -> bool {
    matches!(
        name,
        "eval"
            | "fetch"
            | "window"
            | "document"
            | "globalThis"
            | "global"
            | "process"
            | "require"
            | "console"
            | "setTimeout"
            | "setInterval"
            | "XMLHttpRequest"
            | "WebSocket"
    )
}

fn is_banned_keyword(name: &str) -> bool {
    matches!(
        name,
        "var"
            | "while"
            | "for"
            | "do"
            | "function"
            | "class"
            | "import"
            | "export"
            | "async"
            | "await"
            | "yield"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "new"
            | "delete"
            | "typeof"
            | "instanceof"
            | "void"
            | "debugger"
            | "switch"
            | "case"
            | "default"
            | "with"
    )
}

fn lex(src: &str) -> Result<Vec<Token>, ElabError> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            ';' => {
                toks.push(Token::Semi);
                i += 1;
            }
            ',' => {
                toks.push(Token::Comma);
                i += 1;
            }
            '{' => {
                toks.push(Token::LBrace);
                i += 1;
            }
            '}' => {
                toks.push(Token::RBrace);
                i += 1;
            }
            '+' => {
                toks.push(Token::Plus);
                i += 1;
            }
            '-' => {
                toks.push(Token::Minus);
                i += 1;
            }
            '*' => {
                toks.push(Token::Star);
                i += 1;
            }
            '<' => {
                toks.push(Token::Lt);
                i += 1;
            }
            '!' => {
                toks.push(Token::Bang);
                i += 1;
            }
            '(' => {
                toks.push(Token::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Token::RParen);
                i += 1;
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    toks.push(Token::EqEq);
                    i += 2;
                } else {
                    toks.push(Token::Assign);
                    i += 1;
                }
            }
            '&' => {
                if chars.get(i + 1) == Some(&'&') {
                    toks.push(Token::AndAnd);
                    i += 2;
                } else {
                    return Err(ElabError::Lex(
                        "single `&` is not supported; use `&&`".to_string(),
                    ));
                }
            }
            '|' => {
                if chars.get(i + 1) == Some(&'|') {
                    toks.push(Token::OrOr);
                    i += 2;
                } else {
                    return Err(ElabError::Lex(
                        "single `|` is not supported; use `||`".to_string(),
                    ));
                }
            }
            '/' => {
                if chars.get(i + 1) == Some(&'/') {
                    // Line comment
                    i += 2;
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                } else if chars.get(i + 1) == Some(&'*') {
                    // Block comment
                    i += 2;
                    let mut closed = false;
                    while i + 1 < chars.len() {
                        if chars[i] == '*' && chars[i + 1] == '/' {
                            i += 2;
                            closed = true;
                            break;
                        }
                        i += 1;
                    }
                    if !closed {
                        return Err(ElabError::Lex("unterminated block comment".into()));
                    }
                } else {
                    return Err(ElabError::Lex(
                        "division `/` is not supported in fixed-point v0".into(),
                    ));
                }
            }
            '"' | '\'' => {
                let quote = c;
                i += 1;
                let mut s = String::new();
                loop {
                    let Some(&ch) = chars.get(i) else {
                        return Err(ElabError::Lex("unterminated string literal".to_string()));
                    };
                    if ch == quote {
                        i += 1;
                        break;
                    }
                    if ch == '\\' {
                        i += 1;
                        let Some(&e) = chars.get(i) else {
                            return Err(ElabError::Lex("trailing backslash in string".to_string()));
                        };
                        s.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            '\\' => '\\',
                            '"' => '"',
                            '\'' => '\'',
                            other => {
                                return Err(ElabError::Lex(format!(
                                    "unsupported escape \\{other}"
                                )));
                            }
                        });
                        i += 1;
                    } else {
                        s.push(ch);
                        i += 1;
                    }
                }
                toks.push(Token::Str(s));
            }
            d if d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(&d) = chars.get(i) {
                    if d == '.' {
                        return Err(ElabError::Lex(
                            "floating point literals are forbidden (D8: fixed point only)".into(),
                        ));
                    }
                    if !d.is_ascii_digit() {
                        break;
                    }
                    n.push(d);
                    i += 1;
                }
                let val = n
                    .parse::<i64>()
                    .map_err(|_| ElabError::Lex(format!("integer literal out of range: {n}")))?;
                toks.push(Token::Num(val));
            }
            a if a.is_ascii_alphabetic() || a == '_' => {
                let mut w = String::new();
                while let Some(&ch) = chars.get(i) {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        w.push(ch);
                        i += 1;
                    } else {
                        break;
                    }
                }
                if is_banned_keyword(&w) {
                    return Err(ElabError::BannedIdentifier(w));
                }
                if is_banned_ident(&w) {
                    return Err(ElabError::BannedIdentifier(w));
                }
                match w.as_str() {
                    "true" => toks.push(Token::Bool(true)),
                    "false" => toks.push(Token::Bool(false)),
                    "let" => toks.push(Token::Let),
                    "const" => toks.push(Token::Const),
                    "return" => toks.push(Token::Return),
                    _ => toks.push(Token::Ident(w)),
                }
            }
            other => return Err(ElabError::Lex(format!("unexpected character {other:?}"))),
        }
    }
    Ok(toks)
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// Boolean negation (`!`).
    Not,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// Addition (`+`).
    Add,
    /// Subtraction (`-`).
    Sub,
    /// Multiplication (`*`).
    Mul,
    /// Less than (`<`).
    Lt,
    /// Equality (`==`).
    Eq,
    /// Logical AND (`&&`).
    And,
    /// Logical OR (`||`).
    Or,
}

impl BinOp {
    /// The source symbol — used in type-error messages.
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Lt => "<",
            BinOp::Eq => "==",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }

    /// Left/right binding power. Left-assoc: `rbp = lbp + 1`.
    fn binding_power(self) -> (u8, u8) {
        match self {
            BinOp::Or => (1, 2),
            BinOp::And => (3, 4),
            BinOp::Eq | BinOp::Lt => (5, 6),
            BinOp::Add | BinOp::Sub => (7, 8),
            BinOp::Mul => (9, 10),
        }
    }
}

/// The expression AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// String literal.
    Str(String),
    /// Integer literal.
    Num(i64),
    /// Boolean literal.
    Bool(bool),
    /// Variable identifier reference.
    Ident(String),
    /// Unary operator expression.
    Unary(UnOp, Box<Expr>),
    /// Binary operator expression.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// Pure function call.
    Call {
        /// Name of the function being called.
        callee: String,
        /// Arguments passed to the function.
        args: Vec<Expr>,
    },
}

/// A statement in a JavaScript program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// Variable declaration (`let x = expr;` or `const x = expr;`).
    Let {
        /// Variable name.
        name: String,
        /// Bound expression.
        expr: Expr,
    },
    /// Expression statement (`expr;`).
    Expr(Expr),
    /// Return statement (`return expr;`).
    Return(Expr),
}

/// A sequence of statements comprising a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// Statements in source order.
    pub stmts: Vec<Stmt>,
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn infix_op(&self) -> Option<BinOp> {
        match self.peek()? {
            Token::OrOr => Some(BinOp::Or),
            Token::AndAnd => Some(BinOp::And),
            Token::EqEq => Some(BinOp::Eq),
            Token::Lt => Some(BinOp::Lt),
            Token::Plus => Some(BinOp::Add),
            Token::Minus => Some(BinOp::Sub),
            Token::Star => Some(BinOp::Mul),
            _ => None,
        }
    }

    fn expr_bp(&mut self, min_bp: u8) -> Result<Expr, ElabError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ElabError::Parse(format!(
                "maximum expression nesting depth ({MAX_DEPTH}) exceeded"
            )));
        }
        let res = self.expr_bp_inner(min_bp);
        self.depth -= 1;
        res
    }

    fn expr_bp_inner(&mut self, min_bp: u8) -> Result<Expr, ElabError> {
        let mut lhs = match self.bump() {
            Some(Token::Num(n)) => Expr::Num(n),
            Some(Token::Str(s)) => Expr::Str(s),
            Some(Token::Bool(b)) => Expr::Bool(b),
            Some(Token::Ident(name)) => {
                if self.peek() == Some(&Token::LParen) {
                    self.bump(); // consume '('
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.expr_bp(0)?);
                            if self.peek() == Some(&Token::Comma) {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    match self.bump() {
                        Some(Token::RParen) => Expr::Call { callee: name, args },
                        other => {
                            return Err(ElabError::Parse(format!(
                                "expected ')' after call args, found {other:?}"
                            )));
                        }
                    }
                } else {
                    Expr::Ident(name)
                }
            }
            Some(Token::Bang) => {
                let operand = self.expr_bp(11)?;
                Expr::Unary(UnOp::Not, Box::new(operand))
            }
            Some(Token::LParen) => {
                let inner = self.expr_bp(0)?;
                match self.bump() {
                    Some(Token::RParen) => inner,
                    other => {
                        return Err(ElabError::Parse(format!("expected ')', found {other:?}")));
                    }
                }
            }
            other => {
                return Err(ElabError::Parse(format!(
                    "expected an expression, found {other:?}"
                )));
            }
        };

        while let Some(op) = self.infix_op() {
            let (lbp, rbp) = op.binding_power();
            if lbp < min_bp {
                break;
            }
            self.bump(); // consume operator
            let rhs = self.expr_bp(rbp)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ElabError> {
        match self.peek() {
            Some(Token::Let) | Some(Token::Const) => {
                self.bump(); // consume let/const
                let name = match self.bump() {
                    Some(Token::Ident(n)) => n,
                    other => {
                        return Err(ElabError::Parse(format!(
                            "expected identifier after declaration, found {other:?}"
                        )));
                    }
                };
                match self.bump() {
                    Some(Token::Assign) => {}
                    other => {
                        return Err(ElabError::Parse(format!(
                            "expected '=' after variable name, found {other:?}"
                        )));
                    }
                }
                let expr = self.expr_bp(0)?;
                if self.peek() == Some(&Token::Semi) {
                    self.bump();
                }
                Ok(Stmt::Let { name, expr })
            }
            Some(Token::Return) => {
                self.bump(); // consume return
                let expr = self.expr_bp(0)?;
                if self.peek() == Some(&Token::Semi) {
                    self.bump();
                }
                Ok(Stmt::Return(expr))
            }
            Some(Token::Ident(_)) if self.toks.get(self.pos + 1) == Some(&Token::Assign) => {
                Err(ElabError::Parse(
                    "variable mutation / reassignment is forbidden; Braid is an immutable DAG"
                        .into(),
                ))
            }
            _ => {
                let expr = self.expr_bp(0)?;
                if self.peek() == Some(&Token::Semi) {
                    self.bump();
                }
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_program(&mut self) -> Result<Program, ElabError> {
        let mut stmts = Vec::new();
        while self.pos < self.toks.len() {
            if self.peek() == Some(&Token::Semi) {
                self.bump();
                continue;
            }
            stmts.push(self.parse_stmt()?);
        }
        if stmts.is_empty() {
            return Err(ElabError::Empty);
        }
        Ok(Program { stmts })
    }
}

/// Lex + parse JS source into an AST [`Program`].
pub fn parse_program(src: &str) -> Result<Program, ElabError> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err(ElabError::Empty);
    }
    let mut p = Parser {
        toks,
        pos: 0,
        depth: 0,
    };
    p.parse_program()
}

/// Lex + parse a single JS expression into an [`Expr`].
pub fn parse(src: &str) -> Result<Expr, ElabError> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err(ElabError::Empty);
    }
    let mut p = Parser {
        toks,
        pos: 0,
        depth: 0,
    };
    let expr = p.expr_bp(0)?;
    if p.pos != p.toks.len() {
        return Err(ElabError::Parse(format!(
            "unexpected trailing token {:?}",
            p.toks.get(p.pos)
        )));
    }
    Ok(expr)
}

// ─────────────────────────────────────────────────────────────────────────────
// Elaboration — Program / Expr → Strands over braid-vocab-js
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_binary(
    op: BinOp,
    lt: ValType,
    rt: ValType,
) -> Result<(&'static str, ValType), ElabError> {
    use BinOp::*;
    use ValType::*;
    let picked = match (op, lt, rt) {
        (Add, Str, Str) => ("js.concat", Str),
        (Add, Num, Num) => ("js.add", Num),
        (Sub, Num, Num) => ("js.sub", Num),
        (Mul, Num, Num) => ("js.mul", Num),
        (Lt, Num, Num) => ("js.lt", Bool),
        (Eq, Num, Num) => ("js.eq.num", Bool),
        (Eq, Str, Str) => ("js.eq.str", Bool),
        (And, Bool, Bool) => ("js.and", Bool),
        (Or, Bool, Bool) => ("js.or", Bool),
        _ => {
            return Err(ElabError::TypeError {
                op: op.symbol().to_string(),
                operands: vec![lt, rt],
            });
        }
    };
    Ok(picked)
}

fn emit_expr(
    b: &mut Builder,
    e: &Expr,
    scope: &BTreeMap<String, (Strand, ValType)>,
    depth: usize,
) -> Result<(Strand, ValType), ElabError> {
    if depth > MAX_DEPTH {
        return Err(ElabError::Build(format!(
            "expression tree exceeds maximum elaboration depth ({MAX_DEPTH})"
        )));
    }

    match e {
        Expr::Str(_) => Ok((
            b.strand("js.lit.string", &[]).map_err(ElabError::build)?,
            ValType::Str,
        )),
        Expr::Num(_) => Ok((
            b.strand("js.lit.number", &[]).map_err(ElabError::build)?,
            ValType::Num,
        )),
        Expr::Bool(_) => Ok((
            b.strand("js.lit.boolean", &[]).map_err(ElabError::build)?,
            ValType::Bool,
        )),
        Expr::Ident(name) => {
            if let Some(&(strand, ty)) = scope.get(name) {
                Ok((strand, ty))
            } else {
                Err(ElabError::UnresolvedIdentifier(name.clone()))
            }
        }
        Expr::Unary(UnOp::Not, x) => {
            let (xh, xt) = emit_expr(b, x, scope, depth + 1)?;
            if xt != ValType::Bool {
                return Err(ElabError::TypeError {
                    op: "!".to_string(),
                    operands: vec![xt],
                });
            }
            Ok((
                b.strand("js.not", &[xh]).map_err(ElabError::build)?,
                ValType::Bool,
            ))
        }
        Expr::Binary(op, l, r) => {
            let (lh, lt) = emit_expr(b, l, scope, depth + 1)?;
            let (rh, rt) = emit_expr(b, r, scope, depth + 1)?;
            let (term, out) = resolve_binary(*op, lt, rt)?;
            Ok((b.strand(term, &[lh, rh]).map_err(ElabError::build)?, out))
        }
        Expr::Call { callee, args } => match callee.as_str() {
            "add" if args.len() == 2 => {
                let (a, at) = emit_expr(b, &args[0], scope, depth + 1)?;
                let (c, ct) = emit_expr(b, &args[1], scope, depth + 1)?;
                if at != ValType::Num || ct != ValType::Num {
                    return Err(ElabError::TypeError {
                        op: "add()".into(),
                        operands: vec![at, ct],
                    });
                }
                Ok((
                    b.strand("js.add", &[a, c]).map_err(ElabError::build)?,
                    ValType::Num,
                ))
            }
            "concat" if args.len() == 2 => {
                let (a, at) = emit_expr(b, &args[0], scope, depth + 1)?;
                let (c, ct) = emit_expr(b, &args[1], scope, depth + 1)?;
                if at != ValType::Str || ct != ValType::Str {
                    return Err(ElabError::TypeError {
                        op: "concat()".into(),
                        operands: vec![at, ct],
                    });
                }
                Ok((
                    b.strand("js.concat", &[a, c]).map_err(ElabError::build)?,
                    ValType::Str,
                ))
            }
            "mul" if args.len() == 2 => {
                let (a, at) = emit_expr(b, &args[0], scope, depth + 1)?;
                let (c, ct) = emit_expr(b, &args[1], scope, depth + 1)?;
                if at != ValType::Num || ct != ValType::Num {
                    return Err(ElabError::TypeError {
                        op: "mul()".into(),
                        operands: vec![at, ct],
                    });
                }
                Ok((
                    b.strand("js.mul", &[a, c]).map_err(ElabError::build)?,
                    ValType::Num,
                ))
            }
            "sub" if args.len() == 2 => {
                let (a, at) = emit_expr(b, &args[0], scope, depth + 1)?;
                let (c, ct) = emit_expr(b, &args[1], scope, depth + 1)?;
                if at != ValType::Num || ct != ValType::Num {
                    return Err(ElabError::TypeError {
                        op: "sub()".into(),
                        operands: vec![at, ct],
                    });
                }
                Ok((
                    b.strand("js.sub", &[a, c]).map_err(ElabError::build)?,
                    ValType::Num,
                ))
            }
            other => Err(ElabError::Parse(format!(
                "unsupported or undeclared function `{other}` with {} args",
                args.len()
            ))),
        },
    }
}

/// The deterministic intent string for a source.
fn intent_for(src: &str) -> String {
    format!("JS expression elaborated to Braid IR: {}", src.trim())
}

/// Elaborates a JS source (expressions, let-statements, returns) into an admittable [`Capsule`].
pub fn elaborate_js(src: &str) -> Result<Capsule, ElabError> {
    let source_len = src.chars().count();
    if source_len > MAX_SOURCE_CHARS {
        return Err(ElabError::SourceTooLong {
            len: source_len,
            limit: MAX_SOURCE_CHARS,
        });
    }

    let prog = parse_program(src)?;
    let reg = registry_v0();
    let mut b = Builder::new(&reg, intent_for(src));
    let mut scope: BTreeMap<String, (Strand, ValType)> = BTreeMap::new();
    let mut last_out: Option<Strand> = None;

    for stmt in prog.stmts {
        match stmt {
            Stmt::Let { name, expr } => {
                if scope.contains_key(&name) {
                    return Err(ElabError::DuplicateBinding(name));
                }
                let (strand, ty) = emit_expr(&mut b, &expr, &scope, 0)?;
                scope.insert(name, (strand, ty));
                last_out = Some(strand);
            }
            Stmt::Expr(expr) => {
                let (strand, _ty) = emit_expr(&mut b, &expr, &scope, 0)?;
                last_out = Some(strand);
            }
            Stmt::Return(expr) => {
                let (strand, _ty) = emit_expr(&mut b, &expr, &scope, 0)?;
                last_out = Some(strand);
                break;
            }
        }
    }

    let out_strand = last_out.ok_or(ElabError::Empty)?;
    b.output(out_strand);
    b.build().map_err(ElabError::build)
}

/// The full loop: source → IR → **verify** → manifest.
pub struct Elaboration {
    /// The resulting admitted Capsule.
    pub capsule: Capsule,
    /// The admission verdict from braid-verify.
    pub verdict: Verdict,
    /// Rendered human-readable manifest text.
    pub manifest_text: String,
}

/// Elaborate, then admit + render. Pure dataflow capsules request 0 capabilities.
pub fn elaborate_and_admit(src: &str) -> Result<Elaboration, ElabError> {
    let capsule = elaborate_js(src)?;
    let reg = registry_v0();
    let verdict = verify(&capsule.to_bytes(), &reg, &[]);
    let m = manifest(&capsule, &reg).map_err(|e| ElabError::Render(format!("{e:?}")))?;
    let manifest_text = render_text(&m);
    Ok(Elaboration {
        capsule,
        verdict,
        manifest_text,
    })
}

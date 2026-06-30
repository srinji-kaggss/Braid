//! # braid-elaborate-js — a thin JavaScript-expression frontend (U11–U12, D31)
//!
//! The **first real frontend over the global IR.** `braid-vocab-js` is the
//! elaboration target; this crate compiles JS *text* into an admitted
//! [`braid_ir::Capsule`] via the **one** `braid-verify`. It operationalizes
//! D31's "renders JS useless" — JS becomes an authoring frontend over the
//! verified substrate, not a runtime authority. Zero AI in the path.
//!
//! ## Scope (a thin slice — an expression language, not a JS parser)
//! - **U11**: string + integer literals, binary `+`.
//! - **U12**: boolean literals (`true`/`false`), arithmetic `-` `*`, comparison
//!   `<` `==`, boolean logic `&&` `||`, prefix `!`, full operator precedence +
//!   parentheses. Overloaded operators resolve by operand type to a distinct
//!   typed term (`+` → `js.concat`/`js.add`; `==` → `js.eq.str`/`js.eq.num`);
//!   any type mismatch is **rejected at elaboration** (fail-closed — no
//!   coercion, and no malformed capsule ever reaches the verifier).
//!
//! Still out of scope (later units): identifiers, calls, statements, the
//! `js.eval`/`js.fetch` escalation probes, and literal *values* (the `js.lit.*`
//! terms are valueless — carrying payloads needs a substrate-level `Strand`
//! change, not a vocabulary or frontend change; see `DEBT_REGISTER.md`).
//!
//! ## Boundary
//! A *consumer* crate (like `braid-cli`/`braid-sdk`): it elaborates INTO the
//! substrate through `braid_sdk::Builder`; it is **not** trust-base and builds
//! no second verifier. The verifier remains the sole admission authority (D9).

use braid_ir::Capsule;
use braid_render::{manifest, render_text};
use braid_sdk::{Builder, Strand};
use braid_verify::{verify, Verdict};
use braid_vocab_js::registry_v0;

// ─────────────────────────────────────────────────────────────────────────────
// Value types + errors
// ─────────────────────────────────────────────────────────────────────────────

/// The value types this frontend's type-directed elaboration distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
    Str,
    Num,
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
    /// A character or token the lexer cannot form.
    Lex(String),
    /// A token stream the grammar does not accept.
    Parse(String),
    /// An operator applied to operand type(s) it has no typed term for (no
    /// implicit coercion). `operands` is the operand type list in source order.
    TypeError { op: String, operands: Vec<ValType> },
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
    Plus,
    Minus,
    Star,
    Lt,
    EqEq,
    AndAnd,
    OrOr,
    Bang,
    LParen,
    RParen,
}

fn lex(src: &str) -> Result<Vec<Token>, ElabError> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
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
            // Multi-char operators: the single-char form is an error in this
            // expression slice (no assignment, no bitwise ops).
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    toks.push(Token::EqEq);
                    i += 2;
                } else {
                    return Err(ElabError::Lex(
                        "`=` is not an operator here; did you mean `==`?".to_string(),
                    ));
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
                                return Err(ElabError::Lex(format!("unsupported escape \\{other}")))
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
            // Keywords only — identifiers are a later unit. `true`/`false` lex to
            // boolean literals; anything else is a clear, fail-closed error.
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
                match w.as_str() {
                    "true" => toks.push(Token::Bool(true)),
                    "false" => toks.push(Token::Bool(false)),
                    other => {
                        return Err(ElabError::Lex(format!(
                            "identifiers are not supported yet (expressions only): `{other}`"
                        )))
                    }
                }
            }
            other => return Err(ElabError::Lex(format!("unexpected character {other:?}"))),
        }
    }
    Ok(toks)
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser — precedence climbing (Pratt). Left-associative; `!` is prefix.
//   ||  <  &&  <  == <  <  +  -  <  *      (low → high)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Lt,
    Eq,
    And,
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

/// The expression AST. Operators are *syntactic*; which typed term each
/// elaborates to is decided by operand types at elaboration time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Str(String),
    Num(i64),
    Bool(bool),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
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
        let mut lhs = match self.bump() {
            Some(Token::Num(n)) => Expr::Num(n),
            Some(Token::Str(s)) => Expr::Str(s),
            Some(Token::Bool(b)) => Expr::Bool(b),
            Some(Token::Bang) => {
                // Prefix `!` binds tighter than any binary operator.
                let operand = self.expr_bp(11)?;
                Expr::Unary(UnOp::Not, Box::new(operand))
            }
            Some(Token::LParen) => {
                let inner = self.expr_bp(0)?;
                match self.bump() {
                    Some(Token::RParen) => inner,
                    other => {
                        return Err(ElabError::Parse(format!("expected ')', found {other:?}")))
                    }
                }
            }
            other => {
                return Err(ElabError::Parse(format!(
                    "expected an expression, found {other:?}"
                )))
            }
        };

        while let Some(op) = self.infix_op() {
            let (lbp, rbp) = op.binding_power();
            if lbp < min_bp {
                break;
            }
            self.bump(); // consume the operator
            let rhs = self.expr_bp(rbp)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
}

/// Lex + parse a JS expression into an [`Expr`]. Exposed for reuse/testing.
pub fn parse(src: &str) -> Result<Expr, ElabError> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err(ElabError::Empty);
    }
    let mut p = Parser { toks, pos: 0 };
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
// Elaboration — Expr → Strands over braid-vocab-js, via the SDK
// ─────────────────────────────────────────────────────────────────────────────

/// Type-directed term selection for a binary operator. The fail-closed line:
/// matching operand types pick a single typed term; anything else is a type
/// error, never a coercion.
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
            })
        }
    };
    Ok(picked)
}

fn emit(b: &mut Builder, e: &Expr) -> Result<(Strand, ValType), ElabError> {
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
        Expr::Unary(UnOp::Not, x) => {
            let (xh, xt) = emit(b, x)?;
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
            let (lh, lt) = emit(b, l)?;
            let (rh, rt) = emit(b, r)?;
            let (term, out) = resolve_binary(*op, lt, rt)?;
            Ok((b.strand(term, &[lh, rh]).map_err(ElabError::build)?, out))
        }
    }
}

/// The deterministic intent string for a source. Folding the source text in
/// makes the capsule CID a reproducible function of the human input — the
/// human-reconstructable-loop guarantee (same source ⇒ same CID).
fn intent_for(src: &str) -> String {
    format!("JS expression elaborated to Braid IR: {}", src.trim())
}

/// Elaborate a JS expression into an admittable [`Capsule`]. Pure value
/// construction only, so no capability/confirm is required.
pub fn elaborate_js(src: &str) -> Result<Capsule, ElabError> {
    let expr = parse(src)?;
    let reg = registry_v0();
    let mut b = Builder::new(&reg, intent_for(src));
    let (root, _ty) = emit(&mut b, &expr)?;
    b.output(root);
    b.build().map_err(ElabError::build)
}

/// The full loop: source → IR → **verify** → manifest.
pub struct Elaboration {
    pub capsule: Capsule,
    pub verdict: Verdict,
    pub manifest_text: String,
}

/// Elaborate, then admit + render. The ambient grant set is empty: a pure
/// expression requests no capability, so `∅ ⊆ ∅` passes the attenuation check.
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

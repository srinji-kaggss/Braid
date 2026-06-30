//! # braid-elaborate-js — a thin JavaScript-expression frontend (U11, D31)
//!
//! The **first real frontend over the global IR.** Today `braid-vocab-js`
//! exists and is proven by a *hand-built* SDK capsule
//! (`js_capsule_admits_via_the_one_verifier`), but nothing actually compiles
//! JS *text* into it. This crate closes the first increment of D-ELAB: JS
//! source in, an admitted [`braid_ir::Capsule`] out, via the **one**
//! `braid-verify`. It operationalizes D31's "renders JS useless" — JS becomes
//! an authoring frontend over the verified substrate, not a runtime authority.
//!
//! ## Scope (deliberately a thin vertical slice — U11, not a JS parser)
//! A JS *expression* over string literals, integer-number literals, and the
//! binary `+` operator (left-associative, parenthesizable). `+` elaborates to
//! `js.concat` for two strings and `js.add` for two numbers; a mixed
//! `string + number` is **rejected at elaboration** (fail-closed — no implicit
//! coercion, and no malformed capsule ever reaches the verifier). Identifiers,
//! calls, statements, and the `js.eval`/`js.fetch` escalation probes are U12+.
//!
//! ## Boundary
//! A *consumer* crate (like `braid-cli`/`braid-sdk`): it elaborates INTO the
//! substrate through `braid_sdk::Builder`; it is **not** trust-base and builds
//! no second verifier. The verifier remains the sole admission authority (D9).
//!
//! ## A known seed-vocabulary limitation (honest, not a bug)
//! The v0 `js.lit.*` terms are *valueless* — `js.lit.string` records "a string
//! literal occurs here", not its bytes. So the IR/CID is a function of the
//! expression's *structure*, not literal content. Carrying literal payloads is
//! a `braid-vocab-js` extension (U12), not this frontend's job.

use braid_ir::Capsule;
use braid_render::{manifest, render_text};
use braid_sdk::{Builder, Strand};
use braid_verify::{verify, Verdict};
use braid_vocab_js::registry_v0;

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

/// The two value types this slice's `+` operator distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
    Str,
    Num,
}

impl core::fmt::Display for ValType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ValType::Str => f.write_str("string"),
            ValType::Num => f.write_str("number"),
        }
    }
}

/// Every way the frontend fails closed. None of these produce a capsule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElabError {
    /// No tokens — empty or whitespace-only source.
    Empty,
    /// A character or string the lexer cannot tokenize.
    Lex(String),
    /// A token stream the grammar does not accept.
    Parse(String),
    /// `+` applied to mismatched operand types (no implicit coercion).
    TypeMismatch { left: ValType, right: ValType },
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
            ElabError::TypeMismatch { left, right } => write!(
                f,
                "type error: `+` requires matching operand types (no implicit \
                 coercion); got {left} + {right}"
            ),
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
    Plus,
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
            '(' => {
                toks.push(Token::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Token::RParen);
                i += 1;
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
            other => return Err(ElabError::Lex(format!("unexpected character {other:?}"))),
        }
    }
    Ok(toks)
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser  —  expr := atom ('+' atom)*  ;  atom := num | str | '(' expr ')'
// ─────────────────────────────────────────────────────────────────────────────

/// The expression AST. `Add` is the *syntactic* `+`; whether it elaborates to
/// `js.concat` or `js.add` is decided by operand types at elaboration time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Str(String),
    Num(i64),
    Add(Box<Expr>, Box<Expr>),
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

    fn parse_expr(&mut self) -> Result<Expr, ElabError> {
        let mut left = self.parse_atom()?;
        while matches!(self.peek(), Some(Token::Plus)) {
            self.bump();
            let right = self.parse_atom()?;
            left = Expr::Add(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_atom(&mut self) -> Result<Expr, ElabError> {
        match self.bump() {
            Some(Token::Num(n)) => Ok(Expr::Num(n)),
            Some(Token::Str(s)) => Ok(Expr::Str(s)),
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                match self.bump() {
                    Some(Token::RParen) => Ok(inner),
                    other => Err(ElabError::Parse(format!("expected ')', found {other:?}"))),
                }
            }
            other => Err(ElabError::Parse(format!(
                "expected a literal or '(', found {other:?}"
            ))),
        }
    }
}

/// Parse a token stream into an [`Expr`]. Exposed so U12 can reuse it.
pub fn parse(src: &str) -> Result<Expr, ElabError> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err(ElabError::Empty);
    }
    let mut p = Parser { toks, pos: 0 };
    let expr = p.parse_expr()?;
    if p.pos != p.toks.len() {
        return Err(ElabError::Parse(format!(
            "unexpected trailing token {:?}",
            p.toks.get(p.pos)
        )));
    }
    Ok(expr)
}

// ─────────────────────────────────────────────────────────────────────────────
// Elaboration  —  Expr → Strands over braid-vocab-js, via the SDK
// ─────────────────────────────────────────────────────────────────────────────

fn emit(b: &mut Builder, e: &Expr) -> Result<(Strand, ValType), ElabError> {
    match e {
        Expr::Str(_) => {
            let s = b.strand("js.lit.string", &[]).map_err(ElabError::build)?;
            Ok((s, ValType::Str))
        }
        Expr::Num(_) => {
            let s = b.strand("js.lit.number", &[]).map_err(ElabError::build)?;
            Ok((s, ValType::Num))
        }
        Expr::Add(l, r) => {
            let (lh, lt) = emit(b, l)?;
            let (rh, rt) = emit(b, r)?;
            // The overload resolution + the fail-closed line: matching types
            // pick a term; a mismatch is an elaboration error, never a coercion.
            let term = match (lt, rt) {
                (ValType::Str, ValType::Str) => "js.concat",
                (ValType::Num, ValType::Num) => "js.add",
                _ => {
                    return Err(ElabError::TypeMismatch {
                        left: lt,
                        right: rt,
                    })
                }
            };
            let s = b.strand(term, &[lh, rh]).map_err(ElabError::build)?;
            Ok((s, lt))
        }
    }
}

/// The deterministic intent string for a source. Folding the source text in
/// makes the capsule CID a reproducible function of the human input — the
/// human-reconstructable-loop guarantee (same source ⇒ same CID).
fn intent_for(src: &str) -> String {
    format!("JS expression elaborated to Braid IR (U11): {}", src.trim())
}

/// Elaborate a JS expression into an admittable [`Capsule`]. Pure value
/// construction only, so no capability/confirm is required and `build()`
/// succeeds with `ConfirmPolicy::None`.
pub fn elaborate_js(src: &str) -> Result<Capsule, ElabError> {
    let expr = parse(src)?;
    let reg = registry_v0();
    let mut b = Builder::new(&reg, intent_for(src));
    let (root, _ty) = emit(&mut b, &expr)?;
    b.output(root);
    b.build().map_err(ElabError::build)
}

/// The full thin-slice loop: source → IR → **verify** → manifest. The verdict
/// is read off the one `braid-verify` against the JS vocabulary's registry;
/// the manifest is the human-audit object bound to the capsule's CID.
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

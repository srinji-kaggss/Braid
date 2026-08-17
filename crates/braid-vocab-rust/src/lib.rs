//! # braid-vocab-rust — Rust elaboration target (W1)
//!
//! The inverse direction of the frontends: an **admitted** capsule elaborates
//! into a deterministic, dependency-free Rust API surface.
//!
//! - Registry terms become trait method *declarations* (braid has no VM yet —
//!   U7 — so the API surface is the honest artifact; the host implements the
//!   trait at link time). Declarations compile; nothing is faked.
//! - Vocabulary `Opaque` types become distinct newtypes, so the dimension
//!   contract travels into the generated crate: `DurMs` and `Bytes` are
//!   different Rust types by construction.
//! - The capsule CID is a `const`; the generated `build.rs` re-verifies the
//!   capsule payload through the braid CLI when it is present (warning, not
//!   failure, when absent — the generated crate must not depend on the CLI
//!   being installed).
//!
//! Zero AI in the path. The verifier remains the sole admission authority
//! (D9): this crate refuses to elaborate anything `braid-verify` rejects.

use braid_ir::{Capsule, TermRegistry, TypeTag};
use braid_verify::{verify, Verdict};

/// Failure modes of elaboration. None of these emit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElabError {
    /// The verifier rejected the capsule; elaboration is fail-closed.
    Rejected(String),
}

/// The emitted crate: four deterministic artifacts, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustCrate {
    pub cargo_toml: String,
    pub lib_rs: String,
    pub build_rs: String,
    /// Capsule canonical bytes, hex — `build.rs` re-verifies this payload.
    pub capsule_hex: String,
}

/// Map a closed type-tag to a dependency-free Rust type.
fn rust_type(t: &TypeTag) -> String {
    match t {
        TypeTag::Bool => "bool".to_string(),
        // Fixed-point integer; the term-declared scaling is carried in the
        // doc comment (scaling is vocabulary semantics, not substrate).
        TypeTag::Int => "i64".to_string(),
        TypeTag::Bytes => "Vec<u8>".to_string(),
        TypeTag::Text => "String".to_string(),
        // Raw BLAKE3 digest — keeps generated crates dependency-free.
        TypeTag::Cid => "[u8; 32]".to_string(),
        TypeTag::Opaque(label, _) => newtype_name(label),
        TypeTag::List(inner) => format!("Vec<{}>", rust_type(inner)),
    }
}

/// `js.dom.querySelector` → `js_dom_query_selector`. Term ids are dotted
/// camelCase; emitted Rust identifiers must be snake_case (warning-clean).
fn to_snake_case(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut prev_lower = false;
    for c in id.chars() {
        if !c.is_alphanumeric() {
            out.push('_');
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower {
            out.push('_');
        }
        out.extend(c.to_lowercase());
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
    }
    out
}

/// `Opaque("dur.ms", …)` → `DurMs`. The label is the identity: distinct
/// dimensions are distinct Rust types by construction.
fn newtype_name(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut upper = true;
    for c in label.chars() {
        if c == '.' || c == '-' || c == '_' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        "Opaque".to_string()
    } else {
        out
    }
}

/// Distinct `Opaque` labels in the registry, first-seen order.
fn opaque_labels(registry: &TermRegistry) -> Vec<(String, Vec<TypeTag>)> {
    let mut seen: Vec<(String, Vec<TypeTag>)> = Vec::new();
    let visit = |t: &TypeTag, seen: &mut Vec<(String, Vec<TypeTag>)>| {
        if let TypeTag::Opaque(label, args) = t {
            if !seen.iter().any(|(l, _)| l == label) {
                seen.push((label.clone(), args.clone()));
            }
        }
    };
    for spec in registry.terms() {
        for t in &spec.inputs {
            visit(t, &mut seen);
        }
        visit(&spec.output, &mut seen);
    }
    seen
}

/// Elaborate an admitted capsule into a deterministic Rust crate.
pub fn elaborate(registry: &TermRegistry, capsule: &Capsule) -> Result<RustCrate, ElabError> {
    match verify(&capsule.to_bytes(), registry, &capsule.grants) {
        Verdict::Admit { .. } => {}
        Verdict::Reject { stage, reason } => {
            return Err(ElabError::Rejected(format!("{stage:?}: {reason}")));
        }
    }

    // Newtypes for every distinct Opaque label (dimension contract carrier).
    let mut newtypes = String::new();
    for (label, args) in opaque_labels(registry) {
        let name = newtype_name(&label);
        let fields: Vec<String> = args.iter().map(rust_type).collect();
        let decl = if fields.is_empty() {
            format!("pub struct {name};")
        } else {
            format!("pub struct {name}({});", fields.join(", "))
        };
        newtypes.push_str(&format!(
            "/// Braid opaque type `{label}` — distinct from every other newtype.\n#[derive(Clone, Debug, PartialEq, Eq)]\n{decl}\n\n"
        ));
    }

    // API surface: one trait declaration per registry term.
    let mut fns = String::new();
    for spec in registry.terms() {
        let args: Vec<String> = spec
            .inputs
            .iter()
            .enumerate()
            .map(|(i, t)| format!("a{i}: {}", rust_type(t)))
            .collect();
        let ret = rust_type(&spec.output);
        let cap = spec
            .capability
            .as_ref()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "none".to_string());
        fns.push_str(&format!(
            "    /// Braid term `{id}` — effect {effect:?}, cost {cost}, capability `{cap}`.\n    fn {name}({args}) -> {ret};\n",
            id = spec.id,
            effect = spec.effect,
            cost = spec.cost,
            cap = cap,
            name = to_snake_case(&spec.id),
            args = args.join(", "),
            ret = ret,
        ));
    }

    let capsule_hex = {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let bytes = capsule.to_bytes();
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in &bytes {
            s.push(HEX[(b >> 4) as usize] as char);
            s.push(HEX[(b & 0x0f) as usize] as char);
        }
        s
    };

    let lib_rs = format!(
        "//! Braid capsule `{cid}` — generated by braid-vocab-rust. Do not edit;\n\
         //! re-elaborate from the admitted capsule. Intent: {intent}\n\
         //! Confirm policy: {confirm:?}. Budget: {budget} cost units.\n\
         //! Grants: {grants:?}.\n\
         \n\
         /// Capsule content address (BLAKE3, `lw.braid.capsule.v0`).\n\
         pub const CAPSULE_CID: &str = \"{cid}\";\n\
         \n\
         /// The API surface of the admitted capsule. Declarations only — the\n\
         /// host implements this trait at link time (no VM yet, U7).\n\
         pub trait BraidApi {{\n\
         {fns}}}\n\
         \n\
         {newtypes}\n",
        cid = capsule.cid().to_hex(),
        intent = capsule.intent,
        confirm = capsule.confirm,
        budget = capsule.budget,
        grants = capsule.grants,
        fns = fns,
        newtypes = newtypes,
    );

    let build_rs = format!(
        "//! Generated build script: re-verifies the capsule payload via the\n\
         //! braid CLI when it is installed; warns (never fails) when absent.\n\
         fn main() {{\n\
         \x20   let hex: &[u8] = b\"{hex}\";\n\
         \x20   let payload: Vec<u8> = hex\n\
         \x20       .chunks_exact(2)\n\
         \x20       .map(|p| u8::from_str_radix(core::str::from_utf8(p).unwrap(), 16).unwrap())\n\
         \x20       .collect();\n\
         \x20   let out = std::env::var(\"OUT_DIR\").unwrap();\n\
         \x20   let path = std::path::Path::new(&out).join(\"capsule.enc\");\n\
         \x20   std::fs::write(&path, &payload).unwrap();\n\
         \x20   match std::process::Command::new(\"braid\").arg(\"verify\").arg(&path).output() {{\n\
         \x20       Ok(o) if o.status.success() => {{}}\n\
         \x20       Ok(o) => panic!(\"braid capsule failed admission: {{}}\", String::from_utf8_lossy(&o.stderr)),\n\
         \x20       Err(_) => println!(\"cargo:warning=braid CLI not found; capsule re-verification skipped\"),\n\
         \x20   }}\n\
         }}\n",
        hex = capsule_hex,
    );

    let cargo_toml = format!(
        "[package]\nname = \"braid-capsule-{cid}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        cid = capsule.cid().to_hex(),
    );

    Ok(RustCrate {
        cargo_toml,
        lib_rs,
        build_rs,
        capsule_hex,
    })
}

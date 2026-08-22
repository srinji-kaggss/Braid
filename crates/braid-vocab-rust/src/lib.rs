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
use std::fmt;

/// Failure modes of elaboration. None of these emit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElabError {
    /// The verifier rejected the capsule; elaboration is fail-closed.
    Rejected {
        /// Reason given by the verifier stage.
        reason: String,
        /// Source component where rejection occurred.
        at: &'static str,
    },
}

impl fmt::Display for ElabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { reason, at } => write!(f, "elaboration rejected at {at}: {reason}"),
        }
    }
}

impl std::error::Error for ElabError {}

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

fn step_snake_case(c: char, prev_lower: &mut bool, out: &mut String) {
    if !c.is_alphanumeric() {
        out.push('_');
        *prev_lower = false;
    } else {
        if c.is_uppercase() && *prev_lower {
            out.push('_');
        }
        out.extend(c.to_lowercase());
        *prev_lower = c.is_lowercase() || c.is_ascii_digit();
    }
}

/// `js.dom.querySelector` → `js_dom_query_selector`. Term ids are dotted
/// camelCase; emitted Rust identifiers must be snake_case (warning-clean).
fn to_snake_case(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut prev_lower = false;
    for c in id.chars() {
        step_snake_case(c, &mut prev_lower, &mut out);
    }
    out
}

fn step_newtype_char(c: char, upper: &mut bool, out: &mut String) {
    if c == '.' || c == '-' || c == '_' {
        *upper = true;
    } else if *upper {
        out.extend(c.to_uppercase());
        *upper = false;
    } else {
        out.push(c);
    }
}

/// `Opaque("dur.ms", …)` → `DurMs`. The label is the identity: distinct
/// dimensions are distinct Rust types by construction.
fn newtype_name(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut upper = true;
    for c in label.chars() {
        step_newtype_char(c, &mut upper, &mut out);
    }
    if out.is_empty() {
        "Opaque".to_string()
    } else {
        out
    }
}

fn collect_opaque_tag(t: &TypeTag, seen: &mut Vec<(String, Vec<TypeTag>)>) {
    if let TypeTag::Opaque(label, args) = t {
        if !seen.iter().any(|(l, _)| l == label) {
            seen.push((label.clone(), args.clone()));
        }
    }
}

/// Distinct `Opaque` labels in the registry, first-seen order.
fn opaque_labels(registry: &TermRegistry) -> Vec<(String, Vec<TypeTag>)> {
    let mut seen: Vec<(String, Vec<TypeTag>)> = Vec::new();
    for spec in registry.terms() {
        for t in &spec.inputs {
            collect_opaque_tag(t, &mut seen);
        }
        collect_opaque_tag(&spec.output, &mut seen);
    }
    seen
}

fn verify_capsule_admission(registry: &TermRegistry, capsule: &Capsule) -> Result<(), ElabError> {
    match verify(&capsule.to_bytes(), registry, &capsule.grants) {
        Verdict::Admit { .. } => Ok(()),
        Verdict::Reject { stage, reason } => Err(ElabError::Rejected {
            reason: format!("{stage:?}: {reason}"),
            at: "braid_verify::verify",
        }),
    }
}

fn render_single_newtype(label: &str, args: &[TypeTag], out: &mut String) {
    let name = newtype_name(label);
    let fields: Vec<String> = args.iter().map(rust_type).collect();
    let decl = if fields.is_empty() {
        format!("pub struct {name};")
    } else {
        format!("pub struct {name}({});", fields.join(", "))
    };
    out.push_str(&format!(
        "/// Braid opaque type `{label}` — distinct from every other newtype.\n#[derive(Clone, Debug, PartialEq, Eq)]\n{decl}\n\n"
    ));
}

fn render_newtypes(registry: &TermRegistry) -> String {
    let mut newtypes = String::new();
    for (label, args) in opaque_labels(registry) {
        render_single_newtype(&label, &args, &mut newtypes);
    }
    newtypes
}

fn render_trait_methods(registry: &TermRegistry) -> String {
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
    fns
}

fn encode_capsule_hex(capsule: &Capsule) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = capsule.to_bytes();
    let mut hex_str = String::with_capacity(bytes.len() * 2);
    for byte_val in &bytes {
        hex_str.push(HEX[(byte_val >> 4) as usize] as char);
        hex_str.push(HEX[(byte_val & 0x0f) as usize] as char);
    }
    hex_str
}

fn render_lib_rs(capsule: &Capsule, fns: &str, newtypes: &str) -> String {
    format!(
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
    )
}

fn render_build_rs(capsule_hex: &str) -> String {
    format!(
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
    )
}

fn render_cargo_toml(capsule: &Capsule) -> String {
    format!(
        "[package]\nname = \"braid-capsule-{cid}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        cid = capsule.cid().to_hex(),
    )
}

/// Elaborate an admitted capsule into a deterministic Rust crate.
pub fn elaborate(registry: &TermRegistry, capsule: &Capsule) -> Result<RustCrate, ElabError> {
    verify_capsule_admission(registry, capsule)?;
    let newtypes = render_newtypes(registry);
    let fns = render_trait_methods(registry);
    let capsule_hex = encode_capsule_hex(capsule);
    let lib_rs = render_lib_rs(capsule, &fns, &newtypes);
    let build_rs = render_build_rs(&capsule_hex);
    let cargo_toml = render_cargo_toml(capsule);

    Ok(RustCrate {
        cargo_toml,
        lib_rs,
        build_rs,
        capsule_hex,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use braid_vocab_cms::{edit_section_capsule, registry_v0};

    #[test]
    fn admitted_capsule_elaborates_to_rust() {
        let reg = registry_v0();
        let cap = edit_section_capsule();
        let krate = elaborate(&reg, &cap).expect("admitted capsule elaborates");
        assert!(krate.cargo_toml.contains("braid-capsule-"));
        assert!(krate.lib_rs.contains("pub trait BraidApi"));
        assert!(krate.lib_rs.contains("pub struct CmsDirective;"));
        assert!(krate.lib_rs.contains("pub struct CmsEntity;"));
        assert!(krate.lib_rs.contains("fn cms_edit_section("));
        assert!(krate.lib_rs.contains("fn view_section("));
        assert!(krate.build_rs.contains("capsule.enc"));
    }

    #[test]
    fn rejected_capsule_is_refused() {
        let reg = registry_v0();
        let mut cap = edit_section_capsule();
        cap.budget = 1; // insufficient budget → Verifier rejects at Bounds
        let err = elaborate(&reg, &cap).unwrap_err();
        assert!(matches!(err, ElabError::Rejected { .. }));
    }
}

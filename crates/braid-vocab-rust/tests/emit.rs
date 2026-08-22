//! W1 acceptance: deterministic emission, fail-closed admission, and the
//! emitted API surface compiles as a dependency-free crate.

use braid_vocab_cms::{edit_section_capsule, registry_v0};
use braid_vocab_rust::{elaborate, ElabError, RustCrate};

/// Same input → byte-identical output (emission is deterministic; BTreeMap
/// registry order + strand order, no wall-clock anywhere).
#[test]
fn elaboration_is_deterministic() {
    let a = elaborate(&registry_v0(), &edit_section_capsule()).unwrap();
    let b = elaborate(&registry_v0(), &edit_section_capsule()).unwrap();
    assert_eq!(a, b);
}

/// The dimension contract travels: distinct Opaque labels become distinct
/// Rust newtypes, and no term signature references an undeclared type.
#[test]
fn opaque_newtypes_carry_dimensions() {
    use braid_ir::braid::{Braid, Strand};
    use braid_ir::{Capsule, EffectClass, Exposure, TermRegistry, TermSpec, TypeTag, IR_VERSION};
    let mut reg = TermRegistry::new(1);
    reg.insert(TermSpec {
        id: "dim.dur".into(),
        inputs: vec![],
        output: TypeTag::Opaque("dur.ms".into(), vec![]),
        capability: None,
        effect: EffectClass::Pure,
        source_exposure: Exposure::Public,
        egress_ceiling: None,
        cost: 1,
    })
    .unwrap();
    reg.insert(TermSpec {
        id: "dim.bytes".into(),
        inputs: vec![],
        output: TypeTag::Opaque("bytes".into(), vec![]),
        capability: None,
        effect: EffectClass::Pure,
        source_exposure: Exposure::Public,
        egress_ceiling: None,
        cost: 1,
    })
    .unwrap();
    let c = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "dimensions".into(),
        grants: vec![],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "dim.dur".into(),
                    inputs: vec![],
                },
                Strand {
                    term: "dim.bytes".into(),
                    inputs: vec![],
                },
            ],
            outputs: vec![1],
        },
        budget: 10,
        confirm: braid_ir::ConfirmPolicy::None,
        evidence: vec![],
    };
    let out = elaborate(&reg, &c).unwrap();
    assert!(out.lib_rs.contains("pub struct DurMs;"), "{}", out.lib_rs);
    assert!(out.lib_rs.contains("pub struct Bytes;"), "{}", out.lib_rs);
    assert!(
        out.lib_rs.contains("fn dim_dur() -> DurMs;"),
        "{}",
        out.lib_rs
    );
    assert!(
        out.lib_rs.contains("fn dim_bytes() -> Bytes;"),
        "{}",
        out.lib_rs
    );
}

/// Fail-closed: a capsule the verifier rejects is never elaborated.
#[test]
fn rejected_capsule_refuses_to_elaborate() {
    let mut c = edit_section_capsule();
    c.braid.strands[3].inputs = vec![0]; // Entity into view.section (Types reject)
    match elaborate(&registry_v0(), &c) {
        Err(ElabError::Rejected { reason, .. }) => assert!(reason.contains("Types"), "{reason}"),
        Ok(_) => panic!("rejected capsule elaborated"),
    }
}

/// The emitted lib.rs compiles as a dependency-free crate (rustc present).
#[test]
fn emitted_lib_compiles() {
    let RustCrate { lib_rs, .. } = elaborate(&registry_v0(), &edit_section_capsule()).unwrap();
    let dir = std::env::temp_dir().join(format!("braid-vocab-rust-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lib.rs");
    std::fs::write(&path, lib_rs).unwrap();
    let out = std::process::Command::new("rustc")
        .args(["--crate-type", "lib", "--edition", "2021"])
        .arg(&path)
        .output()
        .expect("rustc must be on PATH");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "emitted lib.rs failed to compile:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

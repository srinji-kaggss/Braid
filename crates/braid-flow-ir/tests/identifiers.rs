use braid_flow_ir::NodeKey;

#[test]
fn node_key_grammar_is_exact_and_bounded() {
    assert!(NodeKey::new("build.rust-1").is_ok());
    assert!(NodeKey::new("Build").is_err());
    assert!(NodeKey::new("1build").is_err());
    assert!(NodeKey::new("build/slash").is_err());
    assert!(NodeKey::new(&"a".repeat(128)).is_ok());
    assert!(NodeKey::new(&"a".repeat(129)).is_err());
}

#[test]
fn hostile_identifier_diagnostics_are_bounded() {
    let error = NodeKey::new(&"a".repeat(1_000_000)).unwrap_err();
    assert_eq!(error.length, 1_000_000);
    assert!(error.to_string().len() < 64);
}

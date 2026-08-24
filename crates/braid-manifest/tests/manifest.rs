use braid_manifest::{
    Archetype, CiStatus, ManifestError, RepoManifest, parse_inventory, safe_name_component,
    validate,
};

const VALID: &str = r#"{
    "name": "braid",
    "archetype": "workspace-crate",
    "owner": "Director",
    "gate_version": "0.1.0",
    "ci_status": "green",
    "entry_docs": [
        "AGENTS.md",
        "README.md",
        ".wwfd/local-ci.sh"
    ],
    "canonical_commands": [
        "cargo test --workspace",
        "cargo build --release"
    ],
    "local_ci": true
}"#;

#[test]
fn valid_manifest_parses_and_roundtrips() {
    let m = validate(VALID).expect("valid manifest parses");
    assert_eq!(m.name, "braid");
    assert_eq!(m.archetype, Archetype::WorkspaceCrate);
    assert_eq!(m.owner, "Director");
    assert_eq!(m.gate_version, "0.1.0");
    assert_eq!(m.ci_status, CiStatus::Green);
    assert_eq!(m.entry_docs.len(), 3);
    assert_eq!(m.canonical_commands.len(), 2);
    assert!(m.local_ci);

    // Canonical bytes round-trip.
    let bytes = m.to_bytes();
    let decoded = RepoManifest::from_bytes(&bytes).expect("canonical round-trip");
    assert_eq!(decoded, m);
    assert_eq!(decoded.cid(), m.cid());
}

#[test]
fn archetype_outside_closed_set_is_named() {
    let bad = VALID.replace("\"workspace-crate\"", "\"weird-container\"");
    assert!(matches!(
        validate(&bad).unwrap_err(),
        ManifestError::BadEnum {
            field: "archetype",
            ..
        }
    ));
}

#[test]
fn ci_status_outside_closed_set_is_named() {
    for bad_value in ["unknown", "degraded", ""] {
        let bad = VALID.replace("\"green\"", &format!("\"{bad_value}\""));
        let err = validate(&bad).unwrap_err();
        assert!(
            matches!(
                &err,
                ManifestError::BadEnum {
                    field: "ci_status",
                    ..
                } | ManifestError::Parse { .. }
            ),
            "ci_status {bad_value:?} must fail closed, got {err:?}"
        );
    }
}

#[test]
fn empty_owner_and_empty_lists_fail_closed() {
    assert!(matches!(
        validate(&VALID.replace("Director", "")).unwrap_err(),
        ManifestError::EmptyField { field: "owner", .. }
    ));
    assert!(matches!(
        validate(&VALID.replace(
            "[\n        \"AGENTS.md\",\n        \"README.md\",\n        \".wwfd/local-ci.sh\"\n    ]",
            "[]"
        ))
        .unwrap_err(),
        ManifestError::EmptyList { field: "entry_docs", .. }
    ));
    assert!(matches!(
        validate(&VALID.replace("\"cargo test --workspace\"", "\"\"")).unwrap_err(),
        ManifestError::EmptyList {
            field: "canonical_commands",
            ..
        }
    ));
}

#[test]
fn tsv_separators_are_banned_in_free_text_and_lists() {
    assert!(matches!(
        validate(&VALID.replace("Director", "Dir,ector")).unwrap_err(),
        ManifestError::BannedChar { field: "owner", .. }
    ));
    assert!(matches!(
        validate(&VALID.replace("Director", "Dir\\tecor")).unwrap_err(),
        ManifestError::BannedChar { field: "owner", .. }
    ));
    assert!(matches!(
        validate(&VALID.replace("README.md", "READ\\nME.md")).unwrap_err(),
        ManifestError::BannedChar {
            field: "entry_docs",
            ..
        }
    ));
}

#[test]
fn unsafe_names_are_rejected() {
    for name in ["../evil", "a/b", "a\\b", ".hidden", "..", ".", "", "a b"] {
        assert!(
            !safe_name_component(name),
            "{name:?} must not be a safe key"
        );
        let bad = VALID.replace("\"braid\"", &format!("\"{name}\""));
        let err = validate(&bad).unwrap_err();
        assert!(
            matches!(err, ManifestError::UnsafeName { .. }),
            "{name:?} got {err:?}"
        );
    }
    for name in ["braid", "nova-container-runtime", "a.b-c_d9"] {
        assert!(safe_name_component(name));
    }
}

#[test]
fn malformed_json_fails_closed() {
    assert!(matches!(
        validate("{ not json"),
        Err(ManifestError::Parse { .. })
    ));
}

#[test]
fn canonical_form_rejects_smuggled_keys() {
    let m = validate(VALID).unwrap();
    let bytes = m.to_bytes();
    let mut header = bytes.clone();
    header[0] ^= 0x01;
    assert!(RepoManifest::from_bytes(&header).is_err());
    let mut content = bytes.clone();
    let mid = content.len() / 2;
    content[mid] ^= 0x01;
    let tampered = RepoManifest::from_bytes(&content).expect("content flip decodes");
    assert_ne!(tampered, m);
    assert_ne!(tampered.cid(), m.cid());
    let other = validate(&VALID.replace("\"braid\"", "\"braid2\"")).unwrap();
    assert_ne!(m.cid(), other.cid());
}

#[test]
fn inventory_contract() {
    const CID: &str = "aa9481d2b2435bc627a4c66a11ff121576b702f1b66e168df9df39a334f852db";
    let ok = parse_inventory(&format!(
        r#"{{ "braid": "{CID}", "keel": null, "wwfd": "{CID}" }}"#
    ))
    .unwrap();
    assert_eq!(ok.len(), 3);
    assert_eq!(ok[0].name, "braid");
    assert!(ok[0].cid.is_some());
    assert_eq!(ok[1].name, "keel");
    assert!(ok[1].cid.is_none(), "null = declared, not yet admitted");
    assert!(matches!(
        parse_inventory("{}").unwrap_err(),
        ManifestError::EmptyList {
            field: "inventory",
            ..
        }
    ));
    assert!(matches!(
        parse_inventory(r#"{ "a": "not-hex" }"#).unwrap_err(),
        ManifestError::Parse { .. }
    ));
    assert!(matches!(
        parse_inventory(r#"{ "../x": null }"#).unwrap_err(),
        ManifestError::UnsafeName { .. }
    ));
}

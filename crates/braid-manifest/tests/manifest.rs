//! braid-manifest acceptance: the negative matrix (each violation names the
//! field), determinism, canonical round-trip, and the inventory contract.

use braid_manifest::{parse_inventory, safe_name_component, validate, ManifestError, RepoManifest};

const VALID: &str = r#"{
  "name": "braid",
  "archetype": "workspace-crate",
  "owner": "Director",
  "gate_version": "none",
  "ci_status": "green",
  "entry_docs": ["AGENTS.md", "README.md", ".wwfd/local-ci.sh"],
  "canonical_commands": ["cargo test --workspace"],
  "local_ci": true
}"#;

#[test]
fn valid_manifest_round_trips_and_cid_is_deterministic() {
    let a = validate(VALID).unwrap();
    let bytes = a.to_bytes();
    let b = RepoManifest::from_bytes(&bytes).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.cid(), b.cid());
    // Same document twice ⇒ same CID (human-reconstructable).
    assert_eq!(a.cid(), validate(VALID).unwrap().cid());
}

#[test]
fn missing_field_is_named() {
    let bad = r#"{ "name": "x", "archetype": "docs", "owner": "o", "gate_version": "g", "ci_status": "none", "entry_docs": ["a"], "canonical_commands": ["b"] }"#;
    let err = validate(bad).unwrap_err();
    assert!(
        matches!(&err, ManifestError::Parse(m) if m.contains("local_ci")),
        "expected the missing field named, got {err:?}"
    );
}

#[test]
fn unknown_top_level_key_is_rejected() {
    let bad = VALID.replace("\"name\"", "\"namey\"");
    assert!(matches!(
        validate(&bad).unwrap_err(),
        ManifestError::Parse(_)
    ));
}

#[test]
fn archetype_outside_closed_set_is_named() {
    let bad = VALID.replace("workspace-crate", "banana");
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
                } | ManifestError::Parse(_)
            ),
            "ci_status {bad_value:?} must fail closed, got {err:?}"
        );
    }
}

#[test]
fn empty_owner_and_empty_lists_fail_closed() {
    assert!(matches!(
        validate(&VALID.replace("Director", "")).unwrap_err(),
        ManifestError::EmptyField("owner")
    ));
    assert!(matches!(
        validate(&VALID.replace(
            "[\"AGENTS.md\", \"README.md\", \".wwfd/local-ci.sh\"]",
            "[]"
        ))
        .unwrap_err(),
        ManifestError::EmptyList("entry_docs")
    ));
    assert!(matches!(
        validate(&VALID.replace("[\"cargo test --workspace\"]", "[\"\"]")).unwrap_err(),
        ManifestError::EmptyList("canonical_commands")
    ));
}

#[test]
fn tsv_separators_are_banned_in_free_text_and_lists() {
    // JSON strings carry the ESCAPED forms; serde decodes them, then the
    // contract check rejects the decoded separator.
    assert!(matches!(
        validate(&VALID.replace("Director", "Dir,ector")).unwrap_err(),
        ManifestError::BannedChar { field: "owner" }
    ));
    assert!(matches!(
        validate(&VALID.replace("Director", "Dir\\tecor")).unwrap_err(),
        ManifestError::BannedChar { field: "owner" }
    ));
    assert!(matches!(
        validate(&VALID.replace("README.md", "READ\\nME.md")).unwrap_err(),
        ManifestError::BannedChar {
            field: "entry_docs"
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
            matches!(err, ManifestError::UnsafeName(_)),
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
        Err(ManifestError::Parse(_))
    ));
}

#[test]
fn canonical_form_rejects_smuggled_keys() {
    let m = validate(VALID).unwrap();
    let bytes = m.to_bytes();
    // Structural flip in the CBOR header must fail strict decode.
    let mut header = bytes.clone();
    header[0] ^= 0x01;
    assert!(RepoManifest::from_bytes(&header).is_err());
    // A content flip (mid-file, inside a text value) legitimately decodes as
    // a DIFFERENT valid document with a different CID — the codec cannot
    // detect content tampering; the store's inventory pin does (covered by
    // the braid-cli catalog integration tests).
    let mut content = bytes.clone();
    let mid = content.len() / 2;
    content[mid] ^= 0x01;
    let tampered = RepoManifest::from_bytes(&content).expect("content flip decodes");
    assert_ne!(tampered, m);
    assert_ne!(tampered.cid(), m.cid());
    // A second valid document differs in bytes ⇒ different CID.
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
        ManifestError::EmptyList("inventory")
    ));
    assert!(matches!(
        parse_inventory(r#"{ "a": "not-hex" }"#).unwrap_err(),
        ManifestError::Parse(_)
    ));
    assert!(matches!(
        parse_inventory(r#"{ "../x": null }"#).unwrap_err(),
        ManifestError::UnsafeName(_)
    ));
}

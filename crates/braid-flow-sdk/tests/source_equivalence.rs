//! Must-fail equivalence: unknown fields and semantic-loss JSON refuse, not silently drop.
//! T5.2 — `cargo test -p braid-flow-sdk --test source_equivalence -- unknown_and_lossy_sources_refuse`

use braid_flow_sdk::validate_json_source;

#[test]
fn unknown_and_lossy_sources_refuse() {
    // Unknown field must refuse closed (deny_unknown_fields, not silently dropped).
    let unknown = br#"{"name": "test", "nodes": [], "unknown_field": 123}"#;
    let err = validate_json_source(unknown);
    assert!(
        err.is_err(),
        "unknown field must be refused, got ok: {err:?}"
    );

    // Semantic-loss marker must refuse (explicit __lossy sentinel).
    let lossy = br#"{"name": "test", "nodes": [], "__lossy": true}"#;
    let err = validate_json_source(lossy);
    assert!(
        err.is_err(),
        "lossy sentinel must be refused, got ok: {err:?}"
    );

    // Semantic loss via second marker.
    let lossy2 = br#"{"name": "test", "semantic_loss": {"field": "x"}}"#;
    assert!(
        validate_json_source(lossy2).is_err(),
        "semantic_loss marker must be refused"
    );

    // Float where int is expected is also semantic loss (serde_json Number is_f64).
    let lossy_float = br#"{"name": "test", "nodes": [1.5]}"#;
    assert!(
        validate_json_source(lossy_float).is_err(),
        "float semantic loss must be refused"
    );

    // Valid JSON with only allowed fields must pass.
    let valid = br#"{"name": "test", "nodes": []}"#;
    assert!(
        validate_json_source(valid).is_ok(),
        "valid source should be admitted"
    );

    // Valid capsule-adjacent interop (inspection) also passes.
    let valid2 = br#"{"intent": "demo", "strands": [], "outputs": []}"#;
    assert!(
        validate_json_source(valid2).is_ok(),
        "valid capsule json should be admitted"
    );
}

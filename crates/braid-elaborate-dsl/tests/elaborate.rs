use braid_elaborate_dsl::{ErrorCode, MAX_SOURCE_BYTES, elaborate};
use braid_test_support::proptest::{self, prelude::*};

const EDIT: &str = include_str!("fixtures/edit-home-hero.brd");
const PUBLISH: &str = include_str!("fixtures/publish-services.brd");
const LISTING: &str = include_str!("fixtures/render-work-listing.brd");

fn assert_error(source: &str, code: ErrorCode) {
    let error = elaborate(source).expect_err("source must fail closed");
    assert_eq!(error.code, code, "unexpected error: {error}");
}

#[test]
fn demo_port_sources_pin_existing_json_path_cids() {
    let cases = [
        (
            EDIT,
            "26f6162e1a5fb5f6e3a46724a991a8b4dc48e08c223016fb86c3c8de38594226",
        ),
        (
            PUBLISH,
            "195a7455e56b42dde32a79218c1b675420bc2de310184d16413a027d6261f33a",
        ),
        (
            LISTING,
            "43ed11f1d863a214ca4ac51bbfaa5078166ddcc228fda717c25fa358bf8da3a6",
        ),
    ];
    for (source, expected) in cases {
        let result = elaborate(source).expect("demo source must admit");
        assert_eq!(result.capsule.cid().to_hex(), expected);
        assert_eq!(result.bytes, result.capsule.to_bytes());
        assert!(result.manifest_text.contains(expected));
    }
}

#[test]
fn same_source_is_byte_deterministic() {
    let first = elaborate(EDIT).unwrap();
    let second = elaborate(EDIT).unwrap();
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.json_ir, second.json_ir);
}

#[test]
fn pipeline_and_explicit_call_are_identical() {
    let piped = LISTING;
    let explicit = LISTING.replace("entity |> proj::listing()", "proj::listing(entity)");
    assert_eq!(
        elaborate(piped).unwrap().bytes,
        elaborate(&explicit).unwrap().bytes
    );
}

#[test]
fn ten_golden_sources_have_pinned_cids() {
    let templates = [
        EDIT.to_owned(),
        PUBLISH.to_owned(),
        LISTING.to_owned(),
        EDIT.replace("edit_home_hero", "edit_home_hero_copy"),
        EDIT.replace("render a preview", "render a second preview"),
        LISTING.replace("render_work_listing", "render_work_listing_copy"),
        LISTING.replace("work case-study", "featured case-study"),
        PUBLISH.replace("publish_services", "publish_services_copy"),
        PUBLISH.replace("services page", "pricing page"),
        EDIT.replace("home hero", "about hero"),
    ];
    let expected = [
        "26f6162e1a5fb5f6e3a46724a991a8b4dc48e08c223016fb86c3c8de38594226",
        "195a7455e56b42dde32a79218c1b675420bc2de310184d16413a027d6261f33a",
        "43ed11f1d863a214ca4ac51bbfaa5078166ddcc228fda717c25fa358bf8da3a6",
        "26f6162e1a5fb5f6e3a46724a991a8b4dc48e08c223016fb86c3c8de38594226",
        "42354a2dd8aabf9a254f1ca3974db9d5a4647c7a0c7ec3133f2811d3bade0088",
        "43ed11f1d863a214ca4ac51bbfaa5078166ddcc228fda717c25fa358bf8da3a6",
        "b1bb95bdf288e56a788a4581eb5b8e16175d14c2537a70c5ce2f6272b71038d9",
        "195a7455e56b42dde32a79218c1b675420bc2de310184d16413a027d6261f33a",
        "0eccba2d5f6240b4792bd14510d4305b3bb392ab6a43a78c89ab0da83213c4ff",
        "d4e98163b06098ae04e215a322973fb02fce0ebe77d0bf1b0019ffd185551d83",
    ];
    for (source, expected_cid) in templates.iter().zip(expected) {
        assert_eq!(
            elaborate(source).unwrap().capsule.cid().to_hex(),
            expected_cid
        );
    }
}

#[test]
fn unknown_term_is_typed_refusal() {
    assert_error(
        &EDIT.replace("view::section", "view::eval"),
        ErrorCode::UnknownTerm,
    );
}

#[test]
fn authority_widening_is_typed_refusal() {
    assert_error(
        &EDIT.replace(
            "capabilities [signal::emit]",
            "capabilities [signal::emit, compute::remote]",
        ),
        ErrorCode::CapabilityMismatch,
    );
}

#[test]
fn missing_derived_capability_is_typed_refusal() {
    assert_error(
        &EDIT.replace("capabilities [signal::emit]", "capabilities []"),
        ErrorCode::CapabilityMismatch,
    );
}

#[test]
fn hidden_effect_is_typed_refusal() {
    assert_error(
        &EDIT.replace("effects [pure, reversible_write]", "effects [pure]"),
        ErrorCode::EffectMismatch,
    );
}

#[test]
fn unconfirmed_publish_is_refused_before_emission() {
    assert_error(
        &PUBLISH.replace("confirm human", "confirm none"),
        ErrorCode::BuildRejected,
    );
}

#[test]
fn duplicate_binding_is_typed_refusal() {
    assert_error(
        &EDIT.replace("text = lit::text();", "entity = lit::text();"),
        ErrorCode::DuplicateBinding,
    );
}

#[test]
fn forward_binding_is_typed_refusal() {
    assert_error(
        &EDIT.replace("entity = lit::entity();", "entity = bytes::id(future);"),
        ErrorCode::UnknownBinding,
    );
}

#[test]
fn unsupported_state_is_loud_refusal() {
    assert_error(
        &EDIT.replace("step main", "state Session {}\nstep main"),
        ErrorCode::UnsupportedConstruct,
    );
}

#[test]
fn floats_are_not_source_values() {
    assert_error(
        &EDIT.replace("lit::text()", "lit::text(3.14)"),
        ErrorCode::InvalidToken,
    );
}

#[test]
fn unsupported_registry_is_typed_refusal() {
    assert_error(
        &EDIT.replace("registry cms::v1", "registry web::v1"),
        ErrorCode::UnsupportedRegistry,
    );
}

#[test]
fn oversized_source_is_refused_before_parsing() {
    let source = "x".repeat(MAX_SOURCE_BYTES + 1);
    assert_error(&source, ErrorCode::SourceTooLarge);
}

#[test]
fn excessive_pipeline_work_is_bounded() {
    let mut calls = String::from("bytes::id(seed)");
    for _ in 0..65 {
        calls.push_str(" |> bytes::id()");
    }
    let source = format!(
        "capsule test::bounded version 0 {{\n\
         intent \"bounded\"; registry cms::v1;\n\
         require {{ capabilities []; effects [pure]; }}\n\
         step main {{ seed = lit::bytes(); result = {calls}; output [result]; }}\n\
         }}"
    );
    assert_error(&source, ErrorCode::LimitExceeded);
}

#[test]
fn duplicate_requirements_are_refused() {
    assert_error(
        &EDIT.replace(
            "capabilities [signal::emit]",
            "capabilities [signal::emit, signal::emit]",
        ),
        ErrorCode::DuplicateField,
    );
}

#[test]
fn embedded_code_and_raw_urls_are_not_expressions() {
    assert_error(
        &EDIT.replace("lit::text()", "eval::javascript(text)"),
        ErrorCode::UnknownTerm,
    );
    assert_error(
        &EDIT.replace("lit::text()", "net::fetch(\"https://example.invalid\")"),
        ErrorCode::UnexpectedToken,
    );
}

#[test]
fn raw_newlines_inside_strings_are_refused() {
    assert_error(
        &EDIT.replace("render a preview", "render a\npreview"),
        ErrorCode::InvalidToken,
    );
}

proptest! {
    #[test]
    fn arbitrary_utf8_source_never_panics(
        source in proptest::collection::vec(any::<char>(), 0..1_024)
            .prop_map(|chars| chars.into_iter().collect::<String>())
    ) {
        // Success and typed refusal are both valid; process survival is the
        // property under test, so consume the outcome explicitly.
        drop(elaborate(&source));
    }
}

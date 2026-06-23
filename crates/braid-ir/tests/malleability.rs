//! T3 — the A4.8 exploit set, replayed against the canonical decoder: every
//! non-canonical byte form is a REJECT, never a normalize-and-accept.

use braid_ir::{decode_strict, encode, CanonError, Value};

#[test]
fn non_minimal_int_head_rejected() {
    // 5 encoded with a one-byte argument (0x18 0x05) instead of inline 0x05.
    assert_eq!(decode_strict(&[0x18, 0x05]), Err(CanonError::NonMinimalInt));
    // Canonical form decodes.
    assert_eq!(decode_strict(&[0x05]), Ok(Value::Int(5)));
}

#[test]
fn indefinite_length_rejected() {
    // 0x9f = indefinite-length array.
    assert!(matches!(
        decode_strict(&[0x9f, 0x01, 0xff]),
        Err(CanonError::ForbiddenForm(_))
    ));
}

#[test]
fn floats_do_not_exist() {
    // f16/f32/f64 heads — the universe has no float (T8/D8).
    for head in [0xf9u8, 0xfa, 0xfb] {
        let mut bytes = vec![head];
        bytes.extend([0u8; 8]);
        assert!(
            matches!(
                decode_strict(&bytes),
                Err(CanonError::ForbiddenForm(_)) | Err(CanonError::TrailingBytes)
            ),
            "float head {head:#x} must be rejected"
        );
    }
}

#[test]
fn tags_and_null_rejected() {
    assert!(matches!(
        decode_strict(&[0xc0, 0x00]),
        Err(CanonError::ForbiddenForm(_))
    )); // tag
    assert!(matches!(
        decode_strict(&[0xf6]),
        Err(CanonError::ForbiddenForm(_))
    )); // null
    assert!(matches!(
        decode_strict(&[0xf7]),
        Err(CanonError::ForbiddenForm(_))
    )); // undefined
}

#[test]
fn trailing_junk_rejected() {
    // A valid int followed by padding — junk-padded artifact (A4.8 case).
    assert_eq!(decode_strict(&[0x01, 0x00]), Err(CanonError::TrailingBytes));
}

#[test]
fn unordered_or_duplicate_map_keys_rejected() {
    // {"b":1,"a":2} — keys out of canonical order.
    let unordered = [0xa2, 0x61, b'b', 0x01, 0x61, b'a', 0x02];
    assert_eq!(decode_strict(&unordered), Err(CanonError::KeyOrder));
    // {"a":1,"a":2} — duplicate.
    let dup = [0xa2, 0x61, b'a', 0x01, 0x61, b'a', 0x02];
    assert_eq!(decode_strict(&dup), Err(CanonError::KeyOrder));
}

#[test]
fn length_first_key_order_is_the_canon() {
    // {"z":1,"aa":2} is canonical (len 1 < len 2) even though "aa" < "z"
    // bytewise — pins the RFC 8949 deterministic order choice.
    let canonical = [0xa2, 0x61, b'z', 0x01, 0x62, b'a', b'a', 0x02];
    assert!(decode_strict(&canonical).is_ok());
    let wrong = [0xa2, 0x62, b'a', b'a', 0x02, 0x61, b'z', 0x01];
    assert_eq!(decode_strict(&wrong), Err(CanonError::KeyOrder));
}

#[test]
fn forged_collection_length_rejected_before_allocation() {
    // Array head claiming 2^32 items with 1 byte of payload.
    assert_eq!(
        decode_strict(&[0x9a, 0xff, 0xff, 0xff, 0xff, 0x01]),
        Err(CanonError::Truncated)
    );
}

#[test]
fn truncated_input_rejected() {
    assert_eq!(decode_strict(&[0x62, b'a']), Err(CanonError::Truncated));
}

#[test]
fn non_text_map_key_rejected() {
    // {1: 2} — int key.
    assert!(matches!(
        decode_strict(&[0xa1, 0x01, 0x02]),
        Err(CanonError::ForbiddenForm(_))
    ));
}

#[test]
fn invalid_utf8_text_rejected() {
    assert_eq!(decode_strict(&[0x61, 0xff]), Err(CanonError::Utf8));
}

#[test]
fn encode_decode_is_identity_on_canonical_values() {
    let v = Value::map(vec![
        ("a", Value::Int(-42)),
        (
            "b",
            Value::List(vec![Value::Bool(true), Value::Bytes(vec![1, 2, 3])]),
        ),
        ("cc", Value::Text("héllo".into())),
    ]);
    let bytes = encode(&v);
    assert_eq!(decode_strict(&bytes), Ok(v));
}

#[test]
fn capsule_grant_order_malleability_rejected() {
    use braid_ir::{Capsule, Value};
    // Take the canonical example, swap grant order at the VALUE level, and
    // confirm the struct parser refuses it (grant-order is canonical too).
    let capsule = braid_vocab_cms::publish_capsule(braid_ir::ConfirmPolicy::HumanConfirm);
    let mut v = capsule.to_canon();
    if let Value::Map(m) = &mut v {
        let grants = m.get_mut("grants").unwrap();
        if let Value::List(items) = grants {
            items.swap(0, 1);
        }
    }
    assert!(Capsule::from_canon(&v).is_err());
}

#[test]
fn capsule_unknown_extra_field_rejected() {
    use braid_ir::{Capsule, Value};
    let capsule = braid_vocab_cms::edit_section_capsule();
    let mut v = capsule.to_canon();
    if let Value::Map(m) = &mut v {
        m.insert("zz_smuggle".into(), Value::Int(1));
    }
    assert!(Capsule::from_canon(&v).is_err());
}

// ── U9 finding: nested-sub-map malleability (was High; now closed) ──
// An extra key smuggled into ANY nested map (braid, a strand, the registry,
// a term) produced distinct bytes that still parsed — the bytes↔Value
// bijection guard passed while the Value→struct projection dropped the key,
// so the CID committed to re-encoded clean bytes, not the admitted bytes.
// These pin every level closed and assert the round-trip identity that the
// projection must uphold.

/// Helper: smuggle a key into the nested map reachable by `path` and confirm
/// the capsule no longer parses.
fn assert_nested_smuggle_rejected(mutate: impl Fn(&mut braid_ir::Value)) {
    use braid_ir::Capsule;
    let capsule = braid_vocab_cms::edit_section_capsule();
    let mut v = capsule.to_canon();
    mutate(&mut v);
    let bytes = braid_ir::canon::encode(&v);
    // Bytes are still individually canonical (the Value round-trips)…
    assert!(
        braid_ir::decode_strict(&bytes).is_ok(),
        "value-level canonical"
    );
    // …but the capsule projection must REJECT the unknown nested field.
    assert!(
        Capsule::from_bytes(&bytes).is_err(),
        "nested smuggled key must be rejected, not silently dropped"
    );
}

#[test]
fn smuggled_key_in_braid_map_rejected() {
    use braid_ir::Value;
    assert_nested_smuggle_rejected(|v| {
        if let Value::Map(top) = v {
            if let Some(Value::Map(braid)) = top.get_mut("braid") {
                braid.insert("zz".into(), Value::Int(7));
            }
        }
    });
}

#[test]
fn smuggled_key_in_strand_map_rejected() {
    use braid_ir::Value;
    assert_nested_smuggle_rejected(|v| {
        if let Value::Map(top) = v {
            if let Some(Value::Map(braid)) = top.get_mut("braid") {
                if let Some(Value::List(strands)) = braid.get_mut("strands") {
                    if let Some(Value::Map(s0)) = strands.get_mut(0) {
                        s0.insert("zz".into(), Value::Int(7));
                    }
                }
            }
        }
    });
}

#[test]
fn smuggled_key_in_registry_term_rejected() {
    use braid_ir::{TermRegistry, Value};
    let reg = braid_vocab_cms::registry_v0();
    let mut v = reg.to_canon();
    if let Value::Map(top) = &mut v {
        if let Some(Value::List(terms)) = top.get_mut("terms") {
            if let Some(Value::Map(t0)) = terms.get_mut(0) {
                t0.insert("zz".into(), Value::Int(7));
            }
        }
    }
    let bytes = braid_ir::canon::encode(&v);
    assert!(braid_ir::decode_strict(&bytes).is_ok());
    let decoded = braid_ir::decode_strict(&bytes).unwrap();
    assert!(TermRegistry::from_canon(&decoded).is_err());
}

#[test]
fn smuggled_key_in_registry_top_map_rejected() {
    use braid_ir::{TermRegistry, Value};
    let mut v = braid_vocab_cms::registry_v0().to_canon();
    if let Value::Map(top) = &mut v {
        top.insert("zz".into(), Value::Int(7));
    }
    assert!(TermRegistry::from_canon(&v).is_err());
}

/// The invariant the projection must always uphold: for any byte string the
/// capsule parser ACCEPTS, re-encoding the parsed struct reproduces the exact
/// input bytes. No accepted artifact can carry bytes the CID doesn't commit.
#[test]
fn accepted_capsule_bytes_round_trip_identically() {
    use braid_ir::Capsule;
    for capsule in [
        braid_vocab_cms::edit_section_capsule(),
        braid_vocab_cms::publish_capsule(braid_ir::ConfirmPolicy::HumanConfirm),
        braid_vocab_cms::laundering_capsule(),
    ] {
        let bytes = capsule.to_bytes();
        let parsed = Capsule::from_bytes(&bytes).expect("canonical");
        assert_eq!(parsed.to_bytes(), bytes, "round-trip identity");
        assert_eq!(parsed.cid(), capsule.cid());
    }
}

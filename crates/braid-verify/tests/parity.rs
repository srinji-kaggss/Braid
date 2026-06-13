//! D9 / scenario #13 — the two independent serialization implementations
//! (braid-ir's authoring codec, braid-verify's admission codec) must agree on
//! every vector and every constructible artifact. Any disagreement is RED:
//! a parse differential here is exactly the trusting-trust gap (T2).

use braid_ir::examples::{edit_section_capsule, laundering_capsule, publish_capsule};
use braid_ir::{registry_v0, ConfirmPolicy};
use braid_verify::decode::{decode_canonical, reencode};
use std::fs;
use std::path::PathBuf;

fn cases() -> Vec<Vec<u8>> {
    vec![
        edit_section_capsule().to_bytes(),
        publish_capsule(ConfirmPolicy::HumanConfirm).to_bytes(),
        publish_capsule(ConfirmPolicy::None).to_bytes(),
        laundering_capsule().to_bytes(),
        braid_ir::canon::encode(&registry_v0().to_canon()),
    ]
}

#[test]
fn independent_decoders_agree_on_all_examples() {
    for bytes in cases() {
        // verify-side strict decode of ir-side bytes…
        let v = decode_canonical(&bytes).expect("verify decoder accepts ir encoder output");
        // …re-encodes to the identical bytes (independent encoder),
        let mut re = Vec::new();
        reencode(&v, &mut re);
        assert_eq!(re, bytes, "encoder/decoder parity broken");
        // …and the ir-side strict decoder agrees on the value.
        assert_eq!(braid_ir::decode_strict(&bytes).unwrap(), v);
    }
}

#[test]
fn parity_on_the_pinned_kat_vector() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/braid/vectors/capsule_v0.kat");
    let raw = fs::read_to_string(path).expect("KAT vectors present");
    let hex_bytes = raw
        .lines()
        .find_map(|l| l.strip_prefix("capsule_bytes_hex = "))
        .expect("capsule_bytes_hex line");
    let bytes = hex::decode(hex_bytes.trim()).unwrap();
    let v = decode_canonical(&bytes).expect("pinned vector decodes");
    let mut re = Vec::new();
    reencode(&v, &mut re);
    assert_eq!(re, bytes);
}

#[test]
fn verify_decoder_rejects_the_malleability_set() {
    use braid_verify::decode::DecodeError;
    assert_eq!(decode_canonical(&[0x18, 0x05]), Err(DecodeError::NonMinimal));
    assert!(matches!(decode_canonical(&[0xf6]), Err(DecodeError::Forbidden(_))));
    assert!(matches!(decode_canonical(&[0xfb, 0, 0, 0, 0, 0, 0, 0, 0]), Err(DecodeError::Forbidden(_))));
    assert_eq!(decode_canonical(&[0x01, 0x00]), Err(DecodeError::Trailing));
    let unordered = [0xa2, 0x61, b'b', 0x01, 0x61, b'a', 0x02];
    assert_eq!(decode_canonical(&unordered), Err(DecodeError::KeyOrder));
}

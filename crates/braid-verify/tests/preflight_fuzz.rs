use braid_test_support::proptest::{self, prelude::*};
use braid_verify::decode::{
    DecodeError, LimitKind, MAX_VALUE_NODES, MAX_WIRE_BYTES, decode_canonical, preflight_canonical,
    reencode,
};
use std::collections::BTreeMap;

fn canonical_value() -> impl Strategy<Value = braid_ir::Value> {
    let leaf = prop_oneof![
        any::<bool>().prop_map(braid_ir::Value::Bool),
        any::<i64>().prop_map(braid_ir::Value::Int),
        proptest::collection::vec(any::<u8>(), 0..32).prop_map(braid_ir::Value::Bytes),
        "[a-zA-Z0-9 _.\\-]{0,24}".prop_map(braid_ir::Value::Text),
    ];
    leaf.prop_recursive(5, 256, 8, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..8).prop_map(braid_ir::Value::List),
            proptest::collection::btree_map("[a-z]{0,8}", inner, 0..8).prop_map(
                |entries: BTreeMap<String, braid_ir::Value>| { braid_ir::Value::Map(entries) }
            ),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_bounded_wire_never_panics_or_bypasses_owned_decode(
        bytes in proptest::collection::vec(any::<u8>(), 0..=64 * 1024)
    ) {
        if preflight_canonical(&bytes).is_ok() {
            let value = decode_canonical(&bytes)
                .expect("bytes accepted by preflight must survive bounded owned decode");
            let mut encoded = Vec::with_capacity(bytes.len());
            reencode(&value, &mut encoded);
            prop_assert_eq!(encoded, bytes);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn canonical_nested_values_preserve_independent_byte_bijection(value in canonical_value()) {
        let bytes = braid_ir::encode(&value);
        let stats = preflight_canonical(&bytes)
            .expect("producer output within the generated envelope must pass preflight");
        let decoded = decode_canonical(&bytes)
            .expect("preflight-approved producer output must survive owned decode");
        prop_assert_eq!(decoded, value);
        prop_assert_eq!(stats.wire_bytes, bytes.len());
    }
}

#[test]
fn hard_wire_ceiling_is_refused_before_scanning() {
    let bytes = vec![0; MAX_WIRE_BYTES + 1];
    assert!(matches!(
        preflight_canonical(&bytes),
        Err(DecodeError::LimitExceeded {
            kind: LimitKind::WireBytes,
            actual,
            limit,
            at: 0,
        }) if actual == (MAX_WIRE_BYTES + 1) as u64 && limit == MAX_WIRE_BYTES as u64
    ));
}

#[test]
fn owned_decode_cannot_bypass_the_value_node_preflight() {
    let item_count = MAX_VALUE_NODES;
    let mut bytes = vec![0x9a];
    bytes.extend_from_slice(&(item_count as u32).to_be_bytes());
    bytes.resize(bytes.len() + item_count as usize, 0xf4);

    assert!(matches!(
        decode_canonical(&bytes),
        Err(DecodeError::LimitExceeded {
            kind: LimitKind::ValueNodes,
            actual,
            limit: MAX_VALUE_NODES,
            ..
        }) if actual == MAX_VALUE_NODES + 1
    ));
}

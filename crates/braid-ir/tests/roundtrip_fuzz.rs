//! Bijection fuzz (T3): for every constructible value, encode→decode is
//! identity and the bytes are accepted by the strict decoder. (The reverse
//! direction — arbitrary bytes — is covered by malleability.rs rejects and
//! the decoder's strictness.)

use braid_ir::{Value, decode_strict, encode};
use braid_test_support::proptest::{self, prelude::*};
use std::collections::BTreeMap;

fn value_strategy() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::Int),
        proptest::collection::vec(any::<u8>(), 0..32).prop_map(Value::Bytes),
        "[a-zA-Z0-9 _.\\-]{0,24}".prop_map(Value::Text),
    ];
    leaf.prop_recursive(4, 64, 8, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..6).prop_map(Value::List),
            proptest::collection::btree_map("[a-z]{1,8}", inner, 0..6)
                .prop_map(|m: BTreeMap<String, Value>| Value::Map(m)),
        ]
    })
}

proptest! {
    #[test]
    fn encode_then_strict_decode_is_identity(v in value_strategy()) {
        let bytes = encode(&v);
        prop_assert_eq!(decode_strict(&bytes).unwrap(), v);
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        // Fail-closed means *reject*, never crash, on garbage.
        let _ = decode_strict(&bytes);
    }
}

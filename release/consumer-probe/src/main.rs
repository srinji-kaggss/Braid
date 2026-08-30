const REGISTRY_CID: &str = "afaa7dcc9ab2f7d1530da72306c7d821c573f8aa9eda2b5c6e463f121f634acd";
const CAPSULE_CID: &str = "ccedc469e6b0513720969ce1a4f169f53365eeadbc853042c411b44c1f15b71f";

fn main() {
    let registry = braid_vocab_cms::registry_v0();
    assert_eq!(registry.cid().to_hex(), REGISTRY_CID);

    let capsule = braid_vocab_cms::edit_section_capsule();
    assert_eq!(capsule.cid().to_hex(), CAPSULE_CID);

    let bytes = capsule.to_bytes();
    let ambient = vec![braid_vocab_cms::cap!(
        braid_vocab_cms::SIGNAL_EMIT_NAME
    )];
    assert_eq!(
        braid_verify::verify(&bytes, &registry, &ambient),
        braid_verify::Verdict::Admit {
            capsule_cid: capsule.cid(),
        }
    );

    let web_registry = braid_vocab_web::registry_v0().expect("web registry is valid");
    assert!(!web_registry.is_empty());

    println!("registry_cid={REGISTRY_CID}");
    println!("capsule_cid={CAPSULE_CID}");
    println!("verdict=admit");
}

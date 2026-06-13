//! The v0 demo alphabet (D16: landing-port flavored, frontend-first).
//!
//! //why this exact shape: the v0 vocabulary is almost entirely pure render +
//! projection reads — the safest first alphabet (nothing irreversible is
//! expressible without `cms.publish`, the single deliberate escalation
//! probe). `vault.read` + `net.egress` exist primarily so the taint
//! trip-wire (T5) has real teeth from day one.

use crate::term::{EffectClass, Exposure, TermRegistry, TermSpec, TypeTag};
use braid_capability::Capability;

/// Pinned to `canvas_syscall::vocabulary::VOCABULARY_VERSION` by
/// `tests/vocab_binding.rs`.
pub const VOCAB_VERSION: u32 = 1;

// A table-row constructor: one positional row per term keeps the registry
// readable as a table; a builder would bury the columns.
#[allow(clippy::too_many_arguments)]
fn t(
    id: &str,
    inputs: Vec<TypeTag>,
    output: TypeTag,
    capability: Option<Capability>,
    effect: EffectClass,
    source_exposure: Exposure,
    egress_ceiling: Option<Exposure>,
    cost: u64,
) -> TermSpec {
    TermSpec {
        id: id.into(),
        inputs,
        output,
        capability,
        effect,
        source_exposure,
        egress_ceiling,
        cost,
    }
}

/// Build the v0 registry. Infallible by construction — the specs below are
/// validated by `TermRegistry::insert` and a unit test pins the build.
pub fn registry_v0() -> TermRegistry {
    use EffectClass::*;
    use Exposure::*;
    use TypeTag::*;

    let specs = vec![
        // ── pure literals + math (the chainable alphabet) ──
        t("lit.text", vec![], Text, None, Pure, Public, None, 1),
        t("lit.bytes", vec![], Bytes, None, Pure, Public, None, 1),
        t("lit.entity", vec![], Entity, None, Pure, Public, None, 1),
        t("text.concat", vec![Text, Text], Text, None, Pure, Public, None, 1),
        t("bytes.id", vec![Bytes], Bytes, None, Pure, Public, None, 1),
        // ── pure render (output is a typed Directive, never DOM — D16) ──
        t("view.section", vec![Text], Directive, None, Pure, Public, None, 2),
        t(
            "view.page",
            vec![List(Box::new(Directive))],
            Directive,
            None,
            Pure,
            Public,
            None,
            3,
        ),
        // ── projection reads ──
        t(
            "proj.listing",
            vec![Entity],
            List(Box::new(Text)),
            Some(Capability::TapeRead),
            Read,
            Internal,
            None,
            5,
        ),
        t(
            "vault.read",
            vec![Entity],
            Bytes,
            Some(Capability::TapeRead),
            Read,
            Vault,
            None,
            5,
        ),
        // ── CMS writes (the landing-port verbs) ──
        t(
            "cms.edit_section",
            vec![Entity, Text],
            Cid,
            Some(Capability::SignalEmit),
            ReversibleWrite,
            Internal,
            None,
            8,
        ),
        t(
            "cms.publish",
            vec![Cid],
            Cid,
            Some(Capability::IntentEmit),
            Irreversible,
            Internal,
            Some(Internal),
            13,
        ),
        // ── the single egress door of the demo alphabet ──
        t(
            "net.egress",
            vec![Bytes],
            Cid,
            Some(Capability::RemoteCompute),
            Egress,
            Internal,
            Some(Internal),
            21,
        ),
    ];

    let mut reg = TermRegistry::new(VOCAB_VERSION);
    for spec in specs {
        reg.insert(spec).expect("registry_v0 specs are statically valid");
    }
    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_v0_builds_and_is_closed() {
        let r = registry_v0();
        assert_eq!(r.len(), 12);
        assert!(r.get("net.egress").is_some());
        assert!(r.get("eval").is_none());
    }

    #[test]
    fn registry_round_trips_canonically() {
        let r = registry_v0();
        let bytes = crate::canon::encode(&r.to_canon());
        let v = crate::canon::decode_strict(&bytes).expect("canonical");
        let r2 = TermRegistry::from_canon(&v).expect("decodes");
        assert_eq!(r, r2);
        assert_eq!(r.cid(), r2.cid());
    }
}

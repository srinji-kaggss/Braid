//! braid-vocab-cms — the kernel/landing-port CMS vocabulary (ADR-088 D7, D16).
//!
//! The first **vocabulary package** over the Braid global IR (D31). A
//! vocabulary package owns its term registry + its capability space; it
//! depends on the substrate (`braid-ir` + `braid-capability`) and nothing
//! domain-specific lives in the substrate. Other packages (`braid-vocab-js`,
//! …) declare their own registries and capability names the same way.
//!
//! This package is the Day-0 CMS demo alphabet (D16: landing-port flavored,
//! frontend-first): pure render + projection reads, with `cms.publish` the
//! single deliberate irreversible escalation probe. The capability names are
//! the kernel's protocol-stable dotted identifiers (`signal.emit`,
//! `compute.remote`, …) — the exact strings the kernel's
//! `braid_vocab_binding.rs` snapshot mirrors, so a kernel consumer binds
//! against this package without a code dependency on the kernel.

use braid_capability::Capability;
use braid_ir::term::{EffectClass, Exposure, TermRegistry, TermSpec, TypeTag};

/// `TypeTag::Opaque("cms.entity", [])` — a reference to a governed work
/// object (page / section / …). Vocabulary-owned domain type (D31).
pub fn entity() -> TypeTag {
    TypeTag::Opaque("cms.entity".into(), Vec::new())
}

/// `TypeTag::Opaque("cms.directive", [])` — a typed render directive
/// (ViewDirective/MotionDirective family — D16: render output is ALWAYS
/// this, never DOM/HTML strings). Vocabulary-owned domain type (D31).
pub fn directive() -> TypeTag {
    TypeTag::Opaque("cms.directive".into(), Vec::new())
}

/// Vocabulary version — pinned to the kernel `canvas_syscall::VOCABULARY_VERSION`
/// by the kernel-side binding test (`canvas-syscall/tests/braid_vocab_binding.rs`).
/// A kernel vocabulary bump is a Braid registry event, never silent drift (D11).
pub const VOCAB_VERSION: u32 = 1;

// ── capability constants (the kernel's dotted names, protocol-stable) ──
// //why named consts on a string-tagged Capability (D31): the dotted name IS
// the identity, so the name lives in ONE place. A rename here is a conscious,
// CID-breaking event that must re-pin the kernel snapshot. `cap!` wraps the
// &'static str into the `Capability` newtype at each use site (the newtype
// holds an owned String; a const fn can't allocate, so we wrap rather than
// store a const Capability).
/// Wrap a `&'static str` dotted name into a `Capability`. A string-tagged
/// `Capability` (D31) holds an owned `String`; a `const fn` can't allocate, so
/// vocabulary packages declare the names as `pub const …_NAME: &str` and wrap
/// at use sites with this macro.
#[macro_export]
macro_rules! cap {
    ($name:expr) => {
        $crate::wrap_cap($name)
    };
}

/// `cap!` backing — public so the macro resolves from any crate.
pub fn wrap_cap(name: &'static str) -> Capability {
    Capability::new(name)
}

pub const SIGNAL_EMIT_NAME: &str = "signal.emit";
pub const SIGNAL_SUBSCRIBE_NAME: &str = "signal.subscribe";
pub const TAPE_READ_NAME: &str = "tape.read";
pub const VIEW_INJECT_NAME: &str = "view.inject";
pub const INTENT_EMIT_NAME: &str = "intent.emit";
pub const MOTION_SCHEDULE_NAME: &str = "motion.schedule";
pub const MOTION_OBSERVE_NAME: &str = "motion.observe";
pub const MOTION_PATCH_NAME: &str = "motion.patch";
pub const MOTION_PLUGIN_REGISTER_NAME: &str = "motion.plugin.register";
pub const MOTION_REPLAY_NAME: &str = "motion.replay";
pub const SHRED_NAME: &str = "efface.shred";
pub const RTBF_NAME: &str = "efface.rtbf";
pub const REMOTE_COMPUTE_NAME: &str = "compute.remote";

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

/// Build the v0 CMS registry. Infallible by construction — the specs are
/// validated by `TermRegistry::insert` and a unit test pins the build.
pub fn registry_v0() -> TermRegistry {
    use EffectClass::*;
    use Exposure::*;
    use TypeTag::*;

    let specs = vec![
        // ── pure literals + math (the chainable alphabet) ──
        t("lit.text", vec![], Text, None, Pure, Public, None, 1),
        t("lit.bytes", vec![], Bytes, None, Pure, Public, None, 1),
        t("lit.entity", vec![], entity(), None, Pure, Public, None, 1),
        t(
            "text.concat",
            vec![Text, Text],
            Text,
            None,
            Pure,
            Public,
            None,
            1,
        ),
        t("bytes.id", vec![Bytes], Bytes, None, Pure, Public, None, 1),
        // ── pure render (output is a typed Directive, never DOM — D16) ──
        t(
            "view.section",
            vec![Text],
            directive(),
            None,
            Pure,
            Public,
            None,
            2,
        ),
        t(
            "view.page",
            vec![List(Box::new(directive()))],
            directive(),
            None,
            Pure,
            Public,
            None,
            3,
        ),
        // ── projection reads ──
        t(
            "proj.listing",
            vec![entity()],
            List(Box::new(Text)),
            Some(cap!(TAPE_READ_NAME)),
            Read,
            Internal,
            None,
            5,
        ),
        t(
            "vault.read",
            vec![entity()],
            Bytes,
            Some(cap!(TAPE_READ_NAME)),
            Read,
            Vault,
            None,
            5,
        ),
        // ── CMS writes (the landing-port verbs) ──
        t(
            "cms.edit_section",
            vec![entity(), Text],
            Cid,
            Some(cap!(SIGNAL_EMIT_NAME)),
            ReversibleWrite,
            Internal,
            None,
            8,
        ),
        t(
            "cms.publish",
            vec![Cid],
            Cid,
            Some(cap!(INTENT_EMIT_NAME)),
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
            Some(cap!(REMOTE_COMPUTE_NAME)),
            Egress,
            Internal,
            Some(Internal),
            21,
        ),
    ];

    let mut reg = TermRegistry::new(VOCAB_VERSION);
    for spec in specs {
        reg.insert(spec)
            .expect("registry_v0 specs are statically valid");
    }
    reg
}

// ── canonical example capsules (the KAT/parity/golden-manifest subjects) ──
// Moved here from braid-ir/examples.rs: these are domain-specific (CMS
// landing-port shapes), so they live in the vocabulary package, not the
// substrate. The first things an authoring agent reads for THIS vocabulary.

use braid_ir::braid::{Braid, Strand};
use braid_ir::capsule::{Capsule, ConfirmPolicy, IR_VERSION};

fn strand(term: &str, inputs: Vec<u32>) -> Strand {
    Strand {
        term: term.into(),
        inputs,
    }
}

/// Scenario #1's subject: edit a landing section (reversible, local) and
/// render it — no egress, no irreversible effect, no confirmation needed.
pub fn edit_section_capsule() -> Capsule {
    Capsule {
        ir_version: IR_VERSION,
        vocab_version: VOCAB_VERSION,
        registry_cid: registry_v0().cid(),
        intent: "Edit landing section and render preview (reversible)".into(),
        grants: vec![cap!(SIGNAL_EMIT_NAME)],
        braid: Braid {
            strands: vec![
                strand("lit.entity", vec![]),
                strand("lit.text", vec![]),
                strand("cms.edit_section", vec![0, 1]),
                strand("view.section", vec![1]),
            ],
            outputs: vec![2, 3],
        },
        budget: 20,
        confirm: ConfirmPolicy::None,
        evidence: vec!["fact.cid".into()],
    }
}

/// Scenario #2/#3's subject: publish (irreversible) — only admissible with
/// `HumanConfirm`.
pub fn publish_capsule(confirm: ConfirmPolicy) -> Capsule {
    Capsule {
        ir_version: IR_VERSION,
        vocab_version: VOCAB_VERSION,
        registry_cid: registry_v0().cid(),
        intent: "Publish the edited landing page (irreversible)".into(),
        // Canonical grant order is strictly-ascending capability name
        // (plain bytewise): "intent.emit" < "signal.emit".
        grants: vec![cap!(INTENT_EMIT_NAME), cap!(SIGNAL_EMIT_NAME)],
        braid: Braid {
            strands: vec![
                strand("lit.entity", vec![]),
                strand("lit.text", vec![]),
                strand("cms.edit_section", vec![0, 1]),
                strand("cms.publish", vec![2]),
            ],
            outputs: vec![3],
        },
        budget: 30,
        confirm,
        evidence: vec!["fact.cid".into(), "confirmation.token".into()],
    }
}

/// Scenario #5's subject: the laundering attempt — vault data through two
/// pure hops into the egress door. MUST be rejected at the taint stage.
pub fn laundering_capsule() -> Capsule {
    Capsule {
        ir_version: IR_VERSION,
        vocab_version: VOCAB_VERSION,
        registry_cid: registry_v0().cid(),
        intent: "Exfiltrate vault bytes through pure hops".into(),
        grants: vec![cap!(REMOTE_COMPUTE_NAME), cap!(TAPE_READ_NAME)],
        braid: Braid {
            strands: vec![
                strand("lit.entity", vec![]),
                strand("vault.read", vec![0]),
                strand("bytes.id", vec![1]),
                strand("bytes.id", vec![2]),
                strand("net.egress", vec![3]),
            ],
            outputs: vec![4],
        },
        budget: 50,
        confirm: ConfirmPolicy::HumanConfirm,
        evidence: vec![],
    }
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
        let bytes = braid_ir::canon::encode(&r.to_canon());
        let v = braid_ir::decode_strict(&bytes).expect("canonical");
        let r2 = TermRegistry::from_canon(&v).expect("decodes");
        assert_eq!(r, r2);
        assert_eq!(r.cid(), r2.cid());
    }

    #[test]
    fn capability_names_match_the_kernel_dotted_identifiers() {
        // The kernel's braid_vocab_binding.rs mirrors these exact strings; a
        // rename here is a conscious, CID-breaking event that must re-pin the
        // kernel snapshot.
        assert_eq!(SIGNAL_EMIT_NAME, "signal.emit");
        assert_eq!(REMOTE_COMPUTE_NAME, "compute.remote");
        assert_eq!(TAPE_READ_NAME, "tape.read");
        assert_eq!(INTENT_EMIT_NAME, "intent.emit");
        assert_eq!(SIGNAL_SUBSCRIBE_NAME, "signal.subscribe");
        assert_eq!(VIEW_INJECT_NAME, "view.inject");
        assert_eq!(MOTION_SCHEDULE_NAME, "motion.schedule");
        assert_eq!(MOTION_OBSERVE_NAME, "motion.observe");
        assert_eq!(MOTION_PATCH_NAME, "motion.patch");
        assert_eq!(MOTION_PLUGIN_REGISTER_NAME, "motion.plugin.register");
        assert_eq!(MOTION_REPLAY_NAME, "motion.replay");
        assert_eq!(SHRED_NAME, "efface.shred");
        assert_eq!(RTBF_NAME, "efface.rtbf");
    }

    #[test]
    fn edit_section_capsule_has_no_irreversible_effects() {
        let capsule = edit_section_capsule();
        let reg = registry_v0();
        for strand in &capsule.braid.strands {
            let term = reg.get(&strand.term).unwrap();
            assert_ne!(term.effect, EffectClass::Irreversible);
            assert_ne!(term.effect, EffectClass::Egress);
        }
        assert_eq!(capsule.confirm, ConfirmPolicy::None);
        assert_eq!(capsule.grants, vec![cap!(SIGNAL_EMIT_NAME)]);
        assert_eq!(capsule.braid.strands.len(), 4);
        assert_eq!(capsule.braid.outputs, vec![2, 3]);
    }

    #[test]
    fn publish_capsule_contains_irreversible_effect() {
        let capsule = publish_capsule(ConfirmPolicy::HumanConfirm);
        let reg = registry_v0();
        let publish_term = reg.get("cms.publish").unwrap();
        assert_eq!(publish_term.effect, EffectClass::Irreversible);
        assert!(capsule
            .braid
            .strands
            .iter()
            .any(|s| s.term == "cms.publish"));
        assert_eq!(capsule.confirm, ConfirmPolicy::HumanConfirm);
        assert!(capsule.grants.contains(&cap!(INTENT_EMIT_NAME)));
    }

    #[test]
    fn laundering_capsule_routes_vault_through_egress() {
        let capsule = laundering_capsule();
        let reg = registry_v0();
        let vault = reg.get("vault.read").unwrap();
        let egress = reg.get("net.egress").unwrap();
        assert_eq!(vault.source_exposure, Exposure::Vault);
        assert_eq!(egress.effect, EffectClass::Egress);
        assert!(capsule.braid.strands.iter().any(|s| s.term == "vault.read"));
        assert!(capsule.braid.strands.iter().any(|s| s.term == "net.egress"));
    }

    #[test]
    fn cms_publish_is_the_only_irreversible_term() {
        let r = registry_v0();
        for term in r.terms() {
            if term.effect == EffectClass::Irreversible {
                assert_eq!(term.id, "cms.publish");
                assert!(term.egress_ceiling.is_some());
            }
        }
    }

    #[test]
    fn net_egress_is_the_only_egress_term() {
        let r = registry_v0();
        for term in r.terms() {
            if term.effect == EffectClass::Egress {
                assert_eq!(term.id, "net.egress");
                assert!(term.egress_ceiling.is_some());
            }
        }
    }

    #[test]
    fn pure_terms_require_no_capability() {
        let r = registry_v0();
        for term in r.terms() {
            if term.effect == EffectClass::Pure {
                assert!(
                    term.capability.is_none(),
                    "pure term {} should not require a capability",
                    term.id
                );
            }
        }
    }

    #[test]
    fn capsule_cid_is_deterministic() {
        let c1 = edit_section_capsule();
        let c2 = edit_section_capsule();
        assert_eq!(c1.cid(), c2.cid());
    }

    #[test]
    fn vault_read_has_vault_exposure() {
        let r = registry_v0();
        let vault = r.get("vault.read").unwrap();
        assert_eq!(vault.source_exposure, Exposure::Vault);
        assert_eq!(vault.effect, EffectClass::Read);
    }
}

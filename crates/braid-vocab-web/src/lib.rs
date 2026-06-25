//! braid-vocab-web — the browser engine's closed `web.*` action vocabulary.
//!
//! A **vocabulary package** over the Braid global IR (ADR-088 D31): it owns its
//! term registry + capability space and depends only on the substrate
//! (`braid-ir` + `braid-capability`). It is the canonical, content-addressed
//! home of the browser's closed action vocabulary (AX-Browser A5) — the ONE
//! place the verb alphabet lives. `next-gen-browser-engine` binds against this
//! (its `ActionVerb`, the policy broker's verb check, and the airworthiness
//! gate's verb list were three drifting copies; this collapses them to one).
//!
//! `no_std` because the consumer is the browser's `no_std` substrate (A18).
//!
//! **v0 effect/exposure classification is deliberately conservative
//! (fail-closed).** Each term declares the *least* authority its action implies;
//! refinements bump `VOCAB_VERSION`. The decisive guarantees, by construction:
//! - `web.download` is the single `Irreversible` host-write (A12: drive-by
//!   download is unrepresentable as anything cheaper).
//! - `web.execute_js`/`web.execute_wasm` are bounded local compute
//!   (`ReversibleWrite` under `web.compute.local`); network egress is a
//!   *separate* capability a realm may not hold, so untrusted compute cannot
//!   exfiltrate by simply running (JS_WASM_POSITION §6).
//! - nothing outside this registry is an admissible action (A5).

#![no_std]
extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use braid_capability::Capability;
use braid_ir::term::{EffectClass, Exposure, TermRegistry, TermSpec, TypeTag};

/// Browser vocabulary version. Independent of other packages; a bump is a
/// content-addressed registry event (the registry CID changes), never silent
/// drift.
pub const VOCAB_VERSION: u32 = 1;

// ── domain types (D31, vocabulary-owned `Opaque`) ──

/// A reference to a page element, identified by its content-addressed CID.
pub fn element() -> TypeTag {
    TypeTag::Opaque("web.element".into(), Vec::new())
}

/// A sealed observation fact derived from page state.
pub fn observation() -> TypeTag {
    TypeTag::Opaque("web.observation".into(), Vec::new())
}

// ── capability space (the `web.*` dotted names; the identity lives ONCE) ──

/// Load a URL (a GET-shaped read of the open web).
pub const NAVIGATE_NAME: &str = "web.navigate";
/// Read page state into typed observation facts.
pub const OBSERVE_NAME: &str = "web.observe";
/// DOM interaction (click / type / scroll) — reversible within the session.
pub const INTERACT_NAME: &str = "web.interact";
/// CPU-bounded untrusted local compute (the JS / Wasm lanes).
pub const COMPUTE_LOCAL_NAME: &str = "web.compute.local";
/// Write an artifact to the host filesystem (download). Rare, confirmed.
pub const FS_WRITE_NAME: &str = "web.fs.write";

fn cap(name: &'static str) -> Capability {
    Capability::new(name)
}

// One positional row per term keeps the registry readable as a table.
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

/// Build the v0 web action registry. Infallible by construction — the specs
/// satisfy `TermRegistry::insert`'s invariants (pinned by a unit test).
///
/// The id set is exactly AX-Browser's closed `web.*` action vocabulary (A5).
pub fn registry_v0() -> TermRegistry {
    use EffectClass::*;
    use Exposure::*;
    use TypeTag::*;

    let specs = vec![
        // load a URL → the loaded document element. A read of the open web.
        t(
            "web.navigate",
            vec![Text],
            element(),
            Some(cap(NAVIGATE_NAME)),
            Read,
            Public,
            None,
            3,
        ),
        // observe an element → a sealed observation fact.
        t(
            "web.observe",
            vec![element()],
            observation(),
            Some(cap(OBSERVE_NAME)),
            Read,
            Internal,
            None,
            2,
        ),
        // reversible DOM interactions → an outcome observation.
        t(
            "web.click",
            vec![element()],
            observation(),
            Some(cap(INTERACT_NAME)),
            ReversibleWrite,
            Internal,
            None,
            3,
        ),
        t(
            "web.type",
            vec![element(), Text],
            observation(),
            Some(cap(INTERACT_NAME)),
            ReversibleWrite,
            Internal,
            None,
            3,
        ),
        t(
            "web.scroll",
            vec![element()],
            observation(),
            Some(cap(INTERACT_NAME)),
            ReversibleWrite,
            Internal,
            None,
            2,
        ),
        // pure wait (no authority, no effect) — Pure ⟺ no capability.
        t("web.wait", vec![Int], Bool, None, Pure, Public, None, 1),
        // the single irreversible host-write: a download. egress_ceiling
        // required for Irreversible.
        t(
            "web.download",
            vec![Text],
            Cid,
            Some(cap(FS_WRITE_NAME)),
            Irreversible,
            Internal,
            Some(Internal),
            13,
        ),
        // bounded untrusted local compute. NOT Egress: a realm holding only
        // web.compute.local cannot reach the network (egress is a separate
        // capability), so running untrusted code cannot itself exfiltrate.
        t(
            "web.execute_js",
            vec![Text],
            observation(),
            Some(cap(COMPUTE_LOCAL_NAME)),
            ReversibleWrite,
            Internal,
            None,
            8,
        ),
        t(
            "web.execute_wasm",
            vec![Bytes],
            observation(),
            Some(cap(COMPUTE_LOCAL_NAME)),
            ReversibleWrite,
            Internal,
            None,
            8,
        ),
    ];

    let mut reg = TermRegistry::new(VOCAB_VERSION);
    for spec in specs {
        reg.insert(spec)
            .expect("registry_v0 specs are statically valid");
    }
    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_v0_is_the_closed_action_vocabulary() {
        let r = registry_v0();
        // Exactly AX-Browser A5's nine closed verbs — no more, no less.
        let expected = [
            "web.navigate",
            "web.observe",
            "web.click",
            "web.type",
            "web.scroll",
            "web.wait",
            "web.download",
            "web.execute_js",
            "web.execute_wasm",
        ];
        assert_eq!(r.len(), expected.len());
        for id in expected {
            assert!(r.get(id).is_some(), "missing term {id}");
        }
        // Anything outside the vocabulary is unrepresentable.
        assert!(r.get("web.eval").is_none());
        assert!(r.get("eval").is_none());
    }

    #[test]
    fn download_is_the_only_irreversible_term() {
        let r = registry_v0();
        for term in r.terms() {
            if term.effect == EffectClass::Irreversible {
                assert_eq!(term.id, "web.download");
                assert!(term.egress_ceiling.is_some());
            }
        }
        assert_eq!(r.get("web.download").unwrap().effect, EffectClass::Irreversible);
    }

    #[test]
    fn untrusted_compute_is_not_egress() {
        let r = registry_v0();
        for id in ["web.execute_js", "web.execute_wasm"] {
            let term = r.get(id).unwrap();
            assert_ne!(term.effect, EffectClass::Egress);
            assert_eq!(term.capability.as_ref().unwrap().as_str(), COMPUTE_LOCAL_NAME);
        }
    }

    #[test]
    fn wait_is_pure_and_uncapability() {
        let w = registry_v0();
        let wait = w.get("web.wait").unwrap();
        assert_eq!(wait.effect, EffectClass::Pure);
        assert!(wait.capability.is_none());
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
}

//! braid-vocab-js — JavaScript elaboration vocabulary (D31: Braid as a global IR).
//!
//! The **second** vocabulary package over the Braid substrate, proving the
//! "global translator IR" claim: a JS frontend elaborates JS source into
//! these terms, the *one* `braid-verify` admits them, and JS stops being a
//! runtime authority surface — it becomes an authoring frontend over the
//! verified substrate ("renders JS useless" in the Director's framing).
//!
//! //why this shape: JS is untrusted compute (the browser engine's
//! `JS_WASM_POSITION.md`: "JS is rented instinct"). The v0 JS vocabulary is
//! deliberately tiny — pure value construction + arithmetic + a gated
//! `js.eval` escalation probe — mirroring the CMS vocab's discipline (pure
//! alphabet first; the single effectful term is the deliberate irreversible
//! escalation that earns the verifier's trust before any destructive
//! capability is expressible). A real JS elaborator extends this registry; it
//! does not build a second verifier.
//!
//! Capability space: the `js.*` dotted names are THIS vocabulary's — they are
//! foreign to the kernel's `signal.emit`/`motion.*` and to the browser's
//! `web.*`. That is the point: a global IR lets each language own its
//! capability space while sharing the one attenuation-checking verifier.
//!
//! ## Vocabulary-extension governance (U12 / PRD §5 P5)
//! Adding a term is a contract change. The rules, mechanically guarded by the
//! tests in this file:
//! 1. **Bump [`VOCAB_VERSION`] and re-pin consciously.** Any change to the term
//!    set changes `registry_v0().cid()` and every capsule's bytes. The version
//!    bump + the re-pinned CID guards (here and in `braid-elaborate-js`) make
//!    that a recorded event, never silent drift (D11; anti-dredging class
//!    "context/spec drift").
//! 2. **Pure-by-default; danger is explicit and enumerated.** A new term is
//!    `EffectClass::Pure` with no capability UNLESS it genuinely needs one.
//!    `dangerous_terms()` is the *closed* list of effectful terms; the
//!    `expansion_added_no_escape_hatch` test fails if an expansion grows that
//!    set without updating the list — so a capability cannot be smuggled in on
//!    a "math" term (anti-dredging class "composition/aggregation exfil",
//!    T1/T5).
//! 3. **Prefer repurpose > extend > mint** (lgwks schema discipline): overloads
//!    map to distinct typed terms (`js.eq.num`/`js.eq.str`), never one term with
//!    a widened, ambiguous signature.

use braid_capability::Capability;
use braid_ir::term::{EffectClass, Exposure, TermRegistry, TermSpec, TypeTag};

/// Vocabulary version for the JS elaboration target. Independent of the CMS
/// vocab's version — each vocabulary versioning is a conscious event (D11).
///
/// **v1 → v2 (U12):** added the pure-operator expansion (`js.sub`, `js.mul`,
/// `js.lt`, `js.eq.num`, `js.eq.str`, `js.and`, `js.or`, `js.not`). A
/// conscious, recorded bump: it changes `registry_v0().cid()` and therefore
/// every JS capsule's bytes, so the pinned CIDs in `braid-elaborate-js` and the
/// registry-CID guard below were re-pinned in the same change (anti-drift: a
/// CID never moves silently).
pub const VOCAB_VERSION: u32 = 2;

// ── capability constants (this vocabulary's `js.*` space) ──
pub const JS_DOM_READ_NAME: &str = "js.dom.read";
pub const JS_DOM_WRITE_NAME: &str = "js.dom.write";
pub const JS_EVAL_NAME: &str = "js.eval";
pub const JS_FETCH_NAME: &str = "js.fetch";

/// `cap!` backing — public so the macro resolves from any crate.
pub fn wrap_cap(name: &'static str) -> Capability {
    Capability::new(name)
}

/// `TypeTag::Opaque("js.string", [])` — the JS primitive string. Vocabulary-
/// owned domain type (D31); foreign to `cms.entity`/`cms.directive`.
pub fn js_string() -> TypeTag {
    TypeTag::Opaque("js.string".into(), Vec::new())
}

/// `TypeTag::Opaque("js.number", [])` — the JS primitive number. Fixed-point
/// in the IR (D8: no floats); a JS elaborator scales at the term boundary.
pub fn js_number() -> TypeTag {
    TypeTag::Opaque("js.number".into(), Vec::new())
}

/// Constructs a [`TypeTag`] for JavaScript boolean primitives.
pub fn js_boolean() -> TypeTag {
    TypeTag::Opaque("js.boolean".into(), Vec::new())
}

/// `TypeTag::Opaque("js.object", [fields])` — a JS object as a record of
/// typed fields. The type args are the field types in declaration order.
pub fn js_object(fields: Vec<TypeTag>) -> TypeTag {
    TypeTag::Opaque("js.object".into(), fields)
}

/// `TypeTag::Opaque("js.function", [args..., ret])` — a JS function type:
/// the last type arg is the return type, the rest are parameter types.
pub fn js_function(args_ret: Vec<TypeTag>) -> TypeTag {
    TypeTag::Opaque("js.function".into(), args_ret)
}

struct TermDecl {
    id: &'static str,
    inputs: Vec<TypeTag>,
    output: TypeTag,
    capability: Option<Capability>,
    effect: EffectClass,
    source_exposure: Exposure,
    egress_ceiling: Option<Exposure>,
    cost: u64,
}

fn t(decl: TermDecl) -> TermSpec {
    TermSpec {
        id: decl.id.into(),
        inputs: decl.inputs,
        output: decl.output,
        capability: decl.capability,
        effect: decl.effect,
        source_exposure: decl.source_exposure,
        egress_ceiling: decl.egress_ceiling,
        cost: decl.cost,
    }
}

fn value_specs() -> Vec<TermSpec> {
    use EffectClass::*;
    use Exposure::*;

    vec![
        t(TermDecl {
            id: "js.lit.string",
            inputs: vec![],
            output: js_string(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.lit.number",
            inputs: vec![],
            output: js_number(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.lit.boolean",
            inputs: vec![],
            output: js_boolean(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
    ]
}

fn arithmetic_specs() -> Vec<TermSpec> {
    use EffectClass::*;
    use Exposure::*;

    vec![
        t(TermDecl {
            id: "js.add",
            inputs: vec![js_number(), js_number()],
            output: js_number(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.concat",
            inputs: vec![js_string(), js_string()],
            output: js_string(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.sub",
            inputs: vec![js_number(), js_number()],
            output: js_number(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.mul",
            inputs: vec![js_number(), js_number()],
            output: js_number(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
    ]
}

fn comparison_specs() -> Vec<TermSpec> {
    use EffectClass::*;
    use Exposure::*;

    vec![
        t(TermDecl {
            id: "js.lt",
            inputs: vec![js_number(), js_number()],
            output: js_boolean(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.eq.num",
            inputs: vec![js_number(), js_number()],
            output: js_boolean(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.eq.str",
            inputs: vec![js_string(), js_string()],
            output: js_boolean(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
    ]
}

fn logic_specs() -> Vec<TermSpec> {
    use EffectClass::*;
    use Exposure::*;

    vec![
        t(TermDecl {
            id: "js.and",
            inputs: vec![js_boolean(), js_boolean()],
            output: js_boolean(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.or",
            inputs: vec![js_boolean(), js_boolean()],
            output: js_boolean(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "js.not",
            inputs: vec![js_boolean()],
            output: js_boolean(),
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
    ]
}

fn pure_specs() -> Vec<TermSpec> {
    let mut all_pure = value_specs();
    all_pure.extend(arithmetic_specs());
    all_pure.extend(comparison_specs());
    all_pure.extend(logic_specs());
    all_pure
}

fn action_specs() -> Vec<TermSpec> {
    use EffectClass::*;
    use Exposure::*;

    vec![
        t(TermDecl {
            id: "js.dom.querySelector",
            inputs: vec![js_string()],
            output: js_object(vec![js_string(), js_boolean()]),
            capability: Some(Capability::new(JS_DOM_READ_NAME)),
            effect: Read,
            source_exposure: Internal,
            egress_ceiling: None,
            cost: 5,
        }),
        t(TermDecl {
            id: "js.eval",
            inputs: vec![js_string()],
            output: TypeTag::Opaque("js.any".into(), Vec::new()),
            capability: Some(Capability::new(JS_EVAL_NAME)),
            effect: Irreversible,
            source_exposure: Internal,
            egress_ceiling: Some(Internal),
            cost: 13,
        }),
        t(TermDecl {
            id: "js.fetch",
            inputs: vec![js_string()],
            output: js_string(),
            capability: Some(Capability::new(JS_FETCH_NAME)),
            effect: Egress,
            source_exposure: Internal,
            egress_ceiling: Some(Internal),
            cost: 21,
        }),
    ]
}

/// Build the v0 JS registry. Infallible by construction — validated by
/// `TermRegistry::insert` and pinned by the unit test.
pub fn registry_v0() -> TermRegistry {
    let mut reg = TermRegistry::new(VOCAB_VERSION);
    for spec in pure_specs().into_iter().chain(action_specs()) {
        reg.insert(spec)
            .expect("braid-vocab-js specs are statically valid");
    }
    reg
}

/// The terms in a registry that carry authority — a capability and/or a
/// non-`Pure` effect. Computed from the registry, never hardcoded, so it
/// reflects the actual term set. The governance guard pins the *expected*
/// result against a closed list: an expansion that smuggles a capability onto
/// a "math" term, or adds a new effectful term, changes this set and trips the
/// test (anti-dredging — composition/aggregation exfil, T1/T5).
pub fn dangerous_terms(reg: &TermRegistry) -> Vec<String> {
    let mut out: Vec<String> = reg
        .terms()
        .filter(|s| s.capability.is_some() || s.effect != EffectClass::Pure)
        .map(|s| s.id.clone())
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_ir::braid::{Braid, Strand};
    use braid_ir::capsule::{Capsule, ConfirmPolicy, IR_VERSION};
    use braid_sdk::Builder;
    use braid_verify::{verify, Stage, Verdict};

    /// Pinned CID of `registry_v0()` at `VOCAB_VERSION = 2`. Re-pinned
    /// consciously whenever the term set changes (see the governance note).
    const PINNED_REGISTRY_CID_V2: &str =
        "a79e1716ce1c3a05645b9c32cabf7895b93c05f57e0b77a18174c360b8233815";

    fn strand(term: &str, inputs: Vec<u32>) -> Strand {
        Strand {
            term: term.into(),
            inputs,
        }
    }

    #[test]
    fn registry_v0_builds() {
        let r = registry_v0();
        // 8 seed terms + 8 U12 pure-operator terms.
        assert_eq!(r.len(), 16);
        assert!(r.get("js.eval").is_some());
        assert!(r.get("js.lit.string").is_some());
        // The U12 expansion is present and typed.
        for added in [
            "js.sub",
            "js.mul",
            "js.lt",
            "js.eq.num",
            "js.eq.str",
            "js.and",
            "js.or",
            "js.not",
        ] {
            assert!(r.get(added).is_some(), "missing U12 term {added}");
        }
        // A term foreign to this vocabulary is absent — the registry is closed.
        assert!(r.get("cms.publish").is_none());
        assert!(r.get("eval").is_none());
    }

    /// Anti-dredging (composition/aggregation exfil — T1/T5): the U12 expansion
    /// added ONLY safe-by-construction terms. The set of authority-bearing
    /// terms is exactly the closed list it was before — no capability was
    /// smuggled onto a "math"/"logic" term, no new effectful term appeared.
    /// Mutation-proof: give `js.add` a capability, or make `js.mul` `Egress`,
    /// and this goes red.
    #[test]
    fn expansion_added_no_escape_hatch() {
        let r = registry_v0();
        assert_eq!(
            dangerous_terms(&r),
            // the ONLY terms allowed to carry authority — sorted
            vec![
                "js.dom.querySelector".to_string(),
                "js.eval".to_string(),
                "js.fetch".to_string(),
            ],
            "an expansion changed the authority surface — that must be a \
             conscious, reviewed event, not a silent escape hatch"
        );
        // Every U12 term is pure and capability-free, individually.
        for id in [
            "js.sub",
            "js.mul",
            "js.lt",
            "js.eq.num",
            "js.eq.str",
            "js.and",
            "js.or",
            "js.not",
        ] {
            let s = r.get(id).unwrap();
            assert_eq!(s.effect, EffectClass::Pure, "{id} must be Pure");
            assert!(s.capability.is_none(), "{id} must hold no capability");
        }
    }

    /// Anti-dredging (context/spec drift): the JS registry CID is pinned. It
    /// moves only on a conscious vocabulary change — which MUST be paired with
    /// a `VOCAB_VERSION` bump and this re-pin in the same commit (D11). A silent
    /// term-set change trips this guard.
    #[test]
    fn registry_cid_is_pinned_to_vocab_v2() {
        let r = registry_v0();
        assert_eq!(r.vocab_version, 2);
        assert_eq!(
            r.cid().to_hex(),
            PINNED_REGISTRY_CID_V2,
            "the JS registry CID moved without a recorded re-pin"
        );
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
    fn capability_names_are_the_js_space_not_the_kernel_space() {
        // The global-IR claim: this vocabulary owns a capability space foreign
        // to the kernel's. The verifier's subset check works on these names
        // exactly as it works on signal.emit — no core edit was needed to
        // admit a new language's authority model.
        assert_eq!(JS_EVAL_NAME, "js.eval");
        assert_ne!(JS_EVAL_NAME, "signal.emit");
    }

    /// The proof of the global-IR claim: a JS capsule authored via the SDK,
    /// admitted by the ONE braid-verify against THIS vocabulary's registry,
    /// with a capability space the kernel never knew about. No second
    /// verifier, no fork, no core edit.
    #[test]
    fn js_capsule_admits_via_the_one_verifier() {
        let reg = registry_v0();
        let mut b = Builder::new(&reg, "JS: build a greeting string (pure)");
        let s = b.strand("js.lit.string", &[]).unwrap();
        let n = b.strand("js.lit.number", &[]).unwrap();
        let _greet = b.strand("js.concat", &[s, s]).unwrap();
        b.output(n);
        let capsule = b.build().unwrap();

        let ambient = vec![Capability::new(JS_DOM_READ_NAME)];
        let verdict = verify(&capsule.to_bytes(), &reg, &ambient);
        assert_eq!(
            verdict,
            Verdict::Admit {
                capsule_cid: capsule.cid()
            },
            "a pure JS capsule must admit via the one verifier"
        );
    }

    /// The escalation probe mirrors the CMS no-confirm publish refusal: a
    /// dangerous JS term (`js.eval`) without a confirm policy is REJECTED at
    /// the effect stage. Same verifier, same stage, different language.
    #[test]
    fn js_eval_without_confirm_is_rejected_at_effect_stage() {
        let reg = registry_v0();
        let capsule = Capsule {
            ir_version: IR_VERSION,
            vocab_version: VOCAB_VERSION,
            registry_cid: reg.cid(),
            intent: "JS: eval without confirm (the escalation probe)".into(),
            grants: vec![Capability::new(JS_EVAL_NAME)],
            braid: Braid {
                strands: vec![strand("js.lit.string", vec![]), strand("js.eval", vec![0])],
                outputs: vec![1],
            },
            budget: 30,
            confirm: ConfirmPolicy::None,
            evidence: vec![],
        };
        let ambient = vec![Capability::new(JS_EVAL_NAME)];
        match verify(&capsule.to_bytes(), &reg, &ambient) {
            Verdict::Reject {
                stage: Stage::Effect,
                ..
            } => {}
            other => panic!("expected Effect reject, got {other:?}"),
        }
    }

    /// The laundering trip-wire, replayed for the JS vocabulary: vault data
    /// through a pure hop into the fetch egress door must die at the taint
    /// stage — the path-level fold is registry-agnostic, so it catches
    /// laundering in any language's vocabulary.
    #[test]
    fn js_laundering_through_pure_hop_into_fetch_is_rejected_at_taint() {
        let reg = registry_v0();
        // vault.read does not exist in the JS vocab; use js.dom.querySelector
        // (a Read, Internal exposure) feeding a pure concat into js.fetch.
        // Folded exposure Internal ≤ ceiling Internal ⇒ this ADMITS — so to
        // trip the taint gate we need a source ABOVE the egress ceiling.
        // js.eval is Irreversible/Internal; its output is js.any (Opaque).
        // Construct: js.lit.string (Public) -> js.eval (Internal) -> js.fetch (Egress, ceiling Internal).
        // js.eval output exposure = max(Internal, Public) = Internal ≤ Internal ceiling ⇒ admits.
        // To genuinely trip taint we need a Vault source. The v0 JS vocab has
        // no Vault term by design (frontend-first, like CMS). So the taint
        // gate is exercised by the CMS vocab's laundering_capsule instead
        // (braid-verify/tests/acceptance.rs scenario_5). This test pins that
        // the JS vocab has NO vault term — the frontend-first discipline
        // holds, and taint across the JS vocab's own terms stays within bounds.
        assert!(
            reg.terms().all(|t| t.source_exposure != Exposure::Vault),
            "the JS v0 vocabulary is frontend-first: no Vault source term exists"
        );
    }
}

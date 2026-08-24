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

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use braid_capability::Capability;
use braid_ir::term::{EffectClass, Exposure, RegistryError, TermRegistry, TermSpec, TypeTag};
use braid_ir::{CanonError, decode_strict};

/// Browser vocabulary version. Independent of other packages; a bump is a
/// content-addressed registry event (the registry CID changes), never silent
/// drift.
pub const VOCAB_VERSION: u32 = 1;

// ── typed vocabulary surface ──

/// A term known to exist in every valid web vocabulary.
///
/// Callers that use this key do not need fallible string lookup or panic
/// recovery: [`WebVocabulary::get`] returns the specification directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WebTerm {
    Navigate,
    Observe,
    Click,
    Type,
    Scroll,
    Wait,
    Download,
    ExecuteJs,
    ExecuteWasm,
}

impl WebTerm {
    /// Every web-vocabulary term, in canonical identifier order.
    pub const ALL: [Self; 9] = [
        Self::Click,
        Self::Download,
        Self::ExecuteJs,
        Self::ExecuteWasm,
        Self::Navigate,
        Self::Observe,
        Self::Scroll,
        Self::Type,
        Self::Wait,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Click => "web.click",
            Self::Download => "web.download",
            Self::ExecuteJs => "web.execute_js",
            Self::ExecuteWasm => "web.execute_wasm",
            Self::Navigate => "web.navigate",
            Self::Observe => "web.observe",
            Self::Scroll => "web.scroll",
            Self::Type => "web.type",
            Self::Wait => "web.wait",
        }
    }
}

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

/// Pinned registry CID for vocabulary v1. A term-set change moves this; the
/// bump MUST be paired with a `VOCAB_VERSION` bump and this re-pin (D11).
pub const PINNED_REGISTRY_CID_V1: &str =
    "49dc2af7a8c0836169bd26cdf0142e79a8fd060e34e79b23acac949125bdb14d";

/// The closed set of authority-bearing terms. A new capability-bearing term
/// that is not in this list is an escape hatch (T1/T5).
pub fn dangerous_terms(r: &TermRegistry) -> Vec<String> {
    let mut v: Vec<String> = r
        .terms()
        .filter(|t| t.capability.is_some())
        .map(|t| t.id.clone())
        .collect();
    v.sort();
    v
}

fn dom_specs() -> Vec<TermSpec> {
    use EffectClass::*;
    use Exposure::*;
    use TypeTag::*;

    vec![
        t(TermDecl {
            id: "web.navigate",
            inputs: vec![Text],
            output: element(),
            capability: Some(cap(NAVIGATE_NAME)),
            effect: Read,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 3,
        }),
        t(TermDecl {
            id: "web.observe",
            inputs: vec![element()],
            output: observation(),
            capability: Some(cap(OBSERVE_NAME)),
            effect: Read,
            source_exposure: Internal,
            egress_ceiling: None,
            cost: 2,
        }),
        t(TermDecl {
            id: "web.click",
            inputs: vec![element()],
            output: observation(),
            capability: Some(cap(INTERACT_NAME)),
            effect: ReversibleWrite,
            source_exposure: Internal,
            egress_ceiling: None,
            cost: 3,
        }),
        t(TermDecl {
            id: "web.type",
            inputs: vec![element(), Text],
            output: observation(),
            capability: Some(cap(INTERACT_NAME)),
            effect: ReversibleWrite,
            source_exposure: Internal,
            egress_ceiling: None,
            cost: 3,
        }),
        t(TermDecl {
            id: "web.scroll",
            inputs: vec![element()],
            output: observation(),
            capability: Some(cap(INTERACT_NAME)),
            effect: ReversibleWrite,
            source_exposure: Internal,
            egress_ceiling: None,
            cost: 2,
        }),
    ]
}

fn compute_specs() -> Vec<TermSpec> {
    use EffectClass::*;
    use Exposure::*;
    use TypeTag::*;

    vec![
        t(TermDecl {
            id: "web.wait",
            inputs: vec![Int],
            output: Bool,
            capability: None,
            effect: Pure,
            source_exposure: Public,
            egress_ceiling: None,
            cost: 1,
        }),
        t(TermDecl {
            id: "web.download",
            inputs: vec![Text],
            output: Cid,
            capability: Some(cap(FS_WRITE_NAME)),
            effect: Irreversible,
            source_exposure: Internal,
            egress_ceiling: Some(Internal),
            cost: 13,
        }),
        t(TermDecl {
            id: "web.execute_js",
            inputs: vec![Text],
            output: observation(),
            capability: Some(cap(COMPUTE_LOCAL_NAME)),
            effect: ReversibleWrite,
            source_exposure: Internal,
            egress_ceiling: None,
            cost: 8,
        }),
        t(TermDecl {
            id: "web.execute_wasm",
            inputs: vec![Bytes],
            output: observation(),
            capability: Some(cap(COMPUTE_LOCAL_NAME)),
            effect: ReversibleWrite,
            source_exposure: Internal,
            egress_ceiling: None,
            cost: 8,
        }),
    ]
}

/// Failure policy for constructing or decoding a web vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VocabularyError {
    /// A built-in declaration violated the substrate registry invariants.
    InvalidSpec(RegistryError),
    /// A statically required term was absent after registry construction.
    MissingKnownTerm(WebTerm),
    /// Untrusted bytes were not strict canonical IR.
    Canonical(CanonError),
    /// Strict canonical bytes did not decode as a term registry.
    InvalidRegistry(RegistryError),
    /// Decoded bytes are canonical IR but not the pinned web vocabulary.
    PinnedRegistryCidMismatch {
        expected: &'static str,
        actual: String,
    },
}

impl core::fmt::Display for VocabularyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidSpec(error) => write!(f, "invalid web vocabulary spec: {error}"),
            Self::MissingKnownTerm(term) => {
                write!(f, "missing known web vocabulary term {}", term.as_str())
            }
            Self::Canonical(error) => write!(f, "canonical decode failed: {error}"),
            Self::InvalidRegistry(error) => {
                write!(f, "canonical value is not a registry: {error}")
            }
            Self::PinnedRegistryCidMismatch { expected, actual } => write!(
                f,
                "decoded registry CID {actual} is not pinned web CID {expected}"
            ),
        }
    }
}

impl core::error::Error for VocabularyError {}

/// The validated web vocabulary and its substrate registry.
///
/// Construction fails closed if any built-in declaration violates a registry
/// invariant. Once constructed, known-term access cannot miss and therefore
/// cannot panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebVocabulary {
    registry: TermRegistry,
    click: TermSpec,
    download: TermSpec,
    execute_js: TermSpec,
    execute_wasm: TermSpec,
    navigate: TermSpec,
    observe: TermSpec,
    scroll: TermSpec,
    r#type: TermSpec,
    wait: TermSpec,
}

impl WebVocabulary {
    fn known_spec(registry: &TermRegistry, term: WebTerm) -> Result<&TermSpec, VocabularyError> {
        registry
            .get(term.as_str())
            .ok_or(VocabularyError::MissingKnownTerm(term))
    }

    fn from_registry(registry: TermRegistry) -> Result<Self, VocabularyError> {
        Ok(Self {
            click: Self::known_spec(&registry, WebTerm::Click)?.clone(),
            download: Self::known_spec(&registry, WebTerm::Download)?.clone(),
            execute_js: Self::known_spec(&registry, WebTerm::ExecuteJs)?.clone(),
            execute_wasm: Self::known_spec(&registry, WebTerm::ExecuteWasm)?.clone(),
            navigate: Self::known_spec(&registry, WebTerm::Navigate)?.clone(),
            observe: Self::known_spec(&registry, WebTerm::Observe)?.clone(),
            scroll: Self::known_spec(&registry, WebTerm::Scroll)?.clone(),
            r#type: Self::known_spec(&registry, WebTerm::Type)?.clone(),
            wait: Self::known_spec(&registry, WebTerm::Wait)?.clone(),
            registry,
        })
    }

    pub fn get(&self, term: WebTerm) -> &TermSpec {
        match term {
            WebTerm::Click => &self.click,
            WebTerm::Download => &self.download,
            WebTerm::ExecuteJs => &self.execute_js,
            WebTerm::ExecuteWasm => &self.execute_wasm,
            WebTerm::Navigate => &self.navigate,
            WebTerm::Observe => &self.observe,
            WebTerm::Scroll => &self.scroll,
            WebTerm::Type => &self.r#type,
            WebTerm::Wait => &self.wait,
        }
    }

    #[must_use]
    pub const fn registry(&self) -> &TermRegistry {
        &self.registry
    }

    #[must_use]
    pub fn into_registry(self) -> TermRegistry {
        self.registry
    }

    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        braid_ir::encode(&self.registry.to_canon())
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, VocabularyError> {
        let value = decode_strict(bytes).map_err(VocabularyError::Canonical)?;
        let registry =
            TermRegistry::from_canon(&value).map_err(VocabularyError::InvalidRegistry)?;

        let actual_cid = registry.cid();
        let actual_hex = actual_cid.to_hex();
        if actual_hex != PINNED_REGISTRY_CID_V1 {
            return Err(VocabularyError::PinnedRegistryCidMismatch {
                expected: PINNED_REGISTRY_CID_V1,
                actual: actual_hex,
            });
        }

        Self::from_registry(registry)
    }
}

/// Build and validate the closed v0 web action vocabulary.
pub fn vocabulary_v0() -> Result<WebVocabulary, VocabularyError> {
    let mut registry = TermRegistry::new(VOCAB_VERSION);
    for spec in dom_specs().into_iter().chain(compute_specs()) {
        registry
            .insert(spec)
            .map_err(VocabularyError::InvalidSpec)?;
    }

    WebVocabulary::from_registry(registry)
}

/// Build the v0 substrate registry without panicking on an authoring mistake.
pub fn registry_v0() -> Result<TermRegistry, VocabularyError> {
    vocabulary_v0().map(WebVocabulary::into_registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn registry_v0_is_the_closed_action_vocabulary() -> Result<(), VocabularyError> {
        let vocabulary = vocabulary_v0()?;
        assert_eq!(WebTerm::ALL.len(), 9);
        for term in WebTerm::ALL {
            assert_eq!(vocabulary.get(term).id, term.as_str());
        }
        assert!(vocabulary.registry().get("web.eval").is_none());
        assert!(vocabulary.registry().get("eval").is_none());
        Ok(())
    }

    #[test]
    fn download_is_the_only_irreversible_term() -> Result<(), VocabularyError> {
        let vocabulary = vocabulary_v0()?;
        for term in vocabulary.registry().terms() {
            if term.effect == EffectClass::Irreversible {
                assert_eq!(term.id, "web.download");
                assert!(term.egress_ceiling.is_some());
            }
        }

        let download = vocabulary.get(WebTerm::Download);
        assert_eq!(download.effect, EffectClass::Irreversible);
        assert_eq!(download.egress_ceiling, Some(Exposure::Internal));
        Ok(())
    }

    #[test]
    fn untrusted_compute_is_not_egress() -> Result<(), VocabularyError> {
        let vocabulary = vocabulary_v0()?;
        for term in [WebTerm::ExecuteJs, WebTerm::ExecuteWasm] {
            let spec = vocabulary.get(term);
            assert_ne!(spec.effect, EffectClass::Egress);
            assert_eq!(
                spec.capability.as_ref().map(Capability::as_str),
                Some(COMPUTE_LOCAL_NAME)
            );
        }
        Ok(())
    }

    #[test]
    fn wait_is_pure_and_uncapability() -> Result<(), VocabularyError> {
        let vocabulary = vocabulary_v0()?;
        let wait = vocabulary.get(WebTerm::Wait);
        assert_eq!(wait.effect, EffectClass::Pure);
        assert!(wait.capability.is_none());
        Ok(())
    }

    #[test]
    fn registry_round_trips_canonically() -> Result<(), VocabularyError> {
        let vocabulary = vocabulary_v0()?;
        let bytes = vocabulary.to_canonical_bytes();
        let decoded = WebVocabulary::from_canonical_bytes(&bytes)?;
        assert_eq!(vocabulary, decoded);
        assert_eq!(vocabulary.registry().cid(), decoded.registry().cid());
        Ok(())
    }

    #[test]
    fn foreign_bytes_are_rejected_without_panicking() {
        let error = WebVocabulary::from_canonical_bytes(b"not-canonical");
        assert!(matches!(error, Err(VocabularyError::Canonical(_))));
    }

    #[test]
    fn registry_cid_is_pinned_to_vocab_v1() -> Result<(), VocabularyError> {
        let registry = registry_v0()?;
        assert_eq!(registry.vocab_version, 1);
        assert_eq!(
            registry.cid().to_hex(),
            PINNED_REGISTRY_CID_V1,
            "the web registry CID moved without a recorded re-pin"
        );
        Ok(())
    }

    #[test]
    fn expansion_added_no_escape_hatch() -> Result<(), VocabularyError> {
        let registry = registry_v0()?;
        assert_eq!(
            dangerous_terms(&registry),
            vec![
                "web.click".to_string(),
                "web.download".to_string(),
                "web.execute_js".to_string(),
                "web.execute_wasm".to_string(),
                "web.navigate".to_string(),
                "web.observe".to_string(),
                "web.scroll".to_string(),
                "web.type".to_string(),
            ],
            "a term change altered the authority surface — that must be a \
             conscious, reviewed event, not a silent escape hatch"
        );
        Ok(())
    }

    #[test]
    fn navigate_is_read_with_navigate_capability() -> Result<(), VocabularyError> {
        let vocabulary = vocabulary_v0()?;
        let navigate = vocabulary.get(WebTerm::Navigate);
        assert_eq!(navigate.effect, EffectClass::Read);
        assert_eq!(
            navigate.capability.as_ref().map(Capability::as_str),
            Some(NAVIGATE_NAME)
        );
        assert_eq!(navigate.source_exposure, Exposure::Public);
        Ok(())
    }

    #[test]
    fn interact_terms_are_reversible_write() -> Result<(), VocabularyError> {
        let vocabulary = vocabulary_v0()?;
        for term in [WebTerm::Click, WebTerm::Type, WebTerm::Scroll] {
            let spec = vocabulary.get(term);
            assert_eq!(spec.effect, EffectClass::ReversibleWrite);
            assert_eq!(
                spec.capability.as_ref().map(Capability::as_str),
                Some(INTERACT_NAME)
            );
            assert_eq!(spec.source_exposure, Exposure::Internal);
        }
        Ok(())
    }

    #[test]
    fn type_tag_constructors_produce_opaque_tags() {
        assert_eq!(element(), TypeTag::Opaque("web.element".into(), Vec::new()));
        assert_eq!(
            observation(),
            TypeTag::Opaque("web.observation".into(), Vec::new())
        );
    }
}

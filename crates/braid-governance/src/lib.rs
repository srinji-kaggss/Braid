//! Braid-side enforcement for Keel generation governance.
//!
//! This crate is intentionally outside `braid-ir` and `braid-verify`: Keel
//! governance is an authoring constraint, not part of Braid's universal IR or
//! capsule admission semantics.  The adapter verifies the signed Keel
//! envelope, then fail-closes every proposed authoring action against it.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use lgwks_std::hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const CHANGE_ENVELOPE_VERSION: u32 = 1;
const DIGEST_DOMAIN: &[u8] = b"keel.change-envelope.v1\0";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum FoundationTier {
    T0Cosmetic,
    T1Application,
    T2SharedOrPersistent,
    T3TrustBoundary,
    T4FoundationalAuthority,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceBudget {
    pub max_files_changed: u32,
    pub max_new_dependencies: u32,
    pub max_tool_calls: u32,
    pub max_effectful_tool_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeEnvelope {
    pub version: u32,
    pub change_id: String,
    pub parent_digest_sha256: Option<String>,
    pub source_baseline_sha256: String,
    pub policy_sha256: String,
    pub intent: String,
    pub non_goals: BTreeSet<String>,
    pub foundation_tier: FoundationTier,
    pub allowed_repositories: BTreeSet<String>,
    pub allowed_paths: BTreeSet<String>,
    pub allowed_symbols: BTreeSet<String>,
    pub allowed_effects: BTreeSet<String>,
    pub allowed_capabilities: BTreeSet<String>,
    pub forbidden_operations: BTreeSet<String>,
    pub requirement_refs: BTreeSet<String>,
    pub invariant_refs: BTreeSet<String>,
    pub required_evidence: BTreeSet<String>,
    pub read_only_evidence_paths: BTreeSet<String>,
    pub budget: ResourceBudget,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedChangeEnvelope {
    pub envelope: ChangeEnvelope,
    pub signer_key_id: String,
    pub verifying_key_hex: String,
    pub signature_hex: String,
}

/// Structured explain-before-author commitment for high-risk work.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignCommitment {
    pub change_id: String,
    pub requirement_refs: BTreeSet<String>,
    pub invariant_refs: BTreeSet<String>,
    pub intended_paths: BTreeSet<String>,
    pub intended_symbols: BTreeSet<String>,
    pub intended_effects: BTreeSet<String>,
    pub evidence_plan: BTreeSet<String>,
    pub unresolved_assumptions: BTreeSet<String>,
}

/// An operation requested by an AI authoring harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationAction<'a> {
    WritePath(&'a str),
    TouchSymbol(&'a str),
    UseEffect(&'a str),
    UseCapability(&'a str),
    AddDependency(&'a str),
    ReadEvidencePath(&'a str),
    ModifyEvidencePath(&'a str),
    InvokeTool { name: &'a str, effectful: bool },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionUsage {
    pub distinct_written_paths: BTreeSet<String>,
    pub added_dependencies: BTreeSet<String>,
    pub tool_calls: u32,
    pub effectful_tool_calls: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GovernanceError {
    UnsupportedVersion(u32),
    MissingField(&'static str),
    InvalidSha256(&'static str),
    InvalidHex(&'static str),
    InvalidVerifyingKey,
    InvalidSignature,
    Serialization(String),
    Denied(String),
    InvalidCommitment(&'static str),
    BudgetExceeded(&'static str),
    MalformedExpiry(&'static str),
    Expired(u64),
}

impl std::fmt::Display for GovernanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported change-envelope version {v}"),
            Self::MissingField(field) => write!(f, "missing required field: {field}"),
            Self::InvalidSha256(field) => write!(f, "invalid SHA-256 field: {field}"),
            Self::InvalidHex(field) => write!(f, "invalid hex field: {field}"),
            Self::InvalidVerifyingKey => write!(f, "invalid Ed25519 verifying key"),
            Self::InvalidSignature => write!(f, "invalid Ed25519 signature"),
            Self::Serialization(msg) => write!(f, "serialization failed: {msg}"),
            Self::Denied(msg) => write!(f, "generation action denied: {msg}"),
            Self::InvalidCommitment(field) => {
                write!(f, "design commitment is outside the admitted envelope: {field}")
            }
            Self::BudgetExceeded(field) => write!(f, "resource budget exceeded: {field}"),
            Self::MalformedExpiry(msg) => {
                write!(f, "expires_at not in strict UTC 'YYYY-MM-DDTHH:MM:SSZ' form: {msg}")
            }
            Self::Expired(unix) => write!(f, "envelope expired at unix {unix}"),
        }
    }
}

impl std::error::Error for GovernanceError {}

impl ChangeEnvelope {
    pub fn validate(&self) -> Result<(), GovernanceError> {
        if self.version != CHANGE_ENVELOPE_VERSION {
            return Err(GovernanceError::UnsupportedVersion(self.version));
        }
        for (name, value) in [
            ("change_id", self.change_id.as_str()),
            ("intent", self.intent.as_str()),
            ("expires_at", self.expires_at.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(GovernanceError::MissingField(name));
            }
        }
        validate_sha256("source_baseline_sha256", &self.source_baseline_sha256)?;
        validate_sha256("policy_sha256", &self.policy_sha256)?;
        if let Some(parent) = &self.parent_digest_sha256 {
            validate_sha256("parent_digest_sha256", parent)?;
        }
        if self.allowed_repositories.is_empty() {
            return Err(GovernanceError::MissingField("allowed_repositories"));
        }
        if self.requirement_refs.is_empty() {
            return Err(GovernanceError::MissingField("requirement_refs"));
        }
        if self.invariant_refs.is_empty() {
            return Err(GovernanceError::MissingField("invariant_refs"));
        }
        if self.required_evidence.is_empty() {
            return Err(GovernanceError::MissingField("required_evidence"));
        }
        Ok(())
    }

    /// Expiry as unix seconds. Strict `YYYY-MM-DDTHH:MM:SSZ` only — no
    /// offsets, no leap seconds, no relaxed parsing (fail-closed: a security
    /// boundary must not guess at malformed input).
    pub fn expiry_unix(&self) -> Result<u64, GovernanceError> {
        let s = self.expires_at.as_str();
        let b = s.as_bytes();
        let sep = |i: usize| b.get(i).copied().unwrap_or(0);
        if b.len() != 20
            || sep(4) != b'-'
            || sep(7) != b'-'
            || sep(10) != b'T'
            || sep(13) != b':'
            || sep(16) != b':'
            || sep(19) != b'Z'
            || !b
                .iter()
                .enumerate()
                .all(|(i, c)| matches!(i, 4 | 7 | 10 | 13 | 16 | 19) || c.is_ascii_digit())
        {
            return Err(GovernanceError::MalformedExpiry(
                "expected YYYY-MM-DDTHH:MM:SSZ",
            ));
        }
        let num = |r: std::ops::Range<usize>| s[r].parse::<u64>().unwrap_or(u64::MAX);
        let (y, mo, d) = (num(0..4), num(5..7), num(8..10));
        let (hh, mi, ss) = (num(11..13), num(14..16), num(17..19));
        if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || hh > 23 || mi > 59 || ss > 59 {
            return Err(GovernanceError::MalformedExpiry("field out of range"));
        }
        // Days-from-civil (Howard Hinnant's algorithm); the round-trip below
        // rejects non-calendar dates like Feb 30.
        let y_adj = y as i64 - if mo <= 2 { 1 } else { 0 };
        let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
        let yoe = y_adj - era * 400;
        let doy =
            (153 * (if mo > 2 { mo as i64 - 3 } else { mo as i64 + 9 }) + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146097 + doe - 719468;
        let z = days + 719468;
        let era2 = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe2 = z - era2 * 146097;
        let yoe2 = (doe2 - doe2 / 1460 + doe2 / 36524 - doe2 / 146096) / 365;
        let y2 = yoe2 + era2 * 400;
        let doy2 = doe2 - (365 * yoe2 + yoe2 / 4 - yoe2 / 100);
        let mp2 = (5 * doy2 + 2) / 153;
        let d2 = doy2 - (153 * mp2 + 2) / 5 + 1;
        let mo2 = if mp2 < 10 { mp2 + 3 } else { mp2 - 9 };
        if y2 + if mo2 <= 2 { 1 } else { 0 } != y as i64 || mo2 as u64 != mo || d2 as u64 != d {
            return Err(GovernanceError::MalformedExpiry("not a real calendar date"));
        }
        Ok(days as u64 * 86_400 + hh * 3_600 + mi * 60 + ss)
    }

    pub fn digest_sha256(&self) -> Result<String, GovernanceError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| GovernanceError::Serialization(error.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(DIGEST_DOMAIN);
        hasher.update(bytes);
        Ok(hex::encode(hasher.finalize()))
    }

    pub fn validate_commitment(
        &self,
        commitment: &DesignCommitment,
    ) -> Result<(), GovernanceError> {
        if commitment.change_id != self.change_id {
            return Err(GovernanceError::InvalidCommitment("change_id"));
        }
        require_subset(
            "requirements",
            &commitment.requirement_refs,
            &self.requirement_refs,
        )
        .map_err(|_| GovernanceError::InvalidCommitment("requirements"))?;
        require_subset(
            "invariants",
            &commitment.invariant_refs,
            &self.invariant_refs,
        )
        .map_err(|_| GovernanceError::InvalidCommitment("invariants"))?;
        require_subset("paths", &commitment.intended_paths, &self.allowed_paths)
            .map_err(|_| GovernanceError::InvalidCommitment("paths"))?;
        require_subset(
            "symbols",
            &commitment.intended_symbols,
            &self.allowed_symbols,
        )
        .map_err(|_| GovernanceError::InvalidCommitment("symbols"))?;
        require_subset(
            "effects",
            &commitment.intended_effects,
            &self.allowed_effects,
        )
        .map_err(|_| GovernanceError::InvalidCommitment("effects"))?;

        if !commitment
            .evidence_plan
            .is_superset(&self.required_evidence)
        {
            return Err(GovernanceError::InvalidCommitment("evidence plan"));
        }
        if self.foundation_tier >= FoundationTier::T3TrustBoundary
            && (!commitment.unresolved_assumptions.is_empty())
        {
            return Err(GovernanceError::InvalidCommitment(
                "unresolved assumptions require re-admission for T3/T4",
            ));
        }
        Ok(())
    }
}

impl SignedChangeEnvelope {
    pub fn from_json_and_verify(bytes: &[u8]) -> Result<Self, GovernanceError> {
        let signed: SignedChangeEnvelope = serde_json::from_slice(bytes)
            .map_err(|error| GovernanceError::Serialization(error.to_string()))?;
        signed.verify()?;
        Ok(signed)
    }

    pub fn verify(&self) -> Result<String, GovernanceError> {
        if self.signer_key_id.trim().is_empty() {
            return Err(GovernanceError::MissingField("signer_key_id"));
        }
        let digest = self.envelope.digest_sha256()?;
        let key_bytes = decode_fixed::<32>("verifying_key_hex", &self.verifying_key_hex)?;
        let signature_bytes = decode_fixed::<64>("signature_hex", &self.signature_hex)?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| GovernanceError::InvalidVerifyingKey)?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(digest.as_bytes(), &signature)
            .map_err(|_| GovernanceError::InvalidSignature)?;
        Ok(digest)
    }
}

/// Stateful, fail-closed enforcement used by an authoring harness.
///
/// This enforces the envelope before the effect occurs.  It does not infer
/// semantic correctness; Keel remains responsible for the final assurance case.
pub struct GovernanceSession {
    signed: SignedChangeEnvelope,
    usage: SessionUsage,
}

impl GovernanceSession {
    pub fn admit(signed: SignedChangeEnvelope) -> Result<Self, GovernanceError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| GovernanceError::Serialization(e.to_string()))?
            .as_secs();
        Self::admit_at(signed, now)
    }

    /// Admission core with an explicit clock — deterministic tests inject the
    /// time; production goes through `admit`. Expiry is enforced here, after
    /// signature verification, so a stale envelope is refused at admission
    /// rather than tolerated (closes the `expires_at` gap).
    pub fn admit_at(signed: SignedChangeEnvelope, now_unix: u64) -> Result<Self, GovernanceError> {
        signed.verify()?;
        let expiry = signed.envelope.expiry_unix()?;
        if expiry <= now_unix {
            return Err(GovernanceError::Expired(expiry));
        }
        Ok(Self {
            signed,
            usage: SessionUsage::default(),
        })
    }

    pub fn envelope(&self) -> &ChangeEnvelope {
        &self.signed.envelope
    }

    pub fn usage(&self) -> &SessionUsage {
        &self.usage
    }

    pub fn authorize(&mut self, action: GenerationAction<'_>) -> Result<(), GovernanceError> {
        let envelope = &self.signed.envelope;
        match action {
            GenerationAction::WritePath(path) => {
                if envelope.read_only_evidence_paths.contains(path) {
                    return Err(GovernanceError::Denied(format!(
                        "evidence path is read-only: {path}"
                    )));
                }
                if !envelope.allowed_paths.contains(path) {
                    return Err(GovernanceError::Denied(format!(
                        "path not admitted: {path}"
                    )));
                }
                self.usage.distinct_written_paths.insert(path.to_string());
                if self.usage.distinct_written_paths.len() as u32
                    > envelope.budget.max_files_changed
                {
                    return Err(GovernanceError::BudgetExceeded("max_files_changed"));
                }
            }
            GenerationAction::TouchSymbol(symbol) => {
                if !envelope.allowed_symbols.contains(symbol) {
                    return Err(GovernanceError::Denied(format!(
                        "symbol not admitted: {symbol}"
                    )));
                }
            }
            GenerationAction::UseEffect(effect) => {
                if !envelope.allowed_effects.contains(effect) {
                    return Err(GovernanceError::Denied(format!(
                        "effect not admitted: {effect}"
                    )));
                }
            }
            GenerationAction::UseCapability(capability) => {
                if !envelope.allowed_capabilities.contains(capability) {
                    return Err(GovernanceError::Denied(format!(
                        "capability not admitted: {capability}"
                    )));
                }
            }
            GenerationAction::AddDependency(name) => {
                if envelope.forbidden_operations.contains("new-dependency") {
                    return Err(GovernanceError::Denied(format!(
                        "new dependencies forbidden: {name}"
                    )));
                }
                self.usage.added_dependencies.insert(name.to_string());
                if self.usage.added_dependencies.len() as u32 > envelope.budget.max_new_dependencies
                {
                    return Err(GovernanceError::BudgetExceeded("max_new_dependencies"));
                }
            }
            GenerationAction::ReadEvidencePath(path) => {
                if !envelope.read_only_evidence_paths.contains(path) {
                    return Err(GovernanceError::Denied(format!(
                        "evidence path not admitted: {path}"
                    )));
                }
            }
            GenerationAction::ModifyEvidencePath(path) => {
                return Err(GovernanceError::Denied(format!(
                    "implementation author may not modify evidence path: {path}"
                )));
            }
            GenerationAction::InvokeTool { name, effectful } => {
                self.usage.tool_calls = self.usage.tool_calls.saturating_add(1);
                if self.usage.tool_calls > envelope.budget.max_tool_calls {
                    return Err(GovernanceError::BudgetExceeded("max_tool_calls"));
                }
                if effectful {
                    self.usage.effectful_tool_calls =
                        self.usage.effectful_tool_calls.saturating_add(1);
                    if self.usage.effectful_tool_calls > envelope.budget.max_effectful_tool_calls {
                        return Err(GovernanceError::BudgetExceeded("max_effectful_tool_calls"));
                    }
                }
                if envelope.forbidden_operations.contains(name) {
                    return Err(GovernanceError::Denied(format!(
                        "tool operation forbidden: {name}"
                    )));
                }
            }
        }
        Ok(())
    }
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), GovernanceError> {
    let bytes = hex::decode(value).map_err(|_| GovernanceError::InvalidSha256(field))?;
    if bytes.len() != 32 || value.len() != 64 || value != value.to_ascii_lowercase() {
        return Err(GovernanceError::InvalidSha256(field));
    }
    Ok(())
}

fn decode_fixed<const N: usize>(
    field: &'static str,
    value: &str,
) -> Result<[u8; N], GovernanceError> {
    let bytes = hex::decode(value).map_err(|_| GovernanceError::InvalidHex(field))?;
    bytes
        .try_into()
        .map_err(|_| GovernanceError::InvalidHex(field))
}

fn require_subset(
    dimension: &'static str,
    child: &BTreeSet<String>,
    parent: &BTreeSet<String>,
) -> Result<(), GovernanceError> {
    if child.is_subset(parent) {
        Ok(())
    } else {
        Err(GovernanceError::Denied(format!(
            "{dimension} exceeds envelope"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn set(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn signed_envelope() -> SignedChangeEnvelope {
        let envelope = ChangeEnvelope {
            version: CHANGE_ENVELOPE_VERSION,
            change_id: "CHG-1".into(),
            parent_digest_sha256: None,
            source_baseline_sha256: "11".repeat(32),
            policy_sha256: "22".repeat(32),
            intent: "repair token redemption".into(),
            non_goals: set(&["change wire format"]),
            foundation_tier: FoundationTier::T4FoundationalAuthority,
            allowed_repositories: set(&["example/auth"]),
            allowed_paths: set(&["src/auth.rs", "tests/auth.rs"]),
            allowed_symbols: set(&["redeem", "TokenLedger"]),
            allowed_effects: set(&["ledger.read", "ledger.atomic-write"]),
            allowed_capabilities: set(&["fs.read", "db.write"]),
            forbidden_operations: set(&["network", "new-dependency"]),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            required_evidence: set(&["property:double-redemption"]),
            read_only_evidence_paths: set(&["spec/auth.md"]),
            budget: ResourceBudget {
                max_files_changed: 2,
                max_new_dependencies: 0,
                max_tool_calls: 4,
                max_effectful_tool_calls: 1,
            },
            expires_at: "2099-01-01T00:00:00Z".into(),
        };
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let digest = envelope.digest_sha256().unwrap();
        let signature = signing_key.sign(digest.as_bytes());
        SignedChangeEnvelope {
            envelope,
            signer_key_id: "test-key".into(),
            verifying_key_hex: hex::encode(signing_key.verifying_key().to_bytes()),
            signature_hex: hex::encode(signature.to_bytes()),
        }
    }

    #[test]
    fn signature_detects_changed_governance() {
        let mut signed = signed_envelope();
        assert!(signed.verify().is_ok());
        signed
            .envelope
            .allowed_effects
            .insert("network.egress".into());
        assert_eq!(signed.verify(), Err(GovernanceError::InvalidSignature));
    }

    #[test]
    fn design_commitment_must_cover_required_evidence() {
        let signed = signed_envelope();
        let commitment = DesignCommitment {
            change_id: "CHG-1".into(),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            intended_paths: set(&["src/auth.rs"]),
            intended_symbols: set(&["redeem"]),
            intended_effects: set(&["ledger.atomic-write"]),
            evidence_plan: BTreeSet::new(),
            unresolved_assumptions: BTreeSet::new(),
        };
        assert_eq!(
            signed.envelope.validate_commitment(&commitment),
            Err(GovernanceError::InvalidCommitment("evidence plan"))
        );
    }

    #[test]
    fn undeclared_effect_is_blocked_before_execution() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert!(session
            .authorize(GenerationAction::UseEffect("ledger.atomic-write"))
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::UseEffect("network.egress")),
            Err(GovernanceError::Denied(
                "effect not admitted: network.egress".into()
            ))
        );
    }

    #[test]
    fn evidence_is_read_only_to_implementation_author() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert!(session
            .authorize(GenerationAction::ReadEvidencePath("spec/auth.md"))
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::ModifyEvidencePath("spec/auth.md")),
            Err(GovernanceError::Denied(
                "implementation author may not modify evidence path: spec/auth.md".into()
            ))
        );
    }

    #[test]
    fn budgets_are_enforced_incrementally() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert!(session
            .authorize(GenerationAction::InvokeTool {
                name: "read",
                effectful: false,
            })
            .is_ok());
        assert!(session
            .authorize(GenerationAction::InvokeTool {
                name: "write",
                effectful: true,
            })
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::InvokeTool {
                name: "write",
                effectful: true,
            }),
            Err(GovernanceError::BudgetExceeded("max_effectful_tool_calls"))
        );
    }

    #[test]
    fn write_path_denied_outside_allowed_set() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert!(session
            .authorize(GenerationAction::WritePath("src/auth.rs"))
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::WritePath("src/sneak.rs")),
            Err(GovernanceError::Denied(
                "path not admitted: src/sneak.rs".into()
            ))
        );
    }

    #[test]
    fn write_path_denied_on_read_only_evidence() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert_eq!(
            session.authorize(GenerationAction::WritePath("spec/auth.md")),
            Err(GovernanceError::Denied(
                "evidence path is read-only: spec/auth.md".into()
            ))
        );
    }

    fn envelope_with_file_budget(n: u32) -> SignedChangeEnvelope {
        let mut signed = signed_envelope();
        signed.envelope.budget.max_files_changed = n;
        signed.envelope.allowed_paths = set(&["a.rs", "b.rs", "c.rs"]);
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let digest = signed.envelope.digest_sha256().unwrap();
        let signature = signing_key.sign(digest.as_bytes());
        signed.signature_hex = hex::encode(signature.to_bytes());
        signed
    }

    #[test]
    fn max_files_changed_budget_enforced() {
        let mut session = GovernanceSession::admit(envelope_with_file_budget(2)).unwrap();
        assert!(session
            .authorize(GenerationAction::WritePath("a.rs"))
            .is_ok());
        assert!(session
            .authorize(GenerationAction::WritePath("b.rs"))
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::WritePath("c.rs")),
            Err(GovernanceError::BudgetExceeded("max_files_changed"))
        );
    }

    #[test]
    fn touch_symbol_denied_outside_allowed_set() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert!(session
            .authorize(GenerationAction::TouchSymbol("redeem"))
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::TouchSymbol("sneak_fn")),
            Err(GovernanceError::Denied(
                "symbol not admitted: sneak_fn".into()
            ))
        );
    }

    #[test]
    fn use_capability_denied_outside_allowed_set() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert!(session
            .authorize(GenerationAction::UseCapability("fs.read"))
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::UseCapability("network.egress")),
            Err(GovernanceError::Denied(
                "capability not admitted: network.egress".into()
            ))
        );
    }

    #[test]
    fn add_dependency_denied_when_forbidden() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert_eq!(
            session.authorize(GenerationAction::AddDependency("tokio")),
            Err(GovernanceError::Denied(
                "new dependencies forbidden: tokio".into()
            ))
        );
    }

    #[test]
    fn forbidden_tool_operation_denied() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert_eq!(
            session.authorize(GenerationAction::InvokeTool {
                name: "network",
                effectful: false,
            }),
            Err(GovernanceError::Denied(
                "tool operation forbidden: network".into()
            ))
        );
    }

    #[test]
    fn max_tool_calls_budget_enforced() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        for i in 0..4 {
            assert!(session
                .authorize(GenerationAction::InvokeTool {
                    name: &format!("tool_{i}"),
                    effectful: false,
                })
                .is_ok());
        }
        assert_eq!(
            session.authorize(GenerationAction::InvokeTool {
                name: "tool_4",
                effectful: false,
            }),
            Err(GovernanceError::BudgetExceeded("max_tool_calls"))
        );
    }

    #[test]
    fn commitment_change_id_must_match() {
        let signed = signed_envelope();
        let commitment = DesignCommitment {
            change_id: "CHG-WRONG".into(),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            intended_paths: set(&["src/auth.rs"]),
            intended_symbols: set(&["redeem"]),
            intended_effects: set(&["ledger.atomic-write"]),
            evidence_plan: set(&["property:double-redemption"]),
            unresolved_assumptions: BTreeSet::new(),
        };
        assert_eq!(
            signed.envelope.validate_commitment(&commitment),
            Err(GovernanceError::InvalidCommitment("change_id"))
        );
    }

    #[test]
    fn commitment_paths_must_be_within_envelope() {
        let signed = signed_envelope();
        let commitment = DesignCommitment {
            change_id: "CHG-1".into(),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            intended_paths: set(&["src/auth.rs", "src/sneak.rs"]),
            intended_symbols: set(&["redeem"]),
            intended_effects: set(&["ledger.atomic-write"]),
            evidence_plan: set(&["property:double-redemption"]),
            unresolved_assumptions: BTreeSet::new(),
        };
        assert_eq!(
            signed.envelope.validate_commitment(&commitment),
            Err(GovernanceError::InvalidCommitment("paths"))
        );
    }

    #[test]
    fn envelope_validation_rejects_bad_version() {
        let mut signed = signed_envelope();
        signed.envelope.version = 99;
        assert_eq!(
            signed.envelope.validate(),
            Err(GovernanceError::UnsupportedVersion(99))
        );
    }

    #[test]
    fn envelope_validation_rejects_empty_intent() {
        let mut signed = signed_envelope();
        signed.envelope.intent = "  ".into();
        assert_eq!(
            signed.envelope.validate(),
            Err(GovernanceError::MissingField("intent"))
        );
    }

    #[test]
    fn envelope_validation_rejects_bad_sha256() {
        let mut signed = signed_envelope();
        signed.envelope.source_baseline_sha256 = "not-hex".into();
        assert_eq!(
            signed.envelope.validate(),
            Err(GovernanceError::InvalidSha256("source_baseline_sha256"))
        );
    }

    #[test]
    fn digest_is_deterministic() {
        let signed = signed_envelope();
        let d1 = signed.envelope.digest_sha256().unwrap();
        let d2 = signed.envelope.digest_sha256().unwrap();
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }

    #[test]
    fn read_evidence_path_denied_outside_allowed_set() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert_eq!(
            session.authorize(GenerationAction::ReadEvidencePath("spec/other.md")),
            Err(GovernanceError::Denied(
                "evidence path not admitted: spec/other.md".into()
            ))
        );
    }

    #[test]
    fn t3_commitment_with_unresolved_assumptions_rejected() {
        let mut signed = signed_envelope();
        signed.envelope.foundation_tier = FoundationTier::T3TrustBoundary;
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let digest = signed.envelope.digest_sha256().unwrap();
        let signature = signing_key.sign(digest.as_bytes());
        signed.signature_hex = hex::encode(signature.to_bytes());

        let commitment = DesignCommitment {
            change_id: "CHG-1".into(),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            intended_paths: set(&["src/auth.rs"]),
            intended_symbols: set(&["redeem"]),
            intended_effects: set(&["ledger.atomic-write"]),
            evidence_plan: set(&["property:double-redemption"]),
            unresolved_assumptions: set(&["network topology is stable"]),
        };
        assert_eq!(
            signed.envelope.validate_commitment(&commitment),
            Err(GovernanceError::InvalidCommitment(
                "unresolved assumptions require re-admission for T3/T4"
            ))
        );
    }

    #[test]
    fn t4_commitment_with_unresolved_assumptions_rejected() {
        let signed = signed_envelope();
        let commitment = DesignCommitment {
            change_id: "CHG-1".into(),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            intended_paths: set(&["src/auth.rs"]),
            intended_symbols: set(&["redeem"]),
            intended_effects: set(&["ledger.atomic-write"]),
            evidence_plan: set(&["property:double-redemption"]),
            unresolved_assumptions: set(&["assumption"]),
        };
        assert_eq!(
            signed.envelope.validate_commitment(&commitment),
            Err(GovernanceError::InvalidCommitment(
                "unresolved assumptions require re-admission for T3/T4"
            ))
        );
    }

    fn envelope_allowing_deps(n: u32) -> SignedChangeEnvelope {
        let mut signed = signed_envelope();
        signed.envelope.forbidden_operations = set(&[]);
        signed.envelope.budget.max_new_dependencies = n;
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let digest = signed.envelope.digest_sha256().unwrap();
        let signature = signing_key.sign(digest.as_bytes());
        signed.signature_hex = hex::encode(signature.to_bytes());
        signed
    }

    #[test]
    fn add_dependency_budget_enforced_when_not_forbidden() {
        let mut session = GovernanceSession::admit(envelope_allowing_deps(1)).unwrap();
        assert!(session
            .authorize(GenerationAction::AddDependency("serde"))
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::AddDependency("tokio")),
            Err(GovernanceError::BudgetExceeded("max_new_dependencies"))
        );
    }

    #[test]
    fn effectful_calls_also_count_toward_total_tool_budget() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        assert!(session
            .authorize(GenerationAction::InvokeTool {
                name: "write",
                effectful: true,
            })
            .is_ok());
        assert!(session
            .authorize(GenerationAction::InvokeTool {
                name: "read1",
                effectful: false,
            })
            .is_ok());
        assert!(session
            .authorize(GenerationAction::InvokeTool {
                name: "read2",
                effectful: false,
            })
            .is_ok());
        assert!(session
            .authorize(GenerationAction::InvokeTool {
                name: "read3",
                effectful: false,
            })
            .is_ok());
        assert_eq!(
            session.authorize(GenerationAction::InvokeTool {
                name: "read4",
                effectful: false,
            }),
            Err(GovernanceError::BudgetExceeded("max_tool_calls"))
        );
    }

    #[test]
    fn invalid_hex_in_verifying_key_rejected() {
        let mut signed = signed_envelope();
        signed.verifying_key_hex = "zz".repeat(32);
        assert_eq!(
            signed.verify(),
            Err(GovernanceError::InvalidHex("verifying_key_hex"))
        );
    }

    #[test]
    fn wrong_length_verifying_key_rejected() {
        let mut signed = signed_envelope();
        signed.verifying_key_hex = "aa".repeat(16);
        assert_eq!(
            signed.verify(),
            Err(GovernanceError::InvalidHex("verifying_key_hex"))
        );
    }

    #[test]
    fn invalid_hex_in_signature_rejected() {
        let mut signed = signed_envelope();
        signed.signature_hex = "zz".repeat(64);
        assert_eq!(
            signed.verify(),
            Err(GovernanceError::InvalidHex("signature_hex"))
        );
    }

    #[test]
    fn wrong_length_signature_rejected() {
        let mut signed = signed_envelope();
        signed.signature_hex = "aa".repeat(32);
        assert_eq!(
            signed.verify(),
            Err(GovernanceError::InvalidHex("signature_hex"))
        );
    }

    #[test]
    fn from_json_and_verify_round_trips() {
        let signed = signed_envelope();
        let json = serde_json::to_vec(&signed).unwrap();
        let recovered = SignedChangeEnvelope::from_json_and_verify(&json).unwrap();
        assert_eq!(recovered.envelope.change_id, signed.envelope.change_id);
    }

    #[test]
    fn from_json_and_verify_rejects_bad_json() {
        let result = SignedChangeEnvelope::from_json_and_verify(b"not json");
        assert!(matches!(result, Err(GovernanceError::Serialization(_))));
    }

    #[test]
    fn commitment_symbols_outside_envelope_rejected() {
        let signed = signed_envelope();
        let commitment = DesignCommitment {
            change_id: "CHG-1".into(),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            intended_paths: set(&["src/auth.rs"]),
            intended_symbols: set(&["redeem", "sneak_fn"]),
            intended_effects: set(&["ledger.atomic-write"]),
            evidence_plan: set(&["property:double-redemption"]),
            unresolved_assumptions: BTreeSet::new(),
        };
        assert_eq!(
            signed.envelope.validate_commitment(&commitment),
            Err(GovernanceError::InvalidCommitment("symbols"))
        );
    }

    #[test]
    fn commitment_effects_outside_envelope_rejected() {
        let signed = signed_envelope();
        let commitment = DesignCommitment {
            change_id: "CHG-1".into(),
            requirement_refs: set(&["REQ-AUTH-17"]),
            invariant_refs: set(&["INV-SINGLE-USE"]),
            intended_paths: set(&["src/auth.rs"]),
            intended_symbols: set(&["redeem"]),
            intended_effects: set(&["ledger.atomic-write", "network.egress"]),
            evidence_plan: set(&["property:double-redemption"]),
            unresolved_assumptions: BTreeSet::new(),
        };
        assert_eq!(
            signed.envelope.validate_commitment(&commitment),
            Err(GovernanceError::InvalidCommitment("effects"))
        );
    }

    #[test]
    fn session_usage_tracks_all_counters() {
        let mut session = GovernanceSession::admit(signed_envelope()).unwrap();
        session
            .authorize(GenerationAction::WritePath("src/auth.rs"))
            .unwrap();
        session
            .authorize(GenerationAction::InvokeTool {
                name: "read",
                effectful: false,
            })
            .unwrap();
        session
            .authorize(GenerationAction::InvokeTool {
                name: "write",
                effectful: true,
            })
            .unwrap();
        let u = session.usage();
        assert_eq!(u.distinct_written_paths.len(), 1);
        assert_eq!(u.tool_calls, 2);
        assert_eq!(u.effectful_tool_calls, 1);
        assert!(u.added_dependencies.is_empty());
    }

    #[test]
    fn expired_envelope_rejected() {
        let mut signed = signed_envelope();
        signed.envelope.expires_at = "2020-01-01T00:00:00Z".into();
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let digest = signed.envelope.digest_sha256().unwrap();
        let signature = signing_key.sign(digest.as_bytes());
        signed.signature_hex = hex::encode(signature.to_bytes());
        assert!(matches!(
            GovernanceSession::admit_at(signed, 1_700_000_000),
            Err(GovernanceError::Expired(_))
        ));
    }

    #[test]
    fn malformed_expiry_rejected() {
        let mut signed = signed_envelope();
        signed.envelope.expires_at = "not-a-date".into();
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let digest = signed.envelope.digest_sha256().unwrap();
        let signature = signing_key.sign(digest.as_bytes());
        signed.signature_hex = hex::encode(signature.to_bytes());
        assert!(matches!(
            GovernanceSession::admit_at(signed, 1_700_000_000),
            Err(GovernanceError::MalformedExpiry(_))
        ));
    }

    #[test]
    fn unexpired_envelope_admits() {
        let signed = signed_envelope();
        assert!(GovernanceSession::admit_at(signed, 1_700_000_000).is_ok());
    }
}

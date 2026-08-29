//! Dense, registry-scoped execution tokens derived from canonical Braid IR.
//!
//! Canonical capsules keep protocol-stable term names because names are the
//! cross-system identity. The admitted hot path should not carry one heap
//! `String` and one heap `Vec` per strand. This module therefore derives a
//! compact, non-canonical view:
//!
//! ```text
//! (registry CID, canonical term name) -> TermToken
//! strand-local Vec<input>             -> one shared input arena
//! ```
//!
//! A [`TermToken`] is meaningful only with the exact registry CID carried by
//! [`TokenProgram`]. It is never a global identifier by itself, and this
//! representation never replaces the canonical bytes used for content
//! addressing or independent admission.

use crate::admission::AdmissionTriad;
use crate::capsule::Capsule;
use crate::cid::Cid;
use crate::term::{TermRegistry, TermSpec};
use alloc::vec::Vec;

/// Dense ordinal of a term in one canonical registry.
///
/// The scope is the registry CID. Persisting or comparing the integer without
/// that CID is a protocol error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct TermToken(u32);

impl TermToken {
    /// Return the dense integer used by the hot path.
    pub const fn get(self) -> u32 {
        self.0
    }

    fn from_index(index: usize) -> Result<Self, TokenError> {
        checked_u32(index, "term registry").map(Self)
    }
}

/// Fixed-width operation header. Inputs live in [`TokenProgram`]'s shared
/// input arena at `input_start..input_start + input_len`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TokenOp {
    term: TermToken,
    input_start: u32,
    input_len: u32,
}

impl TokenOp {
    /// Registry-scoped term token.
    pub const fn term(self) -> TermToken {
        self.term
    }

    /// First input index in the shared input arena.
    pub const fn input_start(self) -> u32 {
        self.input_start
    }

    /// Number of input indices in the shared input arena.
    pub const fn input_len(self) -> u32 {
        self.input_len
    }
}

/// Dense lookup table over an immutable canonical registry.
///
/// Construction allocates one pointer per term once. Token resolution is then
/// O(1), while name-to-token elaboration is O(log n) over the already sorted
/// canonical registry order.
#[derive(Debug)]
pub struct TermTable<'a> {
    registry_cid: Cid,
    terms: Vec<&'a TermSpec>,
}

impl<'a> TermTable<'a> {
    /// Bind a dense table to an exact registry.
    pub fn new(registry: &'a TermRegistry) -> Result<Self, TokenError> {
        if registry.len() > u32::MAX as usize {
            return Err(TokenError::RegistryTooLarge {
                count: registry.len(),
            });
        }
        Ok(Self {
            registry_cid: registry.cid(),
            terms: registry.terms().collect(),
        })
    }

    /// Exact registry identity that scopes every token in this table.
    pub const fn registry_cid(&self) -> Cid {
        self.registry_cid
    }

    /// Number of dense term entries.
    pub fn len(&self) -> usize {
        self.terms.len()
    }

    /// Whether the registry has no terms.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Resolve a canonical term name to its dense token.
    pub fn token_for(&self, term_id: &str) -> Option<TermToken> {
        self.terms
            .binary_search_by(|spec| spec.id.as_str().cmp(term_id))
            .ok()
            .and_then(|index| TermToken::from_index(index).ok())
    }

    /// Resolve a dense token in O(1).
    pub fn resolve(&self, token: TermToken) -> Option<&'a TermSpec> {
        self.terms.get(token.0 as usize).copied()
    }
}

/// Why a canonical graph could not be projected into the dense hot form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    /// Capsule and term table commit to different registries.
    RegistryMismatch {
        /// Registry CID pinned by the capsule.
        capsule: Cid,
        /// Registry CID used for tokenization.
        table: Cid,
    },
    /// A registry cannot fit in the 32-bit token namespace.
    RegistryTooLarge {
        /// Number of terms observed.
        count: usize,
    },
    /// A compact arena or index cannot fit in its 32-bit representation.
    ProgramTooLarge {
        /// Collection or index that overflowed.
        field: &'static str,
        /// Observed host-size value.
        count: usize,
    },
    /// A strand names no term in the bound registry.
    UnknownTerm {
        /// Topological strand index.
        strand: usize,
    },
    /// An input is not a preceding strand.
    InvalidInput {
        /// Topological strand index.
        strand: usize,
        /// Invalid referenced strand.
        input: u32,
    },
    /// A declared output is outside the operation arena.
    InvalidOutput {
        /// Invalid referenced strand.
        output: u32,
    },
}

impl core::fmt::Display for TokenError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RegistryMismatch { capsule, table } => write!(
                f,
                "token registry mismatch: capsule {} table {}",
                capsule.to_hex(),
                table.to_hex()
            ),
            Self::RegistryTooLarge { count } => {
                write!(f, "term registry has {count} entries; maximum is u32::MAX")
            }
            Self::ProgramTooLarge { field, count } => {
                write!(f, "{field} has {count} entries; maximum is u32::MAX")
            }
            Self::UnknownTerm { strand } => {
                write!(f, "unknown term at strand {strand}")
            }
            Self::InvalidInput { strand, input } => {
                write!(f, "strand {strand} references non-preceding input {input}")
            }
            Self::InvalidOutput { output } => {
                write!(f, "output references missing strand {output}")
            }
        }
    }
}

impl core::error::Error for TokenError {}

fn checked_u32(value: usize, field: &'static str) -> Result<u32, TokenError> {
    u32::try_from(value).map_err(|_| TokenError::ProgramTooLarge {
        field,
        count: value,
    })
}

fn total_input_count(capsule: &Capsule) -> Result<usize, TokenError> {
    capsule
        .braid
        .strands
        .iter()
        .try_fold(0usize, |total, strand| {
            total
                .checked_add(strand.inputs.len())
                .ok_or(TokenError::ProgramTooLarge {
                    field: "input arena",
                    count: usize::MAX,
                })
        })
}

/// Dense, immutable execution projection of one canonical capsule.
///
/// This form is intentionally not serialized as the capsule identity. The
/// canonical graph remains the source of truth; the program is a cacheable
/// projection bound to both capsule and registry CIDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenProgram {
    capsule_cid: Cid,
    registry_cid: Cid,
    admission: AdmissionTriad,
    ops: Vec<TokenOp>,
    inputs: Vec<u32>,
    outputs: Vec<u32>,
}

impl TokenProgram {
    /// Derive a compact program and compute the canonical capsule CID.
    ///
    /// This performs representation checks, not independent admission. Only a
    /// verifier-owned wrapper may treat the resulting program as admitted.
    pub fn derive(
        capsule: &Capsule,
        registry: &TermRegistry,
        admission: AdmissionTriad,
    ) -> Result<Self, TokenError> {
        let table = TermTable::new(registry)?;
        Self::derive_bound(capsule, &table, capsule.cid(), admission)
    }

    /// Derive a compact program using a CID already computed from the exact
    /// canonical bytes being admitted.
    ///
    /// This avoids a second canonical encoding in the independent verifier.
    /// Callers outside a verifier should prefer [`Self::derive`].
    pub fn derive_bound(
        capsule: &Capsule,
        table: &TermTable<'_>,
        capsule_cid: Cid,
        admission: AdmissionTriad,
    ) -> Result<Self, TokenError> {
        if capsule.registry_cid != table.registry_cid {
            return Err(TokenError::RegistryMismatch {
                capsule: capsule.registry_cid,
                table: table.registry_cid,
            });
        }

        checked_u32(capsule.braid.strands.len(), "operation arena")?;
        let total_inputs = total_input_count(capsule)?;
        checked_u32(total_inputs, "input arena")?;
        checked_u32(capsule.braid.outputs.len(), "output arena")?;

        let mut ops = Vec::with_capacity(capsule.braid.strands.len());
        let mut inputs = Vec::with_capacity(total_inputs);

        for (strand_index, strand) in capsule.braid.strands.iter().enumerate() {
            let term = table
                .token_for(&strand.term)
                .ok_or(TokenError::UnknownTerm {
                    strand: strand_index,
                })?;
            let input_start = checked_u32(inputs.len(), "input arena")?;
            let input_len = checked_u32(strand.inputs.len(), "strand inputs")?;
            for &input in &strand.inputs {
                if input as usize >= strand_index {
                    return Err(TokenError::InvalidInput {
                        strand: strand_index,
                        input,
                    });
                }
                inputs.push(input);
            }
            ops.push(TokenOp {
                term,
                input_start,
                input_len,
            });
        }

        let mut outputs = Vec::with_capacity(capsule.braid.outputs.len());
        for &output in &capsule.braid.outputs {
            if output as usize >= ops.len() {
                return Err(TokenError::InvalidOutput { output });
            }
            outputs.push(output);
        }

        Ok(Self {
            capsule_cid,
            registry_cid: table.registry_cid,
            admission,
            ops,
            inputs,
            outputs,
        })
    }

    /// Canonical capsule identity this projection was derived from.
    pub const fn capsule_cid(&self) -> Cid {
        self.capsule_cid
    }

    /// Exact registry identity that scopes all operation tokens.
    pub const fn registry_cid(&self) -> Cid {
        self.registry_cid
    }

    /// Current Safety × Capability × Justification projection.
    pub const fn admission(&self) -> AdmissionTriad {
        self.admission
    }

    /// Fixed-width operation arena.
    pub fn ops(&self) -> &[TokenOp] {
        &self.ops
    }

    /// Shared topological input-index arena.
    pub fn input_arena(&self) -> &[u32] {
        &self.inputs
    }

    /// Topological output indices.
    pub fn outputs(&self) -> &[u32] {
        &self.outputs
    }

    /// Inputs for one operation, with bounds checked against the private arena.
    pub fn inputs_for(&self, op_index: usize) -> Option<&[u32]> {
        let op = self.ops.get(op_index)?;
        let start = op.input_start as usize;
        let end = start.checked_add(op.input_len as usize)?;
        self.inputs.get(start..end)
    }

    /// Approximate bytes occupied by the dense variable-sized arenas.
    ///
    /// This excludes allocator metadata and the fixed-size `TokenProgram`
    /// header. It is diagnostic only, never a budget proof.
    pub fn hot_arena_bytes(&self) -> usize {
        self.ops
            .len()
            .saturating_mul(core::mem::size_of::<TokenOp>())
            .saturating_add(
                self.inputs
                    .len()
                    .saturating_mul(core::mem::size_of::<u32>()),
            )
            .saturating_add(
                self.outputs
                    .len()
                    .saturating_mul(core::mem::size_of::<u32>()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::{AdmissionAxis, InvocationDecision, ProofState};
    use crate::braid::{Braid, Strand};
    use crate::capsule::{Capsule, ConfirmPolicy, IR_VERSION};
    use crate::term::{EffectClass, Exposure, TermRegistry, TermSpec, TypeTag};
    use alloc::vec;
    use core::mem::size_of;

    fn fixture() -> (TermRegistry, Capsule) {
        let mut registry = TermRegistry::new(7);
        registry
            .insert(TermSpec {
                id: "z.consume".into(),
                inputs: vec![TypeTag::Int],
                output: TypeTag::Bool,
                capability: None,
                effect: EffectClass::Pure,
                source_exposure: Exposure::Public,
                egress_ceiling: None,
                cost: 1,
            })
            .unwrap();
        registry
            .insert(TermSpec {
                id: "a.literal".into(),
                inputs: vec![],
                output: TypeTag::Int,
                capability: None,
                effect: EffectClass::Pure,
                source_exposure: Exposure::Public,
                egress_ceiling: None,
                cost: 1,
            })
            .unwrap();

        let capsule = Capsule {
            ir_version: IR_VERSION,
            vocab_version: registry.vocab_version,
            registry_cid: registry.cid(),
            intent: "compact fixture".into(),
            grants: vec![],
            braid: Braid {
                strands: vec![
                    Strand {
                        term: "a.literal".into(),
                        inputs: vec![],
                    },
                    Strand {
                        term: "z.consume".into(),
                        inputs: vec![0],
                    },
                ],
                outputs: vec![1],
            },
            budget: 2,
            confirm: ConfirmPolicy::None,
            evidence: vec![],
        };
        (registry, capsule)
    }

    #[test]
    fn operation_header_is_twelve_bytes() {
        assert_eq!(size_of::<TermToken>(), 4);
        assert_eq!(size_of::<TokenOp>(), 12);
    }

    #[test]
    fn canonical_registry_order_defines_dense_tokens() {
        let (registry, _) = fixture();
        let table = TermTable::new(&registry).unwrap();
        assert_eq!(table.token_for("a.literal").unwrap().get(), 0);
        assert_eq!(table.token_for("z.consume").unwrap().get(), 1);
        assert_eq!(
            table.resolve(TermToken(1)).unwrap().id.as_str(),
            "z.consume"
        );
    }

    #[test]
    fn graph_projects_to_fixed_ops_and_one_input_arena() {
        let (registry, capsule) = fixture();
        let triad =
            AdmissionTriad::new(ProofState::Proven, ProofState::Proven, ProofState::Unknown);
        let program = TokenProgram::derive(&capsule, &registry, triad).unwrap();

        assert_eq!(program.ops().len(), 2);
        assert_eq!(program.ops()[0].term().get(), 0);
        assert_eq!(program.ops()[1].term().get(), 1);
        assert_eq!(program.input_arena(), &[0]);
        assert_eq!(program.inputs_for(0), Some(&[] as &[u32]));
        assert_eq!(program.inputs_for(1), Some(&[0][..]));
        assert_eq!(program.outputs(), &[1]);
        assert_eq!(
            program.admission().decision(),
            InvocationDecision::Defer {
                axis: AdmissionAxis::Justification
            }
        );
        assert_eq!(program.hot_arena_bytes(), 32);
    }

    #[test]
    fn token_scope_is_bound_to_registry_cid() {
        let (registry, mut capsule) = fixture();
        capsule.registry_cid = Cid([9; 32]);
        assert!(matches!(
            TokenProgram::derive(&capsule, &registry, AdmissionTriad::default()),
            Err(TokenError::RegistryMismatch { .. })
        ));
    }

    #[test]
    fn invalid_topology_cannot_enter_compact_program() {
        let (registry, mut capsule) = fixture();
        capsule.braid.strands[1].inputs = vec![1];
        assert!(matches!(
            TokenProgram::derive(&capsule, &registry, AdmissionTriad::default()),
            Err(TokenError::InvalidInput {
                strand: 1,
                input: 1
            })
        ));
    }
}

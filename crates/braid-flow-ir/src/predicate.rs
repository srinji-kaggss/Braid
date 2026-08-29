//! Closed, side-effect-free predicate syntax for justification and guards.

use crate::error::{FlowError, FlowResult, LimitKind};
use crate::flow::{CompletionClass, FlowBounds, HARD_MAX_PREDICATE_NODES, OutputPort};
use crate::literal::validate_literal;
use crate::symbol::{FactRef, InputKey, NodeKey};
use alloc::boxed::Box;
use alloc::vec::Vec;
use braid_ir::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueExpr {
    Literal(Value),
    RootInput(InputKey),
    NodeOutput(OutputPort),
    SnapshotFact(FactRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    Const(bool),
    Eq(ValueExpr, ValueExpr),
    Ne(ValueExpr, ValueExpr),
    Lt(ValueExpr, ValueExpr),
    Le(ValueExpr, ValueExpr),
    Gt(ValueExpr, ValueExpr),
    Ge(ValueExpr, ValueExpr),
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
    HasCompletion {
        node: NodeKey,
        class: CompletionClass,
    },
}

impl Predicate {
    /// Canonical semantic representation used for content identity.
    pub fn to_canon(&self) -> Value {
        crate::encode::predicate_to_canon(self)
    }

    pub(crate) fn validate(
        &mut self,
        bounds: &FlowBounds,
        remaining_nodes: &mut usize,
        remaining_literal_bytes: &mut usize,
        remaining_literal_nodes: &mut usize,
    ) -> FlowResult<()> {
        self.validate_at(
            bounds,
            remaining_nodes,
            remaining_literal_bytes,
            remaining_literal_nodes,
            1,
        )
    }

    fn validate_at(
        &mut self,
        bounds: &FlowBounds,
        remaining_nodes: &mut usize,
        remaining_literal_bytes: &mut usize,
        remaining_literal_nodes: &mut usize,
        depth: usize,
    ) -> FlowResult<()> {
        let depth_limit = usize::from(bounds.max_predicate_depth);
        if depth > depth_limit {
            return Err(FlowError::LimitExceeded {
                kind: LimitKind::PredicateDepth,
                actual: depth,
                limit: depth_limit,
                invariant: "INV-FLOW-005",
            });
        }
        if *remaining_nodes == 0 {
            return Err(FlowError::LimitExceeded {
                kind: LimitKind::PredicateNodes,
                actual: HARD_MAX_PREDICATE_NODES + 1,
                limit: HARD_MAX_PREDICATE_NODES,
                invariant: "INV-FLOW-004",
            });
        }
        *remaining_nodes -= 1;
        match self {
            Self::And(items) | Self::Or(items) => {
                if items.is_empty() {
                    return Err(FlowError::EmptyCollection {
                        field: "predicate operands",
                        invariant: "INV-FLOW-005",
                    });
                }
                for child in items.iter_mut() {
                    child.validate_at(
                        bounds,
                        remaining_nodes,
                        remaining_literal_bytes,
                        remaining_literal_nodes,
                        depth + 1,
                    )?;
                }
                items.sort_by_cached_key(|predicate| {
                    braid_ir::encode(&crate::encode::predicate_to_canon(predicate))
                });
                items.dedup();
            }
            Self::Not(inner) => inner.validate_at(
                bounds,
                remaining_nodes,
                remaining_literal_bytes,
                remaining_literal_nodes,
                depth + 1,
            )?,
            Self::Eq(left, right)
            | Self::Ne(left, right)
            | Self::Lt(left, right)
            | Self::Le(left, right)
            | Self::Gt(left, right)
            | Self::Ge(left, right) => {
                validate_expression(left, remaining_literal_bytes, remaining_literal_nodes)?;
                validate_expression(right, remaining_literal_bytes, remaining_literal_nodes)?;
            }
            Self::Const(_) | Self::HasCompletion { .. } => {}
        }
        Ok(())
    }
}

fn validate_expression(
    expression: &ValueExpr,
    remaining_literal_bytes: &mut usize,
    remaining_literal_nodes: &mut usize,
) -> FlowResult<()> {
    if let ValueExpr::Literal(value) = expression {
        validate_literal(value, remaining_literal_bytes, remaining_literal_nodes)?;
    }
    Ok(())
}

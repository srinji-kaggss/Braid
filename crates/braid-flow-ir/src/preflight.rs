//! Allocation-free resource preflight for untrusted canonical Flow bytes.
//!
//! Canonical decoding materializes a generic `Value` before semantic
//! projection. This first pass therefore checks the outer declared bounds,
//! total byte/value envelopes, and canonical nesting without reserving from
//! attacker-controlled container counts.

use crate::error::{FlowError, FlowResult, LimitKind};
use crate::flow::{
    FlowBounds, HARD_MAX_PREDICATE_NODES, HARD_MAX_SOURCE_EDGES, HARD_MAX_SOURCE_NODES,
    PROVISIONAL_MAX_JUSTIFICATION_REFERENCES, PROVISIONAL_MAX_JUSTIFICATION_REFERENCES_PER_FLOW,
    PROVISIONAL_MAX_TYPE_TAG_NODES_PER_FLOW,
};
use crate::literal::{MAX_LITERAL_BYTES, MAX_LITERAL_NODES};

const MAX_WIRE_BYTES: usize = 128 * 1024 * 1024;
const MAX_WIRE_VALUES: usize = 3_000_000;
const INV_BOUNDS: &str = "INV-FLOW-004";
const INV_SHAPE: &str = "INV-FLOW-018";

pub(crate) fn validate(bytes: &[u8]) -> FlowResult<()> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(FlowError::LimitExceeded {
            kind: LimitKind::WireBytes,
            actual: bytes.len(),
            limit: MAX_WIRE_BYTES,
            invariant: INV_BOUNDS,
        });
    }
    let mut scanner = Scanner {
        bytes,
        position: 0,
        values: 0,
        literal_bytes: 0,
        literal_nodes: 0,
        predicate_nodes: 0,
        type_nodes: 0,
        references: 0,
    };
    scanner.flow()?;
    if scanner.position != bytes.len() {
        return Err(malformed("trailing bytes"));
    }
    Ok(())
}

fn malformed(field: &'static str) -> FlowError {
    FlowError::Malformed {
        field,
        invariant: INV_SHAPE,
    }
}

struct Scanner<'a> {
    bytes: &'a [u8],
    position: usize,
    values: usize,
    literal_bytes: usize,
    literal_nodes: usize,
    predicate_nodes: usize,
    type_nodes: usize,
    references: usize,
}

#[derive(Clone, Copy)]
enum FieldRole {
    Literal,
    Guarantees,
    Preserves,
    Type,
    PredicateScalar,
    PredicateComparison,
    PredicateOperands,
    PredicateCompletion,
    ChoiceArms,
    TypeArguments,
    CompletionClasses,
    ExactPair,
    MaybePair,
    Other,
}

fn field_role(key: &str) -> FieldRole {
    match key {
        "literal" => FieldRole::Literal,
        "guarantees" => FieldRole::Guarantees,
        "preserves" => FieldRole::Preserves,
        "primitive" | "opaque" | "list" => FieldRole::Type,
        "const" | "not" => FieldRole::PredicateScalar,
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" => FieldRole::PredicateComparison,
        "and" | "or" => FieldRole::PredicateOperands,
        "has_completion" => FieldRole::PredicateCompletion,
        "arms" => FieldRole::ChoiceArms,
        "arguments" => FieldRole::TypeArguments,
        "on" => FieldRole::CompletionClasses,
        "node" | "node_output" => FieldRole::ExactPair,
        "to" => FieldRole::MaybePair,
        _ => FieldRole::Other,
    }
}

impl Scanner<'_> {
    fn flow(&mut self) -> FlowResult<()> {
        self.record_value(0)?;
        let count = self.container_head(5, "flow")?;
        if count != 7 {
            return Err(malformed("flow"));
        }

        let mut seen = 0u8;
        let mut roots = None;
        let mut nodes = None;
        let mut edges = None;
        let mut terminals = None;
        let mut bounds = None;
        for _ in 0..count {
            let key = self.key()?;
            let bit = match key {
                "name" => 1 << 0,
                "roots" => 1 << 1,
                "nodes" => 1 << 2,
                "edges" => 1 << 3,
                "terminals" => 1 << 4,
                "bounds" => 1 << 5,
                "version" => 1 << 6,
                _ => return Err(malformed("flow field")),
            };
            if seen & bit != 0 {
                return Err(malformed("duplicate flow field"));
            }
            seen |= bit;
            match key {
                "name" => self.scan_text(1, 128, "name")?,
                "roots" => {
                    roots = Some(self.scan_list(1, HARD_MAX_SOURCE_NODES as usize, "roots")?)
                }
                "nodes" => {
                    nodes = Some(self.scan_list(1, HARD_MAX_SOURCE_NODES as usize, "nodes")?)
                }
                "edges" => {
                    edges = Some(self.scan_list(1, HARD_MAX_SOURCE_EDGES as usize, "edges")?)
                }
                "terminals" => {
                    terminals =
                        Some(self.scan_text_list(1, HARD_MAX_SOURCE_NODES as usize, "terminals")?)
                }
                "bounds" => bounds = Some(self.bounds(1)?),
                "version" => {
                    self.record_value(1)?;
                    let version = self.unsigned("version")?;
                    if version > u16::MAX as u64 {
                        return Err(malformed("version"));
                    }
                }
                _ => unreachable!(),
            }
        }
        if seen != 0x7f {
            return Err(malformed("flow"));
        }

        let bounds = bounds.ok_or_else(|| malformed("bounds"))?;
        bounds.validate()?;
        let roots = roots.ok_or_else(|| malformed("roots"))?;
        let nodes = nodes.ok_or_else(|| malformed("nodes"))?;
        let edges = edges.ok_or_else(|| malformed("edges"))?;
        let terminals = terminals.ok_or_else(|| malformed("terminals"))?;
        self.check_count(LimitKind::Roots, roots, bounds.max_nodes as usize)?;
        self.check_count(LimitKind::SourceNodes, nodes, bounds.max_nodes as usize)?;
        self.check_count(
            LimitKind::ExpandedNodes,
            nodes,
            bounds.max_expanded_nodes as usize,
        )?;
        self.check_count(LimitKind::SourceEdges, edges, bounds.max_edges as usize)?;
        self.check_count(
            LimitKind::ExpandedEdges,
            edges,
            bounds.max_expanded_edges as usize,
        )?;
        if terminals == 0 {
            return Err(FlowError::EmptyCollection {
                field: "terminals",
                invariant: "INV-FLOW-014",
            });
        }
        self.check_count(
            LimitKind::Terminals,
            terminals,
            nodes.min(bounds.max_nodes as usize),
        )
    }

    fn bounds(&mut self, depth: usize) -> FlowResult<FlowBounds> {
        self.record_value(depth)?;
        let count = self.container_head(5, "bounds")?;
        if count != 5 {
            return Err(malformed("bounds"));
        }
        let mut seen = 0u8;
        let mut values = [0u32; 5];
        for _ in 0..count {
            let key = self.key()?;
            let index = match key {
                "max_nodes" => 0,
                "max_edges" => 1,
                "max_predicate_depth" => 2,
                "max_expanded_nodes" => 3,
                "max_expanded_edges" => 4,
                _ => return Err(malformed("bounds field")),
            };
            let bit = 1 << index;
            if seen & bit != 0 {
                return Err(malformed("duplicate bounds field"));
            }
            seen |= bit;
            self.record_value(depth + 1)?;
            values[index] =
                u32::try_from(self.unsigned("bound")?).map_err(|_| malformed("bound"))?;
        }
        if seen != 0x1f {
            return Err(malformed("bounds"));
        }
        let predicate_depth =
            u16::try_from(values[2]).map_err(|_| malformed("max_predicate_depth"))?;
        Ok(FlowBounds {
            max_nodes: values[0],
            max_edges: values[1],
            max_predicate_depth: predicate_depth,
            max_expanded_nodes: values[3],
            max_expanded_edges: values[4],
        })
    }

    fn scan_list(&mut self, depth: usize, limit: usize, field: &'static str) -> FlowResult<usize> {
        self.record_value(depth)?;
        let count = self.container_head(4, field)?;
        self.check_count_for_field(count, limit, field)?;
        self.require_minimum_remaining(count, field)?;
        for _ in 0..count {
            self.value(depth + 1)?;
        }
        Ok(count)
    }

    fn scan_text_list(
        &mut self,
        depth: usize,
        limit: usize,
        field: &'static str,
    ) -> FlowResult<usize> {
        self.record_value(depth)?;
        let count = self.container_head(4, field)?;
        self.check_count_for_field(count, limit, field)?;
        self.require_minimum_remaining(count, field)?;
        for _ in 0..count {
            self.scan_text(depth + 1, 128, field)?;
        }
        Ok(count)
    }

    fn scan_text(&mut self, depth: usize, limit: usize, field: &'static str) -> FlowResult<()> {
        self.record_value(depth)?;
        let length = self.container_head(3, field)?;
        if length > limit {
            return Err(malformed(field));
        }
        let raw = self.take(length, field)?;
        core::str::from_utf8(raw).map_err(|_| malformed(field))?;
        Ok(())
    }

    fn value(&mut self, depth: usize) -> FlowResult<()> {
        self.record_value(depth)?;
        let head = self.byte("value")?;
        let major = head >> 5;
        if major == 7 {
            return if matches!(head, 0xf4 | 0xf5) {
                Ok(())
            } else {
                Err(malformed("simple value"))
            };
        }
        let count = self.argument(head, "value")?;
        match major {
            0 | 1 => Ok(()),
            2 => {
                let length = self.usize(count, "bytes")?;
                if length > 32 {
                    return Err(malformed("bytes"));
                }
                self.take(length, "bytes").map(|_| ())
            }
            3 => {
                let length = self.usize(count, "text")?;
                if length > 128 {
                    return Err(malformed("text"));
                }
                let raw = self.take(length, "text")?;
                core::str::from_utf8(raw).map_err(|_| malformed("text"))?;
                Ok(())
            }
            4 => {
                let count = self.usize(count, "list")?;
                self.check_count(LimitKind::CanonicalValues, count, 128)?;
                self.require_minimum_remaining(count, "list")?;
                for _ in 0..count {
                    self.value(depth + 1)?;
                }
                Ok(())
            }
            5 => {
                let count = self.usize(count, "map")?;
                self.check_count(LimitKind::CanonicalValues, count, 7)?;
                let minimum = count.checked_mul(2).ok_or_else(|| malformed("map"))?;
                self.require_minimum_remaining(minimum, "map")?;
                for _ in 0..count {
                    let role = field_role(self.key()?);
                    match role {
                        FieldRole::Literal => self.literal_payload(depth + 1)?,
                        FieldRole::Guarantees => self.reference_list(depth + 1, "guarantees")?,
                        FieldRole::Preserves => self.reference_list(depth + 1, "preserves")?,
                        FieldRole::Type => {
                            self.type_nodes = self.checked_budget(
                                self.type_nodes,
                                1,
                                PROVISIONAL_MAX_TYPE_TAG_NODES_PER_FLOW,
                                LimitKind::TypeTagNodes,
                            )?;
                            self.value(depth + 1)?;
                        }
                        FieldRole::PredicateScalar => {
                            self.predicate_nodes = self.checked_budget(
                                self.predicate_nodes,
                                1,
                                HARD_MAX_PREDICATE_NODES,
                                LimitKind::PredicateNodes,
                            )?;
                            self.value(depth + 1)?;
                        }
                        FieldRole::PredicateComparison => {
                            self.consume_predicate_node()?;
                            self.structural_list(
                                depth + 1,
                                2,
                                2,
                                LimitKind::PredicateNodes,
                                "comparison",
                            )?;
                        }
                        FieldRole::PredicateOperands => {
                            self.consume_predicate_node()?;
                            self.structural_list(
                                depth + 1,
                                1,
                                HARD_MAX_PREDICATE_NODES,
                                LimitKind::PredicateNodes,
                                "predicate operands",
                            )?;
                        }
                        FieldRole::PredicateCompletion => {
                            self.consume_predicate_node()?;
                            self.structural_list(
                                depth + 1,
                                2,
                                2,
                                LimitKind::PredicateNodes,
                                "has_completion",
                            )?;
                        }
                        FieldRole::ChoiceArms => self.structural_list(
                            depth + 1,
                            1,
                            128,
                            LimitKind::ChoiceArms,
                            "choice arms",
                        )?,
                        FieldRole::TypeArguments => self.structural_list(
                            depth + 1,
                            0,
                            128,
                            LimitKind::TypeTagNodes,
                            "opaque arguments",
                        )?,
                        FieldRole::CompletionClasses => self.structural_list(
                            depth + 1,
                            1,
                            3,
                            LimitKind::CompletionClasses,
                            "completion classes",
                        )?,
                        FieldRole::ExactPair => self.structural_list(
                            depth + 1,
                            2,
                            2,
                            LimitKind::CanonicalValues,
                            "tuple",
                        )?,
                        FieldRole::MaybePair => {
                            if self.next_major()? == 4 {
                                self.structural_list(
                                    depth + 1,
                                    2,
                                    2,
                                    LimitKind::CanonicalValues,
                                    "input port",
                                )?;
                            } else {
                                self.value(depth + 1)?;
                            }
                        }
                        FieldRole::Other => self.value(depth + 1)?,
                    }
                }
                Ok(())
            }
            _ => Err(malformed("value")),
        }
    }

    fn consume_predicate_node(&mut self) -> FlowResult<()> {
        self.predicate_nodes = self.checked_budget(
            self.predicate_nodes,
            1,
            HARD_MAX_PREDICATE_NODES,
            LimitKind::PredicateNodes,
        )?;
        Ok(())
    }

    fn structural_list(
        &mut self,
        depth: usize,
        minimum: usize,
        maximum: usize,
        kind: LimitKind,
        field: &'static str,
    ) -> FlowResult<()> {
        self.record_value(depth)?;
        let count = self.container_head(4, field)?;
        if count < minimum {
            return Err(malformed(field));
        }
        self.check_count(kind, count, maximum)?;
        self.require_minimum_remaining(count, field)?;
        for _ in 0..count {
            self.value(depth + 1)?;
        }
        Ok(())
    }

    fn next_major(&self) -> FlowResult<u8> {
        self.bytes
            .get(self.position)
            .map(|head| head >> 5)
            .ok_or_else(|| malformed("value"))
    }

    fn literal_payload(&mut self, depth: usize) -> FlowResult<()> {
        let start = self.position;
        self.literal_value(depth)?;
        let bytes = self
            .position
            .checked_sub(start)
            .ok_or(FlowError::ArithmeticOverflow {
                field: "literal byte count",
                invariant: INV_BOUNDS,
            })?;
        self.literal_bytes = self.checked_budget(
            self.literal_bytes,
            bytes,
            MAX_LITERAL_BYTES,
            LimitKind::LiteralBytes,
        )?;
        Ok(())
    }

    fn literal_value(&mut self, depth: usize) -> FlowResult<()> {
        self.record_value(depth)?;
        self.literal_nodes = self.checked_budget(
            self.literal_nodes,
            1,
            MAX_LITERAL_NODES,
            LimitKind::LiteralNodes,
        )?;
        let head = self.byte("literal")?;
        let major = head >> 5;
        if major == 7 {
            return if matches!(head, 0xf4 | 0xf5) {
                Ok(())
            } else {
                Err(malformed("literal"))
            };
        }
        let count = self.argument(head, "literal")?;
        match major {
            0 | 1 => Ok(()),
            2 | 3 => {
                let length = self.usize(count, "literal")?;
                let raw = self.take(length, "literal")?;
                if major == 3 {
                    core::str::from_utf8(raw).map_err(|_| malformed("literal"))?;
                }
                Ok(())
            }
            4 => {
                let count = self.usize(count, "literal")?;
                self.require_minimum_remaining(count, "literal")?;
                for _ in 0..count {
                    self.literal_value(depth + 1)?;
                }
                Ok(())
            }
            5 => {
                let count = self.usize(count, "literal")?;
                let minimum = count.checked_mul(2).ok_or_else(|| malformed("literal"))?;
                self.require_minimum_remaining(minimum, "literal")?;
                for _ in 0..count {
                    self.key()?;
                    self.literal_value(depth + 1)?;
                }
                Ok(())
            }
            _ => Err(malformed("literal")),
        }
    }

    fn reference_list(&mut self, depth: usize, field: &'static str) -> FlowResult<()> {
        self.record_value(depth)?;
        let count = self.container_head(4, field)?;
        self.check_count(
            LimitKind::References,
            count,
            PROVISIONAL_MAX_JUSTIFICATION_REFERENCES,
        )?;
        self.references = self.checked_budget(
            self.references,
            count,
            PROVISIONAL_MAX_JUSTIFICATION_REFERENCES_PER_FLOW,
            LimitKind::References,
        )?;
        self.require_minimum_remaining(count, field)?;
        for _ in 0..count {
            self.scan_text(depth + 1, 128, field)?;
        }
        Ok(())
    }

    fn checked_budget(
        &self,
        current: usize,
        amount: usize,
        limit: usize,
        kind: LimitKind,
    ) -> FlowResult<usize> {
        let actual = current
            .checked_add(amount)
            .ok_or(FlowError::ArithmeticOverflow {
                field: "preflight semantic budget",
                invariant: INV_BOUNDS,
            })?;
        if actual > limit {
            Err(FlowError::LimitExceeded {
                kind,
                actual,
                limit,
                invariant: INV_BOUNDS,
            })
        } else {
            Ok(actual)
        }
    }

    fn key(&mut self) -> FlowResult<&str> {
        let head = self.byte("map key")?;
        if head >> 5 != 3 {
            return Err(malformed("map key"));
        }
        let length = self.argument(head, "map key")?;
        let length = self.usize(length, "map key")?;
        let raw = self.take(length, "map key")?;
        core::str::from_utf8(raw).map_err(|_| malformed("map key"))
    }

    fn unsigned(&mut self, field: &'static str) -> FlowResult<u64> {
        let head = self.byte(field)?;
        if head >> 5 != 0 {
            return Err(malformed(field));
        }
        self.argument(head, field)
    }

    fn container_head(&mut self, major: u8, field: &'static str) -> FlowResult<usize> {
        let head = self.byte(field)?;
        let value = self.argument(head, field)?;
        if head >> 5 != major {
            return Err(malformed(field));
        }
        self.usize(value, field)
    }

    fn argument(&mut self, head: u8, field: &'static str) -> FlowResult<u64> {
        match head & 0x1f {
            value @ 0..=23 => Ok(u64::from(value)),
            24 => {
                let value = u64::from(self.byte(field)?);
                if value < 24 {
                    Err(FlowError::Canon(braid_ir::CanonError::NonMinimalInt {
                        at: field,
                    }))
                } else {
                    Ok(value)
                }
            }
            25 => {
                let raw: [u8; 2] = self
                    .take(2, field)?
                    .try_into()
                    .map_err(|_| malformed(field))?;
                let value = u64::from(u16::from_be_bytes(raw));
                if value <= u8::MAX as u64 {
                    Err(FlowError::Canon(braid_ir::CanonError::NonMinimalInt {
                        at: field,
                    }))
                } else {
                    Ok(value)
                }
            }
            26 => {
                let raw: [u8; 4] = self
                    .take(4, field)?
                    .try_into()
                    .map_err(|_| malformed(field))?;
                let value = u64::from(u32::from_be_bytes(raw));
                if value <= u16::MAX as u64 {
                    Err(FlowError::Canon(braid_ir::CanonError::NonMinimalInt {
                        at: field,
                    }))
                } else {
                    Ok(value)
                }
            }
            27 => {
                let raw: [u8; 8] = self
                    .take(8, field)?
                    .try_into()
                    .map_err(|_| malformed(field))?;
                let value = u64::from_be_bytes(raw);
                if value <= u32::MAX as u64 {
                    Err(FlowError::Canon(braid_ir::CanonError::NonMinimalInt {
                        at: field,
                    }))
                } else {
                    Ok(value)
                }
            }
            _ => Err(malformed(field)),
        }
    }

    fn byte(&mut self, field: &'static str) -> FlowResult<u8> {
        let value = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| malformed(field))?;
        self.position += 1;
        Ok(value)
    }

    fn take(&mut self, length: usize, field: &'static str) -> FlowResult<&[u8]> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| malformed(field))?;
        let raw = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| malformed(field))?;
        self.position = end;
        Ok(raw)
    }

    fn usize(&self, value: u64, field: &'static str) -> FlowResult<usize> {
        usize::try_from(value).map_err(|_| malformed(field))
    }

    fn record_value(&mut self, depth: usize) -> FlowResult<()> {
        if depth > braid_ir::canon::MAX_DEPTH {
            return Err(FlowError::LimitExceeded {
                kind: LimitKind::CanonicalDepth,
                actual: depth,
                limit: braid_ir::canon::MAX_DEPTH,
                invariant: INV_SHAPE,
            });
        }
        self.values = self
            .values
            .checked_add(1)
            .ok_or(FlowError::ArithmeticOverflow {
                field: "canonical value count",
                invariant: INV_BOUNDS,
            })?;
        if self.values > MAX_WIRE_VALUES {
            return Err(FlowError::LimitExceeded {
                kind: LimitKind::CanonicalValues,
                actual: self.values,
                limit: MAX_WIRE_VALUES,
                invariant: INV_BOUNDS,
            });
        }
        Ok(())
    }

    fn require_minimum_remaining(&self, count: usize, field: &'static str) -> FlowResult<()> {
        let remaining = self
            .bytes
            .len()
            .checked_sub(self.position)
            .ok_or_else(|| malformed(field))?;
        if count > remaining {
            Err(malformed(field))
        } else {
            Ok(())
        }
    }

    fn check_count(&self, kind: LimitKind, actual: usize, limit: usize) -> FlowResult<()> {
        if actual > limit {
            Err(FlowError::LimitExceeded {
                kind,
                actual,
                limit,
                invariant: INV_BOUNDS,
            })
        } else {
            Ok(())
        }
    }

    fn check_count_for_field(
        &self,
        actual: usize,
        limit: usize,
        field: &'static str,
    ) -> FlowResult<()> {
        let kind = match field {
            "roots" => LimitKind::Roots,
            "nodes" => LimitKind::SourceNodes,
            "edges" => LimitKind::SourceEdges,
            "terminals" => LimitKind::Terminals,
            _ => LimitKind::CanonicalValues,
        };
        self.check_count(kind, actual, limit)
    }
}

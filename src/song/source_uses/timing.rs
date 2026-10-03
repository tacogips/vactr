//! Opaque, immutable operands copied before scalar/rest timing is erased.
use super::{layout::FrozenUseSlotLayout, FrozenUseOperation, FrozenUseTraceTerm};
use crate::reader::span::NodeId;
use crate::value::Ratio64;
use crate::vm::fail::{FailCode, Failure};
/// Issued only by the bounded candidate capture pass.
/// ```compile_fail
/// use vactr::song::source_uses::timing::FrozenIndexTiming;
/// fn forge(recipe: &mut FrozenIndexTiming) { recipe.root = 0; }
/// ```
/// ```compile_fail
/// use vactr::song::source_uses::timing::FrozenIndexTimingNode;
/// fn forge(node: &mut FrozenIndexTimingNode) { node.structured = false; }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenIndexTiming {
    root: u32,
    nodes: Vec<FrozenIndexTimingNode>,
    requires_realization: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenIndexTimingNode {
    pub(crate) issuer: NodeId,
    pub(crate) operation: FrozenUseOperation,
    pub(crate) structured: bool,
    pub(crate) leaf: Option<FrozenIndexLeaf>,
    pub(crate) leaf_trace: Vec<FrozenUseTraceTerm>,
    pub(crate) parameters: Vec<FrozenIndexParameter>,
    pub(crate) slots: Vec<FrozenIndexSlot>,
    pub(crate) children: Vec<FrozenIndexChild>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenIndexNumber {
    Int(i32),
    Int64(i64),
    Ratio(Ratio64),
    FloatBits(u32),
    Float64Bits(u64),
}
#[derive(Clone, Debug, PartialEq)]
pub enum FrozenIndexLeaf {
    Rest,
    Scalar { number: Option<FrozenIndexNumber> },
    AtomicList { elements: Vec<FrozenIndexLeaf> },
    Pattern { child: u32 },
    Continuous,
    Dynamic(FrozenIndexDynamicKind),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenIndexDynamicKind {
    Function,
    Native,
    Late,
    Thunk,
    Signal,
    UnresolvedPattern,
}
#[derive(Clone, Debug, PartialEq)]
pub enum FrozenIndexParameter {
    Number(FrozenIndexNumber),
    Literal(FrozenIndexLeaf),
    Pattern { child: u32 },
    Dynamic(FrozenIndexDynamicKind),
}
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenIndexSlot {
    pub(crate) ordinal: u32,
    pub(crate) geometry: Option<FrozenUseSlotLayout>,
    pub(crate) copies: Option<u32>,
    pub(crate) weight: FrozenIndexParameter,
    pub(crate) count: Option<FrozenIndexParameter>,
    pub(crate) child: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenIndexChild {
    pub(crate) role: u32,
    pub(crate) child: u32,
    pub(crate) trace: Vec<FrozenUseTraceTerm>,
}
impl FrozenIndexTiming {
    #[must_use]
    pub const fn root(&self) -> u32 {
        self.root
    }
    #[must_use]
    pub fn nodes(&self) -> &[FrozenIndexTimingNode] {
        &self.nodes
    }
    #[must_use]
    pub const fn requires_realization(&self) -> bool {
        self.requires_realization
    }
    /// Retaining this opaque recipe clones one Rc, not its owned graph.
    pub(crate) fn retained_copy_work(&self) -> Result<u32, Failure> {
        Ok(1)
    }
    pub(crate) fn issue(
        root: u32,
        nodes: Vec<FrozenIndexTimingNode>,
        requires_realization: bool,
        remaining: &mut u32,
        max_depth: u32,
    ) -> Result<Self, Failure> {
        admit(remaining, nodes.len().checked_mul(2).ok_or_else(overflow)?)?;
        if root as usize >= nodes.len() {
            return Err(invalid("index timing root out of range"));
        }
        let mut heights = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            admit(remaining, 1)?;
            admit(remaining, node.leaf_trace.len())?;
            let mut height = 1u32;
            if let Some(leaf) = &node.leaf {
                height = height.max(leaf_height(leaf, index, &heights, remaining, 0, max_depth)?);
            }
            for param in &node.parameters {
                height = height.max(parameter_height(
                    param, index, &heights, remaining, max_depth,
                )?);
            }
            for edge in &node.children {
                admit(
                    remaining,
                    edge.trace.len().checked_add(1).ok_or_else(overflow)?,
                )?;
                height = height.max(
                    reference(edge.child, index, &heights)?
                        .checked_add(1)
                        .ok_or_else(overflow)?,
                );
            }
            for slot in &node.slots {
                admit(remaining, 3)?;
                height = height.max(parameter_height(
                    &slot.weight,
                    index,
                    &heights,
                    remaining,
                    max_depth,
                )?);
                if let Some(count) = &slot.count {
                    height = height.max(parameter_height(
                        count, index, &heights, remaining, max_depth,
                    )?);
                }
                height = height.max(
                    reference(slot.child, index, &heights)?
                        .checked_add(1)
                        .ok_or_else(overflow)?,
                );
                if let Some(g) = &slot.geometry {
                    if g.prefix < Ratio64::ZERO
                        || g.width <= Ratio64::ZERO
                        || g.prefix > Ratio64::ONE
                    {
                        return Err(invalid("invalid normalized index slot"));
                    }
                    if let Some(copies) = slot.copies {
                        let end = g.prefix.checked_add(
                            g.width.checked_mul(Ratio64::from_int(i64::from(copies)))?,
                        )?;
                        if end > Ratio64::ONE {
                            return Err(invalid("index slot exceeds cycle"));
                        }
                    }
                    if let Some(term) = g.copy_trace_term {
                        let mut found = None;
                        for edge in &node.children {
                            admit(remaining, 1)?;
                            if edge.role == slot.ordinal {
                                found = Some(edge);
                                break;
                            }
                        }
                        let edge =
                            found.ok_or_else(|| invalid("index slot has no producer edge"))?;
                        if !matches!(edge.trace.get(term as usize),Some(FrozenUseTraceTerm::Copies{count,..}) if Some(*count)==slot.copies)
                        {
                            return Err(invalid("index copy term differs"));
                        }
                    }
                }
            }
            if height > max_depth {
                return Err(Failure::new(
                    FailCode::DepthExceeded,
                    "index timing graph depth exhausted",
                ));
            }
            heights.push(height);
        }
        admit(remaining, 1)?;
        Ok(Self {
            root,
            nodes,
            requires_realization,
        })
    }
}
fn parameter_height(
    param: &FrozenIndexParameter,
    index: usize,
    heights: &[u32],
    remaining: &mut u32,
    max_depth: u32,
) -> Result<u32, Failure> {
    admit(remaining, 1)?;
    match param {
        FrozenIndexParameter::Pattern { child } => reference(*child, index, heights)?
            .checked_add(1)
            .ok_or_else(overflow),
        FrozenIndexParameter::Literal(leaf) => {
            leaf_height(leaf, index, heights, remaining, 0, max_depth)
        }
        _ => Ok(1),
    }
}

fn reference(child: u32, index: usize, heights: &[u32]) -> Result<u32, Failure> {
    if child as usize >= index {
        return Err(invalid("cyclic or forward index timing reference"));
    }
    heights
        .get(child as usize)
        .copied()
        .ok_or_else(|| invalid("index timing child out of range"))
}
fn leaf_height(
    leaf: &FrozenIndexLeaf,
    index: usize,
    heights: &[u32],
    remaining: &mut u32,
    depth: u32,
    max_depth: u32,
) -> Result<u32, Failure> {
    if depth >= max_depth {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "index atomic leaf depth exhausted",
        ));
    }
    admit(remaining, 1)?;
    match leaf {
        FrozenIndexLeaf::Pattern { child } => reference(*child, index, heights)?
            .checked_add(1)
            .ok_or_else(overflow),
        FrozenIndexLeaf::AtomicList { elements } => {
            admit(remaining, elements.len())?;
            let mut height = 1u32;
            for element in elements {
                height = height.max(
                    leaf_height(element, index, heights, remaining, depth + 1, max_depth)?
                        .checked_add(1)
                        .ok_or_else(overflow)?,
                );
            }
            Ok(height)
        }
        _ => Ok(1),
    }
}
pub(crate) fn admit(remaining: &mut u32, cost: usize) -> Result<(), Failure> {
    let n = u32::try_from(cost).map_err(|_| overflow())?;
    *remaining = remaining.checked_sub(n).ok_or_else(|| {
        Failure::new(
            FailCode::FuelExhausted,
            "index timing capture work exhausted",
        )
    })?;
    Ok(())
}
pub(crate) fn overflow() -> Failure {
    Failure::new(FailCode::Overflow, "index timing arithmetic overflow")
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}
impl FrozenIndexTimingNode {
    #[must_use]
    pub const fn issuer(&self) -> NodeId {
        self.issuer
    }
    #[must_use]
    pub const fn operation(&self) -> FrozenUseOperation {
        self.operation
    }
    #[must_use]
    pub const fn structured(&self) -> bool {
        self.structured
    }
    #[must_use]
    pub fn leaf(&self) -> Option<&FrozenIndexLeaf> {
        self.leaf.as_ref()
    }
    #[must_use]
    pub fn leaf_trace(&self) -> &[FrozenUseTraceTerm] {
        &self.leaf_trace
    }
    #[must_use]
    pub fn parameters(&self) -> &[FrozenIndexParameter] {
        &self.parameters
    }
    #[must_use]
    pub fn slots(&self) -> &[FrozenIndexSlot] {
        &self.slots
    }
    #[must_use]
    pub fn children(&self) -> &[FrozenIndexChild] {
        &self.children
    }
}
impl FrozenIndexSlot {
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    #[must_use]
    pub const fn geometry(&self) -> Option<&FrozenUseSlotLayout> {
        self.geometry.as_ref()
    }
    #[must_use]
    pub const fn copies(&self) -> Option<u32> {
        self.copies
    }
    #[must_use]
    pub const fn weight(&self) -> &FrozenIndexParameter {
        &self.weight
    }
    #[must_use]
    pub const fn count(&self) -> Option<&FrozenIndexParameter> {
        self.count.as_ref()
    }
    #[must_use]
    pub const fn child(&self) -> u32 {
        self.child
    }
}
impl FrozenIndexChild {
    #[must_use]
    pub const fn role(&self) -> u32 {
        self.role
    }
    #[must_use]
    pub const fn child(&self) -> u32 {
        self.child
    }
    #[must_use]
    pub fn trace(&self) -> &[FrozenUseTraceTerm] {
        &self.trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn leaf() -> FrozenIndexTimingNode {
        FrozenIndexTimingNode {
            issuer: NodeId::new(0),
            operation: FrozenUseOperation::Pure,
            structured: false,
            leaf: Some(FrozenIndexLeaf::Rest),
            leaf_trace: Vec::new(),
            parameters: Vec::new(),
            slots: Vec::new(),
            children: Vec::new(),
        }
    }
    #[test]
    fn private_reference_and_literal_validation_cannot_issue_cycles() {
        for case in 0..4 {
            let mut node = leaf();
            match case {
                0 => node.children.push(FrozenIndexChild {
                    role: 0,
                    child: 0,
                    trace: Vec::new(),
                }),
                1 => {
                    node.parameters
                        .push(FrozenIndexParameter::Literal(FrozenIndexLeaf::Pattern {
                            child: 2,
                        }))
                }
                2 => node.slots.push(FrozenIndexSlot {
                    ordinal: 0,
                    geometry: None,
                    copies: Some(1),
                    weight: FrozenIndexParameter::Pattern { child: 1 },
                    count: None,
                    child: 0,
                }),
                _ => {
                    node.leaf = Some(FrozenIndexLeaf::AtomicList {
                        elements: vec![FrozenIndexLeaf::Pattern { child: 0 }],
                    })
                }
            }
            let mut remaining = 1000;
            assert_eq!(
                FrozenIndexTiming::issue(0, vec![node], false, &mut remaining, 256)
                    .unwrap_err()
                    .code,
                FailCode::Type
            );
            assert!(remaining < 1000);
        }
        assert_eq!(
            FrozenIndexTiming::issue(1, vec![leaf()], false, &mut 1000, 256)
                .unwrap_err()
                .code,
            FailCode::Type
        );
    }
    #[test]
    fn nested_literal_depth_and_exact_validation_cost_are_shared() {
        let mut node = leaf();
        node.parameters
            .push(FrozenIndexParameter::Literal(FrozenIndexLeaf::AtomicList {
                elements: vec![FrozenIndexLeaf::AtomicList {
                    elements: vec![FrozenIndexLeaf::Rest],
                }],
            }));
        let mut remaining = 1000;
        let issued =
            FrozenIndexTiming::issue(0, vec![node.clone()], false, &mut remaining, 256).unwrap();
        let cost = 1000 - remaining;
        assert!(cost > 5);
        let mut exact = cost;
        assert_eq!(
            FrozenIndexTiming::issue(0, vec![node.clone()], false, &mut exact, 256).unwrap(),
            issued
        );
        assert_eq!(exact, 0);
        assert_eq!(
            FrozenIndexTiming::issue(0, vec![node.clone()], false, &mut (cost - 1), 256)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        assert_eq!(
            FrozenIndexTiming::issue(0, vec![node], false, &mut 1000, 2)
                .unwrap_err()
                .code,
            FailCode::DepthExceeded
        );
    }
}

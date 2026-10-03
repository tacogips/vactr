//! Static integer predicates copied from the runtime cycle-wise transforms.
use super::{
    use_invalid, FrozenSourceUseNode, FrozenUseMapping, FrozenUseOperation, FrozenUseTraceTerm,
};
use crate::pattern::occ::{ProducerKind, ProducerStep};
use crate::pattern::query::TimeSpan;
use crate::value::Ratio64;
use crate::vm::Failure;

/// Exact runtime integer parameters; negative values and large counts are retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrozenStaticCondition {
    Every { period: i64 },
    WhenMod { modulus: i64, threshold: i64 },
    Chunk { divisions: i64 },
}
impl FrozenStaticCondition {
    /// Normalized fractional anchor region assigned to the transformed child.
    ///
    /// Every and WhenMod select the complete cycle or none. Chunk selects an
    /// exact fractional slice, irrespective of the emitted event's clipped part.
    ///
    /// # Errors
    /// Checked rational construction fails.
    pub fn anchor_slice(self, cycle: i64) -> Result<Option<TimeSpan>, Failure> {
        let active = match self {
            Self::Every { period } => period > 0 && cycle.rem_euclid(period) == 0,
            Self::WhenMod { modulus, threshold } => {
                modulus > 0 && cycle.rem_euclid(modulus) >= threshold
            }
            Self::Chunk { divisions } => {
                if divisions <= 0 {
                    return Ok(None);
                }
                let residue = cycle.rem_euclid(divisions);
                return Ok(Some(TimeSpan::new(
                    Ratio64::new(residue, divisions)?,
                    Ratio64::new(residue + 1, divisions)?,
                )?));
            }
        };
        if active {
            Ok(Some(TimeSpan::new(Ratio64::ZERO, Ratio64::ONE)?))
        } else {
            Ok(None)
        }
    }
    /// Applies the runtime predicate to a whole event anchor, never its part.
    ///
    /// # Errors
    /// Checked rational slice construction fails.
    pub fn selects(self, cycle: i64, anchor: Ratio64) -> Result<bool, Failure> {
        let Some(slice) = self.anchor_slice(cycle)? else {
            return Ok(false);
        };
        let fraction = anchor.frac();
        Ok(slice.begin <= fraction && fraction < slice.end)
    }
}
pub(super) fn validate(node: &FrozenSourceUseNode) -> Result<(), Failure> {
    let FrozenUseMapping::ConditionalStatic(condition) = node.mapping else {
        return Ok(());
    };
    let expected = match condition {
        FrozenStaticCondition::Every { .. } => FrozenUseOperation::Every,
        FrozenStaticCondition::WhenMod { .. } => FrozenUseOperation::WhenMod,
        FrozenStaticCondition::Chunk { .. } => FrozenUseOperation::Chunk,
    };
    if node.operation != expected || node.edges.len() != 2 {
        return Err(use_invalid(
            "static conditional operation or edge count mismatch",
        ));
    }
    let term = |kind, ordinal| FrozenUseTraceTerm::Exact(ProducerStep { kind, ordinal });
    if !node.edges[0].layout.is_empty()
        || !node.edges[1].layout.is_empty()
        || node.edges[0].trace != [term(ProducerKind::Child, 0)]
        || node.edges[1].trace
            != [
                term(ProducerKind::DynamicExpansion, 1),
                term(ProducerKind::Child, 1),
            ]
    {
        return Err(use_invalid(
            "static conditional child trace or layout mismatch",
        ));
    }
    Ok(())
}

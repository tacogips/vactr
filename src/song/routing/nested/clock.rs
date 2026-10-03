//! Original root-to-issuer affine maps; selected source clocks are separate.
use super::*;
pub(super) struct IssuerClock {
    pub factor: Ratio64,
    pub shift: Ratio64,
    pub owner: crate::pattern::TimeSpan,
    pub birth_owner: crate::pattern::TimeSpan,
}
pub(super) fn issuer_clock(
    stage: &Stage<'_>,
    count: usize,
    owner: crate::pattern::TimeSpan,
    offset: Ratio64,
    budget: &mut ResolutionBudget,
) -> Result<Option<IssuerClock>, Failure> {
    use crate::song::source_uses::{layout, FrozenUseMapping as M};
    let mut owner = intersect(owner.map(|t| t.checked_sub(offset))?, stage.cover.window())?;
    let mut birth_owner = stage.cover.window();
    let mut factor = Ratio64::ONE;
    let mut shift = Ratio64::ZERO;
    let mut index = stage.cover.graph().root;
    let mut cursor = 0;
    for &ordinal in stage.identity.edges.iter().take(count) {
        budget.charge(1)?;
        let node = stage
            .cover
            .graph()
            .nodes
            .get(index as usize)
            .ok_or_else(|| invalid("issuer clock node"))?;
        let edge = node
            .edges
            .get(ordinal as usize)
            .ok_or_else(|| invalid("issuer clock edge"))?;
        let Some(end) = super::super::index::trace_end(&edge.trace, stage.trace, cursor, budget)?
        else {
            return Err(invalid("issuer clock source trace mismatch"));
        };
        if !edge.layout.is_empty() {
            budget.charge(edge.layout.len())?;
            let unit = layout::slot_interval(edge, 0, &stage.trace[cursor..end])?;
            if unit.begin != Ratio64::ZERO || unit.end != Ratio64::ONE {
                return Ok(None);
            }
        }
        match node.mapping {
            M::Rate { factor: rate } if rate > Ratio64::ZERO => {
                owner = owner.map(|t| t.checked_mul(rate))?;
                birth_owner = birth_owner.map(|t| t.checked_mul(rate))?;
                factor = factor.checked_mul(rate)?;
                shift = shift.checked_mul(rate)?;
            }
            M::Shift { amount } => {
                owner = owner.map(|t| t.checked_sub(amount))?;
                birth_owner = birth_owner.map(|t| t.checked_sub(amount))?;
                shift = shift.checked_sub(amount)?;
            }
            M::Preserve | M::Parallel | M::SelectContent { .. } => {}
            _ => return Ok(None),
        }
        cursor = end;
        index = edge.child;
    }
    Ok(Some(IssuerClock {
        factor,
        shift,
        owner,
        birth_owner,
    }))
}

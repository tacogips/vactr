//! Validate finite logical ceilings without expanding placements or repeats.
use super::*;
pub(super) fn finish(
    assembly: &mut Assembly,
    routes: &SongRoutePlan,
    clock: SongHostClock,
    remaining: &mut u32,
) -> Result<(), Failure> {
    for pool in &assembly.pools {
        charge(remaining, routes.branches.len())?;
        let route = routes
            .branches
            .iter()
            .find(|b| b.id == pool.logical)
            .ok_or_else(|| fail("physical pool logical route missing"))?;
        if pool.slot >= route.reserved_generations
            || !pool.initial.valid()
            || pool.initial.last_generation != demand::generation_ceiling(route)?
            || pool.initial.config.transition_frame != clock.frame
        {
            return Err(fail("physical pool generation contract invalid"));
        }
    }
    Ok(())
}

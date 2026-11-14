//! Measured physical demand, independent of logical declaration sharing.
use super::*;

pub(super) fn generation_ceiling(branch: &SongBranchRoute) -> Result<u32, Failure> {
    u32::try_from(branch.occurrences)
        .map_err(|_| fail("logical generation ceiling exceeds protocol range"))
}
fn checked_add(total: &mut u32, value: usize) -> Result<(), Failure> {
    let value = u32::try_from(value).map_err(|_| fail("physical demand count overflow"))?;
    *total = total
        .checked_add(value)
        .ok_or_else(|| fail("physical demand count overflow"))?;
    Ok(())
}
pub(super) fn from_assembly(
    assembly: &Assembly,
    routes: &SongRoutePlan,
    remaining: &mut u32,
) -> Result<SongPreparationDemand, Failure> {
    charge(remaining, assembly.resources.len())?;
    let mut required = routes.required;
    required.cell_slots = 0;
    required.sample_resources = 0;
    required.pcm_bytes = 0;
    let mut analysis = SongAnalysisCapacity { slots: 0 };
    for resource in &assembly.resources {
        checked_add(&mut required.cell_slots, resource.cells.len())?;
        analysis.slots = analysis
            .slots
            .checked_add(resource.analysis)
            .ok_or_else(|| fail("analysis demand overflow"))?;
        if let Some(Upload::Sample(data)) = &resource.upload {
            checked_add(&mut required.sample_resources, 1)?;
            let bytes = u64::try_from(data.frames.len())
                .ok()
                .and_then(|n| n.checked_mul(4))
                .ok_or_else(|| fail("PCM demand overflow"))?;
            required.pcm_bytes = required
                .pcm_bytes
                .checked_add(bytes)
                .ok_or_else(|| fail("PCM demand overflow"))?;
        }
    }
    let resources =
        u32::try_from(assembly.resources.len()).map_err(|_| fail("resource count overflow"))?;
    // Every exact lease can return independently; reserve explicit receipt room.
    required.ack_slots = required.ack_slots.max(
        resources
            .checked_add(2)
            .ok_or_else(|| fail("receipt demand overflow"))?,
    );
    let branches =
        u32::try_from(assembly.pools.len()).map_err(|_| fail("branch pool count overflow"))?;
    Ok(SongPreparationDemand {
        required,
        analysis,
        resources,
        branches,
    })
}
pub(super) fn verify_observation(
    demand: &SongPreparationDemand,
    report: SongCapacityReport,
    clock: SongHostClock,
) -> Result<(), Failure> {
    if report.available.sample_rate != clock.sample_rate || report.serial == 0 {
        return Err(fail("capacity and clock observations disagree"));
    }
    report.available.admit(demand.required)?;
    if demand.analysis.slots > report.analysis.slots {
        return Err(fail(
            "physical analysis bank demand exceeds observed capacity",
        ));
    }
    Ok(())
}

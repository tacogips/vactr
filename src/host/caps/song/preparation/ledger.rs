//! One serialized silent command followed by a genuine clock barrier.
use super::*;

fn control(command: SongCommand) -> Step {
    Step::Control(Box::new(command))
}

pub(super) fn steps(
    assembly: &Assembly,
    demand: SongPreparationDemand,
    epoch: SnapshotEpoch,
    max_records: u32,
    remaining: &mut u32,
) -> Result<Vec<Step>, Failure> {
    charge(remaining, assembly.resources.len())?;
    let mut count = 2_usize; // Begin and Seal.
    for resource in &assembly.resources {
        count = count
            .checked_add(1)
            .and_then(|n| n.checked_add(resource.cells.len()))
            .and_then(|n| n.checked_add(usize::from(resource.analysis != 0)))
            .and_then(|n| n.checked_add(usize::from(resource.banks.is_some())))
            .and_then(|n| n.checked_add(usize::from(resource.upload.is_some())))
            .ok_or_else(|| fail("staging script size overflow"))?;
    }
    count = count
        .checked_add(assembly.pools.len())
        .ok_or_else(|| fail("staging script size overflow"))?;
    if count > max_records as usize {
        return Err(fail("preparation record bound exceeded"));
    }
    let receipt_work = count
        .checked_mul(
            assembly
                .resources
                .len()
                .checked_add(1)
                .ok_or_else(|| fail("receipt ledger work overflow"))?,
        )
        .ok_or_else(|| fail("receipt ledger work overflow"))?;
    charge(
        remaining,
        receipt_work
            .checked_add(count)
            .and_then(|words| words.checked_mul(64))
            .ok_or_else(|| fail("script work overflow"))?,
    )?;
    let command_words = std::mem::size_of::<SongCommand>().div_ceil(std::mem::size_of::<u32>());
    charge(
        remaining,
        count
            .checked_mul(command_words)
            .ok_or_else(|| fail("script command storage overflow"))?,
    )?;
    let mut out = Vec::with_capacity(count);
    out.push(control(SongCommand::BeginStaging(SongStagePreparation {
        preparation: SongPreparation {
            epoch,
            branches: demand.branches,
            resources: demand.resources,
            required: demand.required,
        },
        analysis_required: demand.analysis,
    })));
    // Samples precede graphs in Assembly. Every bank is reserved/initialized
    // before its graph upload, so the host validates real retained bindings.
    for (index, resource) in assembly.resources.iter().enumerate() {
        out.push(control(SongCommand::ReserveResource(
            SongResourceReservation {
                epoch,
                resource: resource.key.resource,
                kind: resource.key.kind,
            },
        )));
        for cell in &resource.cells {
            out.push(control(SongCommand::InitCells(SongCellInit {
                lease: resource.key,
                cell: cell.cell,
                value: cell.value,
            })));
        }
        if resource.analysis != 0 {
            out.push(control(SongCommand::ReserveAnalysis(
                SongAnalysisReservation {
                    lease: resource.key,
                    slots: resource.analysis,
                },
            )));
        }
        if let Some(banks) = resource.banks {
            out.push(control(SongCommand::BindGraphBanks(banks)));
        }
        if resource.upload.is_some() {
            out.push(Step::Upload(index));
        }
    }
    for branch in &assembly.pools {
        out.push(control(SongCommand::ConfigureReusableBranch(
            branch.initial,
        )));
    }
    out.push(control(SongCommand::SealPreparation(epoch)));
    Ok(out)
}

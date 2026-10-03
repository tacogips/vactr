//! Measured full private regions for the original unchanged DSP oracles.
use super::*;
use vactr::dsp::bus::SongPrivateDelayLayout;

pub(super) fn configure_private_fixture_regions(
    config: &mut EngineConfig,
    caps: &CapabilitySet,
    sample_rate: u32,
) -> Result<(), String> {
    let rate = sample_rate as f32;
    let room = vactr::dsp::effects::mem_len(EffectKind::Room, rate, caps);
    let delay = usize::try_from(sample_rate)
        .map_err(|e| e.to_string())?
        .checked_mul(4)
        .and_then(|n| n.checked_add(4))
        .ok_or("delay extent overflow")?;
    // The original private templates are empty; master-tail FX are not private.
    let need = room
        .checked_add(delay.checked_mul(2).ok_or("stereo extent overflow")?)
        .ok_or("private extent overflow")?;
    config.bus_seconds = need as f32 / rate + 1.;
    Ok(())
}
fn assert_actual_private_fixture_layout(
    engine: &Engine,
    key: SongLeaseKey,
    template: &BusTemplate,
) -> SongPrivateDelayLayout {
    let layout = engine.song_private_delay_layout(key).unwrap();
    let physical = engine
        .buses()
        .slots
        .iter()
        .position(|s| {
            s.resource == key.resource.id
                && s.gen == key.resource.generation
                && s.state != SlotState::Free
        })
        .unwrap();
    assert_eq!(layout.left.slot as usize, physical);
    assert_eq!(layout.room.offset, 0);
    assert_eq!(layout.left.frames, 192004);
    assert_eq!(layout.right.frames, 192004);
    assert_eq!(layout.left.offset, layout.room.frames + layout.chain_frames);
    assert_eq!(layout.right.offset, layout.left.offset + layout.left.frames);
    assert!(layout.right.offset + layout.right.frames <= layout.storage_frames);
    assert_eq!(template.n, 0);
    layout
}
#[test]
fn migrated_original_private_rig_uses_real_full_regions() {
    for bytes in [false, true] {
        let r = Rig::prepared(bytes, 8);
        let a = assert_actual_private_fixture_layout(
            &r.engine,
            key(3, SongResourceKind::PrivateFx),
            &BusTemplate::new(),
        );
        let b = assert_actual_private_fixture_layout(
            &r.engine,
            key(4, SongResourceKind::PrivateFx),
            &BusTemplate::new(),
        );
        assert_ne!(a.left.slot, b.left.slot);
        assert!(a.left.slot > 0);
        assert_eq!(a.send_frames_per_channel, 16);
    }
}
#[test]
fn original_unrelated_small_track_and_master_contract_stays_supported() {
    let caps = CapabilitySet::browser();
    let cells = AtomicCells::new(4);
    let mut buses = vactr::dsp::bus::BusGraph::new(3, 16, 8192, &cells, 48000., &caps);
    for (id, kind) in [
        (91, SongResourceKind::Track),
        (92, SongResourceKind::Master),
    ] {
        let owner = key(id, kind);
        buses
            .stage_song_bus(owner, Box::new(BusTemplate::new()), &cells, 48000., &caps)
            .unwrap();
    }
    assert_eq!(
        buses
            .slots
            .iter()
            .filter(|s| s.state == SlotState::Staged)
            .count(),
        2
    );
}

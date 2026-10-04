use super::*;
use crate::dsp::arena::StoreKind;
use crate::dsp::caps::CapabilitySet;
use crate::host::native::audio::{NativeAudioHost, MAX_BLOCK};
use crate::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use crate::song::routing::{
    SongAnalysisCapacity, SongCapacityReport, SongHostCapacities, SongHostClock,
};

fn prepared_song() -> PreparedSong {
    let assets = DecodedSongAssetFactory::new(
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::new(),
    );
    let context = crate::session::song::CandidateBuildCtx {
        assets: &assets,
        asset_limits: SongAssetLimits {
            max_resources: 32,
            max_pcm_bytes: 100_000,
            max_source_files: 8,
            max_source_bytes: 20_000,
            max_banks: 8,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    let candidate = crate::session::song::evaluate_song_candidate(
        "song {part [tone: {s :analog > note 60 > gain 0.2}] duration: 1} tail-seconds: 0 > play-song",
        "preparation-issued-test.vact",
        42,
        SnapshotEpoch(41_001),
        &context,
    )
    .unwrap();
    crate::song::prepare_song(candidate).unwrap()
}

#[test]
fn route_work_refusal_happens_before_resource_reserve() {
    let caps = CapabilitySet::native();
    let config = crate::host::song_profile::song_engine_config(
        8000.,
        MAX_BLOCK,
        caps,
        StoreKind::NativeArc,
        2,
    )
    .unwrap();
    let (host, _side) = NativeAudioHost::headless_with_config(config, 1024).unwrap();
    let prepared = prepared_song();
    let epoch = prepared.epoch();
    let mut owner = SongHostPreparation::begin(
        prepared,
        SongPreparationLimits {
            capabilities: caps,
            song: SongLimits::default(),
            max_resources: 32,
            max_pending_records: 128,
            max_graph_bytes: 100_000,
            max_work: 1,
        },
    )
    .unwrap_or_else(|refusal| panic!("preparation refused: {}", refusal.failure));
    owner.report = Some(SongCapacityReport {
        epoch,
        serial: 1,
        available: SongHostCapacities {
            sample_rate: 8000,
            cell_slots: u32::MAX,
            voice_slots: u32::MAX,
            template_slots: u32::MAX,
            bus_slots: u32::MAX,
            sample_resources: u32::MAX,
            pcm_bytes: u64::MAX,
            voice_frames: u64::MAX,
            bus_frames: u64::MAX,
            ack_slots: u32::MAX,
        },
        analysis: SongAnalysisCapacity { slots: u32::MAX },
    });
    owner.clock = Some(SongHostClock {
        frame: 0,
        sample_rate: 8000,
    });
    let failure = owner.prepare(&host).unwrap_err();
    assert_eq!(failure.code, FailCode::FuelExhausted);
    assert!(!owner.cleanup.begun);
    assert!(owner.cleanup.assembly.resources.is_empty());
    assert!(owner.routes.is_none());
    assert!(owner.prepared.as_ref().unwrap().resources().is_empty());
}

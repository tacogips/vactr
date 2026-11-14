//! Public Subject-mode behavior; genuine internal Index coverage is separate.
use std::collections::BTreeMap;
use vactr::pattern::TimeSpan;
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::routing::{prepare_routes, resolve_route, SongHostCapacities};
use vactr::song::{PreparedSong, SnapshotEpoch, SongLimits};
use vactr::value::Ratio64;
fn span(begin: i64, end: i64, denominator: i64) -> TimeSpan {
    TimeSpan::new(
        Ratio64::new(begin, denominator).unwrap(),
        Ratio64::new(end, denominator).unwrap(),
    )
    .unwrap()
}
fn candidate(code: &str) -> PreparedSong {
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let cx = vactr::session::song::CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 100000,
            max_source_files: 8,
            max_source_bytes: 100000,
            max_banks: 8,
            max_walk_nodes: 100000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    vactr::song::prepare_song(
        vactr::session::song::evaluate_song_candidate(
            code,
            "occupancy.vact",
            1,
            SnapshotEpoch(1),
            &cx,
        )
        .unwrap(),
    )
    .unwrap()
}
fn capacities() -> SongHostCapacities {
    SongHostCapacities {
        sample_rate: 48000,
        cell_slots: 4096,
        voice_slots: 8,
        template_slots: 256,
        bus_slots: 256,
        sample_resources: 256,
        pcm_bytes: 16_000_000,
        voice_frames: 8 * 192000,
        bus_frames: 128_000_000,
        ack_slots: 1024,
    }
}
#[test]
fn structured_selected_slice_preserves_subject_clock_and_configuration() {
    let mut song = candidate("fn indexed p:\n\tslice p 2 [{slow 0 2} nil]\nlet base {part [drums: {s :analog}] duration: 4}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song");
    let limits = SongLimits::default();
    let rows = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].whole, Some(span(0, 1, 1)));
    assert_eq!(rows[1].whole, Some(span(1, 2, 1)));
    assert_eq!(rows[0].part, span(0, 1, 1));
    assert_eq!(rows[1].part, span(1, 2, 1));
    assert!(rows.iter().all(|row| row
        .source_origin
        .as_ref()
        .is_some_and(|origin| origin.slice_timings().is_empty())));
    let partial = song.query(span(5, 7, 8), &limits).unwrap();
    assert_eq!(
        partial.len(),
        1,
        "Subject mode samples index at subject whole start"
    );
    assert_eq!(partial[0].whole, Some(span(0, 1, 1)));
    assert_eq!(partial[0].part, span(5, 7, 8));
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &capacities(),
    )
    .unwrap();
    for row in rows.into_iter().chain(partial) {
        assert_eq!(
            resolve_route(&plan, &row, limits).unwrap().configuration,
            span(0, 4, 1)
        );
    }
}

#[test]
fn subject_nil_suppresses_notes_without_reopening_configuration() {
    let mut song = candidate("fn sliced p:\n\tslice p 2 {slow [0 nil] 2}\nlet base {part [drums: {s :analog}] duration: 4}\nlet selected {transform-instrument base :drums :analog sliced}\nsong selected tail-seconds: 0 > play-song");
    let limits = SongLimits::default();
    let rows = song.query(span(0, 4, 1), &limits).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].whole, Some(span(0, 1, 1)));
    assert_eq!(rows[1].whole, Some(span(2, 3, 1)));
    assert!(song.query(span(1, 2, 1), &limits).unwrap().is_empty());
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &capacities(),
    )
    .unwrap();
    for row in rows {
        assert!(row
            .source_origin
            .as_ref()
            .unwrap()
            .slice_timings()
            .is_empty());
        assert_eq!(
            resolve_route(&plan, &row, limits).unwrap().configuration,
            span(0, 4, 1)
        );
    }
}

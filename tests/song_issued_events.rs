//! Public descriptor compatibility; opaque issued proofs remain crate-private.
use std::collections::BTreeMap;
use vactr::{
    pattern::TimeSpan,
    session::song::{evaluate_song_candidate, CandidateBuildCtx},
    song::{
        assets::{DecodedSongAssetFactory, SongAssetLimits},
        prepare_song, SnapshotEpoch, SongLimits,
    },
    value::Ratio64,
};
#[test]
fn public_frozen_descriptors_remain_equal_across_repeated_queries_and_partitions() {
    let assets = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let mut prepared=prepare_song(evaluate_song_candidate("song {part [drums: {s :analog > chord [:c :five] > gain 0.25}] duration: 2} tail-seconds: 0 > play-song","public-issued.vact",1,SnapshotEpoch(991),&CandidateBuildCtx {assets:&assets, asset_limits:SongAssetLimits {max_resources:64,max_pcm_bytes:1_000_000,max_source_files:16,max_source_bytes:100_000,max_banks:16,max_walk_nodes:100_000,max_walk_depth:256},lock:None,cache:None}).unwrap()).unwrap();
    let whole = TimeSpan::new(Ratio64::ZERO, Ratio64::new(2, 1).unwrap()).unwrap();
    let first = prepared.query(whole, &SongLimits::default()).unwrap();
    assert_eq!(
        first,
        prepared.query(whole, &SongLimits::default()).unwrap()
    );
    assert!(!first.is_empty());
    assert!(first.iter().all(|event| !event.controls.is_empty()));
    for event in &first {
        let one = prepared.query(event.part, &SongLimits::default()).unwrap();
        assert!(one.iter().any(|part| part.handle == event.handle
            && part.whole == event.whole
            && part.note == event.note
            && part.controls == event.controls));
    }
}

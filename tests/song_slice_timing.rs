//! Actual isolated candidates expose copied Slice mode without invented timing.
use std::collections::BTreeMap;
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::FrozenPartNode;
use vactr::song::source_uses::{FrozenSliceStructure, FrozenUseMapping};
use vactr::song::{prepare_song, SnapshotEpoch, SongLimits};
fn candidate(code: &str) -> vactr::song::PreparedSong {
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let cx = CandidateBuildCtx {
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
    prepare_song(evaluate_song_candidate(code, "slice.vact", 1, SnapshotEpoch(1), &cx).unwrap())
        .unwrap()
}
#[test]
fn prepared_structured_selected_slice_copies_subject_mode() {
    let mut song=candidate("let base {part [drums: {s :analog}] duration: 4}\nlet changed {transform-instrument base :drums :analog {p -> slice p 2 [0 nil 1]}}\nsong changed > play-song");
    assert!(song
        .snapshot()
        .routing()
        .parts
        .iter()
        .any(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: vactr::song::snapshot::FrozenEdit::Transform { payload, .. },
                ..
            } => payload.source_uses.nodes.iter().any(|node| matches!(
                node.mapping,
                FrozenUseMapping::Slices {
                    structure: FrozenSliceStructure::Subject { .. },
                    ..
                }
            )),
            _ => false,
        }));
    let rows = song
        .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
        .unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        let origin = row.source_origin.unwrap();
        assert!(origin.slice_timings().is_empty());
        assert_eq!(origin.source_whole(), Some(TimeSpan::cycle(0).unwrap()));
    }
}
#[test]
fn prepared_unstructured_ordinary_slice_copies_index_mode() {
    let mut song =
        candidate("fn sliced p:\n\tslice p 2 [0 nil 1]\nsong {part [drums: {sliced {s :analog}}] duration: 4} > play-song");
    assert!(
        song.snapshot()
            .routing()
            .parts
            .iter()
            .any(|part| match &part.node {
                FrozenPartNode::Capture(tracks) => tracks.iter().any(|(_, payload)| payload
                    .source_uses
                    .nodes
                    .iter()
                    .any(|node| matches!(
                        node.mapping,
                        FrozenUseMapping::Slices {
                            structure: FrozenSliceStructure::Index { .. },
                            ..
                        }
                    ))),
                _ => false,
            }),
        "actual captured routing: {:#?}",
        song.snapshot().routing().parts
    );
    let rows = song
        .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.source_origin.is_none()));
}

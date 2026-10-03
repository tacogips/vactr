//! Authentic pre-transform duration and complete issued identity from real candidates.
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenSongEvent};
use vactr::song::source_uses::{certify_source_uses, resolve_source_use, FrozenSourceUseCover};
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
use vactr::value::Ratio64;
use vactr::vm::fail::FailCode;
fn candidate(code: &str) -> PreparedSong {
    let factory =
        DecodedSongAssetFactory::new(Default::default(), Default::default(), Default::default());
    let cx = CandidateBuildCtx {
        assets: &factory,
        asset_limits: SongAssetLimits {
            max_resources: 256,
            max_pcm_bytes: 1_000_000,
            max_source_files: 64,
            max_source_bytes: 1_000_000,
            max_banks: 64,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        },
        lock: None,
        cache: None,
    };
    prepare_song(evaluate_song_candidate(code, "whole.vact", 1, SnapshotEpoch(70), &cx).unwrap())
        .unwrap()
}
fn span(a: i64, b: i64, n: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, n).unwrap(), Ratio64::new(b, n).unwrap()).unwrap()
}
fn cover(song: &PreparedSong, window: TimeSpan) -> FrozenSourceUseCover {
    let inventory = song.snapshot().routing();
    let FrozenPartNode::Edit {
        edit: FrozenEdit::Transform { payload, .. },
        ..
    } = &inventory.parts[inventory.root_part].node
    else {
        panic!("actual transform")
    };
    certify_source_uses(inventory, payload, window, SongLimits::default()).unwrap()
}
fn origins(rows: &[FrozenSongEvent]) -> Vec<&vactr::song::source_uses::FrozenSourceOrigin> {
    rows.iter()
        .filter_map(|row| row.source_origin.as_ref())
        .collect()
}
#[test]
fn nonaligned_sampled_reflection_retains_authentic_pre_grid_whole() {
    let mut song = candidate("let base {part [drums: {fast {s :analog} 3}] duration: 1}\nlet reflected {transform-instrument base :drums :analog {p -> rev p}}\nlet grid {transform-instrument reflected :drums :analog {p -> euclid p 4 4}}\nsong grid tail-seconds: 0 > play-song");
    let rows = song.query(span(0, 1, 4), &SongLimits::default()).unwrap();
    let origin = rows[0].source_origin.as_ref().unwrap();
    assert_eq!(rows[0].whole, Some(span(0, 1, 4)));
    assert_eq!(origin.source_whole(), Some(span(0, 1, 3)));
    assert_eq!(origin.source_part, TimeSpan::point(Ratio64::ZERO));
    assert_eq!(origin.inherited.len(), 1);
    assert_eq!(origin.inherited[0].source_whole(), Some(span(2, 3, 3)));
    resolve_source_use(&cover(&song, span(0, 1, 4)), origin, SongLimits::default()).unwrap();
}
#[test]
fn long_release_whole_survives_point_and_reordered_queries() {
    let mut song = candidate("let base {part [drums: {slow {s :analog} 2}] duration: 1}\nlet selected {transform-instrument base :drums :analog {p -> p}}\nsong selected tail-seconds: 2 > play-song");
    let full = song.query(span(0, 1, 1), &SongLimits::default()).unwrap();
    let original = origins(&full)[0];
    assert_eq!(original.source_whole(), Some(span(0, 2, 1)));
    for window in [span(3, 4, 4), span(1, 1, 2), span(0, 1, 4), span(3, 3, 4)] {
        let rows = song.query(window, &SongLimits::default()).unwrap();
        assert!(!rows.is_empty());
        for origin in origins(&rows) {
            assert_eq!(origin.handle, original.handle);
            assert_eq!(origin.source_whole(), original.source_whole());
            resolve_source_use(&cover(&song, window), origin, SongLimits::default()).unwrap();
        }
    }
}
#[test]
fn same_onset_different_duration_handle_swap_is_rejected() {
    let mut song = candidate("let base {part [drums: {stack [{s :analog} {slow {s :analog} 2}]}] duration: 1}\nlet selected {transform-instrument base :drums :analog {p -> p}}\nsong selected > play-song");
    let rows = song.query(span(0, 1, 1), &SongLimits::default()).unwrap();
    let sources = origins(&rows);
    let first = sources[0];
    let second = sources
        .iter()
        .copied()
        .find(|x| {
            x.handle.occurrence().onset == first.handle.occurrence().onset
                && x.source_whole() != first.source_whole()
        })
        .unwrap();
    let admitted = cover(&song, span(0, 1, 1));
    resolve_source_use(&admitted, first, SongLimits::default()).unwrap();
    resolve_source_use(&admitted, second, SongLimits::default()).unwrap();
    let mut swapped = first.clone();
    swapped.handle = second.handle.clone();
    assert_eq!(swapped.source_whole(), first.source_whole());
    assert_eq!(
        resolve_source_use(&admitted, &swapped, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert_eq!(
        resolve_source_use(
            &admitted,
            &swapped,
            SongLimits {
                max_nodes: 1,
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
}
#[test]
fn every_inherited_frame_rejects_swapped_issued_identity() {
    let mut song = candidate("let base {part [drums: {stack [{s :analog} {slow {s :analog} 2}]}] duration: 1}\nlet inner {transform-instrument base :drums :analog {p -> p}}\nlet outer {transform-instrument inner :drums :analog {p -> p}}\nsong outer > play-song");
    let rows = song.query(span(0, 1, 1), &SongLimits::default()).unwrap();
    let sources = origins(&rows);
    let first = sources[0];
    let second = sources
        .iter()
        .copied()
        .find(|x| {
            x.inherited[0].source_whole() != first.inherited[0].source_whole()
                && x.handle.occurrence().onset == first.handle.occurrence().onset
        })
        .unwrap();
    assert_eq!(first.inherited.len(), 1);
    let inventory = song.snapshot().routing();
    let inner = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } if payload.sources.len() == 1
                && payload.sources[0].root_part != inventory.root_part =>
            {
                Some(payload)
            }
            _ => None,
        })
        .unwrap();
    // The matcher must reject authority drift before any graph-dependent search,
    // including a genuine inner handle substituted into another frame.
    let any_cover =
        certify_source_uses(inventory, inner, span(0, 1, 1), SongLimits::default()).unwrap();
    let mut frame = first.inherited[0].clone();
    frame.handle = second.inherited[0].handle.clone();
    assert_eq!(
        vactr::song::source_uses::resolve_source_use_frame(
            &any_cover,
            &frame,
            SongLimits::default()
        )
        .unwrap_err()
        .code,
        FailCode::Type
    );
    let mut outer = first.clone();
    outer.handle = second.handle.clone();
    assert_eq!(
        resolve_source_use(&cover(&song, span(0, 1, 1)), &outer, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
}
#[test]
fn source_authority_copy_and_matcher_share_exact_quota() {
    let mut song = candidate("let base {part [drums: {s :analog}] duration: 1}\nlet selected {transform-instrument base :drums :analog {p -> p}}\nsong selected > play-song");
    let rows = song.query(span(0, 1, 1), &SongLimits::default()).unwrap();
    let source = origins(&rows)[0];
    let admitted = cover(&song, span(0, 1, 1));
    let mut low = 1;
    let mut high = SongLimits::default().max_nodes;
    while low < high {
        let mid = low + (high - low) / 2;
        match resolve_source_use(
            &admitted,
            source,
            SongLimits {
                max_nodes: mid,
                ..Default::default()
            },
        ) {
            Ok(_) => high = mid,
            Err(error) => {
                assert_eq!(error.code, FailCode::FuelExhausted);
                low = mid + 1;
            }
        }
    }
    assert!(low > 24);
    let exact = SongLimits {
        max_nodes: low,
        ..Default::default()
    };
    assert_eq!(
        resolve_source_use(&admitted, source, exact).unwrap(),
        resolve_source_use(&admitted, &source.clone(), exact).unwrap()
    );
    assert_eq!(
        resolve_source_use(
            &admitted,
            source,
            SongLimits {
                max_nodes: low - 1,
                ..exact
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    let mut quota = low * 2;
    for _ in 0..2 {
        resolve_source_use(
            &admitted,
            source,
            SongLimits {
                max_nodes: quota.min(low),
                ..exact
            },
        )
        .unwrap();
        quota = quota.checked_sub(low).unwrap();
    }
    assert_eq!(quota, 0);
    // Query admission also includes the newly issued/copy authority. Measure the
    // real shared canonical-query boundary rather than reusing historic totals.
    let mut low = 1;
    let mut high = SongLimits::default().max_nodes;
    while low < high {
        let mid = low + (high - low) / 2;
        match song.query(
            span(0, 1, 1),
            &SongLimits {
                max_nodes: mid,
                ..Default::default()
            },
        ) {
            Ok(_) => high = mid,
            Err(error) => {
                assert_eq!(error.code, FailCode::FuelExhausted);
                low = mid + 1;
            }
        }
    }
    let exact_rows = song
        .query(
            span(0, 1, 1),
            &SongLimits {
                max_nodes: low,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(exact_rows, rows);
    assert_eq!(
        song.query(
            span(0, 1, 1),
            &SongLimits {
                max_nodes: low - 1,
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
}

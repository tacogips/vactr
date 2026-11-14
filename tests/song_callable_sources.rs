//! Genuine public function-returned selected source composition.
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
    candidate_with(code, &factory)
}
fn candidate_with(code: &str, factory: &DecodedSongAssetFactory) -> PreparedSong {
    let cx = vactr::session::song::CandidateBuildCtx {
        assets: factory,
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
            "callable.vact",
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
const FIXED: &str = "fn indexed p:\n\tslice {beat -> p} 2 [0 nil]\nlet base {part [drums: {s :analog}] duration: 4}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song";
#[test]
fn public_fixed_getter_issues_index_timings_and_reordered_full_routes() {
    let mut song = candidate(FIXED);
    let limits = SongLimits::default();
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &capacities(),
    )
    .unwrap();
    let rows = song.query(span(0, 4, 1), &limits).unwrap();
    assert_eq!(rows.len(), 4);
    let mut routes = Vec::new();
    for (cycle, row) in rows.iter().enumerate() {
        let c = i64::try_from(cycle).unwrap();
        assert_eq!(row.whole, Some(span(c * 2, c * 2 + 1, 2)));
        let origin = row.source_origin.as_ref().unwrap();
        let [timing] = origin.slice_timings() else {
            panic!("one actual Index issuance")
        };
        assert_eq!(timing.subject_handle(), &origin.handle);
        assert_eq!(Some(timing.index_whole()), row.whole);
        assert_eq!(timing.sample_start(), row.whole.unwrap().begin);
        let route = resolve_route(&plan, row, limits).unwrap();
        assert_eq!(route.configuration, row.whole.unwrap());
        routes.push(route);
    }
    for cycle in [3, 0, 2, 1] {
        let window = span(cycle * 8 + 1, cycle * 8 + 3, 8);
        let partial = song.query(window, &limits).unwrap();
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].part, window);
        assert_eq!(
            partial[0].whole,
            rows[usize::try_from(cycle).unwrap()].whole
        );
        assert_eq!(
            resolve_route(&plan, &partial[0], limits).unwrap(),
            routes[usize::try_from(cycle).unwrap()]
        );
        assert!(song
            .query(span(cycle * 8 + 5, cycle * 8 + 7, 8), &limits)
            .unwrap()
            .is_empty());
    }
    assert_ne!(
        routes[0], routes[1],
        "genuine disconnected applicability remains distinct"
    );
}
#[test]
fn fixed_getter_does_not_mutate_the_original_selected_part() {
    let mut song = candidate("fn indexed p:\n\tslice {beat -> p} 2 [0 nil]\nlet base {part [drums: {s :analog}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nsong {sequence [base selected]} tail-seconds: 0 > play-song");
    let limits = SongLimits::default();
    let original = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(original.len(), 2);
    assert!(original.iter().all(|row| row.source_origin.is_none()));
    let transformed = song.query(span(2, 4, 1), &limits).unwrap();
    assert_eq!(transformed.len(), 2);
    assert!(transformed.iter().all(|row| !row
        .source_origin
        .as_ref()
        .unwrap()
        .slice_timings()
        .is_empty()));
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &capacities(),
    )
    .unwrap();
    for row in transformed {
        resolve_route(&plan, &row, limits).unwrap();
    }
}

#[test]
fn fixed_getter_keeps_actual_closed_instrument_ir_and_query_authority() {
    let factory = DecodedSongAssetFactory::new(
        BTreeMap::from([(
            "ir.wav".into(),
            std::sync::Arc::new(vactr::host::caps::SampleData {
                rate: 48000,
                channels: 2,
                frames: vec![0.25, 0.5].into(),
            }),
        )]),
        BTreeMap::from([(
            vactr::value::intern::intern_kw("fixed"),
            vec!["ir.wav".into()],
        )]),
        BTreeMap::new(),
    );
    let code="inst generated:\n\tsin-osc freq > convolution ir: :fixed\nfn indexed p:\n\tslice {beat -> p} 2 [0 nil]\nlet base {part [drums: {s generated}] duration: 2}\nlet selected {transform-instrument base :drums :generated indexed}\nsong selected tail-seconds: 0 > play-song";
    let mut song = candidate_with(code, &factory);
    assert_eq!(song.snapshot().resource_count(), 1);
    let limits = SongLimits::default();
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &capacities(),
    )
    .unwrap();
    let rows = song.query(span(0, 2, 1), &limits).unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.source_origin.as_ref().unwrap().slice_timings().len(), 1);
        resolve_route(&plan, &row, limits).unwrap();
    }
    assert_eq!(song.snapshot().resource_count(), 1);
}

#[test]
fn public_fixed_callable_resolution_has_exact_shared_work_and_depth() {
    let mut song = candidate(FIXED);
    let limits = SongLimits::default();
    let rows = song.query(span(0, 1, 1), &limits).unwrap();
    assert_eq!(rows.len(), 1);
    let plan = prepare_routes(
        song.snapshot(),
        &vactr::dsp::caps::CapabilitySet::native(),
        &capacities(),
    )
    .unwrap();
    let expected = resolve_route(&plan, &rows[0], limits).unwrap();
    for depth in [false, true] {
        let mut lo = 1;
        let mut hi = if depth {
            limits.max_depth
        } else {
            limits.max_nodes
        };
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let candidate = if depth {
                SongLimits {
                    max_depth: mid,
                    ..limits
                }
            } else {
                SongLimits {
                    max_nodes: mid,
                    ..limits
                }
            };
            match resolve_route(&plan, &rows[0], candidate) {
                Ok(route) => {
                    assert_eq!(route, expected);
                    hi = mid;
                }
                Err(failure) => {
                    assert_eq!(
                        failure.code,
                        if depth {
                            vactr::vm::fail::FailCode::DepthExceeded
                        } else {
                            vactr::vm::fail::FailCode::FuelExhausted
                        },
                        "{failure:?}"
                    );
                    lo = mid + 1;
                }
            }
        }
        assert!(lo > 1);
        let exact = if depth {
            SongLimits {
                max_depth: lo,
                ..limits
            }
        } else {
            SongLimits {
                max_nodes: lo,
                ..limits
            }
        };
        assert_eq!(resolve_route(&plan, &rows[0], exact).unwrap(), expected);
        let short = if depth {
            SongLimits {
                max_depth: lo - 1,
                ..limits
            }
        } else {
            SongLimits {
                max_nodes: lo - 1,
                ..limits
            }
        };
        assert_eq!(
            resolve_route(&plan, &rows[0], short).unwrap_err().code,
            if depth {
                vactr::vm::fail::FailCode::DepthExceeded
            } else {
                vactr::vm::fail::FailCode::FuelExhausted
            }
        );
    }
    for row in rows.iter().rev() {
        assert_eq!(resolve_route(&plan, row, limits).unwrap(), expected);
    }
}

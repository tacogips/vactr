//! Whole-code candidate execution and immutable query/asset evidence.
use std::collections::BTreeMap;
use std::sync::Arc;
use vactr::host::caps::SampleData;
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenEdit, FrozenPartNode};
use vactr::song::source_uses::{certify_source_uses, resolve_source_use, resolve_source_use_frame};
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
use vactr::value::intern::intern_kw;
use vactr::value::Ratio64;
fn limits() -> SongAssetLimits {
    SongAssetLimits {
        max_resources: 256,
        max_pcm_bytes: 4_000_000,
        max_source_files: 64,
        max_source_bytes: 1_000_000,
        max_banks: 64,
        max_walk_nodes: 100_000,
        max_walk_depth: 256,
    }
}
fn factory() -> DecodedSongAssetFactory {
    let samples = BTreeMap::from([(
        "drum.wav".into(),
        Arc::new(SampleData {
            rate: 48000,
            channels: 2,
            frames: vec![0.1, 0.1, 0.0, 0.0].into(),
        }),
    )]);
    let banks = [
        "bd", "sd", "hh", "bd-haus", "sn-dub", "bd-tek", "crash", "pluck", "break", "piano", "cp",
        "sawtooth", "vocal",
    ]
    .into_iter()
    .map(|name| (intern_kw(name), vec!["drum.wav".into()]))
    .collect();
    DecodedSongAssetFactory::new(samples, banks, BTreeMap::new())
}
fn candidate(code: &str) -> Result<PreparedSong, vactr::vm::Failure> {
    let f = factory();
    let cx = CandidateBuildCtx {
        assets: &f,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    prepare_song(evaluate_song_candidate(
        code,
        "score.vact",
        7,
        SnapshotEpoch(9),
        &cx,
    )?)
}

#[test]
fn shared_fast_slow_stack_has_distinct_admitted_uses() {
    let mut song = candidate("let base {part [drums: {s :analog}] duration: 4}\nlet developed {transform-instrument base :drums :analog {p -> stack [{fast p 2} {slow p 2}]}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    let cover = {
        let inventory = song.snapshot().routing();
        let pattern = inventory
            .parts
            .iter()
            .find_map(|part| match &part.node {
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { payload, .. },
                    ..
                } => Some(payload),
                _ => None,
            })
            .expect("transform payload");
        assert_eq!(
            pattern.sources.len(),
            1,
            "policy inventory deduplicates the shared source"
        );
        certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap()
    };
    let events = song.query(window, &SongLimits::default()).unwrap();
    assert!(!events.is_empty());
    let identities: Vec<_> = events
        .iter()
        .map(|event| {
            resolve_source_use(
                &cover,
                event.source_origin.as_ref().expect("selected origin"),
                SongLimits::default(),
            )
            .unwrap()
        })
        .collect();
    assert!(
        identities
            .iter()
            .any(|a| identities.iter().any(|b| a.edges != b.edges)),
        "parallel aliases retain separate use paths"
    );
    assert!(cover.configuration_bound() >= 2);
}

#[test]
fn huge_repeat_short_window_counts_only_touched_placements() {
    let mut song = candidate("let base {part [drums: {s :analog}] duration: 1}\nlet repeated {part-repeat base 1000000000}\nlet developed {transform-instrument repeated :drums :analog {p -> fast p 2}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
    assert_eq!(cover.configuration_bound(), 2);
    assert_eq!(song.query(window, &SongLimits::default()).unwrap().len(), 2);
}
#[test]
fn structured_note_keeps_actual_content_source_entry() {
    let mut song = candidate("let base {part [drums: {s :analog > note [60 64]}] duration: 4}\nlet developed {transform-instrument base :drums :analog {p -> stack [{fast p 2} {slow p 2}]}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    let cover = {
        let inventory = song.snapshot().routing();
        let pattern = inventory
            .parts
            .iter()
            .find_map(|part| match &part.node {
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { payload, .. },
                    ..
                } => Some(payload),
                _ => None,
            })
            .expect("transform payload");
        assert_eq!(
            pattern.sources.len(),
            1,
            "policy inventory deduplicates the shared source"
        );
        certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap()
    };
    let events = song.query(window, &SongLimits::default()).unwrap();
    assert!(!events.is_empty());
    let identities: Vec<_> = events
        .iter()
        .map(|event| {
            resolve_source_use(
                &cover,
                event.source_origin.as_ref().expect("selected origin"),
                SongLimits::default(),
            )
            .unwrap()
        })
        .collect();
    assert!(
        identities
            .iter()
            .any(|a| identities.iter().any(|b| a.edges != b.edges)),
        "parallel aliases retain separate use paths"
    );
    assert!(cover.configuration_bound() >= 2);
}

#[test]
fn cat_maps_late_outer_cycle_to_present_source_track() {
    let mut song = candidate("let drums {part [drums: {s :analog}] duration: 3}\nlet bass {part [bass: {s :fm}] duration: 3}\nlet base sequence [drums bass]\nlet developed {transform-instrument base :drums :analog {p -> cat [p p]}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::from_int(4), Ratio64::from_int(5)).unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
    assert!(cover.configuration_bound() > 0);
    let events = song.query(window, &SongLimits::default()).unwrap();
    let selected: Vec<_> = events
        .iter()
        .filter_map(|event| event.source_origin.as_ref())
        .collect();
    assert!(!selected.is_empty());
    for origin in selected {
        resolve_source_use(&cover, origin, SongLimits::default()).unwrap();
    }
}

#[test]
fn authenticated_origin_outside_mapped_cover_is_rejected() {
    let mut song = candidate("let base {part [drums: {s :analog}] duration: 4}\nlet developed {transform-instrument base :drums :analog {p -> fast p 2}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
    let events = song
        .query(
            TimeSpan::new(Ratio64::ONE, Ratio64::from_int(2)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    assert!(!events.is_empty());
    for event in events {
        assert!(resolve_source_use(
            &cover,
            event.source_origin.as_ref().unwrap(),
            SongLimits::default()
        )
        .is_err());
    }
}

#[test]
fn choose_preserves_source_cycle_and_iter_crosses_boundary() {
    for (operator, begin, end) in [("choose [p p]", 2, 3), ("iter p 2", 1, 2)] {
        let code = format!("let bass {{part [bass: {{s :fm}}] duration: 2}}\nlet drums {{part [drums: {{s :analog}}] duration: 2}}\nlet base sequence [bass drums]\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song");
        let mut song = candidate(&code).unwrap();
        let window = TimeSpan::new(Ratio64::from_int(begin), Ratio64::from_int(end)).unwrap();
        let inventory = song.snapshot().routing();
        let pattern = inventory
            .parts
            .iter()
            .find_map(|part| match &part.node {
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { payload, .. },
                    ..
                } => Some(payload),
                _ => None,
            })
            .unwrap();
        let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
        let events = song.query(window, &SongLimits::default()).unwrap();
        let selected: Vec<_> = events
            .iter()
            .filter_map(|e| e.source_origin.as_ref())
            .collect();
        assert!(!selected.is_empty(), "{operator}");
        assert!(cover.configuration_bound() > 0, "{operator}");
        for origin in selected {
            resolve_source_use(&cover, origin, SongLimits::default()).unwrap();
        }
    }
}
#[test]
fn reflected_point_uses_complete_source_cycle() {
    let mut song = candidate("let base {part [drums: {s :analog}] duration: 2}\nlet developed {transform-instrument base :drums :analog {p -> rev p}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::point(Ratio64::ZERO);
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
    let events = song.query(window, &SongLimits::default()).unwrap();
    assert!(!events.is_empty());
    for event in events {
        resolve_source_use(
            &cover,
            event.source_origin.as_ref().unwrap(),
            SongLimits::default(),
        )
        .unwrap();
    }
}

#[test]
fn nested_origins_resolve_each_source_level_without_note_generations() {
    let mut song = candidate("let base {part [drums: {s :analog > note [60 64]}] duration: 4}\nlet inner {transform-instrument base :drums :analog {p -> fast p 2}}\nlet outer {transform-instrument inner :drums :analog {p -> slow p 2}}\nsong outer > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    let covers: Vec<_> = {
        let inventory = song.snapshot().routing();
        let mut unique = BTreeMap::new();
        for part in &inventory.parts {
            if let FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } = &part.node
            {
                unique.entry(part.revision).or_insert_with(|| {
                    certify_source_uses(inventory, payload, window, SongLimits::default()).unwrap()
                });
            }
        }
        unique.into_values().collect()
    };
    assert_eq!(covers.len(), 2);
    let events = song.query(window, &SongLimits::default()).unwrap();
    assert!(events.len() >= 2);
    let mut outer_ids = Vec::new();
    for event in &events {
        let origin = event.source_origin.as_ref().unwrap();
        assert_eq!(origin.inherited.len(), 1);
        let matching: Vec<_> = covers
            .iter()
            .filter_map(|cover| resolve_source_use(cover, origin, SongLimits::default()).ok())
            .collect();
        assert_eq!(matching.len(), 1);
        outer_ids.push(matching[0].clone());
        let inner = &origin.inherited[0];
        assert_eq!(inner.original_instrument, origin.original_instrument);
        assert_eq!(
            covers
                .iter()
                .filter(
                    |cover| resolve_source_use_frame(cover, inner, SongLimits::default()).is_ok()
                )
                .count(),
            1
        );
    }
    assert!(
        outer_ids.windows(2).any(|pair| pair[0] == pair[1]),
        "different notes share placement/configuration"
    );
}
#[test]
fn sound_wrapper_preserves_source_and_fractional_control() {
    let mut song = candidate("let base {part [drums: {s :analog > note [60 64]}] duration: 2}\nlet developed {transform-instrument base :drums :analog {p -> s p > note 66.75}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
    let events = song.query(window, &SongLimits::default()).unwrap();
    assert!(!events.is_empty());
    for event in events {
        assert_eq!(event.note, Some(vactr::song::ResolvedNote::Float32(66.75)));
        resolve_source_use(
            &cover,
            event.source_origin.as_ref().unwrap(),
            SongLimits::default(),
        )
        .unwrap();
    }
}

#[test]
fn static_operator_profiles_resolve_actual_entry_terms() {
    for operator in [
        "[p p]",
        "[{hold p 2} p]",
        "[{repeat p 2} p]",
        "repeat p 2",
        "hold p 1/2",
        "grid p [true true]",
        "euclid p 3 4",
        "chop p 2",
        "striate p 2",
        "ply p 2",
        "slice p 2 [0 1]",
        "splice p [0 1/2] [0 1]",
        "loop-at p 2",
        "fit p",
        "superimpose p {q -> fast q 2}",
        "off p 1/4 {q -> fast q 2}",
        "chunk p 2 {q -> fast q 2}",
        "every p 2 {q -> fast q 2}",
    ] {
        let code = format!("let base {{part [drums: {{s :analog > note [60 64]}}] duration: 2}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song");
        let mut song = candidate(&code).unwrap_or_else(|e| panic!("candidate {operator}: {e:?}"));
        let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
        let inventory = song.snapshot().routing();
        let pattern = inventory
            .parts
            .iter()
            .find_map(|part| match &part.node {
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { payload, .. },
                    ..
                } => Some(payload),
                _ => None,
            })
            .unwrap();
        let cover = certify_source_uses(inventory, pattern, window, SongLimits::default())
            .unwrap_or_else(|e| panic!("cover {operator}: {e:?}"));
        let events = song
            .query(window, &SongLimits::default())
            .unwrap_or_else(|e| panic!("query {operator}: {e:?}"));
        assert!(!events.is_empty(), "{operator}");
        for event in events {
            resolve_source_use(
                &cover,
                event.source_origin.as_ref().unwrap(),
                SongLimits::default(),
            )
            .unwrap_or_else(|e| panic!("origin {operator}: {e:?}"));
        }
    }
}

#[test]
fn overloaded_segment_profile_resolves_actual_entry_terms() {
    for operator in ["p > gain {segment sine 4}"] {
        let code = format!("let base {{part [drums: {{s :analog > note [60 64]}}] duration: 2}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song");
        let mut song = candidate(&code).unwrap_or_else(|e| panic!("candidate {operator}: {e:?}"));
        let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
        let inventory = song.snapshot().routing();
        let pattern = inventory
            .parts
            .iter()
            .find_map(|part| match &part.node {
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { payload, .. },
                    ..
                } => Some(payload),
                _ => None,
            })
            .unwrap();
        let cover = certify_source_uses(inventory, pattern, window, SongLimits::default())
            .unwrap_or_else(|e| panic!("cover {operator}: {e:?}"));
        let events = song
            .query(window, &SongLimits::default())
            .unwrap_or_else(|e| panic!("query {operator}: {e:?}"));
        assert!(!events.is_empty(), "{operator}");
        for event in events {
            resolve_source_use(
                &cover,
                event.source_origin.as_ref().unwrap(),
                SongLimits::default(),
            )
            .unwrap_or_else(|e| panic!("origin {operator}: {e:?}"));
        }
    }
}

#[test]
fn default_stack_depth_and_shared_diamond_are_bounded() {
    for depth in [200, 300] {
        let mut code = "fn develop p:\n\tlet p0 p\n".to_string();
        for i in 1..=depth {
            code.push_str(&format!("\tlet p{i} {{fast p{} 1}}\n", i - 1));
        }
        code.push_str(&format!("\tfirst [p{depth}]\nlet base {{part [drums: {{s :analog}}] duration: 1}}\nlet developed {{transform-instrument base :drums :analog {{p -> develop p}}}}\nsong developed > play-song"));
        let result = candidate(&code);
        if depth == 300 {
            assert_eq!(
                result.err().expect("over-depth failure").code,
                vactr::vm::fail::FailCode::DepthExceeded
            );
            continue;
        }
        let mut song = result.unwrap();
        let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
        let inventory = song.snapshot().routing();
        let pattern = inventory
            .parts
            .iter()
            .find_map(|part| match &part.node {
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { payload, .. },
                    ..
                } => Some(payload),
                _ => None,
            })
            .unwrap();
        let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
        let events = song.query(window, &SongLimits::default()).unwrap();
        for event in events {
            resolve_source_use(
                &cover,
                event.source_origin.as_ref().unwrap(),
                SongLimits::default(),
            )
            .unwrap();
        }
    }
    let mut code = "fn develop p:\n\tlet p0 p\n".to_string();
    for i in 1..=12 {
        code.push_str(&format!(
            "\tlet p{i} {{stack [{{fast p{} 1}} {{fast p{} 1}}]}}\n",
            i - 1,
            i - 1
        ));
    }
    code.push_str("\tfirst [p12]\nlet base {part [drums: {s :analog}] duration: 1}\nlet developed {transform-instrument base :drums :analog {p -> develop p}}\nsong developed > play-song");
    let song = candidate(&code).unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    assert!(pattern.source_uses.nodes.len() < 100);
    let limits = SongLimits {
        max_nodes: 1000,
        ..SongLimits::default()
    };
    let cover = certify_source_uses(
        inventory,
        pattern,
        TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(),
        limits,
    )
    .unwrap();
    assert_eq!(cover.configuration_bound(), 4096);
}

#[test]
fn copied_recipe_depth_cache_and_malformed_admission() {
    use vactr::song::source_uses::{
        FrozenSourceUseEdge, FrozenSourceUseNode, FrozenUseMapping, FrozenUseOperation,
    };
    let song = candidate("let base {part [drums: {s :analog}] duration: 1}\nlet developed {transform-instrument base :drums :analog {p -> fast p 1}}\nsong developed > play-song").unwrap();
    let inventory = song.snapshot().routing();
    let original = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    for depth in [200, 300] {
        let mut pattern = original.clone();
        for _ in 0..depth {
            let child = pattern.source_uses.root;
            pattern.source_uses.root = pattern.source_uses.nodes.len() as u32;
            pattern.source_uses.nodes.push(FrozenSourceUseNode {
                operation: FrozenUseOperation::Fast,
                mapping: FrozenUseMapping::Rate {
                    factor: Ratio64::ONE,
                },
                edges: vec![FrozenSourceUseEdge {
                    layout: vec![],
                    trace: vec![],
                    child,
                }],
            });
        }
        let result = certify_source_uses(inventory, &pattern, window, SongLimits::default());
        if depth == 200 {
            assert_eq!(result.unwrap().configuration_bound(), 1);
        } else {
            assert_eq!(
                result.err().unwrap().code,
                vactr::vm::fail::FailCode::DepthExceeded
            );
        }
    }
    let mut pattern = original.clone();
    for _ in 0..12 {
        let child = pattern.source_uses.root;
        pattern.source_uses.root = pattern.source_uses.nodes.len() as u32;
        pattern.source_uses.nodes.push(FrozenSourceUseNode {
            operation: FrozenUseOperation::Stack,
            mapping: FrozenUseMapping::Parallel,
            edges: vec![
                FrozenSourceUseEdge {
                    layout: vec![],
                    trace: vec![],
                    child,
                },
                FrozenSourceUseEdge {
                    layout: vec![],
                    trace: vec![],
                    child,
                },
            ],
        });
    }
    let limits = SongLimits {
        max_nodes: 500,
        ..SongLimits::default()
    };
    assert_eq!(
        certify_source_uses(inventory, &pattern, window, limits)
            .unwrap()
            .configuration_bound(),
        4096
    );
    let mut overflowing = original.clone();
    for _ in 0..64 {
        let child = overflowing.source_uses.root;
        overflowing.source_uses.root = overflowing.source_uses.nodes.len() as u32;
        overflowing.source_uses.nodes.push(FrozenSourceUseNode {
            operation: FrozenUseOperation::Stack,
            mapping: FrozenUseMapping::Parallel,
            edges: vec![
                FrozenSourceUseEdge {
                    layout: vec![],
                    trace: vec![],
                    child
                };
                2
            ],
        });
    }
    assert_eq!(
        certify_source_uses(inventory, &overflowing, window, SongLimits::default())
            .unwrap_err()
            .code,
        vactr::vm::fail::FailCode::Overflow
    );
    let root = pattern.source_uses.root as usize;
    pattern.source_uses.nodes[root].edges[0].child = pattern.source_uses.root;
    assert!(certify_source_uses(inventory, &pattern, window, SongLimits::default()).is_err());
    pattern.source_uses.nodes[root].edges[0].child = u32::MAX;
    assert!(certify_source_uses(inventory, &pattern, window, SongLimits::default()).is_err());
}

#[test]
fn nonadjacent_source_fx_placements_keep_distinct_configuration_identity() {
    let mut song = candidate("bus :room-a:\n\tplate mix: 1\nbus :room-b:\n\tspring-reverb mix: 1\nlet base {part [drums: {s :analog > note [60 64]}] duration: 1}\nlet a {instrument-fx base :drums :analog :room-a}\nlet b {instrument-fx base :drums :analog :room-b}\nlet chain {sequence [a b a]}\nlet selected {transform-instrument chain :drums :analog {p -> fast p 1}}\nsong selected > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
    let events = song.query(window, &SongLimits::default()).unwrap();
    let mut ids = BTreeMap::new();
    let mut templates = BTreeMap::new();
    for event in events {
        let origin = event.source_origin.as_ref().unwrap();
        let id = resolve_source_use(&cover, origin, SongLimits::default()).unwrap();
        let cycle = event.whole.unwrap().begin.floor();
        if let Some(old) = ids.insert(cycle, id.clone()) {
            assert_eq!(old, id);
        }
        templates.insert(cycle, event.route.unwrap().1);
    }
    assert_eq!(ids.len(), 3);
    assert_ne!(ids[&0], ids[&1]);
    assert_ne!(ids[&0], ids[&2]);
    assert_eq!(templates[&0], templates[&2]);
    assert_ne!(templates[&0], templates[&1]);
}

#[test]
fn captured_lazy_callback_is_closed_and_strict_limits_are_honored() {
    let mut song = candidate("let base {part [drums: {s :analog}] duration: 1}\nlet developed {transform-instrument base :drums :analog {p -> every p 2 {q -> stack [q p]}}}\nsong developed > play-song").unwrap();
    let window = TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    let cover = certify_source_uses(inventory, pattern, window, SongLimits::default()).unwrap();
    let mut tiny = SongLimits {
        max_nodes: 1,
        ..SongLimits::default()
    };
    assert!(certify_source_uses(inventory, pattern, window, tiny).is_err());
    tiny = SongLimits::default();
    tiny.max_depth = 1;
    assert!(certify_source_uses(inventory, pattern, window, tiny).is_err());
    for event in song.query(window, &SongLimits::default()).unwrap() {
        let origin = event.source_origin.as_ref().unwrap();
        resolve_source_use(&cover, origin, SongLimits::default()).unwrap();
        let tiny = SongLimits {
            max_nodes: 1,
            ..SongLimits::default()
        };
        assert!(resolve_source_use(&cover, origin, tiny).is_err());
    }
}

#[test]
fn genuinely_dynamic_rate_is_diagnosed_before_activation() {
    let song = candidate("let base {part [drums: {s :analog}] duration: 1}\nlet developed {transform-instrument base :drums :analog {p -> fast p {t -> t}}}\nsong developed > play-song").unwrap();
    let inventory = song.snapshot().routing();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload),
            _ => None,
        })
        .unwrap();
    assert!(pattern.source_uses.nodes.iter().any(|node| matches!(
        node.mapping,
        vactr::song::source_uses::FrozenUseMapping::Uncertifiable(
            vactr::song::source_uses::FrozenUseReason::DynamicRate
        )
    )));
    assert!(certify_source_uses(
        inventory,
        pattern,
        TimeSpan::new(Ratio64::ZERO, Ratio64::ONE).unwrap(),
        SongLimits::default()
    )
    .is_err());
}

#[test]
fn actual_lazy_source_callback_uses_complete_pinned_bank_or_fails_preparation() {
    let mut song = candidate("let base {part [drums: {s :analog}] duration: 3}\nlet developed {transform-instrument base :drums :analog {p -> every p 2 {q -> s :bd}}}\nsong developed > play-song").unwrap();
    assert!(song.snapshot().resource_count() > 0);
    let events = song
        .query(
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(3)).unwrap(),
            &SongLimits::default(),
        )
        .unwrap();
    assert!(!events.is_empty());
    assert!(
        events.iter().any(|event| event.source_origin.is_none()),
        "callback actually produces the bank source"
    );
    assert!(candidate("let base {part [drums: {s :analog}] duration: 1}\nlet developed {transform-instrument base :drums :analog {p -> every p 2 {q -> s :missing-bank}}}\nsong developed > play-song").is_err());
}

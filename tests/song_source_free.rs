//! Source-free payload admission preserves real ownership and rejects forged DTOs.
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenRoutingInventory};
use vactr::song::source_uses::{
    certify_source_uses, FrozenPattern, FrozenSourceUseEdge, FrozenSourceUseNode,
    FrozenUseMapping as M, FrozenUseOperation as O, FrozenUseReason as R,
};
use vactr::song::{prepare_song, SnapshotEpoch, SongLimits};
use vactr::vm::fail::{FailCode, Failure};

fn model(code: &str) -> (FrozenRoutingInventory, FrozenPattern) {
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
    let song = prepare_song(
        evaluate_song_candidate(code, "source-free.vact", 1, SnapshotEpoch(55), &cx).unwrap(),
    )
    .unwrap();
    let inventory = song.snapshot().routing().clone();
    let pattern = inventory
        .parts
        .iter()
        .find_map(|part| match &part.node {
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => Some(payload.clone()),
            _ => None,
        })
        .unwrap();
    (inventory, pattern)
}
fn ordinary() -> (FrozenRoutingInventory, FrozenPattern) {
    model("let base {part [drums: {slow {s :analog} 64}] duration: 64}\nlet selected {transform-instrument base :drums :analog {p -> first [p]}}\nsong selected > play-song")
}
fn capture(inventory: &mut FrozenRoutingInventory) -> &mut FrozenPattern {
    inventory
        .parts
        .iter_mut()
        .find_map(|part| match &mut part.node {
            FrozenPartNode::Capture(tracks) => Some(&mut tracks[0].1),
            _ => None,
        })
        .unwrap()
}
fn node(mapping: M, edges: Vec<FrozenSourceUseEdge>) -> FrozenSourceUseNode {
    FrozenSourceUseNode {
        operation: O::Slow,
        mapping,
        edges,
    }
}
fn edge(child: u32) -> FrozenSourceUseEdge {
    FrozenSourceUseEdge {
        child,
        trace: vec![],
        layout: vec![],
    }
}
fn bound(
    inventory: &FrozenRoutingInventory,
    payload: &FrozenPattern,
    limits: SongLimits,
) -> Result<u64, Failure> {
    Ok(
        certify_source_uses(inventory, payload, TimeSpan::cycle(0).unwrap(), limits)?
            .configuration_bound(),
    )
}

#[test]
fn actual_ordinary_slow_capture_and_generated_edit_reach_selected_certification() {
    for code in [
        "let base {part [drums: {slow {s :analog} 64}] duration: 64}\nlet selected {transform-instrument base :drums :analog {p -> first [p]}}\nsong selected > play-song",
        "let base {part [drums: {s :analog}] duration: 64}\nlet changed {replace-track base :drums {slow {s :analog} 64}}\nlet selected {transform-instrument changed :drums :analog {p -> first [p]}}\nsong selected > play-song",
        "let base {part [drums: {s :analog}] duration: 64}\nlet changed {overwrite-region base :drums 0 1 {slow {s :analog} 64}}\nlet selected {transform-instrument changed :drums :analog {p -> first [p]}}\nsong selected > play-song",
    ] {
        let (inventory, pattern) = model(code);
        assert!(!pattern.sources.is_empty());
        assert!(inventory.parts.iter().any(|part| match &part.node {
            FrozenPartNode::Capture(tracks) => tracks.iter().any(|(_, payload)| payload.source_uses.nodes.iter().any(|n| matches!(n.mapping, M::Uncertifiable(R::DynamicRate)))),
            FrozenPartNode::Edit { edit: FrozenEdit::Replace {payload,..} | FrozenEdit::Overwrite {payload,..}, .. } => payload.source_uses.nodes.iter().any(|n| matches!(n.mapping, M::Uncertifiable(R::DynamicRate))),
            _ => false,
        }), "fixture must exercise the actual ordinary DynamicRate graph");
        let result = bound(&inventory, &pattern, SongLimits::default()).unwrap();
        assert!(result >= 1);
    }
}

#[test]
fn capture_family_zero_one_and_generated_edit_inheritance_are_preserved() {
    let (mut inventory, pattern) = ordinary();
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default()).unwrap(),
        1
    );
    capture(&mut inventory).families.clear();
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default()).unwrap(),
        0
    );
    let (mut inventory, pattern) = model("let base {part [drums: {s :analog}] duration: 1}\nlet changed {replace-track base :drums {slow {s :analog} 64}}\nlet selected {transform-instrument changed :drums :analog {p -> first [p]}}\nsong selected > play-song");
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default()).unwrap(),
        4
    );
    for part in &mut inventory.parts {
        if let FrozenPartNode::Edit {
            edit: FrozenEdit::Replace { payload, .. },
            ..
        } = &mut part.node
        {
            payload.families.clear();
        }
    }
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default()).unwrap(),
        4,
        "generated edits retain max(1)"
    );
}

#[test]
fn selected_dynamic_timing_keeps_its_addressed_failure() {
    let (inventory, mut pattern) = ordinary();
    pattern.source_uses.nodes[pattern.source_uses.root as usize].operation = O::Slow;
    pattern.source_uses.nodes[pattern.source_uses.root as usize].mapping =
        M::Uncertifiable(R::DynamicRate);
    let failure = bound(&inventory, &pattern, SongLimits::default()).unwrap_err();
    assert_eq!(failure.code, FailCode::BeyondCapability);
    assert!(failure.message.contains("Slow DynamicRate"));
}

#[test]
fn invalid_source_free_roots_source_references_edges_and_cycles_are_rejected() {
    for case in 0..8 {
        let (mut inventory, pattern) = ordinary();
        let payload = capture(&mut inventory);
        payload.source_uses.root = 0;
        payload.source_uses.nodes = vec![node(M::Uncertifiable(R::DynamicRate), vec![])];
        match case {
            0 => payload.source_uses.root = 9,
            1 => payload.source_uses.nodes[0] = node(M::Source { policy: 0 }, vec![]),
            2 => payload.source_uses.nodes[0] = node(M::Source { policy: 0 }, vec![edge(0)]),
            3 => payload.source_uses.nodes[0].edges.push(edge(7)),
            4 => payload.source_uses.nodes[0] = node(M::Empty, vec![edge(0)]),
            5 => payload.source_uses.nodes[0].edges.push(edge(0)),
            6 => {
                payload.source_uses.nodes.clear();
                payload.source_uses.root = 1;
            }
            _ => payload.source_uses.nodes[0] = node(M::SelectContent { content: 1 }, vec![]),
        }
        assert_eq!(
            bound(&inventory, &pattern, SongLimits::default())
                .unwrap_err()
                .code,
            FailCode::Type,
            "case={case}"
        );
    }
}

#[test]
fn malformed_layout_copy_and_slice_metadata_are_not_hidden_by_dynamic_timing() {
    use vactr::pattern::occ::ProducerKind;
    use vactr::song::source_uses::{layout::FrozenUseSlotLayout, FrozenUseTraceTerm as T};
    use vactr::value::Ratio64;
    for case in 0..5 {
        let (mut inventory, pattern) = ordinary();
        let payload = capture(&mut inventory);
        let mut e = edge(1);
        match case {
            0 => e.trace.push(T::Copies {
                kind: ProducerKind::GeneratedBranch,
                count: 4097,
            }),
            1 => e.layout.push(FrozenUseSlotLayout {
                prefix: Ratio64::from_int(-1),
                width: Ratio64::ONE,
                copy_trace_term: None,
            }),
            2 => e.layout.push(FrozenUseSlotLayout {
                prefix: Ratio64::ZERO,
                width: Ratio64::ONE,
                copy_trace_term: Some(0),
            }),
            3 => {
                e.trace.push(T::Copies {
                    kind: ProducerKind::GeneratedBranch,
                    count: 0,
                });
                e.layout.push(FrozenUseSlotLayout {
                    prefix: Ratio64::ZERO,
                    width: Ratio64::ONE,
                    copy_trace_term: Some(0),
                });
            }
            _ => {}
        }
        payload.source_uses.root = 0;
        payload.source_uses.nodes = vec![
            node(M::Uncertifiable(R::DynamicRate), vec![e]),
            node(M::Empty, vec![]),
        ];
        if case == 4 {
            payload.source_uses.nodes[0].mapping = M::Slices {
                starts: vec![Ratio64::ONE, Ratio64::ZERO],
                splice: false,
                structure: vactr::song::source_uses::FrozenSliceStructure::Subject {
                    issuer: pattern.id,
                },
            };
        }
        assert_eq!(
            bound(&inventory, &pattern, SongLimits::default())
                .unwrap_err()
                .code,
            FailCode::Type,
            "case={case}"
        );
    }
}

#[test]
fn reachable_only_validation_and_empty_graph_convention_remain_explicit() {
    let (mut inventory, pattern) = ordinary();
    let payload = capture(&mut inventory);
    payload.source_uses.root = 0;
    payload.source_uses.nodes = vec![
        node(M::Empty, vec![]),
        node(M::Source { policy: 99 }, vec![edge(99)]),
    ];
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default()).unwrap(),
        1
    );
    capture(&mut inventory).source_uses.nodes.clear();
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default()).unwrap(),
        1
    );
}

fn chain(depth: u32) -> Vec<FrozenSourceUseNode> {
    (0..depth)
        .map(|i| {
            node(
                M::Uncertifiable(R::DynamicRate),
                if i + 1 == depth {
                    vec![]
                } else {
                    vec![edge(i + 1)]
                },
            )
        })
        .collect()
}
#[test]
fn default_stack_depth_and_cache_alias_height_are_checked() {
    for depth in [200, 300] {
        let (mut inventory, pattern) = ordinary();
        let payload = capture(&mut inventory);
        payload.source_uses.root = 0;
        payload.source_uses.nodes = chain(depth);
        let result = bound(&inventory, &pattern, SongLimits::default());
        if depth == 200 {
            assert_eq!(result.unwrap(), 1);
        } else {
            assert_eq!(result.unwrap_err().code, FailCode::DepthExceeded);
        }
    }
    let (mut inventory, pattern) = ordinary();
    let payload = capture(&mut inventory);
    let mut nodes = vec![node(M::Parallel, vec![edge(1), edge(101)])];
    nodes.extend((1..=100).map(|i| {
        node(
            M::Preserve,
            if i == 100 { vec![] } else { vec![edge(i + 1)] },
        )
    }));
    nodes.extend(
        (101..=260).map(|i| node(M::Preserve, vec![edge(if i == 260 { 1 } else { i + 1 })])),
    );
    payload.source_uses.root = 0;
    payload.source_uses.nodes = nodes;
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::DepthExceeded,
        "shallow-first cached100 subtree must not bypass remaining depth below160 parents"
    );
}
#[test]
fn shared_diamond_is_linear_and_global_work_and_depth_remain_bounded() {
    let (mut inventory, pattern) = ordinary();
    let payload = capture(&mut inventory);
    payload.source_uses.root = 0;
    payload.source_uses.nodes = (0..40)
        .map(|i| {
            node(
                M::Parallel,
                if i == 39 {
                    vec![]
                } else {
                    vec![edge(i + 1), edge(i + 1)]
                },
            )
        })
        .collect();
    let limits = SongLimits {
        max_nodes: 1000,
        ..Default::default()
    };
    assert_eq!(
        bound(&inventory, &pattern, limits).unwrap(),
        1,
        "2^39 paths remain40 cached nodes"
    );
    assert_eq!(
        bound(
            &inventory,
            &pattern,
            SongLimits {
                max_nodes: 5,
                ..limits
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    assert_eq!(
        bound(
            &inventory,
            &pattern,
            SongLimits {
                max_depth: 4,
                ..limits
            }
        )
        .unwrap_err()
        .code,
        FailCode::DepthExceeded
    );
}

#[test]
fn unknown_source_callback_resource_and_live_reasons_keep_addressed_diagnostics() {
    for reason in [
        R::DynamicSource,
        R::UncertifiedCallback,
        R::UnclosedResource,
        R::UnsupportedLiveInput,
    ] {
        let (mut inventory, pattern) = ordinary();
        let payload = capture(&mut inventory);
        payload.source_uses.root = 0;
        payload.source_uses.nodes = vec![node(M::Uncertifiable(reason), vec![])];
        let failure = bound(&inventory, &pattern, SongLimits::default()).unwrap_err();
        assert_eq!(failure.code, FailCode::BeyondCapability);
        assert!(failure.message.contains("Slow"));
        assert!(failure.message.contains(&format!("{reason:?}")));
    }
}

#[test]
fn actual_structured_notes_preserve_noncontent_timing_sources() {
    let (inventory, pattern) = model("let base {part [drums: {s :analog > note [60 64]}] duration: 4}\nlet selected {transform-instrument base :drums :analog {p -> first [p]}}\nsong selected > play-song");
    assert!(
        inventory.parts.iter().any(|part| match &part.node {
            FrozenPartNode::Capture(tracks) => tracks.iter().any(|(_, payload)| payload
                .source_uses
                .nodes
                .iter()
                .any(|n| matches!(n.mapping, M::Uncertifiable(R::DynamicSource)))),
            _ => false,
        }),
        "real note timing must exercise DynamicSource"
    );
    assert_eq!(
        bound(&inventory, &pattern, SongLimits::default()).unwrap(),
        1
    );
}

#[test]
fn timing_first_cache_cannot_hide_audible_unknown_source_alias() {
    let (mut inventory, pattern) = ordinary();
    let payload = capture(&mut inventory);
    payload.source_uses.root = 0;
    payload.source_uses.nodes = vec![
        node(M::SelectContent { content: 1 }, vec![edge(1), edge(1)]),
        node(M::Uncertifiable(R::DynamicSource), vec![]),
    ];
    let failure = bound(&inventory, &pattern, SongLimits::default()).unwrap_err();
    assert_eq!(failure.code, FailCode::BeyondCapability);
    assert!(failure.message.contains("DynamicSource"));
}

#[test]
fn noncontent_edges_still_validate_structure_and_protected_capabilities() {
    for case in 0..8 {
        let (mut inventory, pattern) = ordinary();
        let payload = capture(&mut inventory);
        payload.source_uses.root = 0;
        payload.source_uses.nodes = vec![
            node(M::SelectContent { content: 1 }, vec![edge(1), edge(2)]),
            node(M::Uncertifiable(R::DynamicSource), vec![]),
            node(M::Empty, vec![]),
        ];
        match case {
            0 => payload.source_uses.nodes[1] = node(M::Source { policy: 0 }, vec![]),
            1 => payload.source_uses.nodes[1].edges.push(edge(99)),
            2 => payload.source_uses.nodes[1].edges.push(edge(1)),
            3 => payload.source_uses.nodes[0].edges[0].layout.push(
                vactr::song::source_uses::layout::FrozenUseSlotLayout {
                    prefix: vactr::value::Ratio64::ZERO,
                    width: vactr::value::Ratio64::ONE,
                    copy_trace_term: Some(0),
                },
            ),
            4 => payload.source_uses.nodes[1].mapping = M::Uncertifiable(R::UncertifiedCallback),
            5 => payload.source_uses.nodes[1].mapping = M::Uncertifiable(R::UnclosedResource),
            6 => payload.source_uses.nodes[1].mapping = M::Uncertifiable(R::UnsupportedLiveInput),
            _ => {}
        }
        let result = bound(&inventory, &pattern, SongLimits::default());
        match case {
            0..=3 => assert_eq!(result.unwrap_err().code, FailCode::Type, "case={case}"),
            4..=6 => assert_eq!(
                result.unwrap_err().code,
                FailCode::BeyondCapability,
                "case={case}"
            ),
            _ => assert_eq!(
                result.unwrap(),
                1,
                "non-content DynamicSource alone is permitted"
            ),
        }
    }
}

//! Actual closed candidates retain timing operands without promising geometry.
use std::collections::BTreeMap;
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::FrozenPartNode;
use vactr::song::source_uses::timing::{
    FrozenIndexDynamicKind as Dynamic, FrozenIndexLeaf as Leaf, FrozenIndexNumber as Number,
    FrozenIndexTiming,
};
use vactr::song::source_uses::FrozenUseOperation as Operation;
use vactr::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
fn candidate(code: &str) -> PreparedSong {
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
    prepare_song(evaluate_song_candidate(code, "index.vact", 1, SnapshotEpoch(1), &cx).unwrap())
        .unwrap()
}
fn captures(song: &PreparedSong) -> Vec<&FrozenIndexTiming> {
    song.snapshot()
        .routing()
        .parts
        .iter()
        .flat_map(|p| match &p.node {
            FrozenPartNode::Capture(tracks) => tracks
                .iter()
                .map(|(_, p)| p.index_timing().unwrap())
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}
#[test]
fn actual_candidates_distinguish_rest_order_and_relative_index_ordinal() {
    for (list, ordinal) in [("[0 nil]", 0), ("[nil 0]", 1)] {
        let code=format!("fn indexed p:\n\tslice p 2 {list}\nsong {{part [drums: {{indexed {{s :analog}}}}] duration: 2}} > play-song");
        let mut song = candidate(&code);
        let recipe = captures(&song)[0];
        let index = recipe
            .nodes()
            .iter()
            .find(|n| n.operation() == Operation::Steps)
            .unwrap();
        assert_eq!(index.slots().len(), 2);
        let leaf = recipe.nodes()[index.slots()[ordinal].child() as usize].leaf();
        assert_eq!(
            leaf,
            Some(&Leaf::Scalar {
                number: Some(Number::Int(0))
            })
        );
        assert_eq!(
            recipe.nodes()[index.slots()[1 - ordinal].child() as usize].leaf(),
            Some(&Leaf::Rest)
        );
        let geometry = *index.slots()[ordinal].geometry().unwrap();
        let rows = song
            .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].source_origin.is_none());
        assert_eq!(
            rows[0].whole,
            Some(
                TimeSpan::new(
                    geometry.prefix,
                    geometry.prefix.checked_add(geometry.width).unwrap()
                )
                .unwrap()
            )
        );
        assert!(!rows[0].handle.occurrence().producer_ordinals.is_empty());
    }
}
#[test]
fn actual_retained_callback_output_is_present_in_captured_recipe() {
    let code="fn develop p:\n\tslice p 2 [0 nil 1]\nsong {part [drums: {every {s :analog} 2 develop}] duration: 4} > play-song";
    let mut song = candidate(code);
    let recipe = captures(&song)[0];
    let conditional = recipe
        .nodes()
        .iter()
        .find(|n| n.operation() == Operation::Every)
        .unwrap();
    assert_eq!(conditional.children().len(), 2);
    let transformed = &recipe.nodes()[conditional.children()[1].child() as usize];
    assert_eq!(transformed.operation(), Operation::Slice);
    assert!(recipe
        .nodes()
        .iter()
        .any(|n| n.operation() == Operation::Slice));
    assert!(recipe
        .nodes()
        .iter()
        .any(|n| n.operation() == Operation::Steps && n.slots().len() == 3));
    let rows = song
        .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.whole.is_some()));
}
#[test]
fn actual_inventory_cache_clones_share_the_opaque_recipe() {
    let code="fn indexed p:\n\tslice p 2 [0 nil 1]\nlet shared indexed {s :analog}\nlet a {part [drums: shared] duration: 2}\nlet b {part [drums: shared] duration: 2}\nsong {sequence [a b]} > play-song";
    let song = candidate(code);
    let recipes = captures(&song);
    assert_eq!(recipes.len(), 2);
    assert!(std::ptr::eq(recipes[0], recipes[1]));
    assert!(recipes[0]
        .nodes()
        .iter()
        .any(|n| n.operation() == Operation::Steps && n.slots().len() == 3));
}
#[test]
fn genuine_function_and_lazy_block_operands_remain_unresolved_until_query() {
    for (list, kind) in [
        ("[{t -> 0} nil {t -> 1}]", Dynamic::Function),
        ("[{0} nil {1}]", Dynamic::Late),
    ] {
        let code = if kind == Dynamic::Late {
            // Direct lazy native arguments lower these blocks to Late; a named
            // eager body would evaluate them before operand capture.
            format!(
                "song {{part [drums: {{slice {{s :analog}} 2 {list}}}] duration: 2}} > play-song"
            )
        } else {
            format!("fn indexed p:\n\tslice p 2 {list}\nsong {{part [drums: {{indexed {{s :analog}}}}] duration: 2}} > play-song")
        };
        let mut song = candidate(&code);
        let recipe = captures(&song)[0];
        assert!(
            recipe.requires_realization(),
            "list={list}, expected={kind:?}, recipe={recipe:#?}"
        );
        assert!(
            recipe
                .nodes()
                .iter()
                .any(|node| node.leaf() == Some(&Leaf::Dynamic(kind))),
            "list={list}, expected={kind:?}, recipe={recipe:#?}"
        );
        let rows = song
            .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
            .unwrap();
        assert_eq!(rows.len(), 2);
    }
}

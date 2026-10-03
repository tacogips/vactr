//! Remaining genuine configuration-domain and refusal witnesses.
use super::*;

#[test]
fn dynamic_overlapping_wholes_keep_source_start_before_owner_and_long_extent() {
    let mut prepared = from_code(&code(
        "slow {slice {beat -> nil} 1 [cut]} {beat -> + 2 beat}",
    ));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let mut wholes = Vec::new();
    let mut useful = 0;
    for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
        if address.owner().revision != request.revision {
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        for row in index.rows(0, limits, &mut left).unwrap() {
            if row.issuer_start() == Ratio64::ZERO {
                wholes.push(row.footprint().whole);
            }
        }
        let (groups, _, _) = consume(
            snapshot,
            scope,
            span(0, 4, 1),
            &index,
            None,
            None,
            span(0, 1, 1),
            span(1, 3, 2),
            0,
            limits,
            &mut left,
        )
        .unwrap();
        assert!(groups.iter().any(|group| group == &vec![span(1, 3, 2)]),
            "original START0 is eligible before caller owner head; source end1 does not clip whole: {groups:?}");
        useful += 1;
    }
    assert!(useful > 0);
    assert!(wholes.iter().any(|whole| whole.end > Ratio64::ONE));
    assert!(
        wholes.iter().enumerate().any(|(i, a)| wholes
            .iter()
            .skip(i + 1)
            .any(|b| a != b && a.begin < b.end && b.begin < a.end)),
        "actual dynamic factors must produce distinct overlapping uncut wholes: {wholes:?}"
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn fractional_sequence_vary_repeats_preserve_actual_placements_seeds_and_groups() {
    let program = code("slice {beat -> nil} 1 [cut]")
        .replace("duration: 4", "duration: 1")
        .replace("song selected tail-seconds: 0 > play-song",
            "let intro {part [hats: {s :analog}] duration: 1/2}\nsong {part-repeat {sequence [intro selected]} 2 seed-mode: :vary} tail-seconds: 0 > play-song");
    let mut prepared = from_code(&program);
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot
        .routing
        .parts
        .iter()
        .enumerate()
        .find_map(|(i, part)| {
            matches!(
                part.node,
                FrozenPartNode::Edit {
                    edit: FrozenEdit::Transform { .. },
                    ..
                }
            )
            .then_some(i)
        })
        .unwrap();
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 3, 1), limits, &mut left);
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 3, 1), limits, &mut left);
    let mut actual = Vec::new();
    for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
        if address.owner().revision != request.revision {
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        actual.push((
            index.owner().placement.clone(),
            index.owner().offset,
            index.seed(),
        ));
        let (groups, _, _) = consume(
            snapshot,
            scope,
            span(0, 3, 1),
            &index,
            None,
            None,
            span(0, 1, 1),
            span(0, 1, 1),
            0,
            limits,
            &mut left,
        )
        .unwrap();
        assert_eq!(groups, vec![vec![span(0, 1, 1)]]);
    }
    let raw_count = actual.len();
    let mut placements = Vec::new();
    for entry in actual {
        if !placements.contains(&entry) {
            placements.push(entry);
        }
    }
    assert!(
        raw_count > placements.len(),
        "actual continuation records preserved"
    );
    assert_eq!(
        placements.len(),
        2,
        "two intrinsic original placements: {placements:?}"
    );
    assert_eq!(placements[0].1, Ratio64::new(1, 2).unwrap());
    assert_eq!(placements[1].1, Ratio64::new(2, 1).unwrap());
    assert_ne!(placements[0].0, placements[1].0);
    assert_ne!(placements[0].2, placements[1].2);
    assert_eq!(reads.get(), 0);
}

#[test]
fn actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct() {
    let mut prepared = from_code(&code("repeat {slice {beat -> nil} 1 [cut]} 2"));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    let recipe = payload(snapshot, scope).index_timing().unwrap();
    let mut pending = vec![(recipe.root(), Vec::new())];
    let mut sites = Vec::new();
    while let Some((node, prefix)) = pending.pop() {
        let node = &recipe.nodes()[node as usize];
        if node.operation() == FrozenUseOperation::Slice {
            sites.push((node.issuer(), prefix));
        } else {
            for edge in node.children() {
                let mut child = prefix.clone();
                child.extend_from_slice(edge.trace());
                pending.push((edge.child(), child));
            }
        }
    }
    assert_eq!(
        sites.len(),
        2,
        "actual list-repeat capture has two issued sites: {sites:?}"
    );
    assert_ne!(sites[0].1, sites[1].1);
    let mut requests = Vec::new();
    for (issuer, prefix) in &sites {
        requests.push(
            snapshot
                .canonical_index_request(
                    scope,
                    intern_kw("drums"),
                    *issuer,
                    prefix,
                    span(0, 4, 1),
                    0,
                    limits,
                    &mut left,
                )
                .unwrap(),
        );
    }
    retain_index_occupancy(snapshot, requests, limits, &mut left).unwrap();
    let reads = deny(snapshot);
    let mut prefixes = Vec::new();
    for (issuer, prefix) in &sites {
        let request = snapshot
            .canonical_index_request(
                scope,
                intern_kw("drums"),
                *issuer,
                prefix,
                span(0, 4, 1),
                0,
                limits,
                &mut left,
            )
            .unwrap();
        let mut useful = 0;
        for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
            if address.owner().revision != request.revision {
                continue;
            }
            let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
            for row in index.rows(0, limits, &mut left).unwrap() {
                if !prefixes.contains(row.prefix()) {
                    prefixes.push(row.prefix().clone());
                }
            }
            let (groups, _, _) = snapshot
                .retained_geometry_fixture(
                    (scope, intern_kw("drums"), *issuer, prefix, span(0, 4, 1)),
                    &index,
                    None,
                    None,
                    span(0, 4, 1),
                    span(0, 4, 1),
                    0,
                    limits,
                    &mut left,
                )
                .unwrap();
            assert_eq!(
                groups.len(),
                1,
                "one addressed site per genuine request: {groups:?}"
            );
            useful += 1;
        }
        assert!(useful > 0);
    }
    assert_eq!(
        prefixes.len(),
        2,
        "full real site prefixes stay distinct: {prefixes:?}"
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn unsupported_and_continuous_index_evidence_refuse_without_callback_fallback() {
    for body in [
        "euclid {slice {beat -> nil} 1 [cut]} 1 2",
        "slice {beat -> nil} 1 {range saw 0 0}",
    ] {
        let mut prepared = from_code(&code(body));
        let snapshot = &mut prepared.snapshot;
        let scope = snapshot.routing.root_part;
        let limits = SongLimits {
            max_nodes: 100_000,
            ..SongLimits::default()
        };
        let mut left = limits.max_nodes;
        if body.contains("range saw") {
            let reads = deny(snapshot);
            let (issuer, prefix) = path(snapshot, scope);
            let failure = snapshot
                .canonical_index_request(
                    scope,
                    intern_kw("drums"),
                    issuer,
                    &prefix,
                    span(0, 4, 1),
                    0,
                    limits,
                    &mut left,
                )
                .err()
                .unwrap();
            assert_eq!(failure.code, FailCode::Type);
            assert!(failure.message.contains("first-structure"));
            assert!(snapshot.occupancy.is_empty());
            assert!(snapshot.replay.is_none());
            assert_eq!(reads.get(), 0);
            continue;
        }
        retain(snapshot, scope, span(0, 4, 1), limits, &mut left);
        let prior = snapshot.replay.clone().unwrap();
        let reads = deny(snapshot);
        let request = request(snapshot, scope, span(0, 4, 1), limits, &mut left);
        let mut refused = 0;
        for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
            if address.owner().revision != request.revision {
                continue;
            }
            let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
            let failure = consume(
                snapshot,
                scope,
                span(0, 4, 1),
                &index,
                None,
                None,
                span(0, 4, 1),
                span(0, 4, 1),
                0,
                limits,
                &mut left,
            )
            .unwrap_err();
            assert_eq!(
                failure.code,
                FailCode::Type,
                "truthful Unknown/whole absence: {body}"
            );
            refused += 1;
        }
        assert!(refused > 0);
        assert!(Rc::ptr_eq(&prior, snapshot.replay.as_ref().unwrap()));
        assert_eq!(reads.get(), 0);
    }
}

#[test]
fn genuine_public_pattern_repeat_retains_symbolic_copy_configuration_prefixes() {
    use crate::pattern::{combinators::structure, pat::PParam};
    use crate::song::{capture_part, prepare_song, Song, SongSettings};
    use crate::value::value::Value;
    use std::collections::BTreeMap;
    let mut candidate = crate::song::snapshot::tests::candidate();
    let outcomes = candidate
        .evaluator
        .eval_str(
            "fn cut beat:\n\tfirst [0]\nslice {beat -> nil} 1 [cut]",
            crate::reader::span::FileId::new(77),
        )
        .unwrap();
    assert!(outcomes.iter().all(|outcome| outcome.value.is_ok()));
    let Value::Pattern(slice) = outcomes.last().unwrap().value.clone().unwrap() else {
        panic!("actual evaluated Slice Pattern")
    };
    let repeated = Rc::new(structure::repeat(slice, PParam::int(2), None));
    assert!(matches!(
        repeated.node,
        crate::pattern::pat::PatNode::Repeat(..)
    ));
    let original = Rc::new(
        Song::new(
            Rc::new(
                capture_part(
                    BTreeMap::from([(intern_kw("drums"), repeated)]),
                    Ratio64::new(2, 1).unwrap(),
                )
                .unwrap(),
            ),
            SongSettings::default(),
        )
        .unwrap(),
    );
    let assets = crate::song::assets::SongAssetLimits {
        max_resources: 0,
        max_pcm_bytes: 0,
        max_source_files: 0,
        max_source_bytes: 0,
        max_banks: 0,
        max_walk_nodes: 100_000,
        max_walk_depth: 256,
    };
    let mut freeze = crate::session::song::freeze::Freeze::new(&candidate.assets, assets);
    for slot in candidate.evaluator.candidate_slots() {
        freeze.slot(&slot).unwrap();
    }
    let Value::Song(song) = freeze.value(&Value::Song(original)).unwrap() else {
        panic!("actual frozen Song")
    };
    drop(freeze);
    let routing =
        crate::session::song::capture_original_test_routing(&candidate.evaluator, &song, assets)
            .unwrap();
    candidate.song = song;
    candidate.set_routing(routing);
    let mut prepared = prepare_song(candidate).unwrap();
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let FrozenPartNode::Capture(tracks) = &snapshot.routing.parts[scope].node else {
        panic!("original capture")
    };
    let pattern = &tracks[0].1;
    let recipe = pattern.index_timing().unwrap();
    let mut pending = vec![(recipe.root(), Vec::new())];
    let (issuer, prefix) = loop {
        let (node, prefix) = pending.pop().unwrap();
        let node = &recipe.nodes()[node as usize];
        if node.operation() == FrozenUseOperation::Slice {
            break (node.issuer(), prefix);
        }
        for edge in node.children() {
            let mut next = prefix.clone();
            next.extend_from_slice(edge.trace());
            pending.push((edge.child(), next));
        }
    };
    assert!(
        prefix
            .iter()
            .any(|term| matches!(term, FrozenUseTraceTerm::Copies { count: 2, .. })),
        "genuine original Repeat must remain symbolic: {prefix:?}"
    );
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    let issue = |left: &mut u32| {
        snapshot
            .canonical_index_request(
                scope,
                intern_kw("drums"),
                issuer,
                &prefix,
                span(0, 2, 1),
                0,
                limits,
                left,
            )
            .unwrap()
    };
    let request = issue(&mut left);
    retain_index_occupancy(snapshot, vec![request], limits, &mut left).unwrap();
    let reads = deny(snapshot);
    let request = snapshot
        .canonical_index_request(
            scope,
            intern_kw("drums"),
            issuer,
            &prefix,
            span(0, 2, 1),
            0,
            limits,
            &mut left,
        )
        .unwrap();
    let mut useful = 0;
    for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        let rows = index.rows(0, limits, &mut left).unwrap();
        assert!(rows
            .iter()
            .enumerate()
            .any(|(i, a)| rows.iter().skip(i + 1).any(|b| a.prefix() != b.prefix())));
        let (groups, _, _) = snapshot
            .retained_geometry_fixture(
                (scope, intern_kw("drums"), issuer, &prefix, span(0, 2, 1)),
                &index,
                None,
                None,
                span(0, 2, 1),
                span(0, 2, 1),
                0,
                limits,
                &mut left,
            )
            .unwrap();
        assert_eq!(
            groups.len(),
            2,
            "genuine concrete copies cannot collapse: {groups:?}"
        );
        useful += 1;
    }
    assert!(useful > 0);
    assert_eq!(reads.get(), 0);
}

#[test]
fn wrong_original_site_and_foreign_subject_member_timing_refuse_geometry() {
    let body = "stack [{slice {beat -> p} 1 [cut]} {slice {beat -> p} 2 [cut]}]";
    let mut prepared = from_code(&code(body));
    let snapshot = &mut prepared.snapshot;
    let scope = snapshot.routing.root_part;
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let (issuer, prefix) = path(snapshot, scope);
    let recipe = payload(snapshot, scope).index_timing().unwrap();
    let mut pending = vec![(recipe.root(), Vec::new())];
    let alternative = loop {
        let (node, prefix) = pending.pop().unwrap();
        let node = &recipe.nodes()[node as usize];
        if node.operation() == FrozenUseOperation::Slice && node.issuer() != issuer {
            break (node.issuer(), prefix);
        }
        for edge in node.children() {
            let mut next = prefix.clone();
            next.extend_from_slice(edge.trace());
            pending.push((edge.child(), next));
        }
    };
    let mut foreign = from_code(&code(body));
    let foreign_events = foreign.snapshot.query(span(0, 1, 1), &limits).unwrap();
    let foreign_timing = foreign_events
        .iter()
        .flat_map(|event| event.source_origin.as_ref())
        .flat_map(|origin| origin.slice_timings())
        .next()
        .unwrap();
    let reads = deny(snapshot);
    let request = request(snapshot, scope, span(0, 4, 1), limits, &mut left);
    let address = owner_addresses(snapshot, &request, 0, limits, &mut left)
        .unwrap()
        .into_iter()
        .find(|address| address.owner().revision == request.revision)
        .unwrap();
    let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
    assert_eq!(
        snapshot
            .retained_geometry_fixture(
                (
                    scope,
                    intern_kw("drums"),
                    alternative.0,
                    &alternative.1,
                    span(0, 4, 1)
                ),
                &index,
                None,
                None,
                span(0, 4, 1),
                span(0, 4, 1),
                0,
                limits,
                &mut left
            )
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert_eq!(
        snapshot
            .retained_geometry_fixture(
                (scope, intern_kw("drums"), issuer, &prefix, span(0, 4, 1)),
                &index,
                None,
                Some(foreign_timing),
                span(0, 4, 1),
                span(0, 4, 1),
                0,
                limits,
                &mut left
            )
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn selected_source_member_binding_rejects_a_genuine_other_original_subject() {
    let mut prepared = from_code(&nested(
        "slice {beat -> SUBJECT} 1 [cut]",
        "slice {beat -> p} 1 [cut]",
        false,
    ));
    let snapshot = &mut prepared.snapshot;
    let outer = snapshot.routing.root_part;
    let FrozenPartNode::Edit { source: inner, .. } = snapshot.routing.parts[outer].node else {
        panic!("actual selected ancestry")
    };
    let limits = SongLimits {
        max_nodes: 100_000,
        ..SongLimits::default()
    };
    let mut left = limits.max_nodes;
    retain(snapshot, outer, span(0, 1, 1), limits, &mut left);
    let issuer = path(snapshot, inner).0;
    let events = snapshot.query(span(0, 1, 1), &limits).unwrap();
    let timing = events
        .iter()
        .flat_map(|event| event.source_origin.as_ref())
        .flat_map(|origin| {
            origin.slice_timings().iter().chain(
                origin
                    .inherited
                    .iter()
                    .flat_map(|frame| frame.slice_timings()),
            )
        })
        .find(|timing| timing.issuer() == issuer)
        .unwrap();
    assert_ne!(
        timing.subject_handle().revision(),
        snapshot.routing.parts[inner].revision
    );
    let reads = deny(snapshot);
    let selected = &payload(snapshot, outer).sources[0];
    let request = request(snapshot, inner, span(0, 1, 1), limits, &mut left);
    let mut rejected = 0;
    for address in owner_addresses(snapshot, &request, 0, limits, &mut left).unwrap() {
        if address.owner().revision != request.revision {
            continue;
        }
        let index = bind_index(snapshot, &request, address, 0, limits, &mut left).unwrap();
        let boundaries = index.source_boundaries(0, limits, &mut left).unwrap();
        if !boundaries.iter().any(|boundary| {
            boundary.policy_parent().unwrap().revision == snapshot.routing.parts[outer].revision
        }) {
            continue;
        }
        let failure = consume(
            snapshot,
            inner,
            span(0, 1, 1),
            &index,
            Some(selected),
            Some(timing),
            span(0, 1, 1),
            span(0, 1, 1),
            0,
            limits,
            &mut left,
        )
        .unwrap_err();
        assert_eq!(failure.code, FailCode::Type);
        assert!(
            failure.message.contains("membership"),
            "actual selected member seam: {failure:?}"
        );
        rejected += 1;
    }
    assert!(rejected > 0);
    assert_eq!(reads.get(), 0);
}

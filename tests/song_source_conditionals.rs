//! Actual frozen conditional metadata preserves runtime integer/anchor semantics.
use vactr::pattern::TimeSpan;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::snapshot::{FrozenEdit, FrozenPartNode, FrozenRoutingInventory};
use vactr::song::source_uses::{
    certify_source_uses, resolve_source_use, FrozenPattern, FrozenStaticCondition as C,
    FrozenUseMapping as M, FrozenUseOperation as O, FrozenUseReason as R, FrozenUseTraceTerm as T,
};
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
    prepare_song(
        evaluate_song_candidate(code, "conditionals.vact", 1, SnapshotEpoch(71), &cx)
            .unwrap_or_else(|e| panic!("candidate {code}: {e:?}")),
    )
    .unwrap()
}
fn selected(operator: &str) -> PreparedSong {
    candidate(&format!("let base {{part [drums: {{s :analog > note [60 64]}}] duration: 4}}\nlet developed {{transform-instrument base :drums :analog {{p -> {operator}}}}}\nsong developed > play-song"))
}
fn payload(inventory: &FrozenRoutingInventory) -> FrozenPattern {
    match &inventory.parts[inventory.root_part].node {
        FrozenPartNode::Edit {
            edit: FrozenEdit::Transform { payload, .. },
            ..
        } => payload.clone(),
        FrozenPartNode::Capture(tracks) => tracks[0].1.clone(),
        other => panic!("unexpected fixture: {other:?}"),
    }
}
fn condition(pattern: &FrozenPattern) -> C {
    pattern
        .source_uses
        .nodes
        .iter()
        .find_map(|node| match node.mapping {
            M::ConditionalStatic(condition) => Some(condition),
            _ => None,
        })
        .expect("actual copied static conditional")
}
fn certify(
    inventory: &FrozenRoutingInventory,
    pattern: &FrozenPattern,
) -> Result<u64, vactr::vm::Failure> {
    Ok(certify_source_uses(
        inventory,
        pattern,
        TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(4)).unwrap(),
        SongLimits::default(),
    )?
    .configuration_bound())
}
#[test]
fn genuine_every_whenmod_and_chunk_preserve_exact_parameters_and_source_traces() {
    for (operator, expected) in [
        ("every p 2 {q -> fast q 2}", C::Every { period: 2 }),
        ("every p 0 {q -> fast q 2}", C::Every { period: 0 }),
        ("every p -2 {q -> fast q 2}", C::Every { period: -2 }),
        (
            "whenmod p 3 -1 {q -> fast q 2}",
            C::WhenMod {
                modulus: 3,
                threshold: -1,
            },
        ),
        (
            "whenmod p 3 0 {q -> fast q 2}",
            C::WhenMod {
                modulus: 3,
                threshold: 0,
            },
        ),
        (
            "whenmod p 3 3 {q -> fast q 2}",
            C::WhenMod {
                modulus: 3,
                threshold: 3,
            },
        ),
        ("chunk p 2 {q -> fast q 2}", C::Chunk { divisions: 2 }),
        ("chunk p 0 {q -> fast q 2}", C::Chunk { divisions: 0 }),
    ] {
        let mut song = selected(operator);
        let pattern = payload(song.snapshot().routing());
        assert_eq!(condition(&pattern), expected, "{operator}");
        let cover = certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(4)).unwrap(),
            SongLimits::default(),
        )
        .unwrap();
        let node = pattern
            .source_uses
            .nodes
            .iter()
            .find(|n| matches!(n.mapping, M::ConditionalStatic(_)))
            .unwrap();
        assert_eq!(node.edges.len(), 2);
        assert!(matches!(node.edges[0].trace.as_slice(), [T::Exact(step)] if step.ordinal == 0));
        assert!(
            matches!(node.edges[1].trace.as_slice(), [T::Exact(a), T::Exact(b)] if a.ordinal==1 && b.ordinal==1)
        );
        let events = song
            .query(
                TimeSpan::new(Ratio64::ZERO, Ratio64::from_int(4)).unwrap(),
                &SongLimits::default(),
            )
            .unwrap();
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
}
#[test]
fn integral_ratio_float_and_large_count_candidates_are_not_clamped() {
    for (literal, expected) in [("6/3", 2), ("2.0", 2), ("8192", 8192)] {
        let song = selected(&format!("every p {literal} {{q -> first [q]}}"));
        let pattern = payload(song.snapshot().routing());
        assert_eq!(condition(&pattern), C::Every { period: expected });
        certify(song.snapshot().routing(), &pattern).unwrap();
    }
}
#[test]
fn exact_fractional_predicates_match_negative_cycles_and_end_exclusion() {
    for cycle in [-7, -1, 0, 1, 8] {
        assert_eq!(
            C::Every { period: 3 }
                .selects(cycle, Ratio64::ZERO)
                .unwrap(),
            cycle.rem_euclid(3) == 0
        );
        for threshold in [-3, 0, 2, 3, i64::MAX] {
            assert_eq!(
                C::WhenMod {
                    modulus: 3,
                    threshold
                }
                .selects(cycle, Ratio64::ZERO)
                .unwrap(),
                cycle.rem_euclid(3) >= threshold
            );
        }
        let c = C::Chunk { divisions: 3 };
        let slice = c.anchor_slice(cycle).unwrap().unwrap();
        assert_eq!(slice.begin, Ratio64::new(cycle.rem_euclid(3), 3).unwrap());
        assert!(c.selects(cycle, slice.begin).unwrap());
        if slice.end < Ratio64::ONE {
            assert!(!c.selects(cycle, slice.end).unwrap());
        }
    }
    for c in [
        C::Every { period: i64::MIN },
        C::WhenMod {
            modulus: 0,
            threshold: -1,
        },
        C::Chunk { divisions: -2 },
    ] {
        assert_eq!(c.anchor_slice(i64::MIN).unwrap(), None);
    }
    let slice = C::Chunk {
        divisions: i64::MAX,
    }
    .anchor_slice(-1)
    .unwrap()
    .unwrap();
    assert_eq!(slice.end, Ratio64::ONE);
}
#[test]
fn malformed_static_dtos_reject_in_selected_and_ordinary_payloads() {
    let copied_condition = condition(&payload(
        selected("every p 2 {q -> fast q 2}").snapshot().routing(),
    ));
    for selected_payload in [false, true] {
        let song = if selected_payload {
            selected("every p 2 {q -> fast q 2}")
        } else {
            candidate("let base {part [drums: {every {s :analog} 2 {q -> fast q 2}}] duration: 4}\nlet selected {transform-instrument base :drums :analog {p -> first [p]}}\nsong selected > play-song")
        };
        let mut inventory = song.snapshot().routing().clone();
        let outer = payload(&inventory);
        let target_part = if selected_payload {
            inventory.root_part
        } else {
            inventory
                .parts
                .iter()
                .position(|part| matches!(part.node, FrozenPartNode::Capture(_)))
                .unwrap()
        };
        let target = match &mut inventory.parts[target_part].node {
            FrozenPartNode::Capture(tracks) => &mut tracks[0].1,
            FrozenPartNode::Edit {
                edit: FrozenEdit::Transform { payload, .. },
                ..
            } => payload,
            other => panic!("unexpected target: {other:?}"),
        };
        let index = target
            .source_uses
            .nodes
            .iter()
            .position(|n| n.operation == O::Every)
            .unwrap();
        if !selected_payload {
            // Explicit hostile public DTO setup: the genuine ordinary parameter
            // is lazy, so its recipe is DynamicTiming. Replace only its mapping
            // with metadata actually copied from the selected constant fixture.
            // Keep both real prepared edges and certify through the outer Source.
            assert_eq!(
                target.source_uses.nodes[index].mapping,
                M::Uncertifiable(R::DynamicTiming)
            );
            target.source_uses.nodes[index].mapping = M::ConditionalStatic(copied_condition);
        }
        let original = target.clone();
        let outer = if selected_payload {
            original.clone()
        } else {
            outer
        };
        certify(&inventory, &outer).unwrap();
        for defect in 0..5 {
            let mut bad_inventory = inventory.clone();
            let mut bad = original.clone();
            let node = &mut bad.source_uses.nodes[index];
            match defect {
                0 => node.operation = O::Chunk,
                1 => {
                    node.edges.pop();
                }
                2 => node.edges[0].trace.clear(),
                3 => node.edges[1].trace.reverse(),
                _ => node.edges[0].child = u32::MAX,
            }
            let checked = if selected_payload {
                bad
            } else {
                let FrozenPartNode::Capture(tracks) = &mut bad_inventory.parts[target_part].node
                else {
                    unreachable!()
                };
                tracks[0].1 = bad;
                outer.clone()
            };
            assert_eq!(
                certify(&bad_inventory, &checked).unwrap_err().code,
                FailCode::Type,
                "selected={selected_payload} defect={defect}"
            );
        }
    }
}
#[test]
fn dynamic_predicate_remains_addressed_for_selected_but_ordinary_geometry_is_allowed() {
    let song = selected("every p {t -> 2} {q -> first [q]}");
    let pattern = payload(song.snapshot().routing());
    assert!(pattern
        .source_uses
        .nodes
        .iter()
        .any(|n| n.operation == O::Every && n.mapping == M::Uncertifiable(R::DynamicTiming)));
    let error = certify(song.snapshot().routing(), &pattern).unwrap_err();
    assert_eq!(error.code, FailCode::BeyondCapability);
    assert!(error.message.contains("Every") && error.message.contains("DynamicTiming"));
    let song = candidate("let base {part [drums: {every {s :analog} {t -> 2} {q -> first [q]}}] duration: 4}\nlet selected {transform-instrument base :drums :analog {p -> first [p]}}\nsong selected > play-song");
    certify(
        song.snapshot().routing(),
        &payload(song.snapshot().routing()),
    )
    .unwrap();
}
#[test]
fn static_metadata_keeps_certificate_accounting_and_stochastic_recipe_separate() {
    let song = selected("every p 2 {q -> first [q]}");
    let pattern = payload(song.snapshot().routing());
    let window = TimeSpan::cycle(0).unwrap();
    let cover = certify_source_uses(
        song.snapshot().routing(),
        &pattern,
        window,
        SongLimits::default(),
    )
    .unwrap();
    let work = cover.consumed_work();
    let exact = certify_source_uses(
        song.snapshot().routing(),
        &pattern,
        window,
        SongLimits {
            max_nodes: work,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(exact.consumed_work(), work);
    assert_eq!(
        certify_source_uses(
            song.snapshot().routing(),
            &pattern,
            window,
            SongLimits {
                max_nodes: work - 1,
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        FailCode::FuelExhausted
    );
    let song = selected("sometimes-by p 0.5 {q -> first [q]}");
    assert!(payload(song.snapshot().routing())
        .source_uses
        .nodes
        .iter()
        .any(|n| n.mapping == M::Conditional));
}

//! Selected-source query and its original producer framing.
use super::{
    charge_identity, checked_words, identity_overflow, original_sound, SongEventOrigin, SongSource,
};
use crate::pattern::eval::QState;
use crate::pattern::query::{Event, TimeSpan};
use crate::song::EventHandle;
use crate::value::intern::name_of_kw;
use crate::vm::fail::{FailCode, Failure};

/// Canonical selected realization inherits all counters, limits and seed scope.
/// The caller trace is restored even on failure; certified source identity is
/// independent of an outer transform's producer prefix.
pub(crate) fn query_source(
    source: &SongSource,
    span: TimeSpan,
    state: &mut QState<'_, '_>,
) -> Result<Vec<Event>, Failure> {
    let prefix_len = state.producer_len().ok_or_else(|| {
        Failure::new(
            FailCode::Type,
            "selected song source requires a traced canonical song query context",
        )
    })?;
    charge_identity(state, checked_words(prefix_len, 2)?)?;
    let prefix = state.producer().ok_or_else(|| {
        Failure::new(
            FailCode::Type,
            "selected song source requires a traced canonical song query context",
        )
    })?;
    state.with_source_boundary(source, span, &prefix, |state, boundary| {
        for _ in &prefix.steps {
            state.pop_producer();
        }
        let queried =
            crate::song::query::nested_part_issued(source.part(), span, source.track(), state);
        for edge in &prefix.steps {
            state.push_producer(edge.kind, edge.ordinal);
        }
        let mut out = Vec::new();
        for row in queried? {
            let original = original_sound(&row);
            if row.track != source.track() || !source.selector().contains(&original) {
                continue;
            }
            state.spend(
                crate::pattern::eval::song_observation::event_copy_work(&row.event)?,
                None,
            )?;
            let mut event = row.event.clone();
            event.producer = Some(source_trace(&prefix, &row.handle, state)?);
            charge_identity(
                state,
                checked_words(prefix.steps.len(), 2)?
                    .checked_add(1)
                    .ok_or_else(identity_overflow)?,
            )?;
            charge_identity(
                state,
                u64::from(crate::song::source_uses::origin::source_whole_work(
                    &row.handle,
                    event.whole,
                )?),
            )?;
            let issued_handle = row.handle.clone();
            let inherited = event.song_source.take();
            event.song_source = Some(state.bind_issued_source_member(
                boundary,
                &row,
                SongEventOrigin {
                    source_part: event.part,
                    source_whole: event.whole,
                    issued_leaves: None,
                    slice_timings: Vec::new(),
                    issued_handle,
                    entry_trace: prefix.steps.clone(),
                    inherited,
                    handle: row.handle.clone(),
                    original_instrument: original,
                    route: row.route.clone(),
                    commit_mode: row.commit_mode,
                },
            )?);
            out.push(event);
        }
        if let Some(boundary) = boundary {
            state.complete_source_boundary(boundary, &out)?;
            state.finish_issued_source_boundary(boundary)?;
        }
        Ok(out)
    })
}

/// Frames placement, typed occurrence, exact time and tone by explicit role and
/// length. No compact fingerprint, allocation identity or filtered output rank.
fn source_trace(
    prefix: &crate::pattern::occ::ProducerTrace,
    handle: &EventHandle,
    state: &mut QState<'_, '_>,
) -> Result<crate::pattern::occ::ProducerTrace, Failure> {
    use crate::pattern::occ::ProducerTrace;
    let occurrence = handle.occurrence();
    let name = name_of_kw(handle.track());
    let input = handle
        .placement()
        .0
        .len()
        .checked_add(occurrence.producer_ordinals.len())
        .and_then(|n| n.checked_add(name.len()))
        .and_then(|n| n.checked_add(9))
        .ok_or_else(identity_overflow)?;
    let steps = prefix
        .steps
        .len()
        .checked_add(input)
        .and_then(|n| n.checked_add(24))
        .ok_or_else(identity_overflow)?;
    let input_words = checked_words(input, 1)?;
    let words = checked_words(steps, 2)?
        .checked_add(checked_words(prefix.steps.len(), 2)?)
        .and_then(|n| n.checked_add(input_words))
        .ok_or_else(identity_overflow)?;
    charge_identity(state, words)?;
    let mut trace = ProducerTrace {
        steps: Vec::with_capacity(steps),
    };
    trace.steps.extend_from_slice(&prefix.steps);
    frame(&mut trace, 0, handle.placement().0.iter().copied());
    frame(&mut trace, 1, occurrence.producer_ordinals.iter().copied());
    frame(&mut trace, 2, i64_words(occurrence.cycle).into_iter());
    frame(&mut trace, 3, i64_words(occurrence.onset.num()).into_iter());
    frame(&mut trace, 4, i64_words(occurrence.onset.den()).into_iter());
    frame(&mut trace, 5, [handle.tone()].into_iter());
    frame(&mut trace, 6, name.bytes().map(u32::from));
    frame(
        &mut trace,
        7,
        i64_words(handle.revision().0 as i64).into_iter(),
    );
    Ok(trace)
}
fn frame(
    trace: &mut crate::pattern::occ::ProducerTrace,
    role: u32,
    words: impl ExactSizeIterator<Item = u32>,
) {
    use crate::pattern::occ::ProducerKind;
    let length = words.len() as u64;
    trace.push(ProducerKind::ContentSource, role);
    trace.push(ProducerKind::ContentSource, length as u32);
    trace.push(ProducerKind::ContentSource, (length >> 32) as u32);
    for word in words {
        trace.push(ProducerKind::GeneratedBranch, word);
    }
}
fn i64_words(value: i64) -> [u32; 2] {
    [value as u32, ((value as u64) >> 32) as u32]
}

#[cfg(test)]
mod tests {
    use crate::pattern::eval::song_clock::{tests::sampling_with_replay, CanonicalClockProjection};
    use crate::pattern::query::TimeSpan;
    use crate::value::Ratio64;
    fn span(a: i64, b: i64, d: i64) -> TimeSpan {
        TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
    }
    fn code(body: &str) -> String {
        format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong selected tail-seconds: 0")
    }
    #[test]
    fn genuine_grid_empty_subject_has_structural_relation_without_source_membership() {
        sampling_with_replay(
            &code("grid {slice {beat -> nil} 2 [0]} [true true]"),
            |work| {
                let ledger = work.borrow();
                assert_eq!(ledger.observations.len(), 2);
                for (i, row) in ledger.observations.iter().enumerate() {
                    assert!(matches!(row.clock, CanonicalClockProjection::Known(_)));
                    let evidence = row.clock.sampling_evidence();
                    assert_eq!(evidence.len(), 1);
                    assert_eq!(evidence[0].0, Some(span(i as i64, i as i64 + 1, 2)));
                    assert_eq!(evidence[0].2, Ratio64::new(i as i64, 2).unwrap());
                    assert_eq!(row.event.whole, Some(TimeSpan::cycle(0).unwrap()));
                    assert!(row.clock.source_evidence().is_empty());
                }
            },
        );
    }
    #[test]
    fn genuine_cat_fastcat_signed_off_maps_preserve_original_branches() {
        for (body, expected) in [
            (
                "cat [{slice {beat -> nil} 2 [0]} {slice {beat -> nil} 2 [0]}]",
                vec![span(0, 1, 1)],
            ),
            (
                "fastcat [{slice {beat -> nil} 2 [0]} {slice {beat -> nil} 2 [0]}]",
                vec![span(0, 1, 2), span(1, 2, 2)],
            ),
            (
                "off {slice {beat -> nil} 2 [0]} 0.25 {q -> first [q]}",
                vec![span(0, 4, 4), span(-3, 1, 4), span(1, 5, 4)],
            ),
        ] {
            sampling_with_replay(&code(body), |work| {
                let rows = work.borrow().observations.clone();
                assert_eq!(rows.len(), expected.len());
                let wholes: Vec<_> = rows
                    .iter()
                    .map(|row| {
                        row.clock
                            .project_for(
                                &row.owner,
                                row.event.whole.unwrap(),
                                row.issuer_sample_start,
                                work,
                            )
                            .unwrap()
                            .unwrap()
                            .whole
                    })
                    .collect();
                assert_eq!(wholes, expected);
                for pair in rows.windows(2) {
                    assert!(
                        pair[0].prefix != pair[1].prefix
                            || pair[0].event.whole != pair[1].event.whole
                            || pair[0].event.occ != pair[1].event.occ,
                        "original branch/cycle occurrence identity remains distinct"
                    );
                }
            });
        }
    }
    #[test]
    fn genuine_inner_execution_rebinds_two_parent_points_and_empty_source_exactly() {
        let code = "fn cut beat:\n\tfirst [0]\nfn innered p:\n\tslice {beat -> p} 2 [cut nil]\nfn indexed p:\n\tgrid {beat -> p} [true true]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet inner {transform-instrument base :drums :analog innered}\nlet selected {transform-instrument inner :drums :analog indexed}\nsong selected tail-seconds: 0";
        sampling_with_replay(code, |work| {
            let ledger = work.borrow();
            let linked: Vec<_> = ledger
                .observations
                .iter()
                .filter(|row| !row.clock.source_evidence().is_empty())
                .collect();
            assert_eq!(linked.len(), 2);
            assert_eq!(linked[0].owner, linked[1].owner);
            assert_eq!(linked[0].prefix, linked[1].prefix);
            assert_eq!(linked[0].event.occ, linked[1].event.occ);
            assert_eq!(linked[0].event.whole, linked[1].event.whole);
            assert_eq!(linked[0].seed, linked[1].seed);
            let first = linked[0].clock.source_evidence();
            let second = linked[1].clock.source_evidence();
            assert_eq!(first[0].0, TimeSpan::point(Ratio64::ZERO));
            assert_eq!(second[0].0, TimeSpan::point(Ratio64::new(1, 2).unwrap()));
            assert_eq!(first[0].3.len(), 2);
            assert!(
                second[0].3.is_empty(),
                "an empty selected query never mints membership"
            );
            for origin in &first[0].3 {
                assert_eq!(origin.source_whole, Some(span(0, 1, 2)));
                assert_eq!(origin.entry_trace, first[0].1.steps);
                assert_eq!(origin.issued_handle, origin.handle);
            }
            assert_eq!(first[0].2.sampling_evidence()[0].0, Some(span(0, 1, 2)));
            assert_eq!(second[0].2.sampling_evidence()[0].0, Some(span(1, 2, 2)));
            assert_eq!(
                ledger
                    .executions
                    .iter()
                    .filter(|execution| execution.owner() == &linked[0].owner
                        && execution
                            .observations()
                            .iter()
                            .any(|row| !row.clock.source_evidence().is_empty()))
                    .count(),
                1,
                "the genuine inner q executes once, independently of observation copies"
            );
        });
    }
    #[test]
    fn actual_continuous_timing_and_unrelated_samples_cannot_reuse_a_permit() {
        crate::pattern::eval::song_clock::tests::continuous_permit_isolation();
    }
    #[test]
    fn genuine_evaluated_segment_index_keeps_original_outputs_and_replay() {
        sampling_with_replay(
            &code("let index {segment {range saw 0 0} 2}\n\tslice {beat -> nil} 2 index"),
            |work| {
                let ledger = work.borrow();
                assert_eq!(ledger.observations.len(), 2);
                assert!(ledger
                    .observations
                    .iter()
                    .all(|row| matches!(row.clock, CanonicalClockProjection::Known(_))));
                for row in &ledger.observations {
                    assert_eq!(
                        row.event.whole.unwrap().duration().unwrap(),
                        Ratio64::new(1, 2).unwrap()
                    );
                }
            },
        );
    }
    #[test]
    fn genuine_callable_sound_keeps_original_outputs_and_replay() {
        sampling_with_replay(&code("s {beat -> {slice {t -> p} 2 [cut nil]}}"), |work| {
            let ledger = work.borrow();
            assert!(!ledger.observations.is_empty());
            assert!(ledger
                .observations
                .iter()
                .all(|row| matches!(row.clock, CanonicalClockProjection::Known(_))));
        });
    }
    #[test]
    fn genuine_sampling_and_rebinding_share_exact_original_work_one_less() {
        let code = "fn cut beat:\n\tfirst [0]\nfn innered p:\n\tslice {beat -> p} 2 [cut nil]\nfn indexed p:\n\tgrid {beat -> p} [true true]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet inner {transform-instrument base :drums :analog innered}\nlet selected {transform-instrument inner :drums :analog indexed}\nsong selected tail-seconds: 0";
        crate::pattern::eval::song_clock::tests::sampling_exact_work(code);
    }
    #[test]
    fn genuine_source_fault_restores_sibling_and_keeps_membership_uncompleted() {
        crate::pattern::eval::song_clock::tests::sampling_source_failure_restoration();
    }
    #[test]
    fn genuine_sound_resolver_dependency_queries_have_no_sampling_permission() {
        let code = "fn dependent beat:\n\tfirst [61]\nlet captured {part [drums: {s :analog > note dependent}] duration: 1}\nfn resolve p beat:\n\tlet rows {part-events {captured} :drums 0 1}\n\tlet tone {{first rows} :note}\n\tfirst [{slice {t -> p} 2 [{- tone 61} nil]}]\nfn indexed p:\n\ts {beat -> resolve p beat}\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0";
        sampling_with_replay(code, |work| {
            let ledger = work.borrow();
            assert!(
                !ledger.calls.is_empty(),
                "actual resolver and dependent VM execution"
            );
            assert!(!ledger.observations.is_empty());
            for row in &ledger.observations {
                assert!(matches!(row.clock, CanonicalClockProjection::Known(_)));
                assert_eq!(row.clock.sampling_evidence().len(), 1);
                assert!(
                    row.clock.source_evidence().is_empty(),
                    "computational Part query cannot mint selected-source evidence"
                );
                assert!(
                    crate::value::eq::deep_eq(
                        &row.event.value,
                        &crate::value::value::Value::Int(0)
                    )
                    .expect("actual retained numeric Index comparison"),
                    "actual dependent note 61 determines original Index value"
                );
            }
        });
    }
}

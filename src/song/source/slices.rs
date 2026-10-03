//! Query-issued Slice timing and preallocation admission for selected origins.
use super::{charge_identity, SongEventOrigin};
use crate::pattern::eval::QState;
use crate::pattern::occ::{ProducerKind, ProducerStep};
use crate::pattern::pat::Pat;
use crate::pattern::query::{Event, TimeSpan};
use crate::reader::span::NodeId;
use crate::song::source_uses::origin::{source_authority_validation_work, validate_source_whole};
use crate::song::source_uses::FrozenSliceTiming;
use crate::song::EventHandle;
use crate::value::ratio::Ratio64;
use crate::value::value::Sound;
use crate::vm::fail::{FailCode, Failure};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(in crate::song) struct SongSliceTiming {
    issuer: NodeId,
    issuer_trace: Vec<ProducerStep>,
    index_trace: Vec<ProducerStep>,
    index_whole: TimeSpan,
    sample_start: Ratio64,
    subject_handle: EventHandle,
}
fn overflow() -> Failure {
    Failure::new(FailCode::Overflow, "Slice timing work overflow")
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}
fn admit(remaining: &mut u32, cost: u32) -> Result<(), Failure> {
    *remaining = remaining
        .checked_sub(cost)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "Slice timing work exhausted"))?;
    Ok(())
}
fn handle_work(handle: &EventHandle) -> Result<usize, Failure> {
    handle
        .placement()
        .0
        .len()
        .checked_add(handle.occurrence().producer_ordinals.len())
        .and_then(|n| n.checked_add(8))
        .ok_or_else(overflow)
}
fn work(
    issuer: &[ProducerStep],
    index: &[ProducerStep],
    handle: &EventHandle,
) -> Result<u32, Failure> {
    issuer
        .len()
        .checked_add(index.len())
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(handle_work(handle).ok()?))
        .and_then(|n| n.checked_add(13))
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(overflow)
}
pub(in crate::song) fn producer_depth(timing: &SongSliceTiming) -> Result<u32, Failure> {
    u32::try_from(timing.index_trace.len()).map_err(|_| overflow())
}
pub(in crate::song) fn slice_timing_work(timing: &SongSliceTiming) -> Result<u32, Failure> {
    work(
        &timing.issuer_trace,
        &timing.index_trace,
        &timing.subject_handle,
    )
}
fn sound_work(sound: &Sound) -> Result<usize, Failure> {
    match sound {
        Sound::Sample(path) => path.text.len().checked_add(1).ok_or_else(overflow),
        Sound::Osc(addr) => addr.len().checked_add(1).ok_or_else(overflow),
        _ => Ok(1),
    }
}
/// Complete shallow origin clone, including owned vectors; inherited Rc stays shared.
pub(in crate::song) fn slice_origin_clone_work(origin: &SongEventOrigin) -> Result<u32, Failure> {
    let mut cost = handle_work(&origin.handle)?
        .checked_add(handle_work(&origin.issued_handle)?)
        .and_then(|n| n.checked_add(origin.entry_trace.len().checked_mul(2)?))
        .and_then(|n| n.checked_add(sound_work(&origin.original_instrument).ok()?))
        .and_then(|n| n.checked_add(12))
        .ok_or_else(overflow)?;
    if let Some(route) = &origin.route {
        for sound in route.family.family() {
            cost = cost.checked_add(sound_work(sound)?).ok_or_else(overflow)?;
        }
        cost = cost.checked_add(2).ok_or_else(overflow)?;
    }
    for timing in &origin.slice_timings {
        cost = cost
            .checked_add(slice_timing_work(timing)? as usize)
            .ok_or_else(overflow)?;
    }
    u32::try_from(cost).map_err(|_| overflow())
}
/// Only actual index-driven query_slice calls issue a timing record.
pub(crate) fn issue_slice_timing(
    event: &mut Event,
    index_event: &Event,
    slice: &Pat,
    state: &mut QState<'_, '_>,
) -> Result<(), Failure> {
    let Some(origin) = event.song_source.as_ref() else {
        return Ok(());
    };
    let validation = source_authority_validation_work(
        &origin.handle,
        &origin.issued_handle,
        origin.source_whole,
    )?;
    charge_identity(state, u64::from(validation))?;
    let mut validation_remaining = validation;
    validate_source_whole(
        &origin.handle,
        &origin.issued_handle,
        origin.source_part,
        origin.source_whole,
        &mut validation_remaining,
    )?;
    let whole = index_event
        .whole
        .ok_or_else(crate::pattern::combinators::no_whole)?;
    let prefix_len = state
        .producer_len()
        .ok_or_else(|| invalid("Slice source timing requires traced query"))?;
    let index = index_event
        .producer
        .as_ref()
        .ok_or_else(|| invalid("Slice index timing has no producer"))?;
    if let Some(limits) = state.song_limits() {
        let mut frame = Some(&**origin);
        let mut depth = 1u32; // The timing about to be appended.
        while let Some(current) = frame {
            charge_identity(state, 1)?;
            depth = depth
                .checked_add(1)
                .and_then(|n| n.checked_add(u32::try_from(current.slice_timings.len()).ok()?))
                .ok_or_else(overflow)?;
            if depth > limits.max_depth {
                return Err(Failure::new(
                    FailCode::DepthExceeded,
                    "Slice issuing chain depth exhausted",
                ));
            }
            frame = current.inherited.as_deref();
        }
        depth = depth
            .checked_add(u32::try_from(index.steps.len()).map_err(|_| overflow())?)
            .ok_or_else(overflow)?;
        if depth > limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "Slice issuing producer depth exhausted",
            ));
        }
    }
    let cost = prefix_len
        .checked_add(index.steps.len())
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(handle_work(&origin.issued_handle).ok()?))
        .and_then(|n| n.checked_add(origin.slice_timings.len()))
        .and_then(|n| n.checked_add(13))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(overflow)?;
    charge_identity(state, cost)?;
    if Rc::strong_count(origin) > 1 {
        charge_identity(state, u64::from(slice_origin_clone_work(origin)?))?;
    }
    let prefix = state
        .producer()
        .ok_or_else(|| invalid("Slice source timing requires traced query"))?;
    validate_parts(&prefix.steps, &index.steps, whole, index_event.anchor())?;
    let timing = SongSliceTiming {
        issuer: slice.id,
        issuer_trace: prefix.steps,
        index_trace: index.steps.clone(),
        index_whole: whole,
        sample_start: index_event.anchor(),
        subject_handle: origin.issued_handle.clone(),
    };
    // All authority/equality/copy admission precedes make_mut and push.
    Rc::make_mut(
        event
            .song_source
            .as_mut()
            .ok_or_else(|| invalid("Slice source disappeared"))?,
    )
    .slice_timings
    .push(timing);
    Ok(())
}
fn validate_parts(
    issuer: &[ProducerStep],
    index: &[ProducerStep],
    whole: TimeSpan,
    start: Ratio64,
) -> Result<(), Failure> {
    if whole.end < whole.begin
        || start != whole.begin
        || !index.starts_with(issuer)
        || !matches!(
            index.get(issuer.len()),
            Some(ProducerStep {
                kind: ProducerKind::Child,
                ordinal: 1
            })
        )
    {
        return Err(invalid("invalid issued Slice index timing"));
    }
    Ok(())
}
pub(in crate::song) fn admit_slice_timings(
    timings: &[SongSliceTiming],
    subject: &EventHandle,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<(), Failure> {
    if timings.len() > max_depth as usize {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "Slice timing nesting exhausted",
        ));
    }
    admit(
        remaining,
        u32::try_from(timings.len()).map_err(|_| overflow())?,
    )?;
    for timing in timings {
        if producer_depth(timing)? > max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "Slice producer depth exhausted",
            ));
        }
        let compare = source_authority_validation_work(&timing.subject_handle, subject, None)?;
        admit(
            remaining,
            slice_timing_work(timing)?
                .checked_add(compare)
                .ok_or_else(overflow)?,
        )?;
        if timing.subject_handle != *subject {
            return Err(invalid("Slice timing subject differs from issued handle"));
        }
        validate_parts(
            &timing.issuer_trace,
            &timing.index_trace,
            timing.index_whole,
            timing.sample_start,
        )?;
    }
    Ok(())
}
pub(in crate::song) fn copy_admitted_slice_timings(
    timings: &[SongSliceTiming],
    admission_depth: u32,
) -> Vec<FrozenSliceTiming> {
    timings
        .iter()
        .map(|timing| {
            FrozenSliceTiming::from_issued(
                timing.issuer,
                timing.issuer_trace.clone(),
                timing.index_trace.clone(),
                timing.index_whole,
                timing.sample_start,
                timing.subject_handle.clone(),
                admission_depth,
            )
        })
        .collect()
}
pub(in crate::song) fn validate_frozen_slice_timings(
    timings: &[FrozenSliceTiming],
    subject: &EventHandle,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<(), Failure> {
    if timings.len() > max_depth as usize {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "Slice timing nesting exhausted",
        ));
    }
    admit(
        remaining,
        u32::try_from(timings.len()).map_err(|_| overflow())?,
    )?;
    for timing in timings {
        if timing.admission_depth() > max_depth || timing.index_trace().len() > max_depth as usize {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "frozen Slice authority depth exhausted",
            ));
        }
        let cost = work(
            timing.issuer_trace(),
            timing.index_trace(),
            timing.subject_handle(),
        )?
        .checked_add(source_authority_validation_work(
            timing.subject_handle(),
            subject,
            None,
        )?)
        .ok_or_else(overflow)?;
        admit(remaining, cost)?;
        if timing.subject_handle() != subject {
            return Err(invalid("Slice timing subject differs from issued handle"));
        }
        validate_parts(
            timing.issuer_trace(),
            timing.index_trace(),
            timing.index_whole(),
            timing.sample_start(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::eval::{InputCells, QueryCtx, QueryVm};
    use crate::pattern::pat::{PParam, PatNode, SliceCuts};
    use crate::song::{
        capture_part, query_part, InstrumentSelector, Part, SongLimits, SongQueryCtx, SongSource,
    };
    use crate::value::intern::intern_kw;
    use crate::value::value::Value;
    struct NoCalls;
    impl QueryVm for NoCalls {
        fn call(&mut self, _: &Value, _: &[Value]) -> Result<Value, Failure> {
            Err(Failure::new(FailCode::Type, "unexpected callback"))
        }
        fn deref(&mut self, r: &crate::ns::namespace::VarSlotRef) -> Result<Value, Failure> {
            Ok(r.get())
        }
        fn take_output(&mut self) -> Vec<(crate::vm::fail::Origin, Rc<str>)> {
            Vec::new()
        }
        fn put_output(&mut self, output: Vec<(crate::vm::fail::Origin, Rc<str>)>) {
            assert!(output.is_empty());
        }
        fn sound_kit(&mut self) -> Result<Value, Failure> {
            Ok(Value::dict(Default::default()))
        }
    }
    fn ratio(n: i64, d: i64) -> Ratio64 {
        Ratio64::new(n, d).unwrap()
    }
    fn base() -> Rc<Part> {
        let p = crate::pattern::build::pure(
            Value::Sound(Rc::new(Sound::Builtin(intern_kw("analog")))),
            None,
        );
        Rc::new(
            capture_part(
                [(intern_kw("drums"), Rc::new(p))].into(),
                Ratio64::from_int(4),
            )
            .unwrap(),
        )
    }
    fn selected(part: Rc<Part>, structured: bool) -> Rc<Pat> {
        let source = SongSource::new(
            part,
            intern_kw("drums"),
            InstrumentSelector::new(vec![Sound::Builtin(intern_kw("analog"))]).unwrap(),
        )
        .unwrap();
        Rc::new(Pat::new(
            PatNode::SongSource(Rc::new(source)),
            None,
            structured,
        ))
    }
    fn sliced(subject: Rc<Pat>, splice: bool) -> Rc<Pat> {
        let index = Rc::new(crate::pattern::build::value_steps(&[
            Value::Int(0),
            Value::Nil,
            Value::Int(1),
        ]));
        let cuts = SliceCuts::Equal(PParam::Const(Value::Int(2)));
        Rc::new(if splice {
            crate::pattern::combinators::region::splice(subject, cuts, index, None)
        } else {
            crate::pattern::combinators::region::slice(subject, cuts, index, None)
        })
    }
    fn real_origins(pattern: Rc<Pat>, window: TimeSpan) -> Vec<Rc<SongEventOrigin>> {
        let part =
            capture_part([(intern_kw("drums"), pattern)].into(), Ratio64::from_int(4)).unwrap();
        let mut vm = NoCalls;
        let cells = InputCells::new();
        query_part(
            &part,
            window,
            &mut SongQueryCtx {
                vm: &mut vm,
                cells: &cells,
                seed: 1,
                tempo: Default::default(),
                limits: &SongLimits::default(),
            },
        )
        .unwrap()
        .into_iter()
        .map(|row| row.event.song_source.unwrap())
        .collect()
    }
    #[test]
    fn first_structure_slice_issues_real_index_whole_and_sample_start() {
        for splice in [false, true] {
            let rows = real_origins(
                sliced(selected(base(), false), splice),
                TimeSpan::cycle(0).unwrap(),
            );
            assert_eq!(rows.len(), 2);
            for (row, begin, end) in [
                (&rows[0], Ratio64::ZERO, ratio(1, 3)),
                (&rows[1], ratio(2, 3), Ratio64::ONE),
            ] {
                let frozen = super::super::copy_origin(row, &mut 100000, 256).unwrap();
                let timing = &frozen.slice_timings()[0];
                assert_eq!(timing.index_whole(), TimeSpan::new(begin, end).unwrap());
                assert_eq!(timing.sample_start(), begin);
                assert_eq!(timing.subject_handle(), &frozen.handle);
                assert_eq!(frozen.source_whole(), Some(TimeSpan::cycle(0).unwrap()));
                assert_ne!(frozen.source_whole(), Some(timing.index_whole()));
                assert!(timing.index_trace().starts_with(timing.issuer_trace()));
                assert_eq!(timing.index_trace()[timing.issuer_trace().len()].ordinal, 1);
            }
        }
    }
    #[test]
    fn structured_slice_retains_subject_timing_without_index_authority() {
        let rows = real_origins(
            sliced(selected(base(), true), false),
            TimeSpan::cycle(0).unwrap(),
        );
        assert!(!rows.is_empty());
        for row in rows {
            assert!(row.slice_timings.is_empty());
            assert_eq!(row.source_whole(), Some(TimeSpan::cycle(0).unwrap()));
        }
        let ordinary = Rc::new(crate::pattern::build::pure(
            Value::Sound(Rc::new(Sound::Builtin(intern_kw("analog")))),
            None,
        ));
        let pat = sliced(ordinary, false);
        let mut vm = NoCalls;
        let cells = InputCells::new();
        let result = crate::pattern::query::query(
            &pat,
            TimeSpan::cycle(0).unwrap(),
            &mut QueryCtx::new(&mut vm, &cells, 1),
        );
        assert!(result.faults.is_empty());
        assert_eq!(result.events.len(), 2);
        assert!(result
            .events
            .iter()
            .all(|event| event.song_source.is_none()));
    }
    #[test]
    fn nested_rate_reflection_weighted_indices_preserve_all_timing_frames() {
        let inner = sliced(selected(base(), false), false);
        let inner_part = Rc::new(
            capture_part([(intern_kw("drums"), inner)].into(), Ratio64::from_int(4)).unwrap(),
        );
        let outer = sliced(selected(inner_part, false), true);
        let full = real_origins(outer.clone(), TimeSpan::cycle(0).unwrap());
        assert!(!full.is_empty());
        for row in &full {
            let frozen = super::super::copy_origin(row, &mut 100000, 256).unwrap();
            assert_eq!(frozen.slice_timings().len(), 1);
            assert_eq!(frozen.inherited.len(), 1);
            assert_eq!(frozen.inherited[0].slice_timings().len(), 1);
        }
        // Reordered windows and point queries retain the original local clock.
        for window in [
            TimeSpan::new(ratio(2, 3), Ratio64::ONE).unwrap(),
            TimeSpan::point(Ratio64::ZERO),
            TimeSpan::new(Ratio64::ZERO, ratio(1, 3)).unwrap(),
        ] {
            for row in real_origins(outer.clone(), window) {
                let found = full.iter().find(|item| item.handle == row.handle).unwrap();
                assert_eq!(
                    super::super::copy_origin(found, &mut 100000, 256).unwrap(),
                    super::super::copy_origin(&row, &mut 100000, 256).unwrap()
                );
            }
        }
        for node in [
            PatNode::Fast(outer.clone(), PParam::Const(Value::Int(2))),
            PatNode::Slow(outer.clone(), PParam::Const(Value::Int(2))),
            PatNode::Rev(outer),
        ] {
            let wrapped = Rc::new(Pat::new(node, None, true));
            for row in real_origins(wrapped, TimeSpan::cycle(0).unwrap()) {
                assert_eq!(row.slice_timings.len(), 1);
                assert_eq!(row.inherited.as_ref().unwrap().slice_timings.len(), 1);
            }
        }
    }
    #[test]
    fn timing_authority_rejects_handle_swaps_before_copy() {
        let rows = real_origins(
            sliced(selected(base(), false), false),
            TimeSpan::cycle(0).unwrap(),
        );
        let mut bad = (*rows[0]).clone();
        let other = real_origins(selected(base(), true), TimeSpan::cycle(1).unwrap());
        bad.handle = other[0].handle.clone();
        assert_eq!(
            super::super::copy_origin(&bad, &mut 100000, 256)
                .unwrap_err()
                .code,
            FailCode::Type
        );
        let foreign_track = intern_kw("hat");
        let sound = Sound::Builtin(intern_kw("analog"));
        let foreign_part = Rc::new(
            capture_part(
                [(
                    foreign_track,
                    Rc::new(crate::pattern::build::pure(
                        Value::Sound(Rc::new(sound.clone())),
                        None,
                    )),
                )]
                .into(),
                Ratio64::from_int(4),
            )
            .unwrap(),
        );
        let foreign_source = SongSource::new(
            foreign_part,
            foreign_track,
            InstrumentSelector::new(vec![sound]).unwrap(),
        )
        .unwrap();
        let foreign = real_origins(
            Rc::new(Pat::new(
                PatNode::SongSource(Rc::new(foreign_source)),
                None,
                true,
            )),
            TimeSpan::cycle(0).unwrap(),
        );
        let mut foreign_swap = (*rows[0]).clone();
        foreign_swap.handle = foreign[0].handle.clone();
        assert_eq!(
            super::super::copy_origin(&foreign_swap, &mut 100000, 256)
                .unwrap_err()
                .code,
            FailCode::Type
        );
        let mut none = (*rows[0]).clone();
        none.source_whole = None;
        assert_eq!(
            super::super::copy_origin(&none, &mut 100000, 256)
                .unwrap()
                .source_whole(),
            None
        );
    }
    #[test]
    fn slice_timing_copy_and_validation_use_shared_exact_quota() {
        let rows = real_origins(
            sliced(selected(base(), false), false),
            TimeSpan::cycle(0).unwrap(),
        );
        let row = &rows[0];
        let mut remaining = 100000;
        let frozen = super::super::copy_origin(row, &mut remaining, 256).unwrap();
        let cost = 100000 - remaining;
        assert!(cost > 100);
        let mut exact = cost;
        assert_eq!(
            super::super::copy_origin(row, &mut exact, 256).unwrap(),
            frozen
        );
        assert_eq!(exact, 0);
        assert_eq!(
            super::super::copy_origin(row, &mut (cost - 1), 256)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        let mut twice = cost * 2;
        super::super::copy_origin(row, &mut twice, 256).unwrap();
        super::super::copy_origin(row, &mut twice, 256).unwrap();
        assert_eq!(twice, 0);
        let depth = frozen.slice_timings()[0].admission_depth();
        super::super::copy_origin(row, &mut 100000, depth).unwrap();
        assert_eq!(
            super::super::copy_origin(row, &mut 100000, depth - 1)
                .unwrap_err()
                .code,
            FailCode::DepthExceeded
        );
        assert_eq!(
            validate_frozen_slice_timings(
                frozen.slice_timings(),
                &frozen.handle,
                &mut 100000,
                depth - 1
            )
            .unwrap_err()
            .code,
            FailCode::DepthExceeded
        );
    }
    #[test]
    fn weighted_nonunit_offset_index_retains_its_actual_local_whole() {
        let held = Rc::new(Pat::new(
            PatNode::Hold(
                Rc::new(crate::pattern::build::pure(Value::Int(0), None)),
                PParam::Const(Value::Int(2)),
            ),
            None,
            true,
        ));
        let weighted = Rc::new(crate::pattern::build::value_steps(&[
            Value::Pattern(held),
            Value::Int(1),
        ]));
        let fast = Rc::new(Pat::new(
            PatNode::Fast(weighted, PParam::Const(Value::Int(2))),
            None,
            true,
        ));
        let index = Rc::new(Pat::new(
            PatNode::Iter(fast, PParam::Const(Value::Int(3))),
            None,
            true,
        ));
        let subject = selected(base(), false);
        let pat = Rc::new(crate::pattern::combinators::region::slice(
            subject,
            SliceCuts::Equal(PParam::Const(Value::Int(2))),
            index.clone(),
            None,
        ));
        let mut vm = NoCalls;
        let cells = InputCells::new();
        let actual = crate::pattern::query::query_traced(
            &index,
            TimeSpan::cycle(1).unwrap(),
            &mut QueryCtx::new(&mut vm, &cells, 1),
        );
        assert!(actual.faults.is_empty());
        assert!(!actual.events.is_empty());
        let rows = real_origins(pat, TimeSpan::cycle(1).unwrap());
        assert!(!rows.is_empty());
        for origin in rows {
            let timing = &origin.slice_timings[0];
            assert!(actual
                .events
                .iter()
                .any(|event| event.whole == Some(timing.index_whole)));
            assert_eq!(timing.sample_start, timing.index_whole.begin);
        }
    }
    #[test]
    fn continuous_source_none_and_absent_index_whole_are_actual_queries() {
        let sound = Rc::new(crate::pattern::build::pure(
            Value::Sound(Rc::new(Sound::Builtin(intern_kw("analog")))),
            None,
        ));
        let signal = Rc::new(Pat::new(
            PatNode::Signal(Rc::new(crate::pattern::signal::Sig::Phase)),
            None,
            true,
        ));
        let continuous = Rc::new(crate::pattern::combinators::control::control(
            intern_kw("gain"),
            signal.clone(),
            sound,
            None,
        ));
        let part = Rc::new(
            capture_part(
                [(intern_kw("drums"), continuous)].into(),
                Ratio64::from_int(4),
            )
            .unwrap(),
        );
        let rows = real_origins(
            sliced(selected(part, false), false),
            TimeSpan::cycle(0).unwrap(),
        );
        assert!(!rows.is_empty());
        for origin in rows {
            assert_eq!(origin.source_whole(), None);
            assert_eq!(
                super::super::copy_origin(&origin, &mut 100000, 256)
                    .unwrap()
                    .source_whole(),
                None
            );
        }
        let pat = Rc::new(crate::pattern::combinators::region::slice(
            selected(base(), false),
            SliceCuts::Equal(PParam::Const(Value::Int(2))),
            signal,
            None,
        ));
        let part = capture_part([(intern_kw("drums"), pat)].into(), Ratio64::from_int(4)).unwrap();
        let mut vm = NoCalls;
        let cells = InputCells::new();
        let failure = query_part(
            &part,
            TimeSpan::cycle(0).unwrap(),
            &mut SongQueryCtx {
                vm: &mut vm,
                cells: &cells,
                seed: 1,
                tempo: Default::default(),
                limits: &SongLimits::default(),
            },
        )
        .unwrap_err();
        assert_eq!(failure.code, FailCode::NoWhole);
    }
    #[test]
    fn inherited_timing_copy_prices_both_frames_and_rejects_same_onset_duration_swap() {
        let inner = sliced(selected(base(), false), false);
        let part = Rc::new(
            capture_part([(intern_kw("drums"), inner)].into(), Ratio64::from_int(4)).unwrap(),
        );
        let rows = real_origins(
            sliced(selected(part, false), false),
            TimeSpan::cycle(0).unwrap(),
        );
        let origin = &rows[0];
        let mut remaining = 100000;
        let copied = super::super::copy_origin(origin, &mut remaining, 256).unwrap();
        assert_eq!(copied.inherited.len(), 1);
        assert_eq!(copied.inherited[0].slice_timings().len(), 1);
        let cost = 100000 - remaining;
        let mut exact = cost;
        super::super::copy_origin(origin, &mut exact, 256).unwrap();
        assert_eq!(exact, 0);
        assert_eq!(
            super::super::copy_origin(origin, &mut (cost - 1), 256)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        let slow = Rc::new(Pat::new(
            PatNode::Slow(
                Rc::new(crate::pattern::build::pure(
                    Value::Sound(Rc::new(Sound::Builtin(intern_kw("analog")))),
                    None,
                )),
                PParam::Const(Value::Int(2)),
            ),
            None,
            true,
        ));
        let slower = Rc::new(
            capture_part([(intern_kw("drums"), slow)].into(), Ratio64::from_int(4)).unwrap(),
        );
        let swapped = real_origins(selected(slower, true), TimeSpan::cycle(0).unwrap());
        assert_eq!(
            origin.handle.occurrence().onset,
            swapped[0].handle.occurrence().onset
        );
        assert_ne!(origin.source_whole(), swapped[0].source_whole());
        let pure = crate::pattern::build::pure(
            Value::Sound(Rc::new(Sound::Builtin(intern_kw("analog")))),
            None,
        );
        let long = Pat::new(
            PatNode::Slow(Rc::new(pure.clone()), PParam::Const(Value::Int(2))),
            None,
            true,
        );
        let simultaneous = Rc::new(Pat::new(
            PatNode::Stack(vec![pure, long].into()),
            None,
            true,
        ));
        let same_part = Rc::new(
            capture_part(
                [(intern_kw("drums"), simultaneous)].into(),
                Ratio64::from_int(4),
            )
            .unwrap(),
        );
        let same = real_origins(
            sliced(selected(same_part, false), false),
            TimeSpan::cycle(0).unwrap(),
        );
        let first = &same[0];
        let second = same
            .iter()
            .find(|row| {
                row.handle.occurrence().onset == first.handle.occurrence().onset
                    && row.source_whole() != first.source_whole()
            })
            .unwrap();
        assert_eq!(first.handle.revision(), second.handle.revision());
        assert_eq!(first.handle.track(), second.handle.track());
        let mut same_swap = (**first).clone();
        same_swap.handle = second.handle.clone();
        assert_eq!(
            super::super::copy_origin(&same_swap, &mut 100000, 256)
                .unwrap_err()
                .code,
            FailCode::Type
        );
        let mut bad = (**origin).clone();
        bad.handle = swapped[0].handle.clone();
        assert_eq!(
            super::super::copy_origin(&bad, &mut 100000, 256)
                .unwrap_err()
                .code,
            FailCode::Type
        );
    }
}

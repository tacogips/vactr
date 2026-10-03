//! Lazy finite realization over whole canonical cycles and touched placements.
use std::collections::{BTreeMap, BTreeSet};
pub(crate) mod issued;
use issued::IssuedQueryRow;

use crate::clock::tempo::Tempo;
use crate::pattern::eval::{InputCells, QState, QueryCtx, QueryVm};
use crate::pattern::pat::Pat;
use crate::pattern::query::{q, sect, TimeSpan};
use crate::pattern::rng::Hasher;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::Sound;
use crate::vm::fail::{FailCode, Failure};

use super::source::{expand_event, original_sound};
use super::{
    EventHandle, InstrumentRoute, Part, PartEdit, PartNode, PartRevision, PlacementPath,
    RepeatSeedMode, SongEvent, SongLimits,
};

/// A caller-certified fixed view. Actual transitive VM freezing is supplied by
/// snapshot preparation, not by cloning a live query adapter.
pub struct SongQueryCtx<'a> {
    pub vm: &'a mut dyn QueryVm,
    pub cells: &'a InputCells,
    pub seed: u64,
    pub tempo: Tempo,
    pub limits: &'a SongLimits,
}

/// Queries only touched complete root cycles, then clips successful events.
/// Every canonical cycle has an independent admission budget; nested selected
/// streams inherit that cycle's existing state. No arrangement is materialized.
/// # Errors
/// Any source fault, arithmetic overflow or construction/admission limit.
pub fn query_part(
    part: &Part,
    span: TimeSpan,
    cx: &mut SongQueryCtx<'_>,
) -> Result<Vec<SongEvent>, Failure> {
    query_part_impl(part, span, cx, None, None, None).map(issued::public)
}

pub(crate) fn query_part_metered(
    part: &Part,
    span: TimeSpan,
    cx: &mut SongQueryCtx<'_>,
    work: crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<Vec<SongEvent>, Failure> {
    let saved_depth = work.borrow().depth;
    let result = query_part_impl(part, span, cx, Some((work.clone(), depth)), None, None)
        .map(issued::public);
    work.borrow_mut().depth = saved_depth;
    result
}

pub(crate) fn query_part_with_replay(
    part: &Part,
    span: TimeSpan,
    cx: &mut SongQueryCtx<'_>,
    work: crate::pattern::eval::song_observation::SharedIndexWork,
    view: std::rc::Rc<crate::pattern::eval::song_replay::ReplayView>,
) -> Result<Vec<SongEvent>, Failure> {
    query_part_impl(part, span, cx, Some((work, 0)), Some(view), None).map(issued::public)
}

fn query_part_impl(
    part: &Part,
    span: TimeSpan,
    cx: &mut SongQueryCtx<'_>,
    meter: Option<(crate::pattern::eval::song_observation::SharedIndexWork, u32)>,
    replay: Option<std::rc::Rc<crate::pattern::eval::song_replay::ReplayView>>,
    issued_tx: Option<crate::pattern::eval::song_provenance::SharedIssuedTranscript>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    if let Some((work, _)) = &meter {
        work.borrow_mut().charge(1)?;
    }
    part.validate_limits(cx.limits)?;
    validate_span(span)?;
    let Some(span) = finite_span(part, span)? else {
        return Ok(Vec::new());
    };
    let seed = Hasher::new(cx.seed).word(part.seed_identity().0).finish();
    let mut rows = BTreeMap::new();
    let mut cycle = span.begin.floor();
    loop {
        if let Some((work, _)) = &meter {
            work.borrow_mut().charge(1)?;
        }
        let window = TimeSpan::cycle(cycle)?;
        if !span.is_point() && window.begin >= span.end {
            break;
        }
        let mut query = QueryCtx {
            vm: cx.vm,
            cells: cx.cells,
            seed,
            tempo: cx.tempo,
        };
        let mut state = match &meter {
            Some((work, depth)) => {
                QState::new_computational(&mut query, *cx.limits, work.clone(), *depth)?
            }
            None => QState::new_song(&mut query, *cx.limits)?,
        };
        if let Some(tx) = &issued_tx {
            state.install_issued_query(tx.clone());
        }
        if let Some(view) = &replay {
            state.install_song_replay(view.clone());
        }
        let events = canonical(part, window, part.tracks(), &mut state)?;
        for mut event in events {
            if let Some((work, _)) = &meter {
                work.borrow_mut().charge(
                    crate::pattern::eval::song_observation::event_copy_work(&event.event)?
                        + event.handle.placement().0.len() as u64
                        + event.handle.occurrence().producer_ordinals.len() as u64
                        + 1,
                )?;
            }
            if let Some(clipped) = sect(event.event.part, span) {
                event.event.part = clipped;
                issued::union(&mut rows, event, meter.as_ref().map(|(work, _)| work))?;
            }
        }
        // A result array is also bounded; this is distinct from cycle admission.
        cx.limits.check_events(rows.len())?;
        if span.is_point() {
            break;
        }
        cycle = cycle.checked_add(1).ok_or_else(overflow)?;
    }
    if let Some((work, _)) = &meter {
        work.borrow_mut().charge(rows.len() as u64 + 1)?;
    }
    Ok(rows.into_values().collect())
}

/// Canonical observation executes only requested root-cycle windows, with the
/// same seed/placement traversal as ordinary query_part and one shared ledger.
pub(crate) fn observe_part(
    part: &Part,
    span: TimeSpan,
    cx: &mut SongQueryCtx<'_>,
    work: crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<(), Failure> {
    part.validate_limits(cx.limits)?;
    validate_span(span)?;
    let Some(span) = finite_span(part, span)? else {
        return Ok(());
    };
    let original = work
        .borrow()
        .original
        .clone()
        .ok_or_else(|| Failure::new(FailCode::Type, "observation original missing"))?;
    let tx = crate::pattern::eval::song_provenance::IssuedQueryTransaction::begin(
        original,
        work.clone(),
        depth,
    )?;
    let seed = Hasher::new(cx.seed).word(part.seed_identity().0).finish();
    let mut cycle = span.begin.floor();
    loop {
        work.borrow_mut().charge(1)?;
        let window = TimeSpan::cycle(cycle)?;
        if !span.is_point() && window.begin >= span.end {
            break;
        }
        let mut query = QueryCtx {
            vm: cx.vm,
            cells: cx.cells,
            seed,
            tempo: cx.tempo,
        };
        let mut state = QState::new_observed(&mut query, *cx.limits, work.clone(), depth)?;
        state.install_issued_query(tx.clone());
        canonical(part, window, part.tracks(), &mut state)?;
        if span.is_point() {
            break;
        }
        cycle = cycle.checked_add(1).ok_or_else(overflow)?;
    }
    crate::pattern::eval::song_provenance::IssuedQueryTransaction::publish(tx)?;
    Ok(())
}

/// Selected sources share the caller's counters, limits and seed scope. This
/// function deliberately does not create a QueryCtx/QState or rehash a root seed.
#[allow(dead_code)] // Descriptor companion retained for crate compatibility; issued traversal is shared.
pub(crate) fn nested_part(
    part: &Part,
    span: TimeSpan,
    track: KwId,
    state: &mut QState<'_, '_>,
) -> Result<Vec<SongEvent>, Failure> {
    nested_part_issued(part, span, track, state).map(issued::public)
}
pub(crate) fn nested_part_issued(
    part: &Part,
    span: TimeSpan,
    track: KwId,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    let limits = state.song_limits().ok_or_else(|| {
        Failure::new(
            FailCode::Type,
            "selected song source requires a canonical song query context",
        )
    })?;
    part.validate_limits(&limits)?;
    validate_span(span)?;
    let Some(span) = finite_span(part, span)? else {
        return Ok(Vec::new());
    };
    let filter = BTreeSet::from([track]);
    let mut rows = BTreeMap::new();
    let mut cycle = span.begin.floor();
    loop {
        let window = TimeSpan::cycle(cycle)?;
        if !span.is_point() && window.begin >= span.end {
            break;
        }
        for mut row in canonical(part, window, &filter, state)? {
            if let Some(clipped) = sect(row.event.part, span) {
                row.event.part = clipped;
                issued::union(&mut rows, row, state.issued_work())?;
            }
        }
        limits.check_events(rows.len())?;
        if span.is_point() {
            break;
        }
        cycle = cycle.checked_add(1).ok_or_else(overflow)?;
    }
    Ok(rows.into_values().collect())
}

fn canonical(
    part: &Part,
    window: TimeSpan,
    filter: &BTreeSet<KwId>,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    let Some(window) = finite_span(part, window)? else {
        return Ok(Vec::new());
    };
    let placement = PlacementPath(Vec::new());
    let mut rows = walk(
        part,
        window,
        Ratio64::ZERO,
        &placement,
        part.revision(),
        filter,
        state,
    )?;
    for row in &mut rows {
        let original = original_sound(row);
        if let Some(route) = policy(
            part,
            row.track,
            &original,
            row.event.anchor(),
            row.event
                .song_source
                .as_ref()
                .map(|origin| origin.handle.revision()),
            state,
        )? {
            row.route = Some(route);
        }
    }
    check_faults(state)?;
    limits(state)?.check_events(rows.len())?;
    Ok(rows)
}

fn walk(
    part: &Part,
    local: TimeSpan,
    offset: Ratio64,
    path: &PlacementPath,
    revision: PartRevision,
    filter: &BTreeSet<KwId>,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    state.enter()?;
    let result = walk_node(part, local, offset, path, revision, filter, state);
    state.leave();
    result
}

fn walk_node(
    part: &Part,
    local: TimeSpan,
    offset: Ratio64,
    path: &PlacementPath,
    revision: PartRevision,
    filter: &BTreeSet<KwId>,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    state.spend(1, None)?;
    let Some(local) = finite_span(part, local)? else {
        return Ok(Vec::new());
    };
    match part.node() {
        PartNode::Capture(tracks) => {
            let mut out = Vec::new();
            for (&track, pattern) in tracks {
                if !filter.contains(&track) {
                    continue;
                }
                out.extend(pattern_rows(
                    pattern,
                    track,
                    part.duration(),
                    local,
                    offset,
                    path,
                    revision,
                    part.revision(),
                    state,
                )?);
                limits(state)?.check_events(out.len())?;
            }
            Ok(out)
        }
        PartNode::Sequence(children) => {
            let mut begin = Ratio64::ZERO;
            let mut out = Vec::new();
            for (index, child) in children.iter().enumerate() {
                state.spend(1, None)?;
                let end = begin.checked_add(child.duration())?;
                if let Some(overlap) =
                    sect(TimeSpan::new(begin, end)?, local).filter(|_| end > begin)
                {
                    let child_span = overlap.map(|t| t.checked_sub(begin))?;
                    let child_path =
                        extended(path, 1, u32::try_from(index).map_err(|_| overflow())?);
                    let child_seed = edge_seed(state.cx.seed, 1, index as u64);
                    out.extend(scoped_walk(
                        child,
                        child_span,
                        offset.checked_add(begin)?,
                        &child_path,
                        revision,
                        child_seed,
                        filter,
                        state,
                    )?);
                }
                begin = end;
                limits(state)?.check_events(out.len())?;
            }
            Ok(out)
        }
        PartNode::Repeat {
            child,
            count,
            seed_mode,
        } => {
            if child.duration() == Ratio64::ZERO {
                return Ok(Vec::new());
            }
            let (first, last) = repeat_range(local, child.duration(), *count)?;
            let mut out = Vec::new();
            for index in first..last {
                let begin = child
                    .duration()
                    .checked_mul(Ratio64::from_int(i64::from(index)))?;
                let end = begin.checked_add(child.duration())?;
                if let Some(overlap) = sect(TimeSpan::new(begin, end)?, local) {
                    let child_span = overlap.map(|t| t.checked_sub(begin))?;
                    let child_path = extended(path, 2, index);
                    let seed_index = if *seed_mode == RepeatSeedMode::Same {
                        0
                    } else {
                        index
                    };
                    let seed = edge_seed(state.cx.seed, 2, u64::from(seed_index));
                    out.extend(scoped_walk(
                        child,
                        child_span,
                        offset.checked_add(begin)?,
                        &child_path,
                        revision,
                        seed,
                        filter,
                        state,
                    )?);
                    limits(state)?.check_events(out.len())?;
                }
            }
            Ok(out)
        }
        PartNode::Edit { source, edit } => edit_rows(
            part, source, edit, local, offset, path, revision, filter, state,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn scoped_walk(
    part: &Part,
    local: TimeSpan,
    offset: Ratio64,
    path: &PlacementPath,
    revision: PartRevision,
    seed: u64,
    filter: &BTreeSet<KwId>,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    let saved = state.cx.seed;
    state.cx.seed = seed;
    let result = walk(part, local, offset, path, revision, filter, state);
    state.cx.seed = saved;
    result
}

#[allow(clippy::too_many_arguments)]
fn pattern_rows(
    pattern: &Pat,
    track: KwId,
    duration: Ratio64,
    local: TimeSpan,
    offset: Ratio64,
    path: &PlacementPath,
    revision: PartRevision,
    owner_revision: PartRevision,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    // Traverse symbolic topology for its exact seed/offset/path, but never
    // execute unrelated payload callbacks during addressed observation.
    if !state.observes_owner(owner_revision, track, pattern.id)? {
        return Ok(Vec::new());
    }
    let mut out = BTreeMap::new();
    let mut cycle = local.begin.floor();
    loop {
        let window = TimeSpan::cycle(cycle)?;
        if !local.is_point() && window.begin >= local.end {
            break;
        }
        let (events, seal) = if state.owner_cycle_enabled() || state.is_observed() {
            state.spend(path.0.len() as u64 + 1, None)?;
            let owner = crate::pattern::eval::song_observation::CanonicalOwnerFrame {
                revision: owner_revision,
                track,
                root: pattern.id,
                placement: path.clone(),
                offset,
                duration,
                window,
            };
            let observed = state.is_observed();
            let saved_owner = if observed {
                state.spend(path.0.len() as u64 + 1, None)?;
                state.observation_owner(Some(owner.clone()))
            } else {
                None
            };
            let result = state.with_clock_owner(&owner, |state| {
                let issued_mark = state.begin_issued_owner(&owner)?;
                let realized = (|| {
                    let invocation = state.begin_owner_invocation(&owner)?;
                    let (events, execution) =
                        if let Some((events, execution)) = state.replay_owner_cycle(&owner)? {
                            (events, Some(execution))
                        } else {
                            let mark = state.execution_mark(&owner)?;
                            let events = q(pattern, window, state);
                            check_faults(state)?;
                            let execution = state.retain_owner_cycle(&owner, &events, mark)?;
                            (events, execution)
                        };
                    let actual = state.finish_owner_invocation(invocation, execution.clone())?;
                    Ok((events, execution, actual))
                })();
                let (events, execution, actual) = match realized {
                    Ok(result) => result,
                    Err(failure) => {
                        state.abort_issued_owner(issued_mark, &failure);
                        return Err(failure);
                    }
                };
                let seal = state.finish_issued_owner(issued_mark, execution, actual)?;
                Ok((events, seal))
            });
            if observed {
                state.observation_owner(saved_owner);
            }
            result?
        } else {
            (q(pattern, window, state), None)
        };
        for mut event in events {
            let onset = event.anchor();
            if onset < Ratio64::ZERO || onset >= duration {
                continue;
            }
            let Some(clipped) = sect(event.part, local) else {
                continue;
            };
            event.part = clipped;
            let event = event.shifted(offset)?;
            for row in expand_event(event, track, path.clone(), revision, state)? {
                let row = issued::expand(row, seal.as_ref(), state)?;
                issued::union(&mut out, row, state.issued_work())?;
            }
        }
        check_faults(state)?;
        limits(state)?.check_events(out.len())?;
        if local.is_point() {
            break;
        }
        cycle = cycle.checked_add(1).ok_or_else(overflow)?;
    }
    Ok(out.into_values().collect())
}

#[allow(clippy::too_many_arguments)]
fn edit_rows(
    part: &Part,
    source: &Part,
    edit: &PartEdit,
    local: TimeSpan,
    offset: Ratio64,
    path: &PlacementPath,
    revision: PartRevision,
    filter: &BTreeSet<KwId>,
    state: &mut QState<'_, '_>,
) -> Result<Vec<IssuedQueryRow>, Failure> {
    // Each replacement has an explicit edit-origin placement; it never depends
    // on an event's rank after filtering or on a compact node fingerprint.
    let edited_track = match edit {
        PartEdit::ReplaceTrack { track, .. }
        | PartEdit::TransformInstrument { track, .. }
        | PartEdit::OverwriteRegion { track, .. }
        | PartEdit::InstrumentFx { track, .. } => *track,
        PartEdit::DeleteEvent(handle) => handle.track(),
    };
    if !filter.contains(&edited_track) {
        return walk(source, local, offset, path, revision, filter, state);
    }
    let payload_root = match edit {
        PartEdit::TransformInstrument { pattern, .. }
        | PartEdit::OverwriteRegion { pattern, .. }
        | PartEdit::ReplaceTrack { pattern, .. } => Some(pattern.id),
        _ => None,
    };
    let mut rows =
        state.with_source_dependencies(part.revision(), edited_track, payload_root, |state| {
            match edit {
                PartEdit::ReplaceTrack { track, .. } => {
                    let mut remaining = filter.clone();
                    remaining.remove(track);
                    walk(source, local, offset, path, revision, &remaining, state)
                }
                _ => walk(source, local, offset, path, revision, filter, state),
            }
        })?;
    match edit {
        PartEdit::ReplaceTrack { track, pattern } => {
            let replacement = extended(path, 3, part.node_count());
            rows.extend(pattern_rows(
                pattern,
                *track,
                part.duration(),
                local,
                offset,
                &replacement,
                revision,
                part.revision(),
                state,
            )?);
        }
        PartEdit::TransformInstrument {
            track,
            selector,
            pattern,
        } => {
            rows.retain(|row| row.track != *track || !selector.contains(&original_sound(row)));
            let transformed = extended(path, 5, part.node_count());
            rows.extend(pattern_rows(
                pattern,
                *track,
                part.duration(),
                local,
                offset,
                &transformed,
                revision,
                part.revision(),
                state,
            )?);
        }
        PartEdit::DeleteEvent(handle) => {
            rows.retain(|row| !matches_handle(row, handle, path, offset))
        }
        PartEdit::OverwriteRegion {
            track,
            region,
            pattern,
        } => {
            let begin = offset.checked_add(region.begin)?;
            let end = offset.checked_add(region.end)?;
            rows.retain(|row| {
                row.track != *track || row.event.anchor() < begin || row.event.anchor() >= end
            });
            if local.end > region.begin || (local.is_point() && local.begin >= region.begin) {
                let overlap = TimeSpan::new(local.begin.max(region.begin), local.end)?;
                let relative = overlap.map(|t| t.checked_sub(region.begin))?;
                let replacement = extended(path, 4, part.node_count());
                rows.extend(pattern_rows(
                    pattern,
                    *track,
                    region.duration()?,
                    relative,
                    begin,
                    &replacement,
                    revision,
                    part.revision(),
                    state,
                )?);
            }
        }
        PartEdit::InstrumentFx { .. } => {}
    }
    check_faults(state)?;
    limits(state)?.check_events(rows.len())?;
    Ok(rows)
}

fn matches_handle(
    row: &SongEvent,
    handle: &EventHandle,
    prefix: &PlacementPath,
    offset: Ratio64,
) -> bool {
    row.track == handle.track()
        && row.placement.0.strip_prefix(prefix.0.as_slice())
            == Some(handle.placement().0.as_slice())
        && row.handle.tone() == handle.tone()
        && row.handle.occurrence().producer_ordinals == handle.occurrence().producer_ordinals
        && row
            .handle
            .occurrence()
            .onset
            .checked_sub(offset)
            .ok()
            .map(|t| t.floor())
            == Some(handle.occurrence().cycle)
        && row.handle.occurrence().onset.checked_sub(offset).ok() == Some(handle.occurrence().onset)
}

/// Resolve immutable declarations at the event's original whole onset, including
/// when replace/overwrite has removed the earlier declaration's musical content.
fn policy(
    part: &Part,
    track: KwId,
    sound: &Sound,
    onset: Ratio64,
    origin_revision: Option<PartRevision>,
    state: &mut QState<'_, '_>,
) -> Result<Option<InstrumentRoute>, Failure> {
    state.enter()?;
    let result = policy_node(part, track, sound, onset, origin_revision, state);
    state.leave();
    result
}
fn policy_node(
    part: &Part,
    track: KwId,
    sound: &Sound,
    onset: Ratio64,
    origin_revision: Option<PartRevision>,
    state: &mut QState<'_, '_>,
) -> Result<Option<InstrumentRoute>, Failure> {
    state.spend(1, None)?;
    if onset < Ratio64::ZERO || onset >= part.duration() || !part.tracks().contains(&track) {
        return Ok(None);
    }
    match part.node() {
        PartNode::Capture(_) => Ok(None),
        PartNode::Sequence(children) => {
            let mut begin = Ratio64::ZERO;
            for child in children {
                state.spend(1, None)?;
                let end = begin.checked_add(child.duration())?;
                if onset >= begin && onset < end {
                    return policy(
                        child,
                        track,
                        sound,
                        onset.checked_sub(begin)?,
                        origin_revision,
                        state,
                    );
                }
                begin = end;
            }
            Ok(None)
        }
        PartNode::Repeat { child, .. } => {
            if child.duration() == Ratio64::ZERO {
                return Ok(None);
            }
            let index = onset.checked_div(child.duration())?.floor();
            let begin = child.duration().checked_mul(Ratio64::from_int(index))?;
            policy(
                child,
                track,
                sound,
                onset.checked_sub(begin)?,
                origin_revision,
                state,
            )
        }
        PartNode::Edit { source, edit } => {
            // A timing transform retains the route certified in its source
            // context. Earlier child policies must not be reapplied using the
            // newly moved onset; later declarations above this cutoff may win.
            if matches!(edit, PartEdit::TransformInstrument { .. })
                && origin_revision == Some(source.revision())
            {
                return Ok(None);
            }
            if let PartEdit::InstrumentFx {
                track: selected,
                selector,
                template,
            } = edit
            {
                if *selected == track && selector.contains(sound) {
                    return Ok(Some(InstrumentRoute {
                        family: selector.clone(),
                        template: *template,
                    }));
                }
            }
            policy(source, track, sound, onset, origin_revision, state)
        }
    }
}
fn repeat_range(span: TimeSpan, duration: Ratio64, count: u32) -> Result<(u32, u32), Failure> {
    let first = span.begin.checked_div(duration)?.floor().max(0);
    let last = if span.is_point() {
        first.checked_add(1).ok_or_else(overflow)?
    } else {
        {
            let ratio = span.end.checked_div(duration)?;
            ratio
                .floor()
                .checked_add(i64::from(!ratio.is_integral()))
                .ok_or_else(overflow)?
        }
    };
    let first = u32::try_from(first.min(i64::from(count))).map_err(|_| overflow())?;
    let last = u32::try_from(last.max(0).min(i64::from(count))).map_err(|_| overflow())?;
    Ok((first, last))
}
fn finite_span(part: &Part, span: TimeSpan) -> Result<Option<TimeSpan>, Failure> {
    Ok(sect(TimeSpan::new(Ratio64::ZERO, part.duration())?, span)
        .filter(|_| part.duration() > Ratio64::ZERO))
}
fn validate_span(span: TimeSpan) -> Result<(), Failure> {
    TimeSpan::new(span.begin, span.end).map(|_| ())
}
fn extended(path: &PlacementPath, tag: u32, ordinal: u32) -> PlacementPath {
    let mut path = path.clone();
    path.0.extend([tag, ordinal]);
    path
}
fn edge_seed(seed: u64, tag: u64, index: u64) -> u64 {
    Hasher::new(seed).word(tag).word(index).finish()
}
fn limits(state: &QState<'_, '_>) -> Result<SongLimits, Failure> {
    state
        .song_limits()
        .ok_or_else(|| Failure::new(FailCode::Type, "missing song admission policy"))
}
fn check_faults(state: &QState<'_, '_>) -> Result<(), Failure> {
    match state.faults.first() {
        Some(fault) => Err(fault.clone()),
        None => Ok(()),
    }
}
fn overflow() -> Failure {
    Failure::new(FailCode::Overflow, "song placement arithmetic overflow")
}

//! Immutable selected source descriptor for symbolic pattern transforms.
use std::rc::Rc;
mod sampling;
pub(crate) mod slices;
pub(crate) use sampling::query_source;

use crate::pattern::eval::QState;
use crate::pattern::query::{Event, TimeSpan};
use crate::pattern::rng::Hasher;
use crate::song::source_uses::{
    use_overflow, FrozenSourceOrigin, FrozenSourceOriginFrame, FrozenSourceUseNode,
    FrozenUseMapping,
};
use crate::song::{EventHandle, InstrumentRoute, InstrumentSelector, NoteCommitMode, Part};
use crate::value::intern::{name_of_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::value::value::Sound;
use crate::vm::fail::{FailCode, Failure};

/// A validated selected track and complete frozen instrument family. The
/// descriptor retains no callback, VM, mutable host adapter or queried PCM.
#[derive(Clone, Debug)]
pub struct SongSource {
    part: Rc<Part>,
    track: KwId,
    selector: InstrumentSelector,
}
impl SongSource {
    /// Selects an existing root track without realizing any events.
    ///
    /// # Errors
    /// `UnknownField` if the immutable part has no such track.
    pub fn new(part: Rc<Part>, track: KwId, selector: InstrumentSelector) -> Result<Self, Failure> {
        if !part.tracks().contains(&track) {
            return Err(Failure::new(
                FailCode::UnknownField,
                "song source track does not exist",
            ));
        }
        Ok(Self {
            part,
            track,
            selector,
        })
    }
    /// Immutable source arrangement.
    #[must_use]
    pub fn part(&self) -> &Rc<Part> {
        &self.part
    }
    /// Selected root track.
    #[must_use]
    pub const fn track(&self) -> KwId {
        self.track
    }
    /// Complete pre-bank-pick source family.
    #[must_use]
    pub fn selector(&self) -> &InstrumentSelector {
        &self.selector
    }

    /// A seed fingerprint, not occurrence identity. Edits retain the source
    /// seed; revisions and allocation addresses never enter random draws.
    pub(crate) fn seed_hash(&self) -> u64 {
        let mut hash = Hasher::new(0x0073_6f6e_6773_7263)
            .word(self.part.seed_identity().0)
            .ratio(self.part.duration())
            .text(&name_of_kw(self.track))
            .word(self.selector.family().len() as u64);
        for sound in self.selector.family() {
            hash = match sound {
                Sound::Builtin(name) => hash.word(1).text(&name_of_kw(*name)),
                Sound::Sample(path) => hash
                    .word(2)
                    .text(&path.text)
                    .word(path.file.map_or(0, |file| u64::from(file.get()) + 1)),
                Sound::Inst(id) => hash.word(3).word(u64::from(id.get())),
                // Buffer identity is opaque to RNG hashing. Do not inspect
                // mutable readiness/PCM or allocation IDs. Full family equality
                // still distinguishes buffers; a fingerprint collision never
                // certifies occurrence or route identity.
                Sound::Buffer(_) => hash.word(4),
                // Validated selectors exclude external outputs. Preserve a
                // total match without making either a permitted source.
                Sound::MidiOut(channel) => hash.word(5).word(u64::from(*channel)),
                Sound::Osc(address) => hash.word(6).text(address),
            };
        }
        hash.finish()
    }
}

fn checked_words(length: usize, multiplier: u64) -> Result<u64, Failure> {
    u64::try_from(length)
        .ok()
        .and_then(|n| n.checked_mul(multiplier))
        .ok_or_else(identity_overflow)
}
fn identity_overflow() -> Failure {
    Failure::new(
        FailCode::Overflow,
        "song producer identity word count overflow",
    )
}
fn charge_identity(state: &mut QState<'_, '_>, words: u64) -> Result<(), Failure> {
    state.spend(words, None).map_err(|_| {
        Failure::new(
            FailCode::FuelExhausted,
            "song producer identity exceeds shared query budget",
        )
    })
}

pub(crate) fn original_sound(row: &crate::song::SongEvent) -> Sound {
    row.event.song_source.as_ref().map_or_else(
        || row.instrument.clone(),
        |origin| origin.original_instrument.clone(),
    )
}

/// Finalize one individually addressable tone before any musical edit/filter.
pub(crate) fn expand_event(
    mut event: Event,
    track: KwId,
    placement: crate::song::PlacementPath,
    revision: crate::song::PartRevision,
    state: &mut QState<'_, '_>,
) -> Result<Vec<crate::song::SongEvent>, Failure> {
    use crate::song::{OccurrencePath, ResolvedNote, SongEvent};
    use crate::value::intern::{intern_kw, name_of_kw};
    use crate::value::value::Value;
    crate::pattern::combinators::sound::pick_bank(&mut event)?;
    let Value::Sound(sound) = &event.value else {
        return Err(Failure::new(
            FailCode::Type,
            "song tracks must realize audio sounds",
        ));
    };
    if matches!(&**sound, Sound::MidiOut(_) | Sound::Osc(_)) {
        return Err(Failure::new(
            FailCode::Type,
            "song tracks cannot emit external output",
        ));
    }
    let instrument = (**sound).clone();
    let note_key = intern_kw("note");
    let index_key = intern_kw("n");
    let key = if event.controls.contains_key(&intern_kw("freq")) {
        None
    } else if event.controls.contains_key(&note_key) {
        Some(note_key)
    } else if event.controls.contains_key(&index_key)
        && !state.cx.vm.song_sample_backed(&instrument)?
    {
        Some(index_key)
    } else {
        None
    };
    let chord = key
        .and_then(|key| event.controls.get(&key))
        .is_some_and(|value| matches!(value, Value::List(_)));
    let notes: Vec<Option<Value>> = match key.and_then(|key| event.controls.get(&key)) {
        Some(Value::List(list)) => {
            state
                .song_limits()
                .ok_or_else(|| Failure::new(FailCode::Type, "missing song admission policy"))?
                .check_events(list.items.len())?;
            list.items.iter().cloned().map(Some).collect()
        }
        Some(value) => vec![Some(value.clone())],
        None => vec![None],
    };
    let mut out = Vec::new();
    for (ordinal, value) in notes.into_iter().enumerate() {
        state.spend(1, None)?;
        let note = value
            .as_ref()
            .map(|value| match value {
                Value::Keyword(name) => {
                    crate::pattern::combinators::music::note_number(&name_of_kw(*name))
                        .map(ResolvedNote::Int)
                        .ok_or_else(|| Failure::new(FailCode::Type, "invalid song note name"))
                }
                value => ResolvedNote::try_from(value),
            })
            .transpose()?;
        let trace_len = event
            .producer
            .as_ref()
            .ok_or_else(|| {
                Failure::new(
                    FailCode::Type,
                    "song event is missing full producer identity",
                )
            })?
            .steps
            .len();
        // Prepay the mono trace, full serialized handle and union-key copies.
        let identity_words = checked_words(trace_len, 8)?
            .checked_add(checked_words(placement.0.len(), 3)?)
            .ok_or_else(identity_overflow)?;
        charge_identity(state, identity_words)?;
        let mut mono = event.clone();
        if let (Some(key), Some(value)) = (key, value) {
            // Exact numeric widths survive realization and source transforms.
            let value = if matches!(value, Value::Keyword(_)) {
                match note {
                    Some(ResolvedNote::Int(v)) => Value::Int64(v),
                    _ => value,
                }
            } else {
                value
            };
            mono.controls.insert(key, value);
            mono.cells.remove(&key);
        }
        let trace = mono.producer.as_ref().ok_or_else(|| {
            Failure::new(
                FailCode::Type,
                "song event is missing full producer identity",
            )
        })?;
        let onset = mono.anchor();
        mono.occ.anchor_at(onset);
        let tone = if !chord {
            mono.song_source
                .as_ref()
                .map_or(0, |origin| origin.handle.tone())
        } else {
            u32::try_from(ordinal)
                .map_err(|_| Failure::new(FailCode::Overflow, "chord tone ordinal overflow"))?
        };
        let handle = EventHandle::issue(
            revision,
            track,
            placement.clone(),
            OccurrencePath {
                producer_ordinals: trace.ordinals(),
                cycle: onset.floor(),
                onset,
            },
            tone,
        );
        let route = mono
            .song_source
            .as_ref()
            .and_then(|origin| origin.route.clone());
        out.push(SongEvent {
            handle,
            track,
            instrument: instrument.clone(),
            event: mono,
            placement: placement.clone(),
            tone: note,
            commit_mode: NoteCommitMode::Mono,
            route,
        });
    }
    Ok(out)
}

/// Certified original mono source identity retained through ordinary transforms.
/// Tone ordinal is read from `handle.tone()`; transformed note controls remain
/// on the current Event. This metadata never becomes a user control.
#[derive(Clone, Debug)]
pub struct SongEventOrigin {
    pub source_part: TimeSpan,
    /// Typed source-entry path before framed source occurrence words.
    pub entry_trace: Vec<crate::pattern::occ::ProducerStep>,
    /// Immutable original inner origin, retained when selecting a selected stream.
    pub inherited: Option<Rc<SongEventOrigin>>,
    pub handle: EventHandle,
    pub original_instrument: Sound,
    pub route: Option<InstrumentRoute>,
    pub commit_mode: NoteCommitMode,
    pub(crate) issued_handle: EventHandle,
    pub(crate) source_whole: Option<TimeSpan>,
    pub(crate) issued_leaves: Option<Rc<crate::pattern::eval::song_provenance::IssuedSourceLeaves>>,
    pub(in crate::song) slice_timings: Vec<slices::SongSliceTiming>,
}

impl SongEventOrigin {
    /// Authentic source-local whole before outer timing transforms.
    #[must_use]
    pub const fn source_whole(&self) -> Option<TimeSpan> {
        self.source_whole
    }
}

#[cfg(test)]
pub(crate) fn provenance_fixture() -> Event {
    use crate::pattern::occ::ProducerTrace;
    use crate::song::{capture_part, OccurrencePath, PlacementPath};
    use crate::value::intern::intern_kw;
    use crate::value::ratio::Ratio64;
    use crate::value::value::Value;
    let part = capture_part(Default::default(), Ratio64::ONE).unwrap();
    let sound = Sound::Builtin(intern_kw("piano"));
    let handle = EventHandle::issue(
        part.revision(),
        intern_kw("keys"),
        PlacementPath(vec![2, 3]),
        OccurrencePath {
            producer_ordinals: vec![5, 0, 2],
            cycle: 1,
            onset: Ratio64::ONE,
        },
        2,
    );
    let origin = Rc::new(SongEventOrigin {
        source_part: TimeSpan::point(Ratio64::ONE),
        entry_trace: Vec::new(),
        inherited: None,
        handle: handle.clone(),
        issued_handle: handle,
        source_whole: Some(TimeSpan::cycle(1).unwrap()),
        issued_leaves: None,
        slice_timings: Vec::new(),
        original_instrument: sound.clone(),
        route: Some(InstrumentRoute {
            family: InstrumentSelector::new(vec![sound.clone()]).unwrap(),
            template: intern_kw("private-plate"),
        }),
        commit_mode: NoteCommitMode::Mono,
    });
    let mut event = Event::new(
        Some(TimeSpan::cycle(1).unwrap()),
        TimeSpan::cycle(1).unwrap(),
        Value::Sound(Rc::new(sound)),
        None,
    );
    event.song_source = Some(origin);
    event.producer = Some(ProducerTrace::default());
    event
        .controls
        .insert(intern_kw("note"), Value::Float64(60.25));
    event
}

/// Iterative copying keeps nested origin chains off the Rust call stack.
pub(crate) fn copy_origin(
    origin: &super::source::SongEventOrigin,
    remaining: &mut u32,
    max_depth: u32,
) -> Result<FrozenSourceOrigin, crate::vm::fail::Failure> {
    use crate::vm::fail::{FailCode, Failure};
    fn admit(
        origin: &super::source::SongEventOrigin,
        remaining: &mut u32,
        max_depth: u32,
    ) -> Result<(), Failure> {
        super::source_uses::origin::validate_source_whole(
            &origin.handle,
            &origin.issued_handle,
            origin.source_part,
            origin.source_whole,
            remaining,
        )?;
        slices::admit_slice_timings(
            &origin.slice_timings,
            &origin.issued_handle,
            remaining,
            max_depth,
        )?;
        let extra = super::source_uses::origin::source_whole_work(
            &origin.issued_handle,
            origin.source_whole,
        )?;
        *remaining = remaining.checked_sub(extra).ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "source authority copy work exhausted",
            )
        })?;
        let sound_cost = match &origin.original_instrument {
            crate::value::value::Sound::Sample(p) => p.text.len().checked_add(1),
            _ => Some(1),
        }
        .ok_or_else(|| Failure::new(FailCode::Overflow, "source sound copy overflow"))?;
        let count = origin
            .entry_trace
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(origin.handle.placement().0.len()))
            .and_then(|n| n.checked_add(origin.handle.occurrence().producer_ordinals.len()))
            .and_then(|n| n.checked_add(sound_cost))
            .and_then(|n| n.checked_add(4))
            .and_then(|n| n.checked_add(1))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| {
                Failure::new(FailCode::Overflow, "source-origin copy length overflow")
            })?;
        *remaining = remaining.checked_sub(count).ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "aggregate source-origin copy budget exhausted",
            )
        })?;
        Ok(())
    }
    if max_depth == 0 {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "source-origin depth exhausted",
        ));
    }
    // Admit the complete chain before copying or reserving its collections.
    let mut current = Some(origin);
    let mut count = 0u32;
    let mut timing_count = 0u32;
    let mut producer_depth = 0u32;
    while let Some(frame) = current {
        count = count
            .checked_add(1)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "source-origin depth overflow"))?;
        if count > max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "source-origin depth exhausted",
            ));
        }
        timing_count = timing_count
            .checked_add(u32::try_from(frame.slice_timings.len()).map_err(|_| use_overflow())?)
            .ok_or_else(use_overflow)?;
        if count.checked_add(timing_count).ok_or_else(use_overflow)? > max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "aggregate Slice origin depth exhausted",
            ));
        }
        admit(frame, remaining, max_depth - count - timing_count + 1)?;
        for timing in &frame.slice_timings {
            producer_depth = producer_depth.max(slices::producer_depth(timing)?);
        }
        current = frame.inherited.as_deref();
    }
    let admission_depth = count.checked_add(timing_count).ok_or_else(use_overflow)?;
    let authority_depth = if timing_count == 0 {
        count
    } else {
        admission_depth
            .checked_add(producer_depth)
            .ok_or_else(use_overflow)?
    };
    if authority_depth > max_depth {
        return Err(Failure::new(
            FailCode::DepthExceeded,
            "aggregate Slice producer depth exhausted",
        ));
    }
    let mut inherited = Vec::with_capacity((count - 1) as usize);
    current = origin.inherited.as_deref();
    while let Some(frame) = current {
        inherited.push(FrozenSourceOriginFrame {
            source_part: frame.source_part,
            original_instrument: super::snapshot::FrozenSound::from_sound(
                &frame.original_instrument,
            )?,
            handle: frame.handle.clone(),
            issued_handle: frame.issued_handle.clone(),
            source_whole: frame.source_whole,
            slice_timings: slices::copy_admitted_slice_timings(
                &frame.slice_timings,
                authority_depth,
            ),
            entry_trace: frame.entry_trace.clone(),
        });
        current = frame.inherited.as_deref();
    }
    Ok(FrozenSourceOrigin {
        source_part: origin.source_part,
        original_instrument: super::snapshot::FrozenSound::from_sound(&origin.original_instrument)?,
        handle: origin.handle.clone(),
        issued_handle: origin.issued_handle.clone(),
        source_whole: origin.source_whole,
        slice_timings: slices::copy_admitted_slice_timings(&origin.slice_timings, authority_depth),
        entry_trace: origin.entry_trace.clone(),
        inherited,
    })
}

fn last_cycle(window: TimeSpan) -> Result<i64, crate::vm::fail::Failure> {
    window
        .end
        .floor()
        .checked_sub(i64::from(
            !window.is_point() && window.end.frac() == Ratio64::ZERO,
        ))
        .ok_or_else(use_overflow)
}
fn cycle_envelope(window: TimeSpan) -> Result<TimeSpan, crate::vm::fail::Failure> {
    TimeSpan::new(
        Ratio64::from_int(window.begin.floor()),
        Ratio64::from_int(
            last_cycle(window)?
                .checked_add(1)
                .ok_or_else(use_overflow)?,
        ),
    )
}
pub(crate) fn support_intersects(a: TimeSpan, b: TimeSpan) -> bool {
    if a.is_point() {
        return if b.is_point() {
            a.begin == b.begin
        } else {
            b.begin <= a.begin && a.begin < b.end
        };
    }
    if b.is_point() {
        return a.begin <= b.begin && b.begin < a.end;
    }
    a.begin.max(b.begin) < a.end.min(b.end)
}
/// Same operation and exact slot geometry for admission and realized resolution.
pub(crate) fn mapped_edge_support(
    node: &FrozenSourceUseNode,
    edge: &crate::song::source_uses::FrozenSourceUseEdge,
    window: TimeSpan,
    actual: Option<&[crate::pattern::occ::ProducerStep]>,
    remaining: &mut u32,
) -> Result<Option<TimeSpan>, Failure> {
    let mapped = if let FrozenUseMapping::SampleGrid { sampling, .. } = node.mapping {
        let Some(mapped) =
            crate::song::source_uses::sampling::support_envelope(sampling, window, remaining)?
        else {
            return Ok(None);
        };
        mapped
    } else {
        mapped_support(node, window)?
    };
    crate::song::source_uses::layout::map_slot_support(edge, mapped, actual)
}
pub(crate) fn mapped_support(
    node: &FrozenSourceUseNode,
    window: TimeSpan,
) -> Result<TimeSpan, crate::vm::fail::Failure> {
    match &node.mapping {
        FrozenUseMapping::SampleGrid { .. } => Err(Failure::new(
            crate::vm::fail::FailCode::BeyondCapability,
            "sample grid requires bounded support mapping",
        )),
        FrozenUseMapping::Rate { factor } if *factor > Ratio64::ZERO => {
            window.map(|t| t.checked_mul(*factor))
        }
        FrozenUseMapping::Shift { amount } => window.map(|t| t.checked_sub(*amount)),
        FrozenUseMapping::ReflectCycles if window.is_point() => {
            TimeSpan::cycle(window.begin.floor())
        }
        FrozenUseMapping::ReflectCycles if last_cycle(window)? == window.begin.floor() => {
            let boundary = Ratio64::from_int(window.begin.floor())
                .checked_mul(Ratio64::from_int(2))?
                .checked_add(Ratio64::ONE)?;
            TimeSpan::new(
                boundary.checked_sub(window.end)?,
                boundary.checked_sub(window.begin)?,
            )
        }
        FrozenUseMapping::Iterate { count } if *count > 0 => {
            let shift =
                |cycle: i64| Ratio64::new(cycle.rem_euclid(i64::from(*count)), i64::from(*count));
            if last_cycle(window)? == window.begin.floor() {
                window.map(|t| t.checked_add(shift(window.begin.floor())?))
            } else {
                TimeSpan::new(
                    window.begin,
                    window
                        .end
                        .checked_add(Ratio64::new(i64::from(*count) - 1, i64::from(*count))?)?,
                )
            }
        }
        FrozenUseMapping::ReflectCycles
        | FrozenUseMapping::SampleCycles { .. }
        | FrozenUseMapping::CycleConcat => cycle_envelope(window),
        FrozenUseMapping::CycleSelect if !node.edges.is_empty() => {
            let count = i64::try_from(node.edges.len()).map_err(|_| use_overflow())?;
            TimeSpan::new(
                Ratio64::from_int(window.begin.floor().div_euclid(count)),
                Ratio64::from_int(
                    last_cycle(window)?
                        .div_euclid(count)
                        .checked_add(1)
                        .ok_or_else(use_overflow)?,
                ),
            )
        }
        _ => Ok(window),
    }
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    #[test]
    fn cloning_and_payload_changes_retain_certified_origin_and_mono_marker() {
        let event = provenance_fixture();
        let origin = event.song_source.as_ref().unwrap();
        let mut changed = event.clone();
        changed.value = crate::value::value::Value::Int(999);
        changed.controls.clear();
        assert!(Rc::ptr_eq(origin, changed.song_source.as_ref().unwrap()));
        assert_eq!(origin.handle.tone(), 2);
        assert_eq!(origin.handle.placement().0, vec![2, 3]);
        assert_eq!(origin.handle.occurrence().producer_ordinals, vec![5, 0, 2]);
        assert_eq!(origin.commit_mode, NoteCommitMode::Mono);
        assert_eq!(
            origin.original_instrument,
            Sound::Builtin(crate::value::intern::intern_kw("piano"))
        );
        assert_eq!(
            origin.route.as_ref().unwrap().template,
            crate::value::intern::intern_kw("private-plate")
        );
    }
    #[test]
    fn copied_original_member_survives_changed_sound_and_multi_member_family() {
        use crate::value::intern::intern_kw;
        let mut event = provenance_fixture();
        let origin = Rc::make_mut(event.song_source.as_mut().unwrap());
        origin.route.as_mut().unwrap().family = InstrumentSelector::new(vec![
            Sound::Builtin(intern_kw("piano")),
            Sound::Builtin(intern_kw("analog")),
        ])
        .unwrap();
        event.value = crate::value::value::Value::Sound(Rc::new(Sound::Builtin(intern_kw("fm"))));
        let origin = event.song_source.as_ref().unwrap();
        let mut work = 1000;
        let copied = copy_origin(origin, &mut work, 256).unwrap();
        assert_eq!(
            copied.original_instrument,
            crate::song::snapshot::FrozenSound::Builtin(intern_kw("piano"))
        );
        assert_eq!(copied.handle, origin.handle);
        assert!(work < 1000);
        let mut tiny = 1;
        assert_eq!(
            copy_origin(origin, &mut tiny, 256).unwrap_err().code,
            FailCode::FuelExhausted
        );
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;
    use crate::pattern::eval::{InputCells, QueryVm};
    use crate::pattern::pat::{PParam, Pat, PatNode};
    use crate::song::{capture_part, query_part, SongLimits, SongQueryCtx};
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
    fn origins() -> Vec<Rc<SongEventOrigin>> {
        let track = intern_kw("drums");
        let sound = Sound::Builtin(intern_kw("analog"));
        let pure = crate::pattern::build::pure(Value::Sound(Rc::new(sound.clone())), None);
        let slow = Pat::new(
            PatNode::Slow(Rc::new(pure.clone()), PParam::Const(Value::Int(2))),
            None,
            true,
        );
        let stack = Rc::new(Pat::new(
            PatNode::Stack(vec![pure, slow].into()),
            None,
            true,
        ));
        let mut part = Rc::new(capture_part([(track, stack)].into(), Ratio64::ONE).unwrap());
        for _ in 0..2 {
            let selected = Rc::new(
                SongSource::new(
                    part,
                    track,
                    InstrumentSelector::new(vec![sound.clone()]).unwrap(),
                )
                .unwrap(),
            );
            part = Rc::new(
                capture_part(
                    [(
                        track,
                        Rc::new(Pat::new(PatNode::SongSource(selected), None, true)),
                    )]
                    .into(),
                    Ratio64::ONE,
                )
                .unwrap(),
            );
        }
        let mut vm = NoCalls;
        let cells = InputCells::new();
        let limits = SongLimits::default();
        query_part(
            &part,
            TimeSpan::cycle(0).unwrap(),
            &mut SongQueryCtx {
                vm: &mut vm,
                cells: &cells,
                seed: 1,
                tempo: crate::clock::tempo::Tempo::default(),
                limits: &limits,
            },
        )
        .unwrap()
        .into_iter()
        .map(|row| row.event.song_source.unwrap())
        .collect()
    }
    #[test]
    fn live_outer_and_inner_handle_swaps_fail_before_whole_copy() {
        let rows = origins();
        let first = &rows[0];
        let second = rows
            .iter()
            .find(|row| row.source_whole() != first.source_whole())
            .unwrap();
        assert_eq!(
            first.handle.occurrence().onset,
            second.handle.occurrence().onset
        );
        let mut outer = (**first).clone();
        outer.handle = second.handle.clone();
        assert_eq!(
            copy_origin(&outer, &mut 10000, 256).unwrap_err().code,
            FailCode::Type
        );
        let mut inner = (**first).clone();
        Rc::make_mut(inner.inherited.as_mut().unwrap()).handle =
            second.inherited.as_ref().unwrap().handle.clone();
        assert_eq!(
            copy_origin(&inner, &mut 10000, 256).unwrap_err().code,
            FailCode::Type
        );
    }
    #[test]
    fn real_chain_copy_prices_private_handles_and_validation_exactly() {
        let rows = origins();
        let origin = &rows[0];
        let mut remaining = 10000;
        let full = copy_origin(origin, &mut remaining, 256).unwrap();
        let cost = 10000 - remaining;
        assert!(cost > 100);
        let mut exact = cost;
        assert_eq!(copy_origin(origin, &mut exact, 256).unwrap(), full);
        assert_eq!(exact, 0);
        assert_eq!(
            copy_origin(origin, &mut (cost - 1), 256).unwrap_err().code,
            FailCode::FuelExhausted
        );
        assert_eq!(
            copy_origin(origin, &mut 10000, 1).unwrap_err().code,
            FailCode::DepthExceeded
        );
        let mut two = cost * 2;
        copy_origin(origin, &mut two, 256).unwrap();
        copy_origin(origin, &mut two, 256).unwrap();
        assert_eq!(two, 0);
        let handle = &origin.handle;
        assert_eq!(
            super::super::source_uses::origin::source_whole_work(handle, origin.source_whole())
                .unwrap(),
            u32::try_from(
                13 + handle.placement().0.len() + handle.occurrence().producer_ordinals.len()
            )
            .unwrap()
        );
    }
    #[test]
    fn absent_whole_is_faithful_and_external_authority_is_private() {
        // Explicit internal None validation case; the opaque handle is issued by
        // the real query above. No duration is inferred when None is retained.
        let rows = origins();
        let mut origin = (*rows[0]).clone();
        origin.source_whole = None;
        let copied = copy_origin(&origin, &mut 10000, 256).unwrap();
        assert_eq!(copied.source_whole(), None);
        assert_eq!(copied.handle, origin.handle);
        let whole = rows[0].source_whole().unwrap();
        let mut invalid_end = (*rows[0]).clone();
        invalid_end.source_part = TimeSpan::point(whole.end);
        assert_eq!(
            copy_origin(&invalid_end, &mut 10000, 256).unwrap_err().code,
            FailCode::Type
        );
        let mut zero = (*rows[0]).clone();
        zero.source_whole = Some(TimeSpan::point(zero.handle.occurrence().onset));
        zero.source_part = TimeSpan::point(zero.handle.occurrence().onset);
        assert_eq!(
            copy_origin(&zero, &mut 10000, 256).unwrap().source_whole(),
            zero.source_whole()
        );
    }
}

/// Charge each owned source field before the private replay mapper clones it.
pub(crate) fn copy_issued_origin_fields(
    origin: &SongEventOrigin,
    work: &mut crate::pattern::eval::song_observation::CanonicalIndexCollector,
    depth: u32,
) -> Result<SongEventOrigin, Failure> {
    if depth >= work.limits.max_depth {
        return Err(Failure::new(
            crate::vm::fail::FailCode::DepthExceeded,
            "issued origin inherited depth",
        ));
    }
    let before = work.remaining();
    let mut remaining = before;
    slices::admit_slice_timings(
        &origin.slice_timings,
        &origin.issued_handle,
        &mut remaining,
        work.limits.max_depth - depth,
    )?;
    work.charge(u64::from(before - remaining))?;
    let mut cost = origin.entry_trace.len() as u64 * 2
        + origin.handle.placement().0.len() as u64
        + origin.handle.occurrence().producer_ordinals.len() as u64
        + origin.issued_handle.placement().0.len() as u64
        + origin.issued_handle.occurrence().producer_ordinals.len() as u64
        + 3;
    if let Sound::Sample(path) = &origin.original_instrument {
        cost = cost
            .checked_add(path.text.len() as u64)
            .ok_or_else(identity_overflow)?;
    }
    if let Some(route) = &origin.route {
        cost = cost
            .checked_add(route.family.family().len() as u64)
            .ok_or_else(identity_overflow)?;
        for sound in route.family.family() {
            if let Sound::Sample(path) = sound {
                cost = cost
                    .checked_add(path.text.len() as u64)
                    .ok_or_else(identity_overflow)?;
            }
        }
    }
    work.charge(cost)?;
    let mut copied = origin.clone();
    copied.inherited = None;
    copied.issued_leaves = None;
    Ok(copied)
}

/// Freeze only a real live contribution after authentic query-row validation.
pub(crate) fn freeze_issued_source(
    contribution: &crate::song::query::issued::IssuedSourceContribution,
    transcript: &crate::pattern::eval::song_provenance::IssuedQueryTranscript,
    work: &crate::pattern::eval::song_observation::SharedIndexWork,
    depth: u32,
) -> Result<super::source_uses::origin::FrozenIssuedSourceContribution, crate::vm::fail::Failure> {
    use super::source_uses::origin::{FrozenIssuedMemberCopy, FrozenIssuedSourceContribution};
    use crate::vm::fail::{FailCode, Failure};
    let invalid = || Failure::new(FailCode::Type, "foreign augmented source contribution");
    work.borrow_mut().charge(1)?;
    if !contribution
        .augmented_origin
        .issued_leaves
        .as_ref()
        .is_some_and(|leaves| std::rc::Rc::ptr_eq(leaves, &contribution.leaves))
    {
        return Err(invalid());
    }
    let actual = transcript.source_members(&contribution.leaves, work, depth)?;
    if actual.is_empty() {
        return Err(invalid());
    }
    work.borrow_mut().charge(actual.len() as u64 + 2)?;
    let augmented =
        super::snapshot::issued::with_copy_budget(work, depth, |remaining, max_depth| {
            copy_origin(&contribution.augmented_origin, remaining, max_depth)
        })?;
    let mut members = Vec::with_capacity(actual.len());
    for (slot, member) in actual.iter().enumerate() {
        if !member.is_sealed_member(work, depth)? {
            return Err(invalid());
        }
        work.borrow_mut().charge(1)?;
        if !member
            .member()
            .issued_leaves
            .as_ref()
            .is_some_and(|leaves| std::rc::Rc::ptr_eq(leaves, &contribution.leaves))
        {
            return Err(invalid());
        }
        let raw =
            super::snapshot::issued::with_copy_budget(work, depth, |remaining, max_depth| {
                copy_origin(member.member(), remaining, max_depth)
            })?;
        // Meter the nonallocating exact field/chain comparison before equality.
        super::snapshot::issued::with_copy_budget(work, depth, |remaining, _| {
            augmented.admit_raw_comparison(&raw, remaining)
        })?;
        let mut augmented_frame = Some(&*contribution.augmented_origin);
        let mut raw_frame = Some(&**member.member());
        let mut inherited_depth = depth;
        while let (Some(a), Some(b)) = (augmented_frame, raw_frame) {
            work.borrow_mut().charge(1)?;
            if inherited_depth >= work.borrow().limits.max_depth {
                return Err(Failure::new(
                    FailCode::DepthExceeded,
                    "issued inherited member comparison",
                ));
            }
            for origin in [a, b] {
                if let Some(route) = &origin.route {
                    work.borrow_mut()
                        .charge(route.family.family().len() as u64 + 2)?;
                    for sound in route.family.family() {
                        if let crate::value::value::Sound::Sample(path) = sound {
                            work.borrow_mut().charge(path.text.len() as u64 + 1)?;
                        }
                    }
                } else {
                    work.borrow_mut().charge(1)?;
                }
            }
            if a.route != b.route || a.commit_mode != b.commit_mode {
                return Err(Failure::new(
                    FailCode::Type,
                    "augmented inherited member route/commit mismatch",
                ));
            }
            if !matches!((&a.issued_leaves, &b.issued_leaves), (Some(a), Some(b)) if std::rc::Rc::ptr_eq(a,b))
            {
                return Err(Failure::new(
                    FailCode::Type,
                    "augmented inherited member authentic bag mismatch",
                ));
            }
            augmented_frame = a.inherited.as_deref();
            raw_frame = b.inherited.as_deref();
            inherited_depth += 1;
        }
        if augmented_frame.is_some() || raw_frame.is_some() {
            return Err(Failure::new(
                FailCode::Type,
                "augmented inherited member chain length mismatch",
            ));
        }
        if !augmented.preserves_raw_member(&raw) {
            return Err(Failure::new(
                FailCode::Type,
                "augmented member original fields or timing prefix mismatch",
            ));
        }
        members.push(FrozenIssuedMemberCopy::copied(slot, raw));
    }
    Ok(FrozenIssuedSourceContribution::copied(
        contribution,
        augmented,
        members,
    ))
}

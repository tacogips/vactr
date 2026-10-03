//! Checked immutable symbolic Part construction; no eager repeat expansion.
use super::identity::{EventHandle, InstrumentSelector, PartRevision, SeedIdentity};
use super::limits::SongLimits;
use crate::pattern::pat::Pat;
use crate::pattern::query::TimeSpan;
use crate::pattern::rng::Hasher;
use crate::value::intern::{name_of_kw, KwId};
use crate::value::ratio::Ratio64;
use crate::vm::fail::{FailCode, Failure};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Repeated source randomness: identical local seeds, or placement-derived.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatSeedMode {
    #[default]
    Same,
    Vary,
}
/// Immutable edit DATA. Execution/querying is owned by SONG-04. A transform
/// stores its prepared pattern; never an unexecuted user callback.
#[derive(Clone, Debug)]
pub enum PartEdit {
    ReplaceTrack {
        track: KwId,
        pattern: Rc<Pat>,
    },
    TransformInstrument {
        track: KwId,
        selector: InstrumentSelector,
        pattern: Rc<Pat>,
    },
    DeleteEvent(EventHandle),
    OverwriteRegion {
        track: KwId,
        region: TimeSpan,
        pattern: Rc<Pat>,
    },
    InstrumentFx {
        track: KwId,
        selector: InstrumentSelector,
        template: KwId,
    },
}
/// Finite symbolic arrangement nodes. Children are shared immutable values.
#[derive(Clone, Debug)]
pub enum PartNode {
    Capture(BTreeMap<KwId, Rc<Pat>>),
    Sequence(Vec<Rc<Part>>),
    Repeat {
        child: Rc<Part>,
        count: u32,
        seed_mode: RepeatSeedMode,
    },
    Edit {
        source: Rc<Part>,
        edit: PartEdit,
    },
}
/// A validated finite arrangement. Private fields prevent forging durations
/// or attaching an edit without validating the source revision and tracks.
#[derive(Clone, Debug)]
pub struct Part {
    revision: PartRevision,
    duration: Ratio64,
    node: PartNode,
    seed_identity: SeedIdentity,
    tracks: BTreeSet<KwId>,
    nodes: u32,
    depth: u32,
}
/// Candidate-only payload mapping; symbolic structure and certified identity
/// are retained. Implementations cannot replace durations, revisions or edits.
pub(crate) trait PartPayloadMapper {
    fn pattern(&mut self, pattern: &Rc<Pat>) -> Result<Rc<Pat>, Failure>;
    fn child(&mut self, part: &Rc<Part>) -> Result<Rc<Part>, Failure>;
    fn selector(&mut self, selector: &InstrumentSelector) -> Result<InstrumentSelector, Failure>;
}
fn frozen_child(
    mapper: &mut impl PartPayloadMapper,
    original: &Rc<Part>,
) -> Result<Rc<Part>, Failure> {
    let mapped = mapper.child(original)?;
    if mapped.revision != original.revision
        || mapped.duration != original.duration
        || mapped.seed_identity != original.seed_identity
        || mapped.tracks != original.tracks
        || mapped.nodes != original.nodes
        || mapped.depth != original.depth
    {
        return Err(Failure::new(
            FailCode::Type,
            "candidate payload mapping changed finite child identity/shape",
        ));
    }
    Ok(mapped)
}
impl Part {
    pub(crate) fn freeze_payloads(
        &self,
        mapper: &mut impl PartPayloadMapper,
    ) -> Result<Self, Failure> {
        let node = match &self.node {
            PartNode::Capture(tracks) => PartNode::Capture(
                tracks
                    .iter()
                    .map(|(k, p)| Ok((*k, mapper.pattern(p)?)))
                    .collect::<Result<_, Failure>>()?,
            ),
            PartNode::Sequence(parts) => PartNode::Sequence(
                parts
                    .iter()
                    .map(|p| frozen_child(mapper, p))
                    .collect::<Result<_, _>>()?,
            ),
            PartNode::Repeat {
                child,
                count,
                seed_mode,
            } => PartNode::Repeat {
                child: frozen_child(mapper, child)?,
                count: *count,
                seed_mode: *seed_mode,
            },
            PartNode::Edit { source, edit } => {
                let edit = match edit {
                    PartEdit::ReplaceTrack { track, pattern } => PartEdit::ReplaceTrack {
                        track: *track,
                        pattern: mapper.pattern(pattern)?,
                    },
                    PartEdit::TransformInstrument {
                        track,
                        selector,
                        pattern,
                    } => PartEdit::TransformInstrument {
                        track: *track,
                        selector: mapper.selector(selector)?,
                        pattern: mapper.pattern(pattern)?,
                    },
                    PartEdit::OverwriteRegion {
                        track,
                        region,
                        pattern,
                    } => PartEdit::OverwriteRegion {
                        track: *track,
                        region: *region,
                        pattern: mapper.pattern(pattern)?,
                    },
                    PartEdit::InstrumentFx {
                        track,
                        selector,
                        template,
                    } => PartEdit::InstrumentFx {
                        track: *track,
                        selector: mapper.selector(selector)?,
                        template: *template,
                    },
                    PartEdit::DeleteEvent(handle) => PartEdit::DeleteEvent(handle.clone()),
                };
                PartNode::Edit {
                    source: frozen_child(mapper, source)?,
                    edit,
                }
            }
        };
        Ok(Self {
            node,
            revision: self.revision,
            duration: self.duration,
            seed_identity: self.seed_identity,
            tracks: self.tracks.clone(),
            nodes: self.nodes,
            depth: self.depth,
        })
    }
    /// Positive-duration capture. Empty tracks represent explicit timed silence.
    /// # Errors
    /// Nonpositive duration or exceeded construction/track capacities.
    pub fn capture(
        tracks: BTreeMap<KwId, Rc<Pat>>,
        duration: Ratio64,
        limits: &SongLimits,
    ) -> Result<Self, Failure> {
        if duration <= Ratio64::ZERO {
            return Err(Failure::new(
                FailCode::Type,
                "captured part duration must be positive",
            ));
        }
        let keys = tracks.keys().copied().collect();
        let mut named: Vec<_> = tracks.iter().map(|(k, p)| (name_of_kw(*k), p.id)).collect();
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let mut hash = Hasher::new(0x7061_7274).ratio(duration);
        for (name, id) in named {
            hash = hash.text(&name).word(u64::from(id.get()));
        }
        Self::build(
            duration,
            PartNode::Capture(tracks),
            SeedIdentity(hash.finish()),
            keys,
            1,
            1,
            limits,
        )
    }
    /// Capture an iterator without losing duplicate track keys to map insertion.
    pub fn capture_entries(
        tracks: impl IntoIterator<Item = (KwId, Rc<Pat>)>,
        duration: Ratio64,
        limits: &SongLimits,
    ) -> Result<Self, Failure> {
        let mut map = BTreeMap::new();
        for (key, pattern) in tracks {
            if map.insert(key, pattern).is_some() {
                return Err(Failure::new(FailCode::Type, "duplicate part track key"));
            }
            limits.structure(1, 1, map.len())?;
        }
        Self::capture(map, duration, limits)
    }
    /// Sequences a finite list with checked exact addition. Empty is empty music.
    pub fn sequence(parts: Vec<Rc<Self>>, limits: &SongLimits) -> Result<Self, Failure> {
        let mut duration = Ratio64::ZERO;
        let mut nodes = 1u32;
        let mut depth = 1u32;
        let mut tracks = BTreeSet::new();
        let mut hash = Hasher::new(0x7365_7175);
        for part in &parts {
            duration = duration.checked_add(part.duration)?;
            nodes = nodes
                .checked_add(part.nodes)
                .ok_or_else(structure_overflow)?;
            depth = depth.max(part.depth.checked_add(1).ok_or_else(structure_overflow)?);
            tracks.extend(&part.tracks);
            hash = hash.word(part.seed_identity.0);
            limits.structure(nodes, depth, tracks.len())?;
        }
        Self::build(
            duration,
            PartNode::Sequence(parts),
            SeedIdentity(hash.finish()),
            tracks,
            nodes,
            depth,
            limits,
        )
    }
    /// Symbolic finite repeat. Zero discards the child and returns empty music.
    pub fn repeat(
        self: Rc<Self>,
        count: u32,
        mode: RepeatSeedMode,
        limits: &SongLimits,
    ) -> Result<Self, Failure> {
        if count == 0 {
            return Self::sequence(Vec::new(), limits);
        }
        let duration = self
            .duration
            .checked_mul(Ratio64::from_int(i64::from(count)))?;
        let nodes = self.nodes.checked_add(1).ok_or_else(structure_overflow)?;
        let depth = self.depth.checked_add(1).ok_or_else(structure_overflow)?;
        let tracks = self.tracks.clone();
        let seed_identity = self.seed_identity;
        Self::build(
            duration,
            PartNode::Repeat {
                child: self,
                count,
                seed_mode: mode,
            },
            seed_identity,
            tracks,
            nodes,
            depth,
            limits,
        )
    }
    /// Attaches validated immutable edit data with default construction limits.
    pub fn edit(self: Rc<Self>, edit: PartEdit) -> Result<Self, Failure> {
        self.edit_with_limits(edit, &SongLimits::default())
    }
    /// Attaches edit data without bypassing certified revision, track or bounds.
    pub fn edit_with_limits(
        self: Rc<Self>,
        edit: PartEdit,
        limits: &SongLimits,
    ) -> Result<Self, Failure> {
        let track = match &edit {
            PartEdit::ReplaceTrack { track, .. }
            | PartEdit::TransformInstrument { track, .. }
            | PartEdit::OverwriteRegion { track, .. }
            | PartEdit::InstrumentFx { track, .. } => *track,
            PartEdit::DeleteEvent(handle) => {
                if handle.revision() != self.revision {
                    return Err(Failure::new(
                        FailCode::Type,
                        "stale part event handle revision",
                    ));
                }
                handle.track()
            }
        };
        if !self.tracks.contains(&track) {
            return Err(Failure::new(FailCode::UnknownField, "unknown part track"));
        }
        if let PartEdit::OverwriteRegion { region, .. } = &edit {
            if region.begin < Ratio64::ZERO
                || region.end <= region.begin
                || region.end > self.duration
            {
                return Err(Failure::new(
                    FailCode::Type,
                    "overwrite region is outside the part duration",
                ));
            }
        }
        let nodes = self.nodes.checked_add(1).ok_or_else(structure_overflow)?;
        let depth = self.depth.checked_add(1).ok_or_else(structure_overflow)?;
        let duration = self.duration;
        let seed_identity = self.seed_identity;
        let tracks = self.tracks.clone();
        Self::build(
            duration,
            PartNode::Edit { source: self, edit },
            seed_identity,
            tracks,
            nodes,
            depth,
            limits,
        )
    }
    fn build(
        duration: Ratio64,
        node: PartNode,
        seed_identity: SeedIdentity,
        tracks: BTreeSet<KwId>,
        nodes: u32,
        depth: u32,
        limits: &SongLimits,
    ) -> Result<Self, Failure> {
        limits.structure(nodes, depth, tracks.len())?;
        Ok(Self {
            revision: PartRevision::fresh()?,
            duration,
            node,
            seed_identity,
            tracks,
            nodes,
            depth,
        })
    }
    /// Root immutable edit revision.
    #[must_use]
    pub const fn revision(&self) -> PartRevision {
        self.revision
    }
    /// Exact finite cycles, including explicitly timed silence.
    #[must_use]
    pub const fn duration(&self) -> Ratio64 {
        self.duration
    }
    /// Symbolic node without exposing mutation.
    #[must_use]
    pub fn node(&self) -> &PartNode {
        &self.node
    }
    /// Source random identity, retained by edit copies.
    #[must_use]
    pub const fn seed_identity(&self) -> SeedIdentity {
        self.seed_identity
    }
    /// Union of all declared track keys, with no implicit new edit tracks.
    #[must_use]
    pub fn tracks(&self) -> &BTreeSet<KwId> {
        &self.tracks
    }
    /// Symbolic node count (a repetition does not expand its child).
    #[must_use]
    pub const fn node_count(&self) -> u32 {
        self.nodes
    }
    /// Maximum symbolic construction depth.
    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.depth
    }
    /// Revalidates against another host's construction policy.
    pub fn validate_limits(&self, limits: &SongLimits) -> Result<(), Failure> {
        limits.structure(self.nodes, self.depth, self.tracks.len())
    }
}
fn structure_overflow() -> Failure {
    Failure::new(FailCode::Overflow, "song symbolic structure count overflow")
}
/// Validates the language's rational count without truncation or signed wraps.
pub fn checked_repeat_count(count: Ratio64) -> Result<u32, Failure> {
    if !count.is_integral() || count < Ratio64::ZERO {
        return Err(Failure::new(
            FailCode::Type,
            "part repeat count must be a nonnegative integer",
        ));
    }
    u32::try_from(count.num())
        .map_err(|_| Failure::new(FailCode::Overflow, "part repeat count exceeds u32"))
}
/// Captures symbolic patterns with the default bounded construction policy.
pub fn capture_part(tracks: BTreeMap<KwId, Rc<Pat>>, duration: Ratio64) -> Result<Part, Failure> {
    Part::capture(tracks, duration, &SongLimits::default())
}
/// Sequences a finite list with the default policy.
pub fn sequence(parts: Vec<Rc<Part>>) -> Result<Part, Failure> {
    Part::sequence(parts, &SongLimits::default())
}
/// Repeats a child symbolically, retaining its local origin and seed policy.
pub fn part_repeat(part: Rc<Part>, count: u32, mode: RepeatSeedMode) -> Result<Part, Failure> {
    part.repeat(count, mode, &SongLimits::default())
}

#[cfg(test)]
mod freeze_tests {
    use super::*;
    use crate::pattern::pat::PatNode;
    use crate::pattern::step::Step;
    use crate::session::song::freeze::Freeze;
    use crate::song::assets::{
        DecodedSongAssetFactory, SongAssetFactory, SongAssetLimits, SongSourceFile,
    };
    use crate::value::sample::SampleBuf;
    use crate::value::value::Sound;
    use crate::value::Value;
    fn limits() -> SongAssetLimits {
        SongAssetLimits {
            max_resources: 8,
            max_pcm_bytes: 1024,
            max_source_files: 8,
            max_source_bytes: 1024,
            max_banks: 4,
            max_walk_nodes: 100_000,
            max_walk_depth: 256,
        }
    }
    fn preparation() -> crate::song::assets::SongAssetPreparation {
        DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
            .begin(
                SongSourceFile {
                    file: crate::reader::span::FileId::new(0),
                    path: crate::value::value::PathVal {
                        text: Rc::from("score.vact"),
                        file: None,
                    },
                },
                limits(),
            )
            .unwrap()
    }
    #[test]
    fn frozen_children_cannot_change_certified_structure() {
        struct Invalid;
        impl PartPayloadMapper for Invalid {
            fn pattern(&mut self, p: &Rc<Pat>) -> Result<Rc<Pat>, Failure> {
                Ok(p.clone())
            }
            fn child(&mut self, p: &Rc<Part>) -> Result<Rc<Part>, Failure> {
                Ok(Rc::new(Part::capture(
                    BTreeMap::new(),
                    p.duration(),
                    &SongLimits::default(),
                )?))
            }
            fn selector(&mut self, s: &InstrumentSelector) -> Result<InstrumentSelector, Failure> {
                Ok(s.clone())
            }
        }
        let base =
            Rc::new(Part::capture(BTreeMap::new(), Ratio64::ONE, &SongLimits::default()).unwrap());
        let repeated = Part::repeat(base, 2, RepeatSeedMode::Same, &SongLimits::default()).unwrap();
        assert_eq!(
            repeated.freeze_payloads(&mut Invalid).unwrap_err().code,
            FailCode::Type
        );
    }
    #[test]
    fn freeze_value_owned_pattern_depth_200_succeeds_300_is_explicit() {
        let assets = preparation().close().unwrap();
        for (depth, ok) in [(200, true), (300, false)] {
            let mut pat = Pat::new(PatNode::Pure(Step::bare(Value::Int(1))), None, false);
            for _ in 0..depth {
                pat = Pat::new(PatNode::Stack(vec![pat].into()), None, true);
            }
            let value = Value::Pattern(Rc::new(pat));
            let result = Freeze::new(&assets, limits()).value(&value);
            if ok {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.unwrap_err().code, FailCode::DepthExceeded);
            }
        }
    }
    #[test]
    fn freeze_selected_source_over_two_hundred_repeat_nodes_uses_normal_stack() {
        let assets = preparation().close().unwrap();
        let track = crate::value::intern::intern_kw("drums");
        let pat = Rc::new(Pat::new(
            PatNode::Pure(Step::bare(Value::Sound(Rc::new(Sound::Builtin(
                crate::value::intern::intern_kw("analog"),
            ))))),
            None,
            false,
        ));
        let mut part = Rc::new(
            Part::capture(
                BTreeMap::from([(track, pat)]),
                Ratio64::ONE,
                &SongLimits::default(),
            )
            .unwrap(),
        );
        for _ in 0..200 {
            part = Rc::new(
                Part::repeat(part, 1, RepeatSeedMode::Same, &SongLimits::default()).unwrap(),
            );
        }
        let source = Rc::new(
            crate::song::SongSource::new(
                part.clone(),
                track,
                InstrumentSelector::new(vec![Sound::Builtin(crate::value::intern::intern_kw(
                    "analog",
                ))])
                .unwrap(),
            )
            .unwrap(),
        );
        let pattern = Value::Pattern(Rc::new(Pat::new(PatNode::SongSource(source), None, true)));
        let copied = Freeze::new(&assets, limits()).value(&pattern).unwrap();
        assert!(
            matches!(copied,Value::Pattern(p) if p.id==match &pattern{Value::Pattern(p)=>p.id,_=>panic!()})
        );
    }
    #[test]
    fn freeze_wide_value_and_pattern_admit_before_copy() {
        let assets = preparation().close().unwrap();
        let mut policy = limits();
        policy.max_walk_nodes = 16;
        for value in [
            Value::list(vec![Value::Int(1); 1000]),
            Value::Pattern(Rc::new(Pat::new(
                PatNode::Stack(vec![Pat::silence(); 1000].into()),
                None,
                true,
            ))),
        ] {
            assert_eq!(
                Freeze::new(&assets, policy).value(&value).unwrap_err().code,
                FailCode::FuelExhausted
            );
        }
    }
    #[test]
    fn buffers_selected_sources_and_delete_handles_keep_identity() {
        use crate::pattern::{InputCells, TimeSpan};
        use crate::song::{query_part, SongQueryCtx, SongSource};
        let buffer = SampleBuf::ready(48000, vec![0.25f32; 8]);
        let sound = Sound::Buffer(buffer.clone());
        let pat = Rc::new(Pat::new(
            PatNode::Pure(Step::bare(Value::Sound(Rc::new(sound.clone())))),
            None,
            false,
        ));
        let base = Rc::new(
            Part::capture(
                BTreeMap::from([(crate::value::intern::intern_kw("drums"), pat)]),
                Ratio64::ONE,
                &SongLimits::default(),
            )
            .unwrap(),
        );
        let selected = Rc::new(
            SongSource::new(
                base.clone(),
                crate::value::intern::intern_kw("drums"),
                InstrumentSelector::new(vec![sound]).unwrap(),
            )
            .unwrap(),
        );
        let sourced = Rc::new(
            Part::capture(
                BTreeMap::from([(
                    crate::value::intern::intern_kw("drums"),
                    Rc::new(Pat::new(PatNode::SongSource(selected), None, true)),
                )]),
                Ratio64::ONE,
                &SongLimits::default(),
            )
            .unwrap(),
        );
        let mut evaluator = crate::ns::evaluator::Evaluator::new(
            crate::ns::namespace::Prelude::core(),
            Box::new(crate::host::noop::NoopHost),
            Box::new(crate::ns::stage::RecordingSink::default()),
        );
        let mut query = |part: &Part| {
            let (vm, ns) = evaluator.vm_and_ns();
            let mut vm = crate::vm::query_vm::VmQuery::new(vm, ns);
            let cells = InputCells::new();
            let limits = SongLimits::default();
            query_part(
                part,
                TimeSpan::cycle(0).unwrap(),
                &mut SongQueryCtx {
                    vm: &mut vm,
                    cells: &cells,
                    seed: 0,
                    tempo: Default::default(),
                    limits: &limits,
                },
            )
            .unwrap()
        };
        let before = query(&sourced);
        assert_eq!(before.len(), 1);
        let edited = Rc::new(
            sourced
                .clone()
                .edit(PartEdit::DeleteEvent(before[0].handle.clone()))
                .unwrap(),
        );
        let mut prep = preparation();
        let copy = prep.pin_buffer(&buffer).unwrap();
        let assets = prep.close().unwrap();
        let mut freeze = Freeze::new(&assets, limits());
        freeze.keep_copies(&[copy.clone()]);
        let Value::Part(frozen) = freeze.value(&Value::Part(sourced.clone())).unwrap() else {
            panic!()
        };
        assert_eq!(frozen.revision(), sourced.revision());
        assert_eq!(frozen.seed_identity(), sourced.seed_identity());
        let after = query(&frozen);
        assert_eq!(after[0].handle, before[0].handle);
        assert!(matches!(&after[0].instrument,Sound::Buffer(b) if b.id==copy.id));
        let Value::Part(deleted) = freeze.value(&Value::Part(edited)).unwrap() else {
            panic!()
        };
        assert!(query(&deleted).is_empty());
        buffer.fill_at(22050, vec![0.9f32; 8]);
        buffer.fail(FailCode::HostUnavailable, "original changed");
        assert_eq!(copy.rate(), 48000);
        assert_eq!(copy.ready_frames().unwrap()[0], 0.25);
        assert_eq!(query(&frozen)[0].handle, after[0].handle);
    }
    #[test]
    fn buffers_in_prototype_captures_and_memo_share_one_private_copy() {
        use crate::compile::proto::{Arity, Closure, FnProto};
        use crate::types::masks::ForcingMask;
        use std::cell::RefCell;
        let buffer = SampleBuf::ready(48000, vec![0.25f32; 8]);
        buffer.fill_at(48000, vec![0.25; 8]);
        let value = Value::Sound(Rc::new(Sound::Buffer(buffer.clone())));
        let mut prep = preparation();
        let copy = prep.pin_buffer(&buffer).unwrap();
        let assets = prep.close().unwrap();
        let proto = Rc::new(FnProto {
            arity: Arity::fixed(0),
            code: vec![],
            consts: vec![value.clone()],
            locals: 0,
            spans: vec![],
            name: None,
            globals: vec![],
            defs: vec![],
            form_gen: crate::ns::namespace::FormGen::new(0),
            protos: vec![],
            masks: vec![],
            call_sites: vec![],
            list_sites: vec![],
            shapes: vec![],
            captures: 1,
            span: crate::reader::span::Span::new(crate::reader::span::FileId::new(0), 0, 0),
            ctor: None,
        });
        let closure = Rc::new(Closure {
            proto,
            captures: vec![value.clone()].into(),
            mask: ForcingMask::default(),
            memo: Some(RefCell::new(Some(value))),
        });
        let mut freeze = Freeze::new(&assets, limits());
        let Value::Fn(frozen) = freeze.value(&Value::Fn(closure.clone())).unwrap() else {
            panic!()
        };
        let id = |v: &Value| match v {
            Value::Sound(s) => match &**s {
                Sound::Buffer(b) => b.id,
                _ => panic!(),
            },
            _ => panic!(),
        };
        assert_eq!(id(&frozen.proto.consts[0]), copy.id);
        assert_eq!(id(&frozen.captures[0]), copy.id);
        assert_eq!(id(&frozen.memo_value().unwrap()), copy.id);
        *closure.memo.as_ref().unwrap().borrow_mut() = Some(Value::Nil);
        buffer.fill_at(22050, vec![0.9; 8]);
        assert_eq!(id(&frozen.memo_value().unwrap()), copy.id);
        assert_eq!(copy.rate(), 48000);
        assert_eq!(copy.ready_frames().unwrap()[0], 0.25);
    }
}

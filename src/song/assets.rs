//! Candidate-only asset preparation followed by closed, immutable lookup.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::ns::load::SourceLoader;
use crate::reader::span::FileId;
use crate::value::intern::KwId;
use crate::value::sample::SampleBuf;
use crate::value::value::PathVal;
use crate::vm::fail::{FailCode, Failure};

/// Remaining host capacity, including resources occupied by retiring generations.
/// Hosts must supply remaining capacity rather than total arena capacity.
#[derive(Clone, Copy, Debug)]
pub struct SongAssetLimits {
    pub max_resources: u32,
    pub max_pcm_bytes: u64,
    pub max_source_files: u32,
    pub max_source_bytes: u64,
    pub max_banks: u32,
    pub max_walk_nodes: u32,
    pub max_walk_depth: u32,
}

impl SongAssetLimits {
    /// Derive remaining capacity from explicit current and retiring reservations.
    /// The host supplies generation accounting; preparation never guesses it.
    pub fn after_reservations(
        mut self,
        active_resources: u32,
        retiring_resources: u32,
        active_pcm_bytes: u64,
        retiring_pcm_bytes: u64,
    ) -> Result<Self, Failure> {
        let resources = active_resources
            .checked_add(retiring_resources)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "reserved resource count overflow"))?;
        let bytes = active_pcm_bytes
            .checked_add(retiring_pcm_bytes)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "reserved PCM bytes overflow"))?;
        self.max_resources = self
            .max_resources
            .checked_sub(resources)
            .ok_or_else(|| capacity("resource reservations exceed host budget"))?;
        self.max_pcm_bytes = self
            .max_pcm_bytes
            .checked_sub(bytes)
            .ok_or_else(|| capacity("PCM reservations exceed host budget"))?;
        self.validate()
    }

    /// Reject unusable traversal limits; zero remaining asset capacity is valid.
    pub fn validate(self) -> Result<Self, Failure> {
        if self.max_walk_nodes == 0
            || self.max_walk_nodes > 1_000_000
            || self.max_walk_depth == 0
            || self.max_walk_depth > 256
        {
            return Err(failure("asset traversal limits must be positive"));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub struct SongSourceFile {
    pub file: FileId,
    pub path: PathVal,
}

#[derive(Clone, Debug)]
pub enum SongAssetSelector {
    Path(PathVal),
    Bank(KwId),
}

pub trait SongAssetFactory {
    /// Begin with fresh loader state, without active file/bank/page aliases.
    fn begin(
        &self,
        source: SongSourceFile,
        limits: SongAssetLimits,
    ) -> Result<SongAssetPreparation, Failure>;
}

/// Host adapters enumerate a complete bank before any indexed lookup is certified.
pub trait SongAssetBackend: SourceLoader + SampleLoader {
    fn bank_wraps(&self) -> bool {
        false
    }
    fn read_bounded(&mut self, path: &PathVal, _: u64) -> Result<(FileId, Rc<str>), Failure> {
        self.read(path)
    }
    fn load_bounded(&mut self, src: &SampleSrc, _: u64) -> Result<Arc<SampleData>, Failure> {
        self.load(src)
    }
    fn bank_len(&mut self, bank: KwId, remaining: u32) -> Result<u32, Failure>;
}

fn failure(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::HostUnavailable, message)
}

fn capacity(message: &str) -> Failure {
    Failure::new(FailCode::FuelExhausted, message)
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Path(Option<FileId>, Rc<str>),
    Bank(KwId, u32),
    Buffer(u64),
}

fn key(src: &SampleSrc) -> Key {
    match src {
        SampleSrc::Path(p) => Key::Path(p.file, Rc::clone(&p.text)),
        SampleSrc::Bank { kw, index } => Key::Bank(*kw, *index),
        SampleSrc::Buffer { id } => Key::Buffer(*id),
    }
}

type SourceKey = (Option<FileId>, Rc<str>);
type SourceText = (FileId, Rc<str>);
type SourceInventory = BTreeMap<SourceKey, SourceText>;

struct Preparation {
    backend: Option<Box<dyn SongAssetBackend>>,
    limits: SongAssetLimits,
    closed: bool,
    sources: SourceInventory,
    source_bytes: u64,
    samples: BTreeMap<Key, Arc<SampleData>>,
    banks: BTreeMap<KwId, u32>,
    buffers: BTreeMap<u64, Rc<SampleBuf>>,
    pcm_bytes: u64,
}

impl Preparation {
    fn open(&self) -> Result<(), Failure> {
        if self.closed {
            Err(failure("song asset preparation is closed"))
        } else {
            Ok(())
        }
    }

    fn insert(&mut self, k: Key, data: Arc<SampleData>) -> Result<(), Failure> {
        if self.samples.contains_key(&k) {
            return Ok(());
        }
        if self.samples.len() >= self.limits.max_resources as usize {
            return Err(capacity(
                "remaining song sample resource capacity exhausted",
            ));
        }
        if data.rate == 0
            || !(1..=2).contains(&data.channels)
            || data.frames.len() % usize::from(data.channels) != 0
            || data.frames.iter().any(|x| !x.is_finite())
        {
            return Err(failure(
                "song sample has invalid rate, channels, alignment or PCM",
            ));
        }
        let bytes = u64::try_from(data.frames.len())
            .ok()
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| self.pcm_bytes.checked_add(n))
            .ok_or_else(|| Failure::new(FailCode::Overflow, "song PCM byte count overflow"))?;
        if bytes > self.limits.max_pcm_bytes {
            return Err(capacity("remaining song PCM capacity exhausted"));
        }
        self.pcm_bytes = bytes;
        self.samples.insert(k, data);
        Ok(())
    }
}

/// Handles returned to a candidate share this private state only with that candidate.
pub struct SongAssetPreparation(Rc<RefCell<Preparation>>);

impl SongAssetPreparation {
    pub fn new(
        backend: Box<dyn SongAssetBackend>,
        limits: SongAssetLimits,
    ) -> Result<Self, Failure> {
        let limits = limits.validate()?;
        Ok(Self(Rc::new(RefCell::new(Preparation {
            backend: Some(backend),
            limits,
            closed: false,
            sources: BTreeMap::new(),
            source_bytes: 0,
            samples: BTreeMap::new(),
            banks: BTreeMap::new(),
            buffers: BTreeMap::new(),
            pcm_bytes: 0,
        }))))
    }

    #[must_use]
    pub fn source_loader(&self) -> Box<dyn SourceLoader> {
        Box::new(PreparingLoader(Rc::clone(&self.0)))
    }
    #[must_use]
    pub fn sample_loader(&self) -> Box<dyn SampleLoader> {
        Box::new(PreparingLoader(Rc::clone(&self.0)))
    }

    pub fn pin(&mut self, selection: &[SongAssetSelector]) -> Result<(), Failure> {
        let mut st = self.0.borrow_mut();
        st.open()?;
        for selected in selection {
            match selected {
                SongAssetSelector::Path(p) => {
                    let src = SampleSrc::Path(p.clone());
                    let k = key(&src);
                    if !st.samples.contains_key(&k) {
                        if st.samples.len() >= st.limits.max_resources as usize {
                            return Err(capacity("remaining sample resource capacity exhausted"));
                        }
                        let remaining_bytes = st.limits.max_pcm_bytes - st.pcm_bytes;
                        let data = st
                            .backend
                            .as_mut()
                            .ok_or_else(|| failure("song backend is closed"))?
                            .load_bounded(&src, remaining_bytes)?;
                        st.insert(k, data)?;
                    }
                }
                SongAssetSelector::Bank(kw) => {
                    if st.banks.contains_key(kw) {
                        continue;
                    }
                    if st.banks.len() >= st.limits.max_banks as usize {
                        return Err(capacity("song bank capacity exhausted"));
                    }
                    let remaining = st
                        .limits
                        .max_resources
                        .saturating_sub(u32::try_from(st.samples.len()).unwrap_or(u32::MAX));
                    let count = st
                        .backend
                        .as_mut()
                        .ok_or_else(|| failure("song backend is closed"))?
                        .bank_len(*kw, remaining)?;
                    if count == 0 || count > remaining {
                        return Err(capacity(
                            "complete bank exceeds remaining resource capacity",
                        ));
                    }
                    for index in 0..count {
                        let src = SampleSrc::Bank { kw: *kw, index };
                        let remaining_bytes = st.limits.max_pcm_bytes - st.pcm_bytes;
                        let data = st
                            .backend
                            .as_mut()
                            .ok_or_else(|| failure("song backend is closed"))?
                            .load_bounded(&src, remaining_bytes)?;
                        st.insert(key(&src), data)?;
                    }
                    st.banks.insert(*kw, count);
                }
            }
        }
        Ok(())
    }

    pub fn pin_buffer(&mut self, buffer: &SampleBuf) -> Result<Rc<SampleBuf>, Failure> {
        let mut st = self.0.borrow_mut();
        st.open()?;
        if let Some(copy) = st.buffers.get(&buffer.id) {
            return Ok(Rc::clone(copy));
        }
        let frames = buffer.ready_frames()?;
        if buffer.rate() == 0 || frames.len() % 2 != 0 || frames.iter().any(|x| !x.is_finite()) {
            return Err(failure(
                "ready song buffer has invalid rate, stereo alignment or PCM",
            ));
        }
        // Admission precedes the PCM copy and private buffer allocation.
        let bytes = u64::try_from(frames.len())
            .ok()
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| st.pcm_bytes.checked_add(n))
            .ok_or_else(|| Failure::new(FailCode::Overflow, "song buffer byte count overflow"))?;
        if bytes > st.limits.max_pcm_bytes || st.samples.len() >= st.limits.max_resources as usize {
            return Err(capacity("remaining song buffer capacity exhausted"));
        }
        let copy = SampleBuf::ready(buffer.rate(), Rc::clone(&frames));
        st.insert(
            Key::Buffer(copy.id),
            Arc::new(SampleData {
                rate: copy.rate(),
                channels: 2,
                frames: frames.iter().copied().collect(),
            }),
        )?;
        st.buffers.insert(buffer.id, Rc::clone(&copy));
        Ok(copy)
    }

    pub fn close(self) -> Result<PinnedSongAssets, Failure> {
        let mut st = self.0.borrow_mut();
        st.open()?;
        st.closed = true;
        let backend = st
            .backend
            .take()
            .ok_or_else(|| failure("song backend is closed"))?;
        let bank_wraps = backend.bank_wraps();
        drop(backend);
        Ok(PinnedSongAssets {
            bank_wraps,
            pcm_bytes: st.pcm_bytes,
            samples: std::mem::take(&mut st.samples),
            banks: std::mem::take(&mut st.banks),
            buffers: std::mem::take(&mut st.buffers),
            source_files: std::mem::take(&mut st.sources),
        })
    }
}

struct PreparingLoader(Rc<RefCell<Preparation>>);
impl SourceLoader for PreparingLoader {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        let mut st = self.0.borrow_mut();
        st.open()?;
        let k = (path.file, Rc::clone(&path.text));
        if let Some(value) = st.sources.get(&k) {
            return Ok(value.clone());
        }
        if st.sources.len() >= st.limits.max_source_files as usize {
            return Err(capacity("song source file capacity exhausted"));
        }
        let remaining_bytes = st.limits.max_source_bytes - st.source_bytes;
        let value = st
            .backend
            .as_mut()
            .ok_or_else(|| failure("song backend is closed"))?
            .read_bounded(path, remaining_bytes)?;
        let bytes = st
            .source_bytes
            .checked_add(
                u64::try_from(value.1.len())
                    .map_err(|_| Failure::new(FailCode::Overflow, "song source size overflow"))?,
            )
            .ok_or_else(|| Failure::new(FailCode::Overflow, "song source bytes overflow"))?;
        if bytes > st.limits.max_source_bytes {
            return Err(capacity("song source byte capacity exhausted"));
        }
        st.source_bytes = bytes;
        st.sources.insert(k, value.clone());
        Ok(value)
    }
}
impl SampleLoader for PreparingLoader {
    fn load(&mut self, _: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        Err(failure(
            "song samples require explicit complete pinning before lookup",
        ))
    }
    fn register_bank(&mut self, kw: KwId, files: Vec<PathVal>) -> Result<(), Failure> {
        let mut st = self.0.borrow_mut();
        st.open()?;
        st.backend
            .as_mut()
            .ok_or_else(|| failure("song backend is closed"))?
            .register_bank(kw, files)
    }
}

/// Closed PCM and source inventory. No backend or mutable page/file map survives.
pub struct PinnedSongAssets {
    bank_wraps: bool,
    pcm_bytes: u64,
    samples: BTreeMap<Key, Arc<SampleData>>,
    banks: BTreeMap<KwId, u32>,
    buffers: BTreeMap<u64, Rc<SampleBuf>>,
    source_files: SourceInventory,
}
impl PinnedSongAssets {
    pub(crate) fn closed_sample(&self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let k = match src {
            SampleSrc::Bank { kw, index } => {
                let count = self
                    .banks
                    .get(kw)
                    .copied()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| failure("song bank was not pinned"))?;
                if !self.bank_wraps && *index >= count {
                    return Err(failure("page bank index absent from closed inventory"));
                }
                Key::Bank(
                    *kw,
                    if self.bank_wraps {
                        *index % count
                    } else {
                        *index
                    },
                )
            }
            _ => key(src),
        };
        self.samples
            .get(&k)
            .cloned()
            .ok_or_else(|| failure("resource absent from closed inventory"))
    }
    pub(crate) fn closed_bank_geometry(&self, bank: KwId) -> Option<(u32, bool)> {
        self.banks.get(&bank).copied().map(|n| (n, self.bank_wraps))
    }
    /// Exact charged decoded PCM bytes; source text is accounted separately.
    #[must_use]
    pub const fn pcm_bytes(&self) -> u64 {
        self.pcm_bytes
    }
    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.samples.len()
    }
    #[must_use]
    pub fn source_count(&self) -> usize {
        self.source_files.len()
    }
    #[must_use]
    pub fn buffer_copy(&self, original_id: u64) -> Option<Rc<SampleBuf>> {
        self.buffers.get(&original_id).cloned()
    }
}
impl SampleLoader for PinnedSongAssets {
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let k = match src {
            SampleSrc::Bank { kw, index } => {
                let count = self
                    .banks
                    .get(kw)
                    .copied()
                    .ok_or_else(|| failure("song bank was not pinned"))?;
                if !self.bank_wraps && *index >= count {
                    return Err(failure(
                        "page bank index is absent from the closed inventory",
                    ));
                }
                Key::Bank(
                    *kw,
                    if self.bank_wraps {
                        *index % count
                    } else {
                        *index
                    },
                )
            }
            _ => key(src),
        };
        self.samples
            .get(&k)
            .cloned()
            .ok_or_else(|| failure("resource is absent from the closed song inventory"))
    }
}

/// Syntactic dependencies, not a claim that an ambiguous literal is a WAV/bank.
/// The candidate evaluator must classify these using its own frozen registry.
#[derive(Default)]
pub struct SongAssetDependencies {
    consumed_work: u32,
    selected_sources: Vec<Rc<crate::song::source::SongSource>>,
    pub literal_paths: Vec<PathVal>,
    /// Ambiguous keywords from literals/prototypes/captures, not certified banks.
    pub literal_keywords: Vec<KwId>,
    pub sample_paths: Vec<PathVal>,
    pub sound_keywords: Vec<KwId>,
    pub buffers: Vec<Rc<SampleBuf>>,
    pub has_live_signals: bool,
    pub has_external_sounds: bool,
    pub native_functions: Vec<crate::value::value::NativeId>,
    pub instruments: Vec<crate::dsp::graph::InstId>,
}

impl SongAssetDependencies {
    /// Private descriptors reached by the same bounded, non-executing walk.
    pub(crate) fn selected_sources(&self) -> &[Rc<crate::song::source::SongSource>] {
        &self.selected_sources
    }
    /// Work units charged by the successful bounded dependency traversal.
    #[must_use]
    pub const fn consumed_work(&self) -> u32 {
        self.consumed_work
    }
}

/// Walk finite values/prototypes/captures and symbolic nodes without invoking code
/// or expanding repeats. Cycles/shared nodes are visited once, with checked bounds.
pub fn song_asset_dependencies(
    roots: &[crate::value::value::Value],
    limits: SongAssetLimits,
) -> Result<SongAssetDependencies, Failure> {
    let limits = limits.validate()?;
    let mut walk = DependencyWalk {
        limits,
        nodes: 0,
        seen: std::collections::BTreeSet::new(),
        out: SongAssetDependencies::default(),
    };
    for value in roots {
        walk.value(value, 0)?;
    }
    walk.out.consumed_work = walk.nodes;
    Ok(walk.out)
}
struct DependencyWalk {
    limits: SongAssetLimits,
    nodes: u32,
    seen: std::collections::BTreeSet<(u8, usize)>,
    out: SongAssetDependencies,
}
impl DependencyWalk {
    fn enter(&mut self, depth: u32) -> Result<(), Failure> {
        if depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "song dependency depth exhausted",
            ));
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "song dependency count overflow"))?;
        if self.nodes > self.limits.max_walk_nodes {
            return Err(capacity("song dependency work exhausted"));
        }
        Ok(())
    }
    fn mark<T>(&mut self, tag: u8, item: &Rc<T>) -> bool {
        self.seen.insert((tag, Rc::as_ptr(item) as usize))
    }
    fn value(&mut self, value: &crate::value::value::Value, depth: u32) -> Result<(), Failure> {
        use crate::value::value::{Sound, Value};
        self.enter(depth)?;
        match value {
            Value::Path(p) => self.out.literal_paths.push((**p).clone()),
            Value::Keyword(keyword) => self.out.literal_keywords.push(*keyword),
            Value::Sound(s) => match &**s {
                Sound::Sample(p) => self.out.sample_paths.push(p.clone()),
                Sound::Buffer(b) => {
                    b.ready_frames()?;
                    self.out.buffers.push(Rc::clone(b));
                }
                Sound::Builtin(k) => self.out.sound_keywords.push(*k),
                Sound::MidiOut(_) | Sound::Osc(_) => self.out.has_external_sounds = true,
                Sound::Inst(id) => self.out.instruments.push(*id),
            },
            Value::List(l) if self.mark(1, l) => {
                for v in &l.items {
                    self.value(v, depth + 1)?;
                }
            }
            Value::Dict(d) if self.mark(2, d) => {
                for v in d.values() {
                    self.value(v, depth + 1)?;
                }
            }
            Value::Struct(s) if self.mark(3, s) => {
                for (_, v) in &s.fields {
                    self.value(v, depth + 1)?;
                }
            }
            Value::Variant(s) if self.mark(4, s) => {
                for (_, v) in &s.fields {
                    self.value(v, depth + 1)?;
                }
            }
            Value::Fn(c) | Value::Thunk(c) if self.mark(5, c) => {
                for value in &c.captures {
                    self.value(value, depth + 1)?;
                }
                self.proto(&c.proto, depth + 1)?;
                if let Some(value) = c.memo_value() {
                    self.value(&value, depth + 1)?;
                }
            }
            Value::VarRef(slot) => {
                let id = usize::try_from(slot.id())
                    .map_err(|_| Failure::new(FailCode::Overflow, "slot identity overflow"))?;
                if self.seen.insert((6, id)) {
                    self.value(&slot.get(), depth + 1)?;
                }
            }
            Value::Native(id) => self.out.native_functions.push(*id),
            Value::Inst(id) => self.out.instruments.push(*id),
            Value::Signal(s) => self.signal(s, depth + 1)?,
            Value::Pattern(p) => self.pat(p, depth + 1)?,
            Value::Part(p) => self.part(p, depth + 1)?,
            Value::Song(s) => self.part(s.part(), depth + 1)?,
            _ => {}
        }
        Ok(())
    }
    fn proto(
        &mut self,
        proto: &Rc<crate::compile::proto::FnProto>,
        depth: u32,
    ) -> Result<(), Failure> {
        self.enter(depth)?;
        if !self.mark(7, proto) {
            return Ok(());
        }
        for value in &proto.consts {
            self.value(value, depth + 1)?;
        }
        for slot in &proto.globals {
            self.value(&crate::value::value::Value::VarRef(slot.clone()), depth + 1)?;
        }
        for child in &proto.protos {
            self.proto(child, depth + 1)?;
        }
        Ok(())
    }
    fn param(
        &mut self,
        param: &crate::pattern::pat::PParam,
        depth: u32,
        sound: bool,
    ) -> Result<(), Failure> {
        use crate::pattern::pat::PParam;
        use crate::value::value::Value;
        match param {
            PParam::Const(v) | PParam::Fn(v) => {
                if sound {
                    self.sound_value(v, depth)?;
                } else {
                    self.value(v, depth)?;
                }
            }
            PParam::Late(slot) => {
                if sound {
                    self.sound_value(&Value::VarRef(slot.clone()), depth)?;
                } else {
                    self.value(&Value::VarRef(slot.clone()), depth)?;
                }
            }
            PParam::Pat(p) => self.pat(p, depth)?,
        }
        Ok(())
    }
    fn sound_value(
        &mut self,
        value: &crate::value::value::Value,
        depth: u32,
    ) -> Result<(), Failure> {
        use crate::value::value::Value;
        self.enter(depth)?;
        match value {
            Value::Keyword(k) => self.out.sound_keywords.push(*k),
            Value::Path(p) => self.out.sample_paths.push((**p).clone()),
            Value::List(l) if self.mark(8, l) => {
                for v in &l.items {
                    self.sound_value(v, depth + 1)?;
                }
            }
            Value::VarRef(slot) => {
                let id = usize::try_from(slot.id())
                    .map_err(|_| Failure::new(FailCode::Overflow, "slot identity overflow"))?;
                if self.seen.insert((9, id)) {
                    self.sound_value(&slot.get(), depth + 1)?;
                }
            }
            _ => self.value(value, depth + 1)?,
        }
        Ok(())
    }
    fn cuts(&mut self, cuts: &crate::pattern::pat::SliceCuts, depth: u32) -> Result<(), Failure> {
        use crate::pattern::pat::SliceCuts;
        match cuts {
            SliceCuts::Equal(p) => self.param(p, depth, false)?,
            SliceCuts::Manual(params) => {
                for p in params {
                    self.param(p, depth, false)?;
                }
            }
        }
        Ok(())
    }
    fn pat(&mut self, pat: &crate::pattern::pat::Pat, depth: u32) -> Result<(), Failure> {
        use crate::pattern::pat::PatNode;
        self.enter(depth)?;
        if !self
            .seen
            .insert((10, pat as *const crate::pattern::pat::Pat as usize))
        {
            return Ok(());
        }
        match &pat.node {
            PatNode::Rev(child)
            | PatNode::Fit(child)
            | PatNode::Voicing(child)
            | PatNode::ScaleNotes(_, _, child)
            | PatNode::MidiNotes { subject: child, .. } => self.pat(child, depth + 1),
            PatNode::Fast(child, param)
            | PatNode::Slow(child, param)
            | PatNode::Hurry(child, param)
            | PatNode::DegradeBy(child, param)
            | PatNode::Maybe(child, param)
            | PatNode::Hold(child, param)
            | PatNode::Repeat(child, param)
            | PatNode::Iter(child, param)
            | PatNode::Chop(child, param)
            | PatNode::Ply(child, param)
            | PatNode::Striate(child, param)
            | PatNode::LoopAt(child, param)
            | PatNode::Arp(child, param)
            | PatNode::Harp {
                subject: child,
                pos: param,
                ..
            }
            | PatNode::Inversion {
                subject: child,
                n: param,
                ..
            }
            | PatNode::Segment(child, param) => {
                self.pat(child, depth + 1)?;
                self.param(param, depth + 1, false)
            }
            PatNode::Strum(child, time, dir, curve) => {
                self.pat(child, depth + 1)?;
                self.param(time, depth + 1, false)?;
                self.param(dir, depth + 1, false)?;
                self.param(curve, depth + 1, false)
            }
            _ => self.pat_inner(pat, depth),
        }
    }
    #[inline(never)]
    fn pat_inner(&mut self, pat: &crate::pattern::pat::Pat, depth: u32) -> Result<(), Failure> {
        use crate::pattern::pat::PatNode;
        match &pat.node {
            PatNode::Steps(items) => {
                for step in items {
                    self.value(&step.value, depth + 1)?;
                }
            }
            PatNode::Pure(step) => self.value(&step.value, depth + 1)?,
            PatNode::Sound { src, kit } => {
                self.param(src, depth + 1, true)?;
                if let Some(p) = kit {
                    self.param(p, depth + 1, true)?;
                }
            }
            PatNode::Signal(s) => self.signal(s, depth + 1)?,
            PatNode::SongSource(s) => {
                if self.mark(13, s) {
                    self.out.selected_sources.push(s.clone());
                }
                self.part(s.part(), depth + 1)?;
            }
            PatNode::Fast(p, a)
            | PatNode::Slow(p, a)
            | PatNode::Hurry(p, a)
            | PatNode::DegradeBy(p, a)
            | PatNode::Maybe(p, a)
            | PatNode::Hold(p, a)
            | PatNode::Repeat(p, a)
            | PatNode::Iter(p, a)
            | PatNode::Chop(p, a)
            | PatNode::Ply(p, a)
            | PatNode::Striate(p, a)
            | PatNode::LoopAt(p, a)
            | PatNode::Arp(p, a)
            | PatNode::Segment(p, a) => {
                self.pat(p, depth + 1)?;
                self.param(a, depth + 1, false)?;
            }
            PatNode::Rev(p)
            | PatNode::Fit(p)
            | PatNode::Voicing(p)
            | PatNode::ScaleNotes(_, _, p)
            | PatNode::MidiNotes { subject: p, .. }
            | PatNode::Harp { subject: p, .. }
            | PatNode::Inversion { subject: p, .. }
            | PatNode::Strum(p, _, _, _) => self.pat(p, depth + 1)?,
            PatNode::Tune {
                tunings, subject, ..
            } => {
                self.pat(tunings, depth + 1)?;
                self.pat(subject, depth + 1)?;
            }
            PatNode::Every(a, v, p) | PatNode::SometimesBy(a, v, p) => {
                self.param(a, depth + 1, false)?;
                self.value(v, depth + 1)?;
                self.pat(p, depth + 1)?;
            }
            PatNode::WhenMod(a, b, v, p) => {
                self.param(a, depth + 1, false)?;
                self.param(b, depth + 1, false)?;
                self.value(v, depth + 1)?;
                self.pat(p, depth + 1)?;
            }
            PatNode::Choose(ps) | PatNode::Stack(ps) | PatNode::Cat(ps) | PatNode::FastCat(ps) => {
                for p in ps {
                    self.pat(p, depth + 1)?;
                }
            }
            PatNode::Superimpose(p, v) | PatNode::Jux(p, v) => {
                self.pat(p, depth + 1)?;
                self.value(v, depth + 1)?;
            }
            PatNode::Off(p, a, v) | PatNode::Chunk(p, a, v) => {
                self.pat(p, depth + 1)?;
                self.param(a, depth + 1, false)?;
                self.value(v, depth + 1)?;
            }
            PatNode::Slice { pat, cuts, index } | PatNode::Splice { pat, cuts, index } => {
                self.pat(pat, depth + 1)?;
                self.cuts(cuts, depth + 1)?;
                self.pat(index, depth + 1)?;
            }
            PatNode::Grid(p, q) | PatNode::Chord(p, q) | PatNode::Control(_, p, q) => {
                self.pat(p, depth + 1)?;
                self.pat(q, depth + 1)?;
            }
            PatNode::Euclid(p, a, b, c) => {
                self.pat(p, depth + 1)?;
                self.param(a, depth + 1, false)?;
                self.param(b, depth + 1, false)?;
                self.param(c, depth + 1, false)?;
            }
            PatNode::Range(p, a, b) => {
                self.pat(p, depth + 1)?;
                self.param(a, depth + 1, false)?;
                self.param(b, depth + 1, false)?;
            }
        }
        Ok(())
    }
    fn signal(
        &mut self,
        signal: &Rc<crate::pattern::signal::Sig>,
        depth: u32,
    ) -> Result<(), Failure> {
        use crate::pattern::signal::Sig;
        self.enter(depth)?;
        if !self.mark(12, signal) {
            return Ok(());
        }
        match &**signal {
            Sig::Host(_) | Sig::Cc { .. } | Sig::Analyzer(_) | Sig::Ctrl(..) | Sig::Hits(_) => {
                self.out.has_live_signals = true
            }
            Sig::Lag(child, _) | Sig::MapRange(child, ..) => self.signal(child, depth + 1)?,
            _ => {}
        }
        Ok(())
    }
    fn part(&mut self, part: &Rc<crate::song::Part>, depth: u32) -> Result<(), Failure> {
        self.enter(depth)?;
        if !self.mark(11, part) {
            return Ok(());
        }
        if let crate::song::PartNode::Repeat { child, .. } = part.node() {
            return self.part(child, depth + 1);
        }
        self.part_inner(part, depth)
    }
    #[inline(never)]
    fn part_inner(&mut self, part: &Rc<crate::song::Part>, depth: u32) -> Result<(), Failure> {
        use crate::song::{PartEdit, PartNode};
        match part.node() {
            PartNode::Capture(tracks) => {
                for p in tracks.values() {
                    self.pat(p, depth + 1)?;
                }
            }
            PartNode::Sequence(parts) => {
                for p in parts {
                    self.part(p, depth + 1)?;
                }
            }
            PartNode::Repeat { child, .. } => self.part(child, depth + 1)?,
            PartNode::Edit { source, edit } => {
                self.part(source, depth + 1)?;
                match edit {
                    PartEdit::ReplaceTrack { pattern, .. }
                    | PartEdit::TransformInstrument { pattern, .. }
                    | PartEdit::OverwriteRegion { pattern, .. } => self.pat(pattern, depth + 1)?,
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// Portable detached page adapter, also exercised by native behavioral fixtures.
/// Catalog entries are ordered exact lookup keys, not inferred from partial PCM.
pub struct DecodedSongAssetFactory {
    samples: BTreeMap<String, Arc<SampleData>>,
    banks: BTreeMap<KwId, Vec<String>>,
    sources: BTreeMap<String, (FileId, Rc<str>)>,
}
impl DecodedSongAssetFactory {
    #[must_use]
    pub fn new(
        samples: BTreeMap<String, Arc<SampleData>>,
        banks: BTreeMap<KwId, Vec<String>>,
        sources: BTreeMap<String, (FileId, Rc<str>)>,
    ) -> Self {
        Self {
            samples,
            banks,
            sources,
        }
    }
}
struct DecodedBackend {
    samples: BTreeMap<String, Arc<SampleData>>,
    banks: BTreeMap<KwId, Vec<String>>,
    sources: BTreeMap<String, (FileId, Rc<str>)>,
}
impl SongAssetFactory for DecodedSongAssetFactory {
    fn begin(
        &self,
        _: SongSourceFile,
        limits: SongAssetLimits,
    ) -> Result<SongAssetPreparation, Failure> {
        SongAssetPreparation::new(
            Box::new(DecodedBackend {
                samples: self.samples.clone(),
                banks: self.banks.clone(),
                sources: self.sources.clone(),
            }),
            limits,
        )
    }
}
impl SourceLoader for DecodedBackend {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        self.sources
            .get(&*path.text)
            .cloned()
            .ok_or_else(|| failure("source is absent from detached page inventory"))
    }
}
impl SampleLoader for DecodedBackend {
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let lookup = match src {
            SampleSrc::Path(p) => p.text.to_string(),
            SampleSrc::Buffer { id } => {
                return Err(failure(format!("buffer #{id} was not privately pinned")));
            }
            SampleSrc::Bank { kw, index } => self
                .banks
                .get(kw)
                .and_then(|keys| keys.get(*index as usize))
                .cloned()
                .ok_or_else(|| {
                    failure("page bank index is absent from explicit complete catalog")
                })?,
        };
        self.samples
            .get(&lookup)
            .cloned()
            .ok_or_else(|| failure(format!("detached page sample `{lookup}` is missing")))
    }
}
impl SongAssetBackend for DecodedBackend {
    fn bank_len(&mut self, bank: KwId, remaining: u32) -> Result<u32, Failure> {
        let keys = self
            .banks
            .get(&bank)
            .filter(|keys| !keys.is_empty())
            .ok_or_else(|| failure("page bank has no explicit complete catalog"))?;
        let count = u32::try_from(keys.len())
            .map_err(|_| Failure::new(FailCode::Overflow, "page bank count overflow"))?;
        if count > remaining {
            return Err(capacity("complete page bank exceeds remaining capacity"));
        }
        if keys.iter().any(|k| !self.samples.contains_key(k)) {
            return Err(failure("complete page catalog contains missing PCM"));
        }
        Ok(count)
    }
}

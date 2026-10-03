//! Bounded payload copying from an already isolated evaluator. This never clones
//! an active session: slots belong to the fresh candidate and are updated once.
use crate::compile::proto::{Closure, FnProto};
use crate::ns::namespace::VarSlotRef;
use crate::pattern::pat::{PParam, Pat, PatNode, SliceCuts};
use crate::song::assets::{PinnedSongAssets, SongAssetLimits};
use crate::song::part::PartPayloadMapper;
use crate::song::{InstrumentSelector, Part, PartNode, Song};
use crate::value::value::{ListVal, Sound, Value};
use crate::vm::fail::{FailCode, Failure};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub(crate) struct Freeze<'a> {
    assets: &'a PinnedSongAssets,
    pub(super) origins: BTreeMap<crate::reader::span::FileId, Rc<str>>,
    limits: SongAssetLimits,
    work: u32,
    pub(super) audit: bool,
    depth: u32,
    peak_depth: u32,
    heights: BTreeMap<(u8, usize), u32>,
    slots: BTreeSet<u64>,
    slot_heights: BTreeMap<u64, u32>,
    copies: BTreeMap<u64, Rc<crate::value::sample::SampleBuf>>,
    copied: BTreeMap<(u8, usize), Value>,
    protos: BTreeMap<usize, Rc<FnProto>>,
    visiting: BTreeSet<(u8, usize)>,
}
impl<'a> Freeze<'a> {
    pub(crate) fn new(assets: &'a PinnedSongAssets, limits: SongAssetLimits) -> Self {
        Self {
            assets,
            origins: BTreeMap::new(),
            limits,
            work: 0,
            audit: false,
            depth: 0,
            peak_depth: 0,
            heights: BTreeMap::new(),
            slots: BTreeSet::new(),
            slot_heights: BTreeMap::new(),
            copies: BTreeMap::new(),
            copied: BTreeMap::new(),
            protos: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }
    /// Actual traversal and collection work charged by this copy pass.
    /// Failed admission may exceed its limit; consumers debit successful passes only.
    #[must_use]
    pub(crate) const fn consumed_work(&self) -> u32 {
        self.work
    }
    pub(crate) fn keep_copies(&mut self, copies: &[Rc<crate::value::sample::SampleBuf>]) {
        self.copies
            .extend(copies.iter().map(|copy| (copy.id, copy.clone())));
    }
    fn enter(&mut self) -> Result<(), Failure> {
        self.work = self
            .work
            .checked_add(1)
            .ok_or_else(|| fail("freeze work overflow"))?;
        if self.work > self.limits.max_walk_nodes {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "song freeze work exhausted",
            ));
        }
        if self.depth >= self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "song freeze depth exhausted",
            ));
        }
        self.depth += 1;
        self.peak_depth = self.peak_depth.max(self.depth);
        Ok(())
    }
    fn admit(&mut self, count: usize) -> Result<(), Failure> {
        let count = u32::try_from(count).map_err(|_| fail("freeze collection length overflow"))?;
        self.work = self
            .work
            .checked_add(count)
            .ok_or_else(|| fail("freeze work overflow"))?;
        if self.work > self.limits.max_walk_nodes {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "song freeze collection exceeds remaining work",
            ));
        }
        Ok(())
    }
    pub(crate) fn slot(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
        if self.slots.insert(slot.id()) {
            let previous_peak = self.peak_depth;
            self.peak_depth = self.depth;
            let value = self.value(&slot.get());
            if let Ok(value) = value {
                slot.restore(value, slot.version());
                self.slot_heights
                    .insert(slot.id(), self.peak_depth - self.depth + 1);
                self.peak_depth = previous_peak.max(self.peak_depth);
                Ok(())
            } else {
                self.slots.remove(&slot.id());
                self.peak_depth = previous_peak.max(self.peak_depth);
                value.map(|_| ())
            }
        } else if let Some(height) = self.slot_heights.get(&slot.id()).copied() {
            self.admit_height(height)
        } else {
            // A currently visited private global can close a recursive function
            // cycle; its immutable payload is still being copied, not cached.
            Ok(())
        }
    }
    pub(crate) fn value(&mut self, value: &Value) -> Result<Value, Failure> {
        if let Value::Pattern(pattern) = value {
            return self.copy_pattern(pattern).map(Value::Pattern);
        }
        let previous_peak = self.peak_depth;
        self.enter()?;
        self.peak_depth = self.depth;
        let result = match value {
            Value::Part(part) => self.cached((8, Rc::as_ptr(part) as usize), |this| {
                this.admit(match part.node() {
                    PartNode::Capture(v) => v.len(),
                    PartNode::Sequence(v) => v.len(),
                    _ => 1,
                })?;
                Ok(Value::Part(Rc::new(part.freeze_payloads(this)?)))
            }),
            Value::Song(song) => self.cached((9, Rc::as_ptr(song) as usize), |this| {
                Ok(Value::Song(Rc::new(Song::new(
                    this.child(song.part())?,
                    *song.settings(),
                )?)))
            }),
            _ => match value_key(value) {
                Some(key) => self.cached(key, |this| this.value_inner(value)),
                None => self.value_inner(value),
            },
        };
        self.depth -= 1;
        self.peak_depth = previous_peak.max(self.peak_depth);
        result
    }
    /// Unary descendants share an explicit bounded spine instead of several
    /// large recursive match frames per pattern. Each original Rc retains its
    /// cache/cycle identity and its logical depth until reconstruction finishes.
    fn copy_pattern(&mut self, root: &Rc<Pat>) -> Result<Rc<Pat>, Failure> {
        let initial_depth = self.depth;
        let previous_peak = self.peak_depth;
        self.peak_depth = self.depth;
        let mut spine: Vec<&Rc<Pat>> = Vec::new();
        let result = (|| {
            let mut current = root;
            let mut copied = loop {
                self.enter()?;
                let key = (7, Rc::as_ptr(current) as usize);
                if let Some(Value::Pattern(copy)) = self.copied.get(&key) {
                    let copy = copy.clone();
                    self.admit_cached_height(key)?;
                    self.depth -= 1;
                    break copy;
                }
                if self.visiting.contains(&key) {
                    return Err(fail("cyclic immutable candidate payload"));
                }
                // Account for pending storage before Vec can grow.
                self.admit(1)?;
                self.visiting.insert(key);
                spine.push(current);
                if let Some(child) = unary_child(&current.node) {
                    current = child;
                    continue;
                }
                let copy = Rc::new(self.pat_inner(current)?);
                self.visiting.remove(&key);
                self.copied.insert(key, Value::Pattern(copy.clone()));
                self.heights.insert(key, self.peak_depth - self.depth + 1);
                spine.pop();
                self.depth -= 1;
                break copy;
            };
            while let Some(original) = spine.last().copied() {
                let node = self.finish_unary(&original.node, copied)?;
                copied = Rc::new(Pat {
                    node,
                    span: original.span,
                    structured: original.structured,
                    id: original.id,
                });
                let key = (7, Rc::as_ptr(original) as usize);
                self.visiting.remove(&key);
                self.copied.insert(key, Value::Pattern(copied.clone()));
                self.heights.insert(key, self.peak_depth - self.depth + 1);
                spine.pop();
                self.depth -= 1;
            }
            Ok(copied)
        })();
        for original in spine {
            self.visiting.remove(&(7, Rc::as_ptr(original) as usize));
        }
        self.depth = initial_depth;
        self.peak_depth = previous_peak.max(self.peak_depth);
        result
    }
    fn finish_unary(&mut self, node: &PatNode, child: Rc<Pat>) -> Result<PatNode, Failure> {
        use PatNode::*;
        Ok(match node {
            Fast(_, a) => Fast(child, self.param(a)?),
            Slow(_, a) => Slow(child, self.param(a)?),
            Hurry(_, a) => Hurry(child, self.param(a)?),
            Rev(_) => Rev(child),
            Fit(_) => Fit(child),
            Voicing(_) => Voicing(child),
            ScaleNotes(a, b, _) => ScaleNotes(*a, *b, child),
            DegradeBy(_, a) => DegradeBy(child, self.param(a)?),
            Maybe(_, a) => Maybe(child, self.param(a)?),
            Hold(_, a) => Hold(child, self.param(a)?),
            Repeat(_, a) => Repeat(child, self.param(a)?),
            Iter(_, a) => Iter(child, self.param(a)?),
            Chop(_, a) => Chop(child, self.param(a)?),
            Ply(_, a) => Ply(child, self.param(a)?),
            Striate(_, a) => Striate(child, self.param(a)?),
            LoopAt(_, a) => LoopAt(child, self.param(a)?),
            Arp(_, a) => Arp(child, self.param(a)?),
            Segment(_, a) => Segment(child, self.param(a)?),
            _ => return Err(fail("freeze unary reconstruction invariant")),
        })
    }
    #[inline(never)]
    fn cached(
        &mut self,
        key: (u8, usize),
        build: impl FnOnce(&mut Self) -> Result<Value, Failure>,
    ) -> Result<Value, Failure> {
        if let Some(copy) = self.copied.get(&key).cloned() {
            self.admit_cached_height(key)?;
            return Ok(copy);
        }
        if !self.visiting.insert(key) {
            return Err(fail("cyclic immutable candidate payload"));
        }
        let copied = build(self);
        self.visiting.remove(&key);
        if let Ok(copy) = &copied {
            self.copied.insert(key, copy.clone());
            self.heights.insert(key, self.peak_depth - self.depth + 1);
        }
        copied
    }
    fn admit_cached_height(&mut self, key: (u8, usize)) -> Result<(), Failure> {
        let height = self
            .heights
            .get(&key)
            .copied()
            .ok_or_else(|| fail("cached height missing"))?;
        self.admit_height(height)
    }
    fn admit_height(&mut self, height: u32) -> Result<(), Failure> {
        let peak = self
            .depth
            .checked_add(height - 1)
            .ok_or_else(|| fail("cached depth overflow"))?;
        if peak > self.limits.max_walk_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "cached payload exceeds remaining depth",
            ));
        }
        self.peak_depth = self.peak_depth.max(peak);
        Ok(())
    }
    fn value_inner(&mut self, value: &Value) -> Result<Value, Failure> {
        match value {
            Value::List(v) => self.admit(v.items.len())?,
            Value::Dict(v) => self.admit(v.len())?,
            Value::Struct(v) => self.admit(v.fields.len())?,
            Value::Variant(v) => self.admit(v.fields.len())?,
            _ => {}
        }
        let copied = match value {
            Value::List(list) => Value::List(Rc::new(ListVal {
                items: list
                    .items
                    .iter()
                    .map(|v| self.value(v))
                    .collect::<Result<_, _>>()?,
                prov: list.prov.clone(),
            })),
            Value::Dict(dict) => Value::Dict(Rc::new(
                dict.iter()
                    .map(|(k, v)| Ok((k.clone(), self.value(v)?)))
                    .collect::<Result<_, Failure>>()?,
            )),
            Value::Struct(s) => {
                let mut copy = (**s).clone();
                copy.fields = s
                    .fields
                    .iter()
                    .map(|(k, v)| Ok((*k, self.value(v)?)))
                    .collect::<Result<_, Failure>>()?;
                Value::Struct(Rc::new(copy))
            }
            Value::Variant(s) => {
                let mut copy = (**s).clone();
                copy.fields = s
                    .fields
                    .iter()
                    .map(|(k, v)| Ok((*k, self.value(v)?)))
                    .collect::<Result<_, Failure>>()?;
                Value::Variant(Rc::new(copy))
            }
            Value::Fn(c) => Value::Fn(self.closure(c)?),
            Value::Thunk(c) => Value::Thunk(self.closure(c)?),
            Value::VarRef(slot) => {
                self.slot(slot)?;
                value.clone()
            }
            Value::Path(path) => Value::Path(Rc::new(super::normalize_path(path, &self.origins))),
            Value::Sound(sound) => Value::Sound(Rc::new(self.sound(sound)?)),
            Value::Pattern(pat) => Value::Pattern(Rc::new(self.pat_inner(pat)?)),
            _ => value.clone(),
        };
        Ok(copied)
    }
    fn sound(&mut self, sound: &Sound) -> Result<Sound, Failure> {
        Ok(match sound {
            Sound::Buffer(buffer) => Sound::Buffer(
                self.assets
                    .buffer_copy(buffer.id)
                    .or_else(|| self.copies.get(&buffer.id).cloned())
                    .ok_or_else(|| fail("candidate buffer was not pinned"))?,
            ),
            Sound::Sample(path) => Sound::Sample(super::normalize_path(path, &self.origins)),
            _ => sound.clone(),
        })
    }
    fn closure(&mut self, closure: &Closure) -> Result<Rc<Closure>, Failure> {
        let proto = self.proto(&closure.proto)?;
        self.admit(closure.captures.len())?;
        let captures = closure
            .captures
            .iter()
            .map(|v| self.value(v))
            .collect::<Result<_, _>>()?;
        let memo = closure
            .memo
            .as_ref()
            .map(|cell| {
                cell.borrow()
                    .as_ref()
                    .map(|v| self.value(v))
                    .transpose()
                    .map(RefCell::new)
            })
            .transpose()?;
        Ok(Rc::new(Closure {
            proto,
            captures,
            mask: closure.mask.clone(),
            memo,
        }))
    }
    fn proto(&mut self, proto: &Rc<FnProto>) -> Result<Rc<FnProto>, Failure> {
        let previous_peak = self.peak_depth;
        self.enter()?;
        self.peak_depth = self.depth;
        let result = self.proto_inner(proto);
        self.depth -= 1;
        self.peak_depth = previous_peak.max(self.peak_depth);
        result
    }
    fn proto_inner(&mut self, proto: &Rc<FnProto>) -> Result<Rc<FnProto>, Failure> {
        let id = Rc::as_ptr(proto) as usize;
        if let Some(copy) = self.protos.get(&id).cloned() {
            self.admit_cached_height((10, id))?;
            return Ok(copy);
        }
        if self.audit
            && proto.code.iter().any(|op| {
                matches!(
                    op,
                    crate::vm::ops::Op::UpdGlobal(_) | crate::vm::ops::Op::DefGlobal(_)
                )
            })
        {
            return Err(Failure::new(
                FailCode::EffectInQuery,
                "song callback writes global state",
            ));
        }
        self.admit(super::prototype_copy_work(proto)?)?;
        for slot in &proto.globals {
            self.slot(slot)?;
        }
        let copy = FnProto {
            arity: proto.arity.clone(),
            code: proto.code.clone(),
            consts: proto
                .consts
                .iter()
                .map(|v| self.value(v))
                .collect::<Result<_, _>>()?,
            locals: proto.locals,
            spans: proto.spans.clone(),
            name: proto.name,
            globals: proto.globals.clone(),
            defs: proto.defs.clone(),
            form_gen: proto.form_gen,
            protos: proto
                .protos
                .iter()
                .map(|p| self.proto(p))
                .collect::<Result<_, _>>()?,
            masks: proto.masks.clone(),
            call_sites: proto.call_sites.clone(),
            list_sites: proto.list_sites.clone(),
            shapes: proto.shapes.clone(),
            captures: proto.captures,
            span: proto.span,
            ctor: proto.ctor,
        };
        let copy = Rc::new(copy);
        self.protos.insert(id, Rc::clone(&copy));
        self.heights
            .insert((10, id), self.peak_depth - self.depth + 1);
        Ok(copy)
    }
    fn param(&mut self, param: &PParam) -> Result<PParam, Failure> {
        Ok(match param {
            PParam::Const(v) => PParam::Const(self.value(v)?),
            PParam::Fn(v) => PParam::Fn(self.value(v)?),
            PParam::Late(slot) => {
                self.slot(slot)?;
                PParam::Late(slot.clone())
            }
            PParam::Pat(p) => PParam::Pat(self.pattern(p)?),
        })
    }
    fn cuts(&mut self, cuts: &SliceCuts) -> Result<SliceCuts, Failure> {
        Ok(match cuts {
            SliceCuts::Equal(p) => SliceCuts::Equal(self.param(p)?),
            SliceCuts::Manual(ps) => {
                SliceCuts::Manual(ps.iter().map(|p| self.param(p)).collect::<Result<_, _>>()?)
            }
        })
    }
    fn pat(&mut self, pat: &Pat) -> Result<Pat, Failure> {
        self.enter()?;
        let result = self.pat_inner(pat);
        self.depth -= 1;
        result
    }
    fn pat_inner(&mut self, pat: &Pat) -> Result<Pat, Failure> {
        use PatNode::*;
        match &pat.node {
            Steps(v) => self.admit(v.len())?,
            Choose(v) | Stack(v) | Cat(v) | FastCat(v) => self.admit(v.len())?,
            _ => {}
        }
        let node = match &pat.node {
            Steps(..) | Pure(..) | Sound { .. } | Signal(..) | SongSource(..) => {
                self.pat_leaf(&pat.node)?
            }
            Choose(..) | Stack(..) | Cat(..) | FastCat(..) => self.pat_vector(&pat.node)?,
            Every(..)
            | SometimesBy(..)
            | WhenMod(..)
            | Superimpose(..)
            | Jux(..)
            | Off(..)
            | Chunk(..)
            | Slice { .. }
            | Splice { .. }
            | Grid(..)
            | Chord(..)
            | Control(..)
            | Euclid(..)
            | Range(..)
            | MidiNotes { .. } => self.pat_complex(&pat.node)?,
            Fast(..) | Slow(..) | Hurry(..) | Rev(..) | Fit(..) | Voicing(..) | ScaleNotes(..)
            | DegradeBy(..) | Maybe(..) | Hold(..) | Repeat(..) | Iter(..) | Chop(..) | Ply(..)
            | Striate(..) | LoopAt(..) | Arp(..) | Segment(..) => self.pat_unary(&pat.node)?,
        };
        Ok(Pat {
            node,
            span: pat.span,
            structured: pat.structured,
            id: pat.id,
        })
    }
    #[inline(never)]
    fn pat_leaf(&mut self, node: &PatNode) -> Result<PatNode, Failure> {
        use PatNode::*;
        Ok(match node {
            Steps(items) => Steps(
                items
                    .iter()
                    .map(|step| {
                        let mut s = step.clone();
                        s.value = self.value(&s.value)?;
                        Ok(s)
                    })
                    .collect::<Result<_, Failure>>()?,
            ),
            Pure(step) => {
                let mut s = step.clone();
                s.value = self.value(&s.value)?;
                Pure(s)
            }
            Sound { src, kit } => Sound {
                src: self.param(src)?,
                kit: kit.as_ref().map(|p| self.param(p)).transpose()?,
            },
            Signal(s) => Signal(s.clone()),
            SongSource(s) => SongSource(Rc::new(crate::song::SongSource::new(
                self.child(s.part())?,
                s.track(),
                self.selector(s.selector())?,
            )?)),
            _ => return Err(fail("freeze pattern dispatch invariant")),
        })
    }
    #[inline(never)]
    fn pat_unary(&mut self, node: &PatNode) -> Result<PatNode, Failure> {
        use PatNode::*;
        Ok(match node {
            Fast(p, a) => Fast(self.pattern(p)?, self.param(a)?),
            Slow(p, a) => Slow(self.pattern(p)?, self.param(a)?),
            Hurry(p, a) => Hurry(self.pattern(p)?, self.param(a)?),
            Rev(p) => Rev(self.pattern(p)?),
            Fit(p) => Fit(self.pattern(p)?),
            Voicing(p) => Voicing(self.pattern(p)?),
            ScaleNotes(a, b, p) => ScaleNotes(*a, *b, self.pattern(p)?),
            DegradeBy(p, a) => DegradeBy(self.pattern(p)?, self.param(a)?),
            Maybe(p, a) => Maybe(self.pattern(p)?, self.param(a)?),
            Hold(p, a) => Hold(self.pattern(p)?, self.param(a)?),
            Repeat(p, a) => Repeat(self.pattern(p)?, self.param(a)?),
            Iter(p, a) => Iter(self.pattern(p)?, self.param(a)?),
            Chop(p, a) => Chop(self.pattern(p)?, self.param(a)?),
            Ply(p, a) => Ply(self.pattern(p)?, self.param(a)?),
            Striate(p, a) => Striate(self.pattern(p)?, self.param(a)?),
            LoopAt(p, a) => LoopAt(self.pattern(p)?, self.param(a)?),
            Arp(p, a) => Arp(self.pattern(p)?, self.param(a)?),
            Segment(p, a) => Segment(self.pattern(p)?, self.param(a)?),
            _ => return Err(fail("freeze pattern dispatch invariant")),
        })
    }
    #[inline(never)]
    fn pat_complex(&mut self, node: &PatNode) -> Result<PatNode, Failure> {
        use PatNode::*;
        Ok(match node {
            Every(a, v, p) => Every(self.param(a)?, self.value(v)?, self.pattern(p)?),
            SometimesBy(a, v, p) => SometimesBy(self.param(a)?, self.value(v)?, self.pattern(p)?),
            WhenMod(a, b, v, p) => WhenMod(
                self.param(a)?,
                self.param(b)?,
                self.value(v)?,
                self.pattern(p)?,
            ),
            Superimpose(p, v) => Superimpose(self.pattern(p)?, self.value(v)?),
            Jux(p, v) => Jux(self.pattern(p)?, self.value(v)?),
            Off(p, a, v) => Off(self.pattern(p)?, self.param(a)?, self.value(v)?),
            Chunk(p, a, v) => Chunk(self.pattern(p)?, self.param(a)?, self.value(v)?),
            Slice { pat, cuts, index } => Slice {
                pat: self.pattern(pat)?,
                cuts: self.cuts(cuts)?,
                index: self.pattern(index)?,
            },
            Splice { pat, cuts, index } => Splice {
                pat: self.pattern(pat)?,
                cuts: self.cuts(cuts)?,
                index: self.pattern(index)?,
            },
            Grid(p, q) => Grid(self.pattern(p)?, self.pattern(q)?),
            Chord(p, q) => Chord(self.pattern(p)?, self.pattern(q)?),
            Control(k, p, q) => Control(*k, self.pattern(p)?, self.pattern(q)?),
            Euclid(p, a, b, c) => Euclid(
                self.pattern(p)?,
                self.param(a)?,
                self.param(b)?,
                self.param(c)?,
            ),
            Range(p, a, b) => Range(self.pattern(p)?, self.param(a)?, self.param(b)?),
            MidiNotes { .. } => return Err(fail("live MIDI cannot enter a song snapshot")),
            _ => return Err(fail("freeze pattern dispatch invariant")),
        })
    }
    #[inline(never)]
    fn pat_vector(&mut self, node: &PatNode) -> Result<PatNode, Failure> {
        use PatNode::*;
        Ok(match node {
            Choose(ps) => Choose(ps.iter().map(|p| self.pat(p)).collect::<Result<_, _>>()?),
            Stack(ps) => Stack(ps.iter().map(|p| self.pat(p)).collect::<Result<_, _>>()?),
            Cat(ps) => Cat(ps.iter().map(|p| self.pat(p)).collect::<Result<_, _>>()?),
            FastCat(ps) => FastCat(ps.iter().map(|p| self.pat(p)).collect::<Result<_, _>>()?),
            _ => return Err(fail("freeze pattern dispatch invariant")),
        })
    }
}
fn value_key(value: &Value) -> Option<(u8, usize)> {
    Some(match value {
        Value::List(v) => (1, Rc::as_ptr(v) as usize),
        Value::Dict(v) => (2, Rc::as_ptr(v) as usize),
        Value::Struct(v) => (3, Rc::as_ptr(v) as usize),
        Value::Variant(v) => (4, Rc::as_ptr(v) as usize),
        Value::Fn(v) => (5, Rc::as_ptr(v) as usize),
        Value::Thunk(v) => (6, Rc::as_ptr(v) as usize),
        Value::Pattern(v) => (7, Rc::as_ptr(v) as usize),
        Value::Part(v) => (8, Rc::as_ptr(v) as usize),
        Value::Song(v) => (9, Rc::as_ptr(v) as usize),
        _ => return None,
    })
}
fn unary_child(node: &PatNode) -> Option<&Rc<Pat>> {
    use PatNode::*;
    match node {
        Fast(p, _)
        | Slow(p, _)
        | Hurry(p, _)
        | Rev(p)
        | Fit(p)
        | Voicing(p)
        | ScaleNotes(_, _, p)
        | DegradeBy(p, _)
        | Maybe(p, _)
        | Hold(p, _)
        | Repeat(p, _)
        | Iter(p, _)
        | Chop(p, _)
        | Ply(p, _)
        | Striate(p, _)
        | LoopAt(p, _)
        | Arp(p, _)
        | Segment(p, _) => Some(p),
        _ => None,
    }
}
impl PartPayloadMapper for Freeze<'_> {
    fn pattern(&mut self, pattern: &Rc<Pat>) -> Result<Rc<Pat>, Failure> {
        match self.value(&Value::Pattern(pattern.clone()))? {
            Value::Pattern(p) => Ok(p),
            _ => Err(fail("freeze pattern invariant")),
        }
    }
    fn child(&mut self, part: &Rc<Part>) -> Result<Rc<Part>, Failure> {
        match self.value(&Value::Part(part.clone()))? {
            Value::Part(p) => Ok(p),
            _ => Err(fail("freeze part invariant")),
        }
    }
    fn selector(&mut self, selector: &InstrumentSelector) -> Result<InstrumentSelector, Failure> {
        self.admit(selector.family().len())?;
        InstrumentSelector::new(
            selector
                .family()
                .iter()
                .map(|s| self.sound(s))
                .collect::<Result<_, _>>()?,
        )
    }
}
fn fail(message: &str) -> Failure {
    Failure::new(FailCode::HostUnavailable, message)
}

pub(super) fn children(pattern: &Pat) -> Vec<&Pat> {
    use PatNode::*;
    match &pattern.node {
        Fast(p, _)
        | Slow(p, _)
        | Hurry(p, _)
        | Rev(p)
        | Every(_, _, p)
        | WhenMod(_, _, _, p)
        | SometimesBy(_, _, p)
        | DegradeBy(p, _)
        | Maybe(p, _)
        | Hold(p, _)
        | Repeat(p, _)
        | Superimpose(p, _)
        | Off(p, _, _)
        | Jux(p, _)
        | Iter(p, _)
        | Chop(p, _)
        | Ply(p, _)
        | Striate(p, _)
        | LoopAt(p, _)
        | Fit(p)
        | Chunk(p, _, _)
        | ScaleNotes(_, _, p)
        | Voicing(p)
        | Arp(p, _)
        | Segment(p, _)
        | Range(p, _, _)
        | Euclid(p, _, _, _)
        | MidiNotes { subject: p, .. } => vec![p],
        Grid(p, q)
        | Control(_, p, q)
        | Chord(p, q)
        | Slice {
            pat: p, index: q, ..
        }
        | Splice {
            pat: p, index: q, ..
        } => vec![p, q],
        Choose(ps) | Stack(ps) | Cat(ps) | FastCat(ps) => ps.iter().collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod spine_tests;

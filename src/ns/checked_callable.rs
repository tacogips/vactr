//! Weak declaration certification and bounded retained-reference traversal.
use super::{FormGen, SlotKind, VarSlot, VarSlotRef};
use crate::types::ty::CallableSchema;
use crate::value::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

#[derive(Clone)]
pub(crate) struct CheckedDependency {
    pub(super) slot: Weak<VarSlot>,
    version: u64,
    owner: Option<FormGen>,
    kind: SlotKind,
    pub(super) closure: Option<Weak<crate::compile::proto::Closure>>,
}
impl CheckedDependency {
    pub(super) fn capture(slot: &VarSlotRef) -> Self {
        Self {
            slot: Rc::downgrade(&slot.0),
            version: slot.version(),
            owner: slot.owner(),
            kind: slot.kind(),
            closure: match slot.get() {
                Value::Fn(f) => Some(Rc::downgrade(&f)),
                _ => None,
            },
        }
    }
    pub(super) fn valid(&self) -> bool {
        let Some(slot) = self.slot.upgrade() else {
            return false;
        };
        if !slot.bound.get()
            || slot.version.get() != self.version
            || slot.owner.get() != self.owner
            || slot.kind.get() != self.kind
        {
            return false;
        }
        if let Some(expected) = &self.closure {
            let Some(expected) = expected.upgrade() else {
                return false;
            };
            if !matches!(&*slot.value.borrow(), Value::Fn(actual) if Rc::ptr_eq(actual, &expected))
            {
                return false;
            }
        }
        true
    }
}
#[derive(Clone)]
pub(super) struct CheckedCallable {
    pub(super) own: CheckedDependency,
    pub(super) schema: CallableSchema,
    pub(super) dependencies: Vec<CheckedDependency>,
}
impl CheckedCallable {
    pub(super) fn valid(&self) -> bool {
        self.own.valid() && self.dependencies.iter().all(CheckedDependency::valid)
    }
}
pub(crate) type CheckedInputs = BTreeMap<u64, (CheckedDependency, Vec<CheckedDependency>)>;

use crate::compile::proto::{Closure, FnProto};
use crate::dsp::graph::{UGenInput, UGenNode};
use crate::pattern::pat::{PParam, Pat, PatNode, SliceCuts};
use crate::pattern::signal::Sig;
use crate::song::{Part, PartEdit, PartNode};
use crate::tex::texnode::{TexKind, TexNode, VParam};

const MAX_WORK: usize = 1_000_000;
const MAX_DEPTH: u32 = 256;
type WalkKey = (u8, usize, Vec<usize>);
/// Embedded pattern arrays stay borrowed through their owning Rc and an exact
/// index path. Copying a deeply owned Pat tree would recurse before admission.
enum Walk {
    Value(Value),
    Proto(Rc<FnProto>),
    Pattern(Rc<Pat>, Vec<usize>),
    Part(Rc<Part>),
    Signal(Rc<Sig>),
    Texture(Rc<TexNode>),
    Ugen(Rc<UGenNode>),
}
impl Walk {
    fn key(&self) -> Option<WalkKey> {
        let (tag, pointer) = match self {
            Self::Value(v) => match v {
                Value::VarRef(v) => (0, Rc::as_ptr(&v.0) as usize),
                Value::Fn(v) | Value::Thunk(v) => (1, Rc::as_ptr(v) as usize),
                Value::List(v) => (2, Rc::as_ptr(v) as usize),
                Value::Dict(v) => (3, Rc::as_ptr(v) as usize),
                Value::Struct(v) => (4, Rc::as_ptr(v) as usize),
                Value::Variant(v) => (5, Rc::as_ptr(v) as usize),
                Value::Pattern(v) => (6, Rc::as_ptr(v) as usize),
                Value::Part(v) => (7, Rc::as_ptr(v) as usize),
                Value::Song(v) => (8, Rc::as_ptr(v) as usize),
                Value::Signal(v) => (9, Rc::as_ptr(v) as usize),
                Value::Tex(v) => (10, Rc::as_ptr(v) as usize),
                Value::UGen(v) => (11, Rc::as_ptr(v) as usize),
                _ => return None,
            },
            Self::Proto(v) => (12, Rc::as_ptr(v) as usize),
            Self::Pattern(v, path) => return Some((13, Rc::as_ptr(v) as usize, path.clone())),
            Self::Part(v) => (14, Rc::as_ptr(v) as usize),
            Self::Signal(v) => (15, Rc::as_ptr(v) as usize),
            Self::Texture(v) => (16, Rc::as_ptr(v) as usize),
            Self::Ugen(v) => (17, Rc::as_ptr(v) as usize),
        };
        Some((tag, pointer, Vec::new()))
    }
}
enum Frame {
    Enter(Walk, u32),
    Finish(WalkKey, Vec<Option<WalkKey>>),
}
struct WalkState<'a> {
    remaining: usize,
    inputs: &'a CheckedInputs,
    own: u64,
    deps: BTreeMap<u64, CheckedDependency>,
}
impl WalkState<'_> {
    fn charge(&mut self, count: usize) -> Option<()> {
        self.remaining = self.remaining.checked_sub(count)?;
        Some(())
    }
    fn value(&mut self, children: &mut Vec<Walk>, v: &Value) -> Option<()> {
        self.charge(1)?;
        children.push(Walk::Value(v.clone()));
        Some(())
    }
    fn pattern(&mut self, children: &mut Vec<Walk>, p: &Rc<Pat>) -> Option<()> {
        self.charge(1)?;
        children.push(Walk::Pattern(p.clone(), Vec::new()));
        Some(())
    }
    fn param(&mut self, children: &mut Vec<Walk>, p: &PParam) -> Option<()> {
        match p {
            PParam::Const(v) | PParam::Fn(v) => self.value(children, v),
            PParam::Late(v) => self.value(children, &Value::VarRef(v.clone())),
            PParam::Pat(v) => self.pattern(children, v),
        }
    }
    fn cuts(&mut self, children: &mut Vec<Walk>, cuts: &SliceCuts) -> Option<()> {
        match cuts {
            SliceCuts::Equal(p) => self.param(children, p),
            SliceCuts::Manual(ps) => {
                self.charge(ps.len())?;
                for p in ps {
                    self.param(children, p)?;
                }
                Some(())
            }
        }
    }
    fn slot(&mut self, children: &mut Vec<Walk>, slot: &VarSlotRef) -> Option<()> {
        if slot.id() == self.own || slot.kind() == SlotKind::Prelude {
            return Some(());
        }
        let (stamp, inherited) = self.inputs.get(&slot.id())?;
        self.charge(inherited.len().checked_add(1)?)?;
        if !stamp.valid() || !inherited.iter().all(CheckedDependency::valid) {
            return None;
        }
        self.deps.insert(slot.id(), stamp.clone());
        for old in inherited {
            self.deps.insert(old.slot.upgrade()?.id, old.clone());
        }
        self.charge(1)?;
        children.push(Walk::Value(slot.get()));
        Some(())
    }
    fn children(&mut self, work: &Walk) -> Option<Vec<Walk>> {
        let mut c = Vec::new();
        match work {
            Walk::Value(v) => match v {
                Value::VarRef(slot) => self.slot(&mut c, slot)?,
                Value::Fn(f) | Value::Thunk(f) => {
                    self.charge(f.captures.len().checked_add(2)?)?;
                    c.push(Walk::Proto(f.proto.clone()));
                    for v in &f.captures {
                        self.value(&mut c, v)?;
                    }
                    if let Some(memo) = &f.memo {
                        if let Some(v) = &*memo.borrow() {
                            self.value(&mut c, v)?;
                        }
                    }
                }
                Value::List(v) => {
                    self.charge(v.items.len())?;
                    for v in &v.items {
                        self.value(&mut c, v)?;
                    }
                }
                Value::Dict(v) => {
                    self.charge(v.len())?;
                    for v in v.values() {
                        self.value(&mut c, v)?;
                    }
                }
                Value::Struct(v) => {
                    self.charge(v.fields.len())?;
                    for (_, v) in &v.fields {
                        self.value(&mut c, v)?;
                    }
                }
                Value::Variant(v) => {
                    self.charge(v.fields.len())?;
                    for (_, v) in &v.fields {
                        self.value(&mut c, v)?;
                    }
                }
                Value::Pattern(p) => {
                    self.charge(1)?;
                    c.push(Walk::Pattern(p.clone(), Vec::new()));
                }
                Value::Part(p) => {
                    self.charge(1)?;
                    c.push(Walk::Part(p.clone()));
                }
                Value::Song(s) => {
                    self.charge(1)?;
                    c.push(Walk::Part(s.part().clone()));
                }
                Value::Signal(s) => {
                    self.charge(1)?;
                    c.push(Walk::Signal(s.clone()));
                }
                Value::Tex(t) => {
                    self.charge(1)?;
                    c.push(Walk::Texture(t.clone()));
                }
                Value::UGen(u) => {
                    self.charge(1)?;
                    c.push(Walk::Ugen(u.clone()));
                }
                Value::Nil
                | Value::Bool(_)
                | Value::Int(_)
                | Value::Int64(_)
                | Value::Float(_)
                | Value::Float64(_)
                | Value::Ratio(_)
                | Value::Keyword(_)
                | Value::Str(_)
                | Value::Native(_)
                | Value::EventHandle(_)
                | Value::Inst(_)
                | Value::Range(_)
                | Value::Path(_)
                | Value::Url(_)
                | Value::Sound(_) => {}
            },
            Walk::Proto(p) => {
                self.charge(
                    p.globals
                        .len()
                        .checked_add(p.protos.len())?
                        .checked_add(p.consts.len())?,
                )?;
                for slot in &p.globals {
                    self.slot(&mut c, slot)?;
                }
                for proto in &p.protos {
                    c.push(Walk::Proto(proto.clone()));
                }
                for value in &p.consts {
                    self.value(&mut c, value)?;
                }
            }
            Walk::Pattern(root, path) => self.pat_children(&mut c, root, path)?,
            Walk::Part(p) => match p.node() {
                PartNode::Capture(tracks) => {
                    self.charge(tracks.len())?;
                    for p in tracks.values() {
                        self.pattern(&mut c, p)?;
                    }
                }
                PartNode::Sequence(parts) => {
                    self.charge(parts.len())?;
                    for p in parts {
                        c.push(Walk::Part(p.clone()));
                    }
                }
                PartNode::Repeat { child, .. } => {
                    self.charge(1)?;
                    c.push(Walk::Part(child.clone()));
                }
                PartNode::Edit { source, edit } => {
                    self.charge(1)?;
                    c.push(Walk::Part(source.clone()));
                    match edit {
                        PartEdit::ReplaceTrack { pattern, .. }
                        | PartEdit::TransformInstrument { pattern, .. }
                        | PartEdit::OverwriteRegion { pattern, .. } => {
                            self.pattern(&mut c, pattern)?
                        }
                        PartEdit::DeleteEvent(_) | PartEdit::InstrumentFx { .. } => {}
                    }
                }
            },
            Walk::Signal(s) => match &**s {
                Sig::Lag(inner, _) | Sig::MapRange(inner, _, _) => {
                    self.charge(1)?;
                    c.push(Walk::Signal(inner.clone()));
                }
                Sig::Sine
                | Sig::Saw
                | Sig::Tri
                | Sig::Square
                | Sig::Rand
                | Sig::Perlin
                | Sig::IRand(_)
                | Sig::Time
                | Sig::Beat
                | Sig::Phase
                | Sig::Cycle
                | Sig::Host(_)
                | Sig::Cc { .. }
                | Sig::Analyzer(_)
                | Sig::Ctrl(_, _)
                | Sig::Hits(_) => {}
            },
            Walk::Texture(t) => {
                self.charge(t.params.len().checked_add(2)?)?;
                for p in &t.params {
                    match p {
                        VParam::Const(_) => {}
                        VParam::Late(v) => self.slot(&mut c, v)?,
                        VParam::Fn(v) => self.value(&mut c, v)?,
                        VParam::Pat(v) => self.pattern(&mut c, v)?,
                        VParam::Sig(v) => c.push(Walk::Signal(v.clone())),
                    }
                }
                match &t.kind {
                    TexKind::Blend(_, t) | TexKind::Modulate(_, t) => {
                        c.push(Walk::Texture(t.clone()))
                    }
                    TexKind::Chain(a, b) => {
                        c.push(Walk::Texture(a.clone()));
                        c.push(Walk::Texture(b.clone()));
                    }
                    TexKind::Osc
                    | TexKind::Noise
                    | TexKind::Voronoi
                    | TexKind::Shape
                    | TexKind::Gradient
                    | TexKind::Solid
                    | TexKind::Text(_)
                    | TexKind::SrcOut(_)
                    | TexKind::Rotate
                    | TexKind::Scale
                    | TexKind::Pixelate
                    | TexKind::Tile
                    | TexKind::TileX
                    | TexKind::TileY
                    | TexKind::Kaleid
                    | TexKind::Scroll
                    | TexKind::Posterize
                    | TexKind::Shift
                    | TexKind::Invert
                    | TexKind::Contrast
                    | TexKind::Brightness
                    | TexKind::Luma
                    | TexKind::Thresh
                    | TexKind::Color
                    | TexKind::Saturate
                    | TexKind::Hue
                    | TexKind::Colorama => {}
                }
            }
            Walk::Ugen(u) => {
                self.charge(u.args.len())?;
                for (_, input) in &u.args {
                    match input {
                        UGenInput::Node(u) => c.push(Walk::Ugen(u.clone())),
                        UGenInput::Signal(s) => c.push(Walk::Signal(s.clone())),
                        UGenInput::Const(_)
                        | UGenInput::Param(_)
                        | UGenInput::Keyword(_)
                        | UGenInput::Sidechain(_)
                        | UGenInput::List(_) => {}
                    }
                }
            }
        }
        Some(c)
    }
    fn pat_children(&mut self, c: &mut Vec<Walk>, root: &Rc<Pat>, path: &[usize]) -> Option<()> {
        let mut pat = &**root;
        for &index in path {
            match &pat.node {
                PatNode::Choose(ps)
                | PatNode::Stack(ps)
                | PatNode::Cat(ps)
                | PatNode::FastCat(ps) => pat = ps.get(index)?,
                _ => return None,
            }
        }
        match &pat.node {
            PatNode::Steps(steps) => {
                self.charge(steps.len())?;
                for step in steps {
                    self.value(c, &step.value)?;
                }
            }
            PatNode::Pure(step) => self.value(c, &step.value)?,
            PatNode::Sound { src, kit } => {
                self.param(c, src)?;
                if let Some(p) = kit {
                    self.param(c, p)?;
                }
            }
            PatNode::Signal(s) => {
                self.charge(1)?;
                c.push(Walk::Signal(s.clone()));
            }
            PatNode::SongSource(s) => {
                self.charge(1)?;
                c.push(Walk::Part(s.part().clone()));
            }
            PatNode::Fast(p, x)
            | PatNode::Slow(p, x)
            | PatNode::Hurry(p, x)
            | PatNode::DegradeBy(p, x)
            | PatNode::Maybe(p, x)
            | PatNode::Hold(p, x)
            | PatNode::Repeat(p, x)
            | PatNode::Iter(p, x)
            | PatNode::Chop(p, x)
            | PatNode::Ply(p, x)
            | PatNode::Striate(p, x)
            | PatNode::LoopAt(p, x)
            | PatNode::Arp(p, x)
            | PatNode::Segment(p, x) => {
                self.pattern(c, p)?;
                self.param(c, x)?;
            }
            PatNode::Rev(p)
            | PatNode::Fit(p)
            | PatNode::Voicing(p)
            | PatNode::ScaleNotes(_, _, p)
            | PatNode::MidiNotes { subject: p, .. } => self.pattern(c, p)?,
            PatNode::Every(x, f, p) | PatNode::SometimesBy(x, f, p) => {
                self.param(c, x)?;
                self.value(c, f)?;
                self.pattern(c, p)?;
            }
            PatNode::WhenMod(a, b, f, p) => {
                self.param(c, a)?;
                self.param(c, b)?;
                self.value(c, f)?;
                self.pattern(c, p)?;
            }
            PatNode::Choose(ps) | PatNode::Stack(ps) | PatNode::Cat(ps) | PatNode::FastCat(ps) => {
                self.charge(ps.len().checked_mul(path.len().checked_add(2)?)?)?;
                for index in 0..ps.len() {
                    let mut next = path.to_vec();
                    next.push(index);
                    c.push(Walk::Pattern(root.clone(), next));
                }
            }
            PatNode::Superimpose(p, f) | PatNode::Jux(p, f) => {
                self.pattern(c, p)?;
                self.value(c, f)?;
            }
            PatNode::Off(p, x, f) | PatNode::Chunk(p, x, f) => {
                self.pattern(c, p)?;
                self.param(c, x)?;
                self.value(c, f)?;
            }
            PatNode::Slice { pat, cuts, index } | PatNode::Splice { pat, cuts, index } => {
                self.pattern(c, pat)?;
                self.cuts(c, cuts)?;
                self.pattern(c, index)?;
            }
            PatNode::Grid(a, b) | PatNode::Control(_, a, b) | PatNode::Chord(a, b) => {
                self.pattern(c, a)?;
                self.pattern(c, b)?;
            }
            PatNode::Euclid(p, a, b, r) => {
                self.pattern(c, p)?;
                self.param(c, a)?;
                self.param(c, b)?;
                self.param(c, r)?;
            }
            PatNode::Range(p, a, b) => {
                self.pattern(c, p)?;
                self.param(c, a)?;
                self.param(c, b)?;
            }
        }
        Some(())
    }
}

pub(super) fn callable_dependencies(
    closure: &Rc<Closure>,
    inputs: &CheckedInputs,
    own: u64,
) -> Option<Vec<CheckedDependency>> {
    let mut state = WalkState {
        remaining: MAX_WORK,
        inputs,
        own,
        deps: BTreeMap::new(),
    };
    let mut pending = vec![Frame::Enter(Walk::Value(Value::Fn(closure.clone())), 0)];
    let mut active = BTreeSet::new();
    let mut heights: BTreeMap<WalkKey, u32> = BTreeMap::new();
    while let Some(frame) = pending.pop() {
        state.charge(1)?;
        match frame {
            Frame::Enter(work, depth) => {
                if depth >= MAX_DEPTH {
                    return None;
                }
                if let Walk::Pattern(_, path) = &work {
                    state.charge(path.len())?;
                }
                let Some(key) = work.key() else {
                    continue;
                };
                if let Some(height) = heights.get(&key) {
                    if depth.checked_add(*height)? >= MAX_DEPTH {
                        return None;
                    }
                    continue;
                }
                // A retained mutable reference cycle cannot prove a finite type
                // dependency graph; no partial certificate is installed.
                state.charge(key.2.len())?;
                if !active.insert(key.clone()) {
                    return None;
                }
                let children = state.children(&work)?;
                state.charge(children.len().checked_mul(2)?)?;
                for child in &children {
                    if let Walk::Pattern(_, path) = child {
                        state.charge(path.len())?;
                    }
                }
                let keys = children.iter().map(Walk::key).collect();
                pending.push(Frame::Finish(key, keys));
                for child in children.into_iter().rev() {
                    pending.push(Frame::Enter(child, depth.checked_add(1)?));
                }
            }
            Frame::Finish(key, children) => {
                let mut height = 0;
                for child in children {
                    let child_height = match child {
                        Some(child) => *heights.get(&child)?,
                        None => 0,
                    };
                    height = height.max(child_height.checked_add(1)?);
                }
                active.remove(&key);
                heights.insert(key, height);
            }
        }
    }
    Some(state.deps.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ns::{evaluator::Evaluator, namespace::Prelude, stage::RecordingSink};
    use crate::reader::span::FileId;
    use crate::song::{capture_part, InstrumentSelector, Song, SongSettings, SongSource};
    use crate::value::{
        intern::{intern_kw, intern_sym},
        ratio::Ratio64,
    };
    fn evaluator() -> Evaluator {
        Evaluator::new(
            Prelude::core(),
            Box::new(crate::host::noop::NoopHost),
            Box::new(RecordingSink::default()),
        )
    }
    fn closure(ev: &mut Evaluator, value: Value) -> Rc<Closure> {
        let outcomes = ev
            .eval_str("var cell 1\nfn basis p:\n\tfirst [p]", FileId::new(1))
            .unwrap();
        assert!(outcomes
            .iter()
            .all(|o| o.value.is_ok() && o.diags.is_empty()));
        let Value::Fn(f) = ev.ns().session_value("basis").unwrap() else {
            panic!()
        };
        Rc::new(Closure {
            proto: f.proto.clone(),
            captures: vec![value].into(),
            mask: f.mask.clone(),
            memo: None,
        })
    }
    #[test]
    fn captured_symbolic_parts_songs_and_selected_sources_keep_weak_cells() {
        let mut ev = evaluator();
        let initial = closure(&mut ev, Value::Nil);
        let cell = ev.ns().session_slot(intern_sym("cell")).unwrap();
        let pat = Rc::new(Pat::new(
            PatNode::Fast(Rc::new(Pat::silence()), PParam::Late(cell.clone())),
            None,
            true,
        ));
        let track = intern_kw("voice");
        let part =
            Rc::new(capture_part(BTreeMap::from([(track, pat.clone())]), Ratio64::ONE).unwrap());
        let sound = crate::value::value::Sound::Builtin(intern_kw("analog"));
        let selected = Rc::new(
            SongSource::new(
                part.clone(),
                track,
                InstrumentSelector::new(vec![sound]).unwrap(),
            )
            .unwrap(),
        );
        let values = [
            Value::Pattern(pat),
            Value::Part(part.clone()),
            Value::Song(Rc::new(Song::new(part, SongSettings::default()).unwrap())),
            Value::Pattern(Rc::new(Pat::new(PatNode::SongSource(selected), None, true))),
        ];
        let inputs = ev.ns().checked_inputs().unwrap();
        for value in values {
            let f = Rc::new(Closure {
                proto: initial.proto.clone(),
                captures: vec![value].into(),
                mask: initial.mask.clone(),
                memo: None,
            });
            let references = Rc::strong_count(&cell.0);
            let deps = callable_dependencies(&f, &inputs, 0).expect("retained graph certified");
            assert_eq!(deps.len(), 1);
            assert!(deps[0].valid());
            let value = cell.get();
            let version = cell.version();
            cell.set(Value::str("changed"));
            assert!(!deps[0].valid());
            cell.restore(value, version);
            assert!(deps[0].valid());
            assert_eq!(
                Rc::strong_count(&cell.0),
                references,
                "weak certificate must not retain extra slot references"
            );
        }
    }
    fn chain(mut pat: Rc<Pat>, count: usize) -> Rc<Pat> {
        for _ in 0..count {
            pat = Rc::new(Pat::new(PatNode::Rev(pat), None, true));
        }
        pat
    }
    #[test]
    fn deeper_shared_alias_validates_completed_subgraph_height() {
        for (depth, ok) in [(200, true), (250, false)] {
            let mut ev = evaluator();
            let shared = chain(Rc::new(Pat::silence()), 20);
            let pat = Rc::new(Pat::new(
                PatNode::Grid(shared.clone(), chain(shared, depth)),
                None,
                true,
            ));
            let f = closure(&mut ev, Value::Pattern(pat));
            assert_eq!(
                callable_dependencies(&f, &ev.ns().checked_inputs().unwrap(), 0).is_some(),
                ok
            );
        }
    }
    #[test]
    fn owned_pattern_arrays_are_borrowed_and_wide_children_admit_before_clone() {
        let mut pat = Pat::silence();
        for _ in 0..200 {
            pat = Pat::new(PatNode::Stack(vec![pat].into()), None, true);
        }
        let mut ev = evaluator();
        let f = closure(&mut ev, Value::Pattern(Rc::new(pat)));
        assert!(callable_dependencies(&f, &ev.ns().checked_inputs().unwrap(), 0).is_some());
        let inputs = BTreeMap::new();
        let mut state = WalkState {
            remaining: 8,
            inputs: &inputs,
            own: 0,
            deps: BTreeMap::new(),
        };
        let work = Walk::Value(Value::list(vec![Value::Int(0); 1000]));
        assert!(state.children(&work).is_none());
    }
}

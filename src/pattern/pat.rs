//! Patterns as inspectable node trees (design 10.1).

use std::rc::Rc;

use crate::ns::namespace::VarSlotRef;
use crate::pattern::rng::Hasher;
use crate::pattern::signal::Sig;
use crate::pattern::step::Step;
use crate::reader::span::{NodeId, Span};
use crate::value::intern::{name_of_kw, KwId};
use crate::value::value::Value;

/// A pattern: a lazy, infinite function of time to events.
///
/// `structured` is set at construction (10.1 "Sound first"): false for a
/// single sound, a scalar or a signal. It is read only by the
/// structure-giving steps. `id` is a structural hash of the node, so a
/// rebuilt identical pattern keeps its random draws and occurrence keys;
/// it is the `NodeId` of 10.2/10.3 (the design sketch leaves the field
/// implicit).
#[derive(Clone, Debug)]
pub struct Pat {
    pub node: PatNode,
    pub span: Option<Span>,
    pub structured: bool,
    pub id: NodeId,
}

/// A number, a late-bound var, a function of time or a pattern. Every
/// numeric combinator parameter is a `PParam` (10.1).
#[derive(Clone, Debug)]
pub enum PParam {
    Const(Value),
    Late(VarSlotRef),
    Fn(Value),
    Pat(Rc<Pat>),
}

/// How a sample is cut: `Equal(count)` or manual slice starts in `[0, 1]`.
#[derive(Clone, Debug)]
pub enum SliceCuts {
    Equal(PParam),
    Manual(Box<[PParam]>),
}

/// The pattern node kinds (design 10.1). `Hurry`, `Maybe`, `Choose`, `Hold`
/// and `Repeat` are the step-level constructors the design names in prose;
/// `alt` builds `Cat`. `Pure` is the atomic one-value-per-cycle node (a
/// scalar, one sound, or a chord list that must not subdivide).
#[derive(Clone, Debug)]
pub enum PatNode {
    /// One cycle; a nested list subdivides its step; `nil` is a rest.
    Steps(Box<[Step]>),
    /// One atomic value per cycle (a list stays one value, e.g. a chord).
    Pure(Step),
    /// Every `s` call; `kit` is the `kit:` argument when given.
    Sound {
        src: PParam,
        kit: Option<PParam>,
    },
    Signal(Rc<Sig>),
    Fast(Rc<Pat>, PParam),
    Slow(Rc<Pat>, PParam),
    /// `fast` that also multiplies `speed`.
    Hurry(Rc<Pat>, PParam),
    Rev(Rc<Pat>),
    Every(PParam, Value, Rc<Pat>),
    WhenMod(PParam, PParam, Value, Rc<Pat>),
    SometimesBy(PParam, Value, Rc<Pat>),
    DegradeBy(Rc<Pat>, PParam),
    /// Keeps each event with the given probability (`maybe`, default 0.5).
    Maybe(Rc<Pat>, PParam),
    /// One child per cycle, chosen by the hash RNG.
    Choose(Box<[Pat]>),
    /// A step weight inside a step list (`hold :bd 3`).
    Hold(Rc<Pat>, PParam),
    /// A step replicated n times inside a step list (`repeat :bd 2`).
    Repeat(Rc<Pat>, PParam),
    Stack(Box<[Pat]>),
    Cat(Box<[Pat]>),
    FastCat(Box<[Pat]>),
    Superimpose(Rc<Pat>, Value),
    Off(Rc<Pat>, PParam, Value),
    Jux(Rc<Pat>, Value),
    Iter(Rc<Pat>, PParam),
    Chop(Rc<Pat>, PParam),
    Ply(Rc<Pat>, PParam),
    Striate(Rc<Pat>, PParam),
    Slice {
        pat: Rc<Pat>,
        cuts: SliceCuts,
        index: Rc<Pat>,
    },
    Splice {
        pat: Rc<Pat>,
        cuts: SliceCuts,
        index: Rc<Pat>,
    },
    LoopAt(Rc<Pat>, PParam),
    Fit(Rc<Pat>),
    Chunk(Rc<Pat>, PParam, Value),
    /// `grid subject bools` (Tidal's `struct`, 20 Q3).
    Grid(Rc<Pat>, Rc<Pat>),
    /// `euclid subject pulses steps rotation`.
    Euclid(Rc<Pat>, PParam, PParam, PParam),
    /// `Control(name, value pattern, subject)`.
    Control(KwId, Rc<Pat>, Rc<Pat>),
    /// `ScaleNotes(root, scale name, subject)`.
    ScaleNotes(KwId, KwId, Rc<Pat>),
    /// `Chord(chord value pattern, subject)`: sets `note` to the chord tones.
    Chord(Rc<Pat>, Rc<Pat>),
    Voicing(Rc<Pat>),
    /// `Arp(subject, mode)`.
    Arp(Rc<Pat>, PParam),
    Segment(Rc<Pat>, PParam),
    Range(Rc<Pat>, PParam, PParam),
    /// Live MIDI note input after `s` (11.7): no events under `query`.
    MidiNotes {
        subject: Rc<Pat>,
        channel: Option<u8>,
    },
}

impl Pat {
    /// A pattern node with its structural id.
    #[must_use]
    pub fn new(node: PatNode, span: Option<Span>, structured: bool) -> Self {
        let id = structural_id(&node, span);
        Self {
            node,
            span,
            structured,
            id,
        }
    }

    /// The same pattern with a source span (the id is recomputed).
    #[must_use]
    pub fn with_span(self, span: Option<Span>) -> Self {
        Self::new(self.node, span, self.structured)
    }

    /// The empty pattern.
    #[must_use]
    pub fn silence() -> Self {
        Self::new(PatNode::Steps(Box::new([])), None, true)
    }
}

impl PParam {
    /// True for a constant, which is the same at every time.
    #[must_use]
    pub const fn is_const(&self) -> bool {
        matches!(self, PParam::Const(_))
    }

    /// An integer constant.
    #[must_use]
    pub const fn int(n: i32) -> Self {
        PParam::Const(Value::Int(n))
    }
}

/// Feeds a value into a hash: scalars by content, lists recursively (bounded),
/// everything else by its tag.
pub(crate) fn hash_value(h: Hasher, v: &Value, depth: u32) -> Hasher {
    match v {
        Value::Nil => h.word(1),
        Value::Bool(b) => h.word(2).word(u64::from(*b)),
        Value::Int(i) => h.word(3).int(i64::from(*i)),
        Value::Int64(i) => h.word(3).int(*i),
        Value::Float(x) => h.word(4).word(f64::from(*x).to_bits()),
        Value::Float64(x) => h.word(4).word(x.to_bits()),
        Value::Ratio(r) => h.word(5).ratio(*r),
        Value::Keyword(k) => h.word(6).text(&name_of_kw(*k)),
        Value::Str(s) => h.word(7).text(s),
        Value::Url(s) => h.word(8).text(s),
        Value::Path(p) => h.word(9).text(&p.text),
        Value::List(l) if depth < 8 => {
            let mut h = h.word(10).word(l.items.len() as u64);
            for item in l.items.iter() {
                h = hash_value(h, item, depth + 1);
            }
            h
        }
        Value::Pattern(p) => h.word(11).word(u64::from(p.id.get())),
        _ => h.word(12),
    }
}

fn hash_param(h: Hasher, p: &PParam) -> Hasher {
    match p {
        PParam::Const(v) => hash_value(h.word(20), v, 0),
        PParam::Late(_) => h.word(21),
        PParam::Fn(v) => hash_value(h.word(22), v, 0),
        PParam::Pat(p) => h.word(23).word(u64::from(p.id.get())),
    }
}

fn hash_pat(h: Hasher, p: &Pat) -> Hasher {
    h.word(u64::from(p.id.get()))
}

fn structural_id(node: &PatNode, span: Option<Span>) -> NodeId {
    let mut h = Hasher::new(0x0070_6174);
    if let Some(s) = span {
        h = h
            .word(u64::from(s.file.get()))
            .word(u64::from(s.start))
            .word(u64::from(s.end));
    }
    h = match node {
        PatNode::Steps(steps) => {
            let mut h = h.word(1).word(steps.len() as u64);
            for s in steps.iter() {
                h = hash_value(h, &s.value, 0);
            }
            h
        }
        PatNode::Pure(s) => hash_value(h.word(41), &s.value, 0),
        PatNode::Sound { src, kit } => {
            let h = hash_param(h.word(2), src);
            match kit {
                Some(k) => hash_param(h.word(1), k),
                None => h.word(0),
            }
        }
        PatNode::Signal(s) => h.word(3).text(&format!("{s:?}")),
        PatNode::Fast(p, k) => hash_param(hash_pat(h.word(4), p), k),
        PatNode::Slow(p, k) => hash_param(hash_pat(h.word(5), p), k),
        PatNode::Hurry(p, k) => hash_param(hash_pat(h.word(6), p), k),
        PatNode::Rev(p) => hash_pat(h.word(7), p),
        PatNode::Every(n, f, p) => hash_pat(hash_value(hash_param(h.word(8), n), f, 0), p),
        PatNode::WhenMod(a, b, f, p) => {
            hash_pat(hash_value(hash_param(hash_param(h.word(9), a), b), f, 0), p)
        }
        PatNode::SometimesBy(x, f, p) => hash_pat(hash_value(hash_param(h.word(10), x), f, 0), p),
        PatNode::DegradeBy(p, x) => hash_param(hash_pat(h.word(11), p), x),
        PatNode::Maybe(p, x) => hash_param(hash_pat(h.word(12), p), x),
        PatNode::Choose(ps) => ps.iter().fold(h.word(13), hash_pat),
        PatNode::Hold(p, x) => hash_param(hash_pat(h.word(14), p), x),
        PatNode::Repeat(p, x) => hash_param(hash_pat(h.word(15), p), x),
        PatNode::Stack(ps) => ps.iter().fold(h.word(16), hash_pat),
        PatNode::Cat(ps) => ps.iter().fold(h.word(17), hash_pat),
        PatNode::FastCat(ps) => ps.iter().fold(h.word(18), hash_pat),
        PatNode::Superimpose(p, f) => hash_value(hash_pat(h.word(19), p), f, 0),
        PatNode::Off(p, t, f) => hash_value(hash_param(hash_pat(h.word(20), p), t), f, 0),
        PatNode::Jux(p, f) => hash_value(hash_pat(h.word(21), p), f, 0),
        PatNode::Iter(p, x) => hash_param(hash_pat(h.word(22), p), x),
        PatNode::Chop(p, x) => hash_param(hash_pat(h.word(23), p), x),
        PatNode::Ply(p, x) => hash_param(hash_pat(h.word(24), p), x),
        PatNode::Striate(p, x) => hash_param(hash_pat(h.word(25), p), x),
        PatNode::Slice { pat, cuts, index } => {
            hash_pat(hash_cuts(hash_pat(h.word(26), pat), cuts), index)
        }
        PatNode::Splice { pat, cuts, index } => {
            hash_pat(hash_cuts(hash_pat(h.word(27), pat), cuts), index)
        }
        PatNode::LoopAt(p, x) => hash_param(hash_pat(h.word(28), p), x),
        PatNode::Fit(p) => hash_pat(h.word(29), p),
        PatNode::Chunk(p, x, f) => hash_value(hash_param(hash_pat(h.word(30), p), x), f, 0),
        PatNode::Grid(p, b) => hash_pat(hash_pat(h.word(31), p), b),
        PatNode::Euclid(p, k, n, r) => {
            hash_param(hash_param(hash_param(hash_pat(h.word(32), p), k), n), r)
        }
        PatNode::Control(k, v, p) => hash_pat(hash_pat(h.word(33).text(&name_of_kw(*k)), v), p),
        PatNode::ScaleNotes(r, s, p) => {
            hash_pat(h.word(34).text(&name_of_kw(*r)).text(&name_of_kw(*s)), p)
        }
        PatNode::Chord(c, p) => hash_pat(hash_pat(h.word(35), c), p),
        PatNode::Voicing(p) => hash_pat(h.word(36), p),
        PatNode::Arp(p, m) => hash_param(hash_pat(h.word(37), p), m),
        PatNode::Segment(p, n) => hash_param(hash_pat(h.word(38), p), n),
        PatNode::Range(p, lo, hi) => hash_param(hash_param(hash_pat(h.word(39), p), lo), hi),
        PatNode::MidiNotes { subject, channel } => {
            hash_pat(h.word(40), subject).word(channel.map_or(0, |c| u64::from(c) + 1))
        }
    };
    // Node ids are u32 (6.5.1); the fold keeps the high bits' entropy.
    let v = h.finish();
    NodeId::new((v ^ (v >> 32)) as u32)
}

fn hash_cuts(h: Hasher, cuts: &SliceCuts) -> Hasher {
    match cuts {
        SliceCuts::Equal(n) => hash_param(h.word(1), n),
        SliceCuts::Manual(ps) => ps.iter().fold(h.word(2), hash_param),
    }
}

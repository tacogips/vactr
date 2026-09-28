//! Voices (design 12.2, 11.3, 11.7).
//!
//! A voice is an instantiated template: per-node scalar state for
//! `NODE_CAP` nodes, per-voice effect units and a delay-memory region, all
//! allocated once in `VoicePool::new` (capacity = `max_voices`). Starting a
//! voice only resets and fills that storage.
//!
//! Controls: every template control takes the event's value when the event
//! carries it, else the template default; `Ctl::Cell` is read at voice start
//! (a late-bound control), except a template-default cell, which is re-read
//! every block (a signal-driven control-rate input, 12.8.6). When the
//! template does not read `amp`/`pan` the voice applies the event's `amp`
//! (1 when absent) and `pan` (0.5) itself, SuperDirt style, and the orbit
//! controls `lpf hpf resonance crush shape vowel` run as per-voice post
//! effects.
//!
//! Lifetime: a scheduled voice holds its gate for `legato` seconds when the
//! event carries it, else for `attack + decay`; an open (live-input) voice
//! holds it until released. A voice ends when its envelopes finish, else
//! when its one-shot sample players finish, else after an implicit
//! `release`-second fade once the gate closes. A short gate is a 3 ms
//! linear fade (12.8.9).

use crate::dsp::arena::SampleStore;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellRead;
use crate::dsp::effects::prim::{Biquad, Shape};
use crate::dsp::effects::{FxCtx, FxUnit};
use crate::dsp::graph::NODE_CAP;
use crate::dsp::ugen::{
    self, Inp, Kx, Node, NodeState, Src, Template, AMP, BANK, MAX_PARAMS, MAX_PORTS, MAX_VOICE_FX,
    PAN,
};
use crate::host::wire::{AudioEvent, Ctl, Release, SlotControl, VoiceTag};
use crate::sched::slots::{CtlId, SlotId};

/// The short gate, seconds (12.8.9).
pub const SHORT_GATE: f32 = 0.003;

const ATTACK: CtlId = CtlId::new(9);
const DECAY: CtlId = CtlId::new(10);
const RELEASE: CtlId = CtlId::new(12);
const LPF: CtlId = CtlId::new(35);
const HPF: CtlId = CtlId::new(36);
const RESONANCE: CtlId = CtlId::new(37);
const DELAY: CtlId = CtlId::new(38);
const CRUSH: CtlId = CtlId::new(41);
const SHAPE: CtlId = CtlId::new(42);
const VOWEL: CtlId = CtlId::new(43);
/// `orbit` (scheduler route; honored when an event carries it).
pub const ORBIT: CtlId = CtlId::new(46);
/// `bus` (scheduler route; the event carries the bus id).
pub const BUS: CtlId = CtlId::new(47);
const LEGATO: CtlId = CtlId::new(49);

/// Formants of `a e i o u` (Hz).
const FORMANTS: [[f32; 3]; 5] = [
    [800.0, 1150.0, 2900.0],
    [350.0, 2000.0, 2800.0],
    [270.0, 2140.0, 2950.0],
    [450.0, 800.0, 2830.0],
    [325.0, 700.0, 2700.0],
];

/// The value of control `id` in an event (the last entry wins).
#[must_use]
pub fn event_ctl(ev: &AudioEvent, id: CtlId) -> Option<Ctl> {
    ev.controls()
        .iter()
        .rev()
        .find(|(c, _)| *c == id)
        .map(|(_, c)| *c)
}

/// A control's value: a constant, or the cell's current value.
pub fn resolve<C: CellRead + ?Sized>(c: Ctl, cells: &C) -> f32 {
    match c {
        Ctl::Const(v) => v,
        Ctl::Cell(id) => cells.get(id),
    }
}

/// The per-voice orbit effects (`lpf hpf resonance crush shape vowel`).
#[derive(Clone, Copy, Default, Debug)]
pub struct PostFx {
    lpf: Option<f32>,
    hpf: Option<f32>,
    res: f32,
    crush: Option<f32>,
    shape: Option<f32>,
    vowel: Option<usize>,
    bq: [Biquad; 5],
}

impl PostFx {
    fn from_event<C: CellRead + ?Sized>(ev: &AudioEvent, cells: &C) -> Self {
        let get = |id| event_ctl(ev, id).map(|c| resolve(c, cells));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Self {
            lpf: get(LPF).filter(|f| *f < 20_000.0),
            hpf: get(HPF).filter(|f| *f > 20.0),
            res: get(RESONANCE).unwrap_or(0.0).clamp(0.0, 1.0),
            crush: get(CRUSH).filter(|c| *c < 16.0).map(|c| c.max(1.0)),
            shape: get(SHAPE).filter(|s| *s > 0.0).map(|s| s.min(0.99)),
            vowel: get(VOWEL).map(|v| (v.max(0.0) as usize).min(4)),
            bq: [Biquad::identity(); 5],
        }
    }

    fn active(&self) -> bool {
        self.lpf.is_some()
            || self.hpf.is_some()
            || self.crush.is_some()
            || self.shape.is_some()
            || self.vowel.is_some()
    }

    fn run(&mut self, y: &mut [f32], sr: f32) {
        if !self.active() {
            return;
        }
        let q = 0.707 + 9.0 * self.res;
        if let Some(f) = self.lpf {
            self.bq[0].set(Shape::Lowpass, f, q, 0.0, sr);
            self.bq[0].run_buf(y);
        }
        if let Some(f) = self.hpf {
            self.bq[1].set(Shape::Highpass, f, q, 0.0, sr);
            self.bq[1].run_buf(y);
        }
        if let Some(v) = self.vowel {
            for x in y.iter_mut() {
                let dry = *x;
                let mut acc = 0.0;
                for (k, f) in FORMANTS[v].iter().enumerate() {
                    self.bq[2 + k].set(Shape::Bandpass, *f, 8.0, 0.0, sr);
                    acc += self.bq[2 + k].run(dry);
                }
                *x = acc;
            }
        }
        if let Some(s) = self.shape {
            let k = 2.0 * s / (1.0 - s);
            for x in y.iter_mut() {
                *x = (1.0 + k) * *x / (1.0 + k * x.abs());
            }
        }
        if let Some(c) = self.crush {
            let levels = 2f32.powf(c - 1.0);
            for x in y.iter_mut() {
                *x = (*x * levels).round() / levels;
            }
        }
    }
}

/// One voice.
#[derive(Debug)]
pub struct Voice {
    pub active: bool,
    /// The template slot it plays.
    pub tmpl: usize,
    pub slot: SlotId,
    pub gen: u32,
    pub tag: Option<VoiceTag>,
    /// Start time, seconds on the audio timebase.
    pub start: f64,
    /// Frames to wait in the first block.
    pub delay: usize,
    /// A live-input voice (no scheduled end).
    pub open: bool,
    /// Scheduled gate frames left (`usize::MAX` while held open).
    pub gate_left: usize,
    /// Short-gate frames left and total, when gating.
    pub fade: Option<(u32, u32)>,
    /// The implicit release level (templates without envelopes).
    ienv: f32,
    /// Bus slot and orbit.
    pub bus: usize,
    pub orbit: usize,
    pub delay_send: f32,
    pub amp: f32,
    pub pan: f32,
    pub bank: Option<u32>,
    release: f32,
    pvals: [f32; MAX_PARAMS],
    /// Template-default cells re-read every block.
    pcell: [Option<crate::dsp::cells::CellId>; MAX_PARAMS],
    post: PostFx,
    post_r: PostFx,
    pub seed: u32,
    nodes: Box<[NodeState]>,
    fx: Box<[FxUnit]>,
    mem: Box<[f32]>,
}

impl Voice {
    fn new(voice_mem: usize) -> Self {
        Self {
            active: false,
            tmpl: 0,
            slot: SlotId::new(0),
            gen: 0,
            tag: None,
            start: 0.0,
            delay: 0,
            open: false,
            gate_left: 0,
            fade: None,
            ienv: 1.0,
            bus: 0,
            orbit: 0,
            delay_send: 0.0,
            amp: 1.0,
            pan: 0.5,
            bank: None,
            release: 0.1,
            pvals: [0.0; MAX_PARAMS],
            pcell: [None; MAX_PARAMS],
            post: PostFx::default(),
            post_r: PostFx::default(),
            seed: 0,
            nodes: vec![NodeState::default(); NODE_CAP].into_boxed_slice(),
            fx: vec![FxUnit::empty(); MAX_VOICE_FX].into_boxed_slice(),
            mem: vec![0.0; voice_mem].into_boxed_slice(),
        }
    }

    /// Floats of delay memory this voice owns.
    #[must_use]
    pub fn mem_len(&self) -> usize {
        self.mem.len()
    }

    /// The value of template control `k` (for tests and probes).
    #[must_use]
    pub fn param(&self, k: usize) -> f32 {
        self.pvals.get(k).copied().unwrap_or(0.0)
    }

    /// True once the gate closed (released or scheduled end reached).
    #[must_use]
    pub fn released(&self) -> bool {
        self.gate_left == 0
    }

    /// Closes the gate: the envelopes enter their release stage.
    pub fn release(&mut self) {
        self.gate_left = 0;
    }

    /// Starts a 3 ms short gate (no-op when already gating).
    pub fn short_gate(&mut self, sr: f32) {
        if self.fade.is_none() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = (SHORT_GATE * sr).max(1.0) as u32;
            self.fade = Some((n, n));
        }
        self.gate_left = 0;
    }

    /// Starts the voice for `ev` on template `t` (no allocation).
    #[allow(clippy::too_many_arguments)]
    pub fn start<C: CellRead + ?Sized>(
        &mut self,
        t: &Template,
        tmpl: usize,
        ev: &AudioEvent,
        tag: Option<VoiceTag>,
        delay: usize,
        start: f64,
        bus: usize,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
        seed: u32,
    ) {
        self.active = true;
        self.tmpl = tmpl;
        self.slot = ev.slot;
        self.gen = ev.gen;
        self.tag = tag;
        self.open = tag.is_some();
        self.start = start;
        self.delay = delay;
        self.fade = None;
        self.ienv = 1.0;
        self.seed = seed;
        self.bus = bus;
        for (k, &(id, default)) in t.params().iter().enumerate() {
            let src = event_ctl(ev, id).unwrap_or(default);
            self.pvals[k] = resolve(src, cells);
            self.pcell[k] = match (event_ctl(ev, id), default) {
                (None, Ctl::Cell(c)) => Some(c),
                _ => None,
            };
        }
        let get = |id| event_ctl(ev, id).map(|c| resolve(c, cells));
        self.amp = if t.reads_amp {
            1.0
        } else {
            get(AMP).unwrap_or(1.0)
        };
        self.pan = if t.reads_pan {
            0.5
        } else {
            get(PAN).unwrap_or(0.5)
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            self.bank = get(BANK).filter(|b| *b >= 0.0).map(|b| b as u32);
            self.orbit = get(ORBIT).unwrap_or(0.0).max(0.0) as usize;
        }
        self.delay_send = get(DELAY).unwrap_or(0.0).clamp(0.0, 1.0);
        self.release = get(RELEASE).unwrap_or(0.1).max(1.0e-3);
        self.post = PostFx::from_event(ev, cells);
        self.post_r = self.post;
        self.gate_left = if self.open {
            usize::MAX
        } else {
            let hold = get(LEGATO)
                .unwrap_or(0.0)
                .max(get(ATTACK).unwrap_or(0.01).max(0.0) + get(DECAY).unwrap_or(0.1).max(0.0));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let frames = (hold * sr).max(1.0) as usize;
            frames
        };
        let n = t.n_nodes;
        self.nodes[..n].fill(NodeState::default());
        let mem_len = self.mem.len();
        self.mem[..t.mem_total.min(mem_len)].fill(0.0);
        for spec in t.nodes() {
            if let Node::Effect { kind, fx } = spec.node {
                let fx = usize::from(fx);
                let (off, len) = region(spec, mem_len);
                let params = &t.fx_params[fx][..usize::from(t.fx_n[fx])];
                self.fx[fx].configure(kind, params, cells, &mut self.mem[off..off + len], sr, caps);
            }
        }
    }

    /// True when a resource is in use by this voice.
    #[must_use]
    pub fn uses(&self, t: &Template, resource: u32) -> bool {
        self.active
            && (t.resource == resource
                || self.bank == Some(resource)
                || t.refs().contains(&resource))
    }

    fn finished(&self, t: &Template) -> bool {
        let nodes = &self.nodes[..t.n_nodes];
        if t.envs > 0 {
            return t
                .nodes()
                .iter()
                .zip(nodes)
                .filter(|(s, _)| s.node.is_env())
                .all(|(_, st)| st.done());
        }
        if t.players > 0 {
            return t
                .nodes()
                .iter()
                .zip(nodes)
                .filter(|(s, _)| matches!(s.node, Node::SamplePlay(_)))
                .all(|(_, st)| st.done());
        }
        self.ienv <= 0.0
    }
}

fn region(spec: &ugen::NodeSpec, mem_len: usize) -> (usize, usize) {
    let off = (spec.mem_off as usize).min(mem_len);
    let len = (spec.mem_len as usize).min(mem_len - off);
    (off, len)
}

/// Shared buffers and context for rendering voices in one block.
pub struct RenderCtx<'a, C: CellRead + ?Sized> {
    pub sr: f32,
    pub max_block: usize,
    /// `NODE_CAP * max_block` node output buffers.
    pub bufs: &'a mut [f32],
    /// `max_block` floats: the voice output.
    pub out: &'a mut [f32],
    /// `max_block` floats: the separately routed auxiliary output.
    pub out_r: &'a mut [f32],
    /// Direct, independently addressable channels three and four.
    pub out_3: &'a mut [f32],
    pub out_4: &'a mut [f32],
    /// `2 * max_block` floats: effect-node right channel and dry scratch.
    pub tmp: &'a mut [f32],
    pub dry: &'a mut [f32],
    pub cells: &'a C,
    pub store: &'a SampleStore,
    /// Validated interleaved stereo host input for this engine block.
    pub host_input: Option<&'a [f32]>,
    pub fx: FxCtx<'a>,
}

/// Renders one voice for a block of `frames`; returns the rendered range
/// start (the voice's first-block delay) and whether the voice ended. The
/// mono output is in `rc.out[..frames - start]`.
pub fn render<C: CellRead + ?Sized>(
    v: &mut Voice,
    t: &Template,
    frames: usize,
    rc: &mut RenderCtx<'_, C>,
) -> (usize, bool) {
    let off = v.delay.min(frames);
    v.delay = 0;
    let m = frames - off;
    rc.out_r[..m].fill(0.0);
    rc.out_3[..m].fill(0.0);
    rc.out_4[..m].fill(0.0);
    let mb = rc.max_block;
    for (k, cell) in v.pcell.iter().enumerate().take(t.n_params) {
        if let Some(c) = cell {
            v.pvals[k] = rc.cells.get(*c);
        }
    }
    let gate = if v.gate_left == usize::MAX {
        m
    } else {
        v.gate_left.min(m)
    };
    for i in 0..t.n_nodes {
        let spec = &t.nodes[i];
        let (done, rest) = rc.bufs.split_at_mut(i * mb);
        let out = &mut rest[..m];
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        for (p, src) in spec.inputs.iter().enumerate() {
            ins[p] = match *src {
                Src::Default(x) | Src::Const(x) => Inp::Val(x),
                Src::Param(k) => Inp::Val(v.pvals[usize::from(k)]),
                Src::Cell(c) => Inp::Val(rc.cells.get(c)),
                Src::Node(j) => {
                    let j = usize::from(j) * mb;
                    Inp::Buf(&done[j..j + m])
                }
            };
        }
        let (moff, mlen) = region(spec, v.mem.len());
        let mem = &mut v.mem[moff..moff + mlen];
        if matches!(spec.node, Node::AuxOut) {
            for (k, right) in rc.out_r[..m].iter_mut().enumerate() {
                *right += ins[0].at(k);
            }
            out.fill(0.0);
            continue;
        }
        if matches!(spec.node, Node::HostInputL | Node::HostInputR) {
            let channel = usize::from(matches!(spec.node, Node::HostInputR));
            if let Some(input) = rc.host_input {
                for (frame, sample) in out.iter_mut().enumerate() {
                    *sample = input[(off + frame) * 2 + channel];
                }
            } else {
                out.fill(0.0);
            }
            continue;
        }
        if matches!(spec.node, Node::Out3 | Node::Out4) {
            let stem = if matches!(spec.node, Node::Out3) {
                &mut rc.out_3
            } else {
                &mut rc.out_4
            };
            for (k, sample) in stem[..m].iter_mut().enumerate() {
                *sample += ins[0].at(k);
            }
            out.fill(0.0);
            continue;
        }
        if let Node::FrameKeyframe { slot } = spec.node {
            if let Some(data) = t.frame_payloads.get(usize::from(slot)) {
                ugen::frame_keyframe::render(data, &ins, out);
            } else {
                out.fill(0.0);
            }
            continue;
        }
        if let Node::StageLinked { slot } = spec.node {
            if let Some(data) = t.stage_payloads.get(usize::from(slot)) {
                let kx = Kx {
                    sr: rc.sr,
                    gate,
                    bank: v.bank,
                    store: rc.store,
                    caps: rc.fx.caps,
                    stats: rc.fx.stats,
                    seed: v.seed.wrapping_add(u32::try_from(i).unwrap_or(0)),
                };
                ugen::stage_linked::render(data, &ins, &mut v.nodes[i], mem, out, &kx);
            } else {
                out.fill(0.0);
            }
            continue;
        }
        if let Node::Effect { fx, .. } = spec.node {
            let unit = &mut v.fx[usize::from(fx)];
            for (p, src) in spec.inputs.iter().enumerate().skip(1) {
                if !matches!(src, Src::Default(_)) {
                    unit.set(p - 1, ins[p].first());
                }
            }
            unit.update(rc.cells);
            let (r, _) = rc.tmp.split_at_mut(m);
            for (k, (y, x)) in out.iter_mut().zip(r.iter_mut()).enumerate() {
                *y = ins[0].at(k);
                *x = *y;
            }
            unit.run(mem, out, r, rc.dry, &mut rc.fx);
            for (y, x) in out.iter_mut().zip(r.iter()) {
                *y = 0.5 * (*y + *x);
            }
            continue;
        }
        let mut kx = Kx {
            sr: rc.sr,
            gate,
            bank: v.bank,
            store: rc.store,
            caps: rc.fx.caps,
            stats: rc.fx.stats,
            seed: v.seed.wrapping_add(u32::try_from(i).unwrap_or(0)),
        };
        ugen::run(spec, &ins, &mut v.nodes[i], mem, out, &mut kx);
    }
    let y = &mut rc.out[..m];
    y.fill(0.0);
    for &s in &t.sinks[..t.n_sinks] {
        let s = usize::from(s) * mb;
        for (a, b) in y.iter_mut().zip(&rc.bufs[s..s + m]) {
            *a += *b;
        }
    }
    let implicit = t.envs == 0 && t.players == 0;
    let step = 1.0 / (v.release * rc.sr);
    for (k, (a, right)) in y.iter_mut().zip(rc.out_r[..m].iter_mut()).enumerate() {
        let mut g = v.amp;
        if implicit {
            if k >= gate {
                v.ienv = (v.ienv - step).max(0.0);
            }
            g *= v.ienv;
        }
        if let Some((left, total)) = v.fade.as_mut() {
            #[allow(clippy::cast_precision_loss)]
            {
                g *= *left as f32 / *total as f32;
            }
            *left = left.saturating_sub(1);
        }
        *a = if a.is_finite() { *a * g } else { 0.0 };
        *right = if right.is_finite() { *right * g } else { 0.0 };
        let third = &mut rc.out_3[k];
        let fourth = &mut rc.out_4[k];
        *third = if third.is_finite() { *third * g } else { 0.0 };
        *fourth = if fourth.is_finite() { *fourth * g } else { 0.0 };
    }
    v.post.run(y, rc.sr);
    if t.has_aux {
        v.post_r.run(&mut rc.out_r[..m], rc.sr);
    }
    if v.gate_left != usize::MAX {
        v.gate_left -= gate;
    }
    let ended = matches!(v.fade, Some((0, _))) || v.finished(t);
    (off, ended)
}

/// The fixed voice pool.
#[derive(Debug)]
pub struct VoicePool {
    pub voices: Box<[Voice]>,
    next_seed: u32,
}

impl VoicePool {
    /// `capacity` voices, each with `voice_mem` floats of delay memory.
    #[must_use]
    pub fn new(capacity: usize, voice_mem: usize) -> Self {
        Self {
            voices: (0..capacity.max(1))
                .map(|_| Voice::new(voice_mem))
                .collect(),
            next_seed: 0x2545_F491,
        }
    }

    /// A free voice index.
    #[must_use]
    pub fn free(&self) -> Option<usize> {
        self.voices.iter().position(|v| !v.active)
    }

    /// The next per-voice seed (deterministic).
    pub fn seed(&mut self) -> u32 {
        self.next_seed = self
            .next_seed
            .wrapping_mul(747_796_405)
            .wrapping_add(2_891_336_453);
        self.next_seed
    }

    /// The number of sounding voices.
    #[must_use]
    pub fn active(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
}

#[derive(Clone, Copy, Debug)]
struct SlotEntry {
    slot: SlotId,
    gen: u32,
    /// The effective time of the control that set `gen` (`-inf` when the
    /// generation is known only from an event: the conservative default).
    eff: f64,
}

/// The audio side's per-slot generation table (11.3).
#[derive(Debug)]
pub struct SlotGens {
    slots: Box<[Option<SlotEntry>]>,
}

impl SlotGens {
    /// A table of `n` slots (the least recently inserted is reused when
    /// full).
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self {
            slots: vec![None; n.max(1)].into_boxed_slice(),
        }
    }

    fn entry(&mut self, slot: SlotId) -> &mut SlotEntry {
        let i = self
            .slots
            .iter()
            .position(|s| s.is_some_and(|s| s.slot == slot))
            .or_else(|| self.slots.iter().position(Option::is_none))
            .unwrap_or(0);
        let fresh = SlotEntry {
            slot,
            gen: 0,
            eff: f64::NEG_INFINITY,
        };
        let e = self.slots[i].get_or_insert(fresh);
        if e.slot != slot {
            *e = fresh;
        }
        e
    }

    /// Applies a `SlotControl` (11.3): later dequeues drop older-generation
    /// events at or after `effective_time`; voices of older generations that
    /// started before it get `release` (`None` rings on, `Natural` releases
    /// open voices, `Panic` short-gates), and stale-started ones (at or after
    /// it) are short-gated.
    pub fn control(&mut self, c: SlotControl, voices: &mut [Voice], sr: f32) {
        let e = self.entry(c.slot);
        if c.new_gen > e.gen || (c.new_gen == e.gen && e.eff == f64::NEG_INFINITY) {
            e.gen = c.new_gen;
            e.eff = c.effective_time;
        } else if c.new_gen == e.gen {
            e.eff = e.eff.min(c.effective_time);
        }
        for v in voices.iter_mut() {
            if !v.active || v.slot != c.slot || v.gen >= c.new_gen {
                continue;
            }
            if v.start < c.effective_time {
                match c.release {
                    Release::None => {}
                    Release::Natural => {
                        if v.open {
                            v.release();
                        }
                    }
                    Release::Panic => v.short_gate(sr),
                }
            } else {
                v.short_gate(sr);
            }
        }
    }

    /// The dequeue rule: false for a stale event (older generation, at or
    /// after the effective time). A newer generation seen on an event is the
    /// piggyback backstop: older events are then dropped from now on.
    pub fn admit(&mut self, ev: &AudioEvent) -> bool {
        let e = self.entry(ev.slot);
        if ev.gen < e.gen && ev.time >= e.eff {
            return false;
        }
        if ev.gen > e.gen {
            e.gen = ev.gen;
            e.eff = f64::NEG_INFINITY;
        }
        true
    }
}

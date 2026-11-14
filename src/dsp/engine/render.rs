use super::*;
use crate::dsp::bus::{BusSlot, OrbitDelay};
use crate::dsp::cells::CellRead;
use crate::dsp::effects::prim::{balance_gains, pan_gains};
use crate::dsp::effects::FxCtx;
use crate::dsp::fft::FFT_SIZE;
use crate::dsp::ugen::Template;
use crate::dsp::voice::{self, RenderCtx, Voice};

impl Engine {
    pub(super) fn render<C: CellRead + ?Sized>(
        &mut self,
        cells: &C,
        input: Option<&[f32]>,
        out: &mut [f32],
        n: usize,
        channels: usize,
    ) {
        self.buses.clear(n);
        if let Some(input) = input {
            let master = self.buses.master();
            let slot = &mut self.buses.slots[master];
            for (frame, lr) in input.chunks_exact(2).enumerate() {
                slot.l[frame] = lr[0];
                slot.r[frame] = lr[1];
            }
        }
        for o in self.orbits.iter_mut() {
            o.clear(n);
        }
        let mut stats = self.counters.fx;
        let Self {
            pool,
            templates,
            buses,
            orbits,
            tags,
            bufs,
            vout,
            vout_r,
            vout_3,
            vout_4,
            tmp,
            dry,
            fx_scratch,
            analysis,
            store,
            fft,
            cfg,
            sr,
            mix_l,
            mix_r,
            mix_3,
            mix_4,
            song_stager,
            song_runtime,
            ..
        } = self;
        mix_3[..n].fill(0.0);
        mix_4[..n].fill(0.0);
        for (vi, v) in pool.voices.iter_mut().enumerate() {
            if !v.active {
                continue;
            }
            let Some(t) = templates.get(v.tmpl) else {
                v.active = false;
                continue;
            };
            let owner = song_runtime.as_ref().and_then(|r| r.voices[vi]);
            let (private, bank_analysis) = if let Some(owner) = owner {
                let Some(stager) = song_stager.as_mut() else {
                    v.active = false;
                    continue;
                };
                let Ok((private, bank)) = stager.graph_views(owner.instrument) else {
                    v.active = false;
                    continue;
                };
                (Some(private), bank)
            } else {
                (None, &mut analysis[..])
            };
            let scoped_cells = SongRenderCells {
                legacy: cells,
                song: private,
            };
            let scope = store.song_read_scope(owner.map(|o| o.epoch));
            let mut rc = RenderCtx {
                sr: *sr,
                max_block: cfg.max_block,
                bufs,
                out: vout,
                out_r: vout_r,
                out_3: vout_3,
                out_4: vout_4,
                tmp,
                dry,
                cells: &scoped_cells,
                store: &scope,
                host_input: if owner.is_some() { None } else { input },
                fx: FxCtx {
                    sr: *sr,
                    store: &scope,
                    fft,
                    caps: &cfg.caps,
                    scratch: fx_scratch,
                    analysis: bank_analysis,
                    stats: &mut stats,
                },
            };
            let (off, ended) = voice::render(v, t, n, &mut rc);
            let bus = &mut buses.slots[v.bus];
            if owner.is_some() {
                let (gl, gr) = if t.has_aux || t.stereo {
                    balance_gains(v.pan)
                } else {
                    pan_gains(v.pan)
                };
                for (k, sample) in rc.out[..n - off].iter().enumerate() {
                    let left = *sample * gl;
                    let right = if t.has_aux || t.stereo {
                        rc.out_r[k] * gr
                    } else {
                        *sample * gr
                    };
                    bus.l[off + k] += left;
                    bus.r[off + k] += right;
                    bus.song_delay.send_l[off + k] += left * v.delay_send;
                    bus.song_delay.send_r[off + k] += right * v.delay_send;
                }
            } else {
                let orbit = &mut orbits[v.orbit.min(orbits.len() - 1)];
                mix_voice(
                    t,
                    v,
                    &rc,
                    n,
                    off,
                    (bus, orbit),
                    (&mut mix_3[..n], &mut mix_4[..n]),
                );
            }
            if ended {
                v.active = false;
                bus.users = bus.users.saturating_sub(1);
                tags.remove_voice(u32::try_from(vi).unwrap_or(u32::MAX));
                if let Some(runtime) = song_runtime.as_mut() {
                    runtime.voices[vi] = None;
                }
            }
        }
        let m = buses.master();
        for o in orbits.iter_mut() {
            let slot = &mut buses.slots[m];
            o.run(n, *sr, &mut slot.l, &mut slot.r);
        }
        {
            let scope = store.song_read_scope(None);
            let mut fx = FxCtx {
                sr: *sr,
                store: &scope,
                fft,
                caps: &cfg.caps,
                scratch: fx_scratch,
                analysis,
                stats: &mut stats,
            };
            buses.render(n, cells, dry, &mut fx, &mut mix_l[..n], &mut mix_r[..n]);
        }
        self.counters.fx = stats;
        if let Err(reason) = self.render_song_branches(cells, n) {
            let _ = reason;
            self.fault(FaultCode::BadResource, 0);
        }
        let mut energy = 0.0;
        for k in 0..n {
            let (l, r) = (guard(self.mix_l[k]), guard(self.mix_r[k]));
            if let Some(o) = out.get_mut(channels * k) {
                *o = l;
            }
            if let Some(o) = out.get_mut(channels * k + 1) {
                *o = r;
            }
            if channels == 4 {
                out[4 * k + 2] = guard(self.mix_3[k]);
                out[4 * k + 3] = guard(self.mix_4[k]);
            }
            let mono = 0.5 * (l + r);
            energy += mono * mono;
            self.master_ring[self.ring_pos] = mono;
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;
        }
        #[allow(clippy::cast_precision_loss)]
        let rms = (energy / n.max(1) as f32).sqrt();
        self.sigs.amp += 0.2 * (rms - self.sigs.amp);
    }
}

fn mix_voice(
    t: &Template,
    v: &Voice,
    rc: &RenderCtx<'_, impl CellRead + ?Sized>,
    n: usize,
    off: usize,
    (bus, orbit): (&mut BusSlot, &mut OrbitDelay),
    (mix_3, mix_4): (&mut [f32], &mut [f32]),
) {
    let (gl, gr) = if t.has_aux || t.stereo {
        balance_gains(v.pan)
    } else {
        pan_gains(v.pan)
    };
    for (k, y) in rc.out[..n - off].iter().enumerate() {
        let (left, right) = if t.has_aux || t.stereo {
            (*y * gl, rc.out_r[k] * gr)
        } else {
            (*y * gl, *y * gr)
        };
        bus.l[off + k] += left;
        bus.r[off + k] += right;
        orbit.in_l[off + k] += left * v.delay_send;
        orbit.in_r[off + k] += right * v.delay_send;
        mix_3[off + k] += rc.out_3[k];
        mix_4[off + k] += rc.out_4[k];
    }
}

impl Engine {
    /// Starts a voice for `ev` at frame `delay` of the next block; on a full
    /// pool the oldest open input voice is stolen (short gate, counted) and
    /// the start waits for its slot.
    pub(super) fn start<C: CellRead + ?Sized>(
        &mut self,
        ev: &AudioEvent,
        tag: Option<VoiceTag>,
        delay: usize,
        cells: &C,
    ) {
        let Some(ts) = self.templates.live(ev.inst) else {
            self.counters.dropped += 1;
            return;
        };
        let Some(vi) = self.pool.free() else {
            let free_wait = self.deferred.iter().position(|d| d.0.is_none());
            match (self.tags.oldest(), free_wait) {
                (Some((_, victim)), Some(w)) => {
                    self.pool.voices[victim as usize].short_gate(self.sr);
                    self.tags.remove_voice(victim);
                    self.counters.stolen += 1;
                    self.deferred[w] = (Some(*ev), tag);
                }
                _ => self.counters.dropped += 1,
            }
            return;
        };
        if let Some(runtime) = &mut self.song_runtime {
            runtime.voices[vi] = None;
        }
        let get = |id| event_ctl(ev, id).map(|c| resolve(c, cells));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let bus = self
            .buses
            .route(get(BUS).map(|b| BusId::new(b.max(0.0) as u32)));
        let seed = self.pool.seed();
        let Some(t) = self.templates.get(ts) else {
            return;
        };
        #[allow(clippy::cast_precision_loss)]
        let start = self.now() + delay as f64 / f64::from(self.sr);
        let v = &mut self.pool.voices[vi];
        v.start(
            t,
            ts,
            ev,
            tag,
            delay,
            start,
            bus,
            cells,
            self.sr,
            &self.cfg.caps,
            seed,
        );
        v.orbit = v.orbit.min(self.orbits.len() - 1);
        self.orbits[v.orbit].set(get(DELAYTIME), get(DELAYFEEDBACK));
        let slot = &mut self.buses.slots[bus];
        slot.users += 1;
        slot.set_room(get(ROOM), get(SIZE));
        if let Some(tag) = tag {
            let _ = self.tags.insert(tag, u32::try_from(vi).unwrap_or(u32::MAX));
        }
        self.choke_cut_group(vi);
    }

    /// Cut-group choke (design-music.md "cut group": `s :break > cut 1 >
    /// d1`; open/closed hat choke, `digital-hat`'s `hat-open`). A voice
    /// that just started with a nonzero cut group quickly releases every
    /// other still-sounding voice that shares both that group and its
    /// orbit, both read from `AudioEvent::voice_hint` (`cut_group`/
    /// `hint_orbit`) rather than `Voice::orbit`, which the `orbit` control
    /// does not currently reach (see the field's doc comment). The design
    /// documents the group but not its scope; same orbit and same cut
    /// group is chosen here. The release reuses `Voice::short_gate`, the
    /// same bounded, allocation-free, click-free ~3 ms fade
    /// (`voice::SHORT_GATE`) the pool-exhaustion steal path already uses,
    /// so it is deterministic and identical on native and browser, both
    /// running this same engine core. `cut == 0` means no group: such a
    /// voice never chokes another voice and is never chokeable.
    pub(super) fn choke_cut_group(&mut self, vi: usize) {
        let v = &self.pool.voices[vi];
        if v.cut == 0 {
            return;
        }
        let (cut, orbit) = (v.cut, v.cut_orbit);
        let sr = self.sr;
        for (i, other) in self.pool.voices.iter_mut().enumerate() {
            if i != vi
                && other.active
                && other.cut == cut
                && other.cut_orbit == orbit
                && self
                    .song_runtime
                    .as_ref()
                    .is_none_or(|r| r.voices[i].is_none())
            {
                other.short_gate(sr);
            }
        }
    }

    /// Dequeues due events and renders one block of `n <= max_block`.
    pub(super) fn block<C: CellStore, S: ControlSource>(
        &mut self,
        io: &mut EngineIo<'_, C, S>,
        input: Option<&[f32]>,
        out: &mut [f32],
        n: usize,
        channels: usize,
    ) {
        let _ = self.apply_song_runtime_frame(self.frame, io.acks);
        for i in 0..MAX_DEFERRED_STARTS {
            if self.pool.free().is_none() {
                break;
            }
            if let (Some(ev), tag) = self.deferred[i] {
                self.deferred[i] = (None, None);
                self.start(&ev, tag, 0, io.cells);
            }
        }
        while self.n_pending < self.pending.len() {
            let Some(ev) = io.events.pop() else {
                break;
            };
            self.pending[self.n_pending] = ev;
            self.n_pending += 1;
        }
        let start = self.now();
        #[allow(clippy::cast_precision_loss)]
        let end = start + n as f64 / f64::from(self.sr);
        let mut k = 0;
        while k < self.n_pending {
            let ev = self.pending[k];
            if ev.time >= end {
                k += 1;
                continue;
            }
            self.pending.copy_within(k + 1..self.n_pending, k);
            self.n_pending -= 1;
            if !self.gens.admit(&ev) {
                self.counters.dropped += 1;
                continue;
            }
            let delay = if ev.time < start {
                self.counters.late += 1;
                0
            } else {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let f = ((ev.time - start) * f64::from(self.sr)).round() as usize;
                f.min(n - 1)
            };
            self.start(&ev, None, delay, io.cells);
        }
        self.render(io.cells, input, out, n, channels);
        self.frame += n as u64;
    }
}

use crate::dsp::song::SongRenderCells;

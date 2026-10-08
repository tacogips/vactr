use super::*;
use crate::dsp::bus::{BusSlot, OrbitDelay};
use crate::dsp::cells::CellRead;
use crate::dsp::effects::prim::{balance_gains, pan_gains};
use crate::dsp::effects::FxCtx;
use crate::dsp::fft::FFT_SIZE;
use crate::dsp::ramp::RampedCells;
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
        let skip_effects = self.output.skips_effects();
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
        if !skip_effects {
            let m = buses.master();
            for o in orbits.iter_mut() {
                let slot = &mut buses.slots[m];
                o.run(n, *sr, &mut slot.l, &mut slot.r);
            }
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
        } else {
            buses.clear(n);
            mix_l[..n].fill(0.0);
            mix_r[..n].fill(0.0);
        }
        self.counters.fx = stats;
        if !skip_effects {
            if let Err(reason) = self.render_song_branches(cells, n) {
                let _ = reason;
                self.fault(FaultCode::BadResource, 0);
            }
        }
        let mut energy = 0.0;
        let mut peak = 0.0f32;
        for k in 0..n {
            let gain = self.output.gate_sample();
            let mut l = guard(self.mix_l[k]);
            let mut r = guard(self.mix_r[k]);
            let mut stem_l = guard(self.mix_3[k]);
            let mut stem_r = guard(self.mix_4[k]);
            if gain != 1.0 {
                l *= gain;
                r *= gain;
                stem_l *= gain;
                stem_r *= gain;
            }
            peak = peak.max(l.abs()).max(r.abs());
            if let Some(o) = out.get_mut(channels * k) {
                *o = l;
            }
            if let Some(o) = out.get_mut(channels * k + 1) {
                *o = r;
            }
            if channels == 4 {
                out[4 * k + 2] = stem_l;
                out[4 * k + 3] = stem_r;
            }
            let mono = 0.5 * (l + r);
            energy += mono * mono;
            self.master_ring[self.ring_pos] = mono;
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;
        }
        let clear_done = if matches!(
            self.output.phase,
            output::Phase::ClearingCut | output::Phase::ClearingIdle
        ) {
            self.clear_output_memory(n)
        } else {
            false
        };
        let end_frame = self.frame.saturating_add(n as u64);
        self.output.after_block(
            peak,
            self.pool.active() > 0,
            self.song_runtime
                .as_ref()
                .is_some_and(|r| r.has_live_branches()),
            end_frame,
            n,
            self.sr,
            &self.cfg,
            clear_done,
        );
        self.buses.hold_retiring(matches!(
            self.output.phase,
            output::Phase::Draining | output::Phase::ClearingIdle
        ));
        #[allow(clippy::cast_precision_loss)]
        let rms = (energy / n.max(1) as f32).sqrt();
        self.sigs.amp += 0.2 * (rms - self.sigs.amp);
    }

    fn clear_output_memory(&mut self, frames: usize) -> bool {
        let budget = (frames.saturating_mul(self.cfg.clear_samples_per_quantum) / 128).max(1);
        let mut left = budget;
        let mut did_work = false;
        while left > 0 {
            if self.output.clear.orbit < self.orbits.len() {
                let index = self.output.clear.orbit;
                let len = self.orbits[index].memory_len();
                let count = self.orbits[index].clear_memory(self.output.clear.offset, left);
                left = left.saturating_sub(count);
                did_work |= count > 0;
                self.output.clear.offset = self.output.clear.offset.saturating_add(count);
                if self.output.clear.offset >= len {
                    self.output.clear.orbit += 1;
                    self.output.clear.offset = 0;
                } else if count == 0 {
                    break;
                }
                continue;
            }
            while self.output.clear.bus < self.buses.slots.len() {
                let slot = &self.buses.slots[self.output.clear.bus];
                if slot.song_key.is_some()
                    || !matches!(
                        slot.state,
                        crate::dsp::bus::SlotState::Live | crate::dsp::bus::SlotState::Retiring
                    )
                {
                    self.output.clear.bus += 1;
                    self.output.clear.offset = 0;
                    self.output.clear.region = 0;
                    self.output.clear.reinitializing = false;
                } else {
                    break;
                }
            }
            if self.output.clear.bus >= self.buses.slots.len() {
                return true;
            }
            let index = self.output.clear.bus;
            if !self.output.clear.reinitializing {
                let len = self.buses.clear_len(index);
                let count = self
                    .buses
                    .clear_slot_memory(index, self.output.clear.offset, left);
                left = left.saturating_sub(count);
                self.output.clear.offset = self.output.clear.offset.saturating_add(count);
                did_work |= count > 0;
                if self.output.clear.offset >= len {
                    self.output.clear.reinitializing = true;
                    self.output.clear.region = 0;
                } else if count == 0 {
                    break;
                }
                continue;
            }

            let Some(region_len) = self.buses.clear_region_len(index, self.output.clear.region)
            else {
                self.output.clear.bus += 1;
                self.output.clear.offset = 0;
                self.output.clear.region = 0;
                self.output.clear.reinitializing = false;
                continue;
            };
            if region_len > left && did_work {
                break;
            }
            let sr = self.sr;
            let caps = &self.cfg.caps;
            let initialized =
                self.buses
                    .reinitialize_clear_region(index, self.output.clear.region, sr, caps);
            if initialized.is_none() {
                break;
            }
            self.output.clear.region += 1;
            if self
                .buses
                .clear_region_len(index, self.output.clear.region)
                .is_none()
            {
                self.output.clear.bus += 1;
                self.output.clear.offset = 0;
                self.output.clear.region = 0;
                self.output.clear.reinitializing = false;
            }
            // Re-initialization is limited to one carved region per block.
            break;
        }
        self.output.clear.orbit >= self.orbits.len()
            && self.output.clear.bus >= self.buses.slots.len()
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
        self.output.admit(self.frame.saturating_add(delay as u64));
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
        let mut ramps = std::mem::take(&mut self.ramps);
        ramps.reap(self.frame);
        let cells = RampedCells::new(&ramps, io.cells, self.frame);
        for i in 0..MAX_DEFERRED_STARTS {
            if self.pool.free().is_none() {
                break;
            }
            if let (Some(ev), tag) = self.deferred[i] {
                self.deferred[i] = (None, None);
                self.start(&ev, tag, 0, &cells);
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
            self.start(&ev, None, delay, &cells);
        }
        self.render(&cells, input, out, n, channels);
        self.ramps = ramps;
        self.frame += n as u64;
    }
}

use crate::dsp::song::SongRenderCells;

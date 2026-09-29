use super::{guard, Engine};
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
            ..
        } = self;
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
            cells,
            store,
            host_input: input,
            fx: FxCtx {
                sr: *sr,
                store,
                fft,
                caps: &cfg.caps,
                scratch: fx_scratch,
                analysis,
                stats: &mut stats,
            },
        };
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
            let (off, ended) = voice::render(v, t, n, &mut rc);
            let bus = &mut buses.slots[v.bus];
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
            if ended {
                v.active = false;
                bus.users = bus.users.saturating_sub(1);
                tags.remove_voice(u32::try_from(vi).unwrap_or(u32::MAX));
            }
        }
        let m = buses.master();
        for o in orbits.iter_mut() {
            let slot = &mut buses.slots[m];
            o.run(n, *sr, &mut slot.l, &mut slot.r);
        }
        buses.render(
            n,
            cells,
            rc.dry,
            &mut rc.fx,
            &mut mix_l[..n],
            &mut mix_r[..n],
        );
        self.counters.fx = stats;
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

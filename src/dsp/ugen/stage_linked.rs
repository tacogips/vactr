//! Immutable, event-local Stages chain adaptation with at most 36 records.
//! The authored rows replace source module serial state and generated tables.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const MAX_SEGMENTS: usize = 36;
pub const MAX_PAYLOADS: usize = 2;
pub const WIRE_VERSION: u8 = 1;
pub const STATE_FLOATS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageData {
    pub len: u8,
    pub rows: [[f32; 4]; MAX_SEGMENTS],
}

impl StageData {
    pub const EMPTY: Self = Self {
        len: 0,
        rows: [[0.0; 4]; MAX_SEGMENTS],
    };

    pub fn from_flat(xs: &[f32]) -> Result<Self, &'static str> {
        if xs.is_empty() || xs.len() % 4 != 0 || xs.len() > MAX_SEGMENTS * 4 {
            return Err("segments: requires 1..36 type/loop/primary/secondary rows (stride 4)");
        }
        let mut data = Self::EMPTY;
        for (index, row) in xs.chunks_exact(4).enumerate() {
            data.rows[index].copy_from_slice(row);
        }
        data.len = u8::try_from(xs.len() / 4).map_err(|_| "segments: too many rows")?;
        data.validate()?;
        Ok(data)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let len = usize::from(self.len);
        if len == 0 || len > MAX_SEGMENTS {
            return Err("segments: count must be 1..36");
        }
        for row in self.rows.iter().take(len) {
            if row.iter().any(|x| !x.is_finite())
                || row[0].fract() != 0.0
                || !(0.0..=3.0).contains(&row[0])
                || (row[1] != 0.0 && row[1] != 1.0)
                || !(0.0..=1.0).contains(&row[2])
                || !(0.0..=1.0).contains(&row[3])
            {
                return Err(
                    "segments: type 0..3 and loop 0/1 must be discrete; controls 0..1 finite",
                );
            }
        }
        Ok(())
    }

    /// Published Stages director-plus-steps shape. Other lists retain the
    /// ordinary linked-chain renderer.
    #[must_use]
    pub fn is_sequencer(&self) -> bool {
        let len = usize::from(self.len);
        len >= 3
            && self.rows[0][0] != 1.0
            && self.rows[0][1] == 0.0
            && self.rows[1..len].iter().all(|row| row[0] == 1.0)
    }
}

fn step_bounds(data: &StageData) -> (usize, usize) {
    let count = usize::from(data.len);
    let mut first = None;
    let mut last = 1;
    for (index, row) in data.rows.iter().enumerate().take(count).skip(1) {
        if row[1] == 1.0 {
            first.get_or_insert(index);
            last = index;
        }
    }
    first.map_or((1, count - 1), |start| (start, last))
}

fn random_next(st: &mut NodeState) -> u32 {
    let mut x = st.u[1];
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    st.u[1] = x;
    x
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn render_sequencer(
    data: &StageData,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let (first, last) = step_bounds(data);
    let count = last - first + 1;
    let mode = (data.rows[0][3] * 7.0).floor().min(6.0) as u8;
    let address = data.rows[0][2];
    let sr = kx.sr.max(1.0);
    if st.u[0] == 0 {
        st.u[0] = 1;
        st.u[1] = kx.seed ^ 0x9e37_79b9;
        if st.u[1] == 0 {
            st.u[1] = 1;
        }
        mem[0] = if mode == 1 { last as f32 } else { first as f32 };
        mem[5] = 1.0;
    }
    for (frame, sample) in out.iter_mut().enumerate() {
        let gate = ins[1].at(frame) >= 0.5;
        let trigger = ins[2].at(frame) >= 0.5;
        let gate_rise = gate && mem[2] < 0.5;
        let trigger_rise = trigger && mem[3] < 0.5;
        mem[2] = f32::from(gate);
        mem[3] = f32::from(trigger);
        if address < 0.0625 {
            mem[4] = 0.0;
        }
        let address_reset = mode != 6 && address > 0.125 && mem[4] < 0.5;
        if address_reset {
            mem[4] = 1.0;
        }
        let mut active = (mem[0] as usize).clamp(first, last);
        if trigger_rise || address_reset {
            active = if mode == 1 { last } else { first };
            st.u[2] = 0;
            mem[5] = 1.0;
        } else if mode == 6 {
            active = first + ((address * count as f32).floor() as usize).min(count - 1);
        } else if gate_rise {
            active = match mode {
                0 => {
                    if active == last {
                        first
                    } else {
                        active + 1
                    }
                }
                1 => {
                    if active == first {
                        last
                    } else {
                        active - 1
                    }
                }
                2 => {
                    if count == 1 {
                        first
                    } else {
                        let mut next = active as isize + mem[5] as isize;
                        if next > last as isize || next < first as isize {
                            mem[5] = -mem[5];
                            next = active as isize + mem[5] as isize;
                        }
                        next as usize
                    }
                }
                3 => {
                    if count == 1 {
                        first
                    } else {
                        st.u[2] = st.u[2].wrapping_add(1) % (2 * (count - 1)) as u32;
                        if st.u[2] % 2 == 0 {
                            first
                        } else {
                            first + ((st.u[2] as usize + 1) / 2)
                        }
                    }
                }
                4 => first + random_next(st) as usize % count,
                5 => {
                    if count == 1 {
                        first
                    } else {
                        first
                            + ((active - first + 1 + random_next(st) as usize % (count - 1))
                                % count)
                    }
                }
                _ => active,
            };
        }
        mem[0] = active as f32;
        let row = data.rows[active];
        let tau = 0.00005 + row[3] * row[3] * 0.5;
        let alpha = 1.0 - (-1.0 / (tau * sr)).exp();
        mem[1] += (row[2] - mem[1]) * alpha;
        *sample = if ins[3].at(frame) >= 0.5 {
            if count == 1 {
                0.0
            } else {
                (active - first) as f32 / (count - 1) as f32
            }
        } else {
            mem[1]
        };
    }
}

fn duration(control: f32) -> f32 {
    0.002 + control * control * 2.0
}

fn curve(phase: f32, shape: f32) -> f32 {
    phase.powf(2.0_f32.powf((shape - 0.5) * 5.0))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn render(
    data: &StageData,
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS || data.len == 0 {
        out.fill(0.0);
        return;
    }
    if data.is_sequencer() {
        render_sequencer(data, ins, st, mem, out, kx);
        return;
    }
    let count = usize::from(data.len);
    let sr = kx.sr.max(1.0);
    let mut loop_start = None;
    let mut loop_end = None;
    let mut has_step = false;
    let mut step_inside = false;
    for (index, row) in data.rows.iter().take(count).enumerate() {
        has_step |= row[0] == 1.0;
        if row[1] == 1.0 {
            loop_start.get_or_insert(index);
            loop_end = Some(index);
        }
    }
    if let (Some(start), Some(end)) = (loop_start, loop_end) {
        step_inside = data.rows[start..=end].iter().any(|row| row[0] == 1.0);
    }
    for (frame, sample) in out.iter_mut().enumerate() {
        let gate = ins[1].at(frame) >= 0.5;
        let trigger = ins[2].at(frame) >= 0.5;
        let channel = ins[3].at(frame) >= 0.5;
        let gate_rise = gate && mem[6] < 0.5;
        let gate_fall = !gate && mem[6] >= 0.5;
        let trigger_rise = trigger && mem[7] < 0.5;
        mem[6] = f32::from(gate);
        mem[7] = f32::from(trigger);
        if st.u[0] == 0 || gate_rise || trigger_rise {
            let prior = (mem[0] as usize).min(count - 1);
            let next = if st.u[0] != 0 && has_step && data.rows[prior][0] == 1.0 {
                (prior + 1) % count
            } else if st.u[0] != 0
                && loop_start.is_some_and(|start| prior >= start)
                && loop_end.is_some_and(|end| prior <= end)
                && !step_inside
            {
                (loop_end.unwrap_or(count - 1) + 1) % count
            } else {
                0
            };
            mem[0] = next as f32;
            mem[1] = 0.0;
            mem[2] = mem[3];
            mem[5] = 1.0;
            st.u[0] = 1;
        }
        if gate_fall && !has_step && loop_end.is_some_and(|end| end + 1 < count) {
            mem[0] = (loop_end.unwrap_or(0) + 1) as f32;
            mem[1] = 0.0;
            mem[2] = mem[3];
        }
        let active = (mem[0] as usize).min(count - 1);
        let row = data.rows[active];
        let typ = row[0] as u8;
        let primary = row[2];
        let secondary = row[3];
        let mut complete = false;
        let mut target = mem[3];
        if mem[5] >= 0.5 {
            match typ {
                0 => {
                    mem[1] = (mem[1] + 1.0 / (duration(primary) * sr)).min(1.0);
                    let end = if active + 1 == count {
                        0.0
                    } else {
                        data.rows[active + 1][2]
                    };
                    target = mem[2] + (end - mem[2]) * curve(mem[1], secondary);
                    complete = mem[1] >= 1.0;
                }
                1 => {
                    let alpha = 1.0 - (-1.0 / (duration(secondary) * sr)).exp();
                    target = mem[3] + (primary - mem[3]) * alpha;
                }
                2 => {
                    target = primary;
                    mem[1] = (mem[1] + 1.0 / (duration(secondary) * sr)).min(1.0);
                    complete = mem[1] >= 1.0;
                }
                _ => {
                    let hz = (ins[0].at(frame) * 2.0_f32.powf((primary - 0.5) * 2.0))
                        .clamp(0.01, sr * 0.4);
                    mem[4] = (mem[4] + hz / sr).fract();
                    target = 0.5
                        + 0.5
                            * ((mem[4] * TAU).sin() * (1.0 - secondary)
                                + (mem[4] * 2.0 - 1.0) * secondary);
                    mem[1] = mem[4];
                    complete = mem[4] < hz / sr;
                }
            }
        }
        mem[3] = target;
        if complete {
            let next = if loop_end == Some(active) {
                loop_start.unwrap_or(0)
            } else {
                active + 1
            };
            if next >= count {
                mem[5] = 0.0;
            } else {
                mem[0] = next as f32;
                mem[1] = 0.0;
                mem[2] = target;
            }
        }
        *sample = if channel { mem[1] * 2.0 - 1.0 } else { target };
    }
}

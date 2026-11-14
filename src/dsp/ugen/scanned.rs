//! Scanned synthesis kernel based on the physical model in
//! `design-docs/specs/design-fm1-voices.md`; this implementation is original.

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Number of ports in the pinned contract.
pub const PORT_COUNT: usize = 7;
/// Stable port names and defaults; order is part of the template contract.
pub const PORTS: [(&str, f32); PORT_COUNT] = [
    ("freq", 440.0),
    ("scan-stiffness", 0.5),
    ("scan-damping", 0.3),
    ("scan-centering", 0.1),
    ("scan-hammer", 0.3),
    ("scan-position", 0.5),
    ("scan-update", 400.0),
];

/// Stable indices for the named ports.
pub mod port {
    pub const FREQ: usize = 0;
    pub const SCAN_STIFFNESS: usize = 1;
    pub const SCAN_DAMPING: usize = 2;
    pub const SCAN_CENTERING: usize = 3;
    pub const SCAN_HAMMER: usize = 4;
    pub const SCAN_POSITION: usize = 5;
    pub const SCAN_UPDATE: usize = 6;
}

/// Number of masses in the closed, uniformly coupled ring.
pub const MASSES: usize = 64;
/// Three mass arrays plus eight persistent scalar values.
pub const STATE_FLOATS: usize = 3 * MASSES + 8;

const POS: usize = 0;
const VEL: usize = POS + MASSES;
const PREV: usize = VEL + MASSES;
const PHASE: usize = PREV + MASSES;
const COUNTDOWN: usize = PHASE + 1;
const ONSET_PEAK: usize = COUNTDOWN + 1;
const DC_X: usize = ONSET_PEAK + 1;
const DC_Y: usize = DC_X + 1;
const INITIALIZED: usize = DC_Y + 1;
const DC_POLE: f32 = 0.995;

/// Returns spring, centering, and damping coefficients for normalized controls.
#[must_use]
pub(crate) fn coefficients(stiffness: f32, centering: f32, damping: f32) -> (f32, f32, f32) {
    let stiffness = control(stiffness, 0.5);
    let centering = control(centering, 0.1);
    let damping = control(damping, 0.3);
    (
        0.05 + 0.55 * stiffness,
        0.2 * centering,
        0.01 + 0.5 * damping,
    )
}

fn control(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn initialize(mem: &mut [f32], hammer: f32, position: f32, interval: f32) {
    let center = position * MASSES as f32;
    let width = (hammer * 32.0).max(1.0);
    let (pos, rest) = mem.split_at_mut(VEL);
    let prev = &mut rest[MASSES..2 * MASSES];
    let mut peak = 0.0_f32;
    for (index, sample) in pos.iter_mut().enumerate() {
        let distance = (index as f32 - center).abs();
        let distance = distance.min(MASSES as f32 - distance);
        *sample = if distance < width {
            0.5 * (1.0 + (std::f32::consts::PI * distance / width).cos())
        } else {
            0.0
        };
        prev[index] = *sample;
        peak = peak.max(sample.abs());
    }
    mem[PHASE] = 0.0;
    mem[COUNTDOWN] = interval;
    mem[ONSET_PEAK] = peak.max(1.0e-6);
    mem[DC_X] = 0.0;
    mem[DC_Y] = 0.0;
    mem[INITIALIZED] = 1.0;
}

fn update(mem: &mut [f32], k: f32, c: f32, d: f32) {
    let pos = &mem[POS..POS + MASSES];
    let vel = &mem[VEL..VEL + MASSES];
    let mut acceleration = [0.0_f32; MASSES];
    for index in 0..MASSES {
        let left = (index + MASSES - 1) % MASSES;
        let right = (index + 1) % MASSES;
        acceleration[index] =
            k * (pos[left] + pos[right] - 2.0 * pos[index]) - c * pos[index] - d * vel[index];
    }
    mem.copy_within(POS..POS + MASSES, PREV);
    for index in 0..MASSES {
        let velocity = mem[VEL + index] + acceleration[index];
        mem[VEL + index] = velocity;
        mem[POS + index] += velocity;
    }
}

fn catmull_rom(array: &[f32], phase: f32) -> f32 {
    let position = phase * MASSES as f32;
    let base = position.floor() as usize;
    let t = position - base as f32;
    let p0 = array[(base + MASSES - 1) % MASSES];
    let p1 = array[base % MASSES];
    let p2 = array[(base + 1) % MASSES];
    let p3 = array[(base + 2) % MASSES];
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
}

/// Renders the scanned ring without allocating.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS || !kx.sr.is_finite() || kx.sr <= 0.0 {
        out.fill(0.0);
        return;
    }
    let stiffness = control(
        ins[port::SCAN_STIFFNESS].first(),
        PORTS[port::SCAN_STIFFNESS].1,
    );
    let damping = control(ins[port::SCAN_DAMPING].first(), PORTS[port::SCAN_DAMPING].1);
    let centering = control(
        ins[port::SCAN_CENTERING].first(),
        PORTS[port::SCAN_CENTERING].1,
    );
    let hammer = control(ins[port::SCAN_HAMMER].first(), PORTS[port::SCAN_HAMMER].1);
    let position = control(
        ins[port::SCAN_POSITION].first(),
        PORTS[port::SCAN_POSITION].1,
    );
    let update_hz = ins[port::SCAN_UPDATE]
        .first()
        .is_finite()
        .then(|| ins[port::SCAN_UPDATE].first().clamp(50.0, 2000.0))
        .unwrap_or(PORTS[port::SCAN_UPDATE].1);
    let interval = (kx.sr / update_hz).round().max(1.0);
    if mem[INITIALIZED] == 0.0 {
        initialize(mem, hammer, position, interval);
    }
    let (k, c, d) = coefficients(stiffness, centering, damping);
    let frequency = ins[port::FREQ]
        .first()
        .is_finite()
        .then(|| ins[port::FREQ].first().clamp(20.0, 8000.0))
        .unwrap_or(PORTS[port::FREQ].1);
    let phase_step = frequency / kx.sr;
    let mut phase = mem[PHASE].rem_euclid(1.0);
    let mut countdown = mem[COUNTDOWN].clamp(0.0, interval);
    let mut dc_x = mem[DC_X];
    let mut dc_y = mem[DC_Y];
    let onset_peak = mem[ONSET_PEAK].max(1.0e-6);
    for sample in out {
        if countdown <= 0.0 {
            update(mem, k, c, d);
            countdown = interval;
        }
        let blend = 1.0 - countdown / interval;
        let before = catmull_rom(&mem[PREV..PREV + MASSES], phase);
        let after = catmull_rom(&mem[POS..POS + MASSES], phase);
        let scan = before * (1.0 - blend) + after * blend;
        let input = scan / onset_peak;
        let filtered = input - dc_x + DC_POLE * dc_y;
        dc_x = input;
        dc_y = filtered;
        *sample = filtered.clamp(-1.0, 1.0);
        phase = (phase + phase_step).rem_euclid(1.0);
        countdown -= 1.0;
    }
    mem[PHASE] = phase;
    mem[COUNTDOWN] = countdown;
    mem[DC_X] = dc_x;
    mem[DC_Y] = dc_y;
}

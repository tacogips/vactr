//! Original bowed-string waveguide using a hyperbolic friction curve.

const MU_STATIC: f32 = 0.8;
const MU_DYNAMIC: f32 = 0.3;
const SLIP_SPEED: f32 = 0.1;
const BOW_POSITION: f32 = 0.12;
const LOSS_POLE: f32 = 0.82;

/// Per-string waveguide state. Delay samples live in the caller's arena.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct BowedString {
    head: usize,
    loss_state: f32,
    previous_bow_velocity: f32,
}

impl BowedString {
    /// Delay-line capacity at a host sample rate, with two interpolation guards.
    #[must_use]
    pub(super) fn mem_floats(sr: f32) -> usize {
        (sr.max(1.0) / 20.0).ceil() as usize + 2
    }

    pub(super) fn load(src: &[f32]) -> Self {
        Self {
            head: finite(src.first().copied().unwrap_or(0.0), 0.0).max(0.0) as usize,
            loss_state: finite(src.get(1).copied().unwrap_or(0.0), 0.0),
            previous_bow_velocity: finite(src.get(2).copied().unwrap_or(0.0), 0.0),
        }
    }

    pub(super) fn store(self, dst: &mut [f32]) {
        if let Some(value) = dst.get_mut(0) {
            *value = self.head as f32;
        }
        if let Some(value) = dst.get_mut(1) {
            *value = finite(self.loss_state, 0.0);
        }
        if let Some(value) = dst.get_mut(2) {
            *value = finite(self.previous_bow_velocity, 0.0);
        }
    }

    /// Advances one sample and returns the velocity at the bridge.
    #[inline]
    pub(super) fn tick(
        &mut self,
        mem: &mut [f32],
        bow_velocity: f32,
        bow_force: f32,
        frequency: f32,
        sr: f32,
    ) -> f32 {
        let capacity = mem.len();
        if capacity < 4 {
            return 0.0;
        }
        let rate = finite(sr, 48_000.0).max(1.0);
        let hz = finite(frequency, 20.0).clamp(20.0, rate * 0.45);
        let omega = std::f32::consts::TAU * hz / rate;
        let filter_phase = (LOSS_POLE * omega.sin()).atan2(1.0 - LOSS_POLE * omega.cos());
        let filter_delay = filter_phase / omega.max(1.0e-6);
        let delay = (rate / hz - filter_delay * 0.5).clamp(2.0, (capacity - 2) as f32);
        self.head %= capacity;

        let loop_velocity = read_delay(mem, self.head, delay);
        let velocity = finite(bow_velocity, 0.0).clamp(0.0, 1.0);
        let force = finite(bow_force, 0.0).clamp(0.0, 1.0);

        // The one-loop approximation uses the prior bridge sample as local
        // string velocity, avoiding a second full delay in the nonlinear loop.
        let relative_velocity = velocity - self.previous_bow_velocity;
        let friction = if force == 0.0 || velocity == 0.0 {
            0.0
        } else {
            let magnitude = relative_velocity.abs();
            let coefficient =
                MU_DYNAMIC + (MU_STATIC - MU_DYNAMIC) / (1.0 + magnitude / SLIP_SPEED);
            relative_velocity.signum() * coefficient * force
        };

        let filtered = self.loss_state + (loop_velocity - self.loss_state) * (1.0 - LOSS_POLE);
        self.loss_state = finite(filtered, 0.0);
        let next = if force == 0.0 || velocity == 0.0 {
            self.loss_state * 0.95
        } else {
            (self.loss_state * 0.9995 + friction * (0.45 * (1.0 - BOW_POSITION))).clamp(-1.5, 1.5)
        };
        let bridge = finite(loop_velocity, 0.0);
        mem[self.head] = finite(next, 0.0);
        self.head = (self.head + 1) % capacity;
        self.previous_bow_velocity = finite(loop_velocity, 0.0);
        bridge
    }
}

#[inline]
fn read_delay(mem: &[f32], head: usize, delay: f32) -> f32 {
    let len = mem.len();
    let whole = delay as usize;
    let frac = delay - whole as f32;
    let a = mem[(head + len - whole % len) % len];
    let b = mem[(head + len - (whole + 1) % len) % len];
    a + (b - a) * frac
}

#[inline]
fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

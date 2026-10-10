//! Original one-sided impact model for the hurdy-gurdy chien bridge.

use std::f32::consts::TAU;

const DUTY: f32 = 0.2;
const RESONANCE_HZ: f32 = 2_500.0;
const RESONANCE_Q: f32 = 3.0;

/// Returns the raised-cosine stroke envelope for a normalized cycle phase.
#[inline]
pub(super) fn stroke(phase: f32) -> f32 {
    let phase = if phase.is_finite() {
        phase.rem_euclid(1.0)
    } else {
        0.0
    };
    if phase >= DUTY {
        0.0
    } else {
        0.5 - 0.5 * (TAU * phase / DUTY).cos()
    }
}

/// Fixed resonator and contact state. It is stored in the per-voice arena.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Rattle {
    previous_penetration: f32,
    y1: f32,
    y2: f32,
}

impl Rattle {
    pub(super) fn load(src: &[f32]) -> Self {
        Self {
            previous_penetration: finite(src.first().copied().unwrap_or(0.0)),
            y1: finite(src.get(1).copied().unwrap_or(0.0)),
            y2: finite(src.get(2).copied().unwrap_or(0.0)),
        }
    }

    pub(super) fn store(self, dst: &mut [f32]) {
        if let Some(value) = dst.get_mut(0) {
            *value = finite(self.previous_penetration);
        }
        if let Some(value) = dst.get_mut(1) {
            *value = finite(self.y1);
        }
        if let Some(value) = dst.get_mut(2) {
            *value = finite(self.y2);
        }
    }

    /// Adds a bounded impact when bridge motion closes the speed-dependent gap.
    #[inline]
    pub(super) fn tick(
        &mut self,
        bridge_force: f32,
        wheel: f32,
        threshold: f32,
        gain: f32,
        sr: f32,
    ) -> f32 {
        let wheel = finite(wheel).clamp(0.0, 1.0);
        let threshold = finite(threshold).clamp(0.0, 1.0);
        let amount = finite(gain).clamp(0.0, 1.0);
        if amount == 0.0 {
            return 0.0;
        }
        let gap = (1.0 - (wheel - threshold).clamp(0.0, 1.0)) * 0.5;
        let penetration = (finite(bridge_force).abs() - gap).max(0.0);
        let impulse = (penetration - self.previous_penetration).max(0.0) * 1.8;
        self.previous_penetration = penetration;

        let rate = finite(sr).max(1.0);
        let omega = TAU * RESONANCE_HZ.min(rate * 0.45) / rate;
        let radius = (-omega / (2.0 * RESONANCE_Q)).exp();
        let resonated = impulse + 2.0 * radius * omega.cos() * self.y1 - radius * radius * self.y2;
        self.y2 = self.y1;
        self.y1 = finite(resonated).clamp(-2.0, 2.0);
        (impulse * amount * 12.0 + self.y1 * amount * 0.8).clamp(-1.0, 1.0)
    }
}

#[inline]
fn finite(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

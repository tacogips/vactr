//! Fixed memory ring bank; counters and filter histories belong to the effect.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

pub(crate) const HEADER: usize = 128;

#[derive(Clone, Copy)]
pub(crate) struct Bank {
    pub stride: usize,
    offset: usize,
    head_base: usize,
}
impl Bank {
    pub fn new(seconds: f32, sr: f32) -> Self {
        Self {
            stride: (seconds * sr.max(1.0)).ceil() as usize + 4,
            offset: HEADER,
            head_base: 0,
        }
    }
    pub fn region(seconds: f32, sr: f32, offset: usize, head_base: usize) -> Self {
        let mut bank = Self::new(seconds, sr);
        bank.offset = offset;
        bank.head_base = head_base;
        bank
    }
    pub fn len(self, lines: usize) -> usize {
        HEADER + lines * self.stride
    }
    pub fn read(self, mem: &[f32], line: usize, delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.stride - 2) as f32);
        let whole = d as usize;
        let pos = mem[self.head_base + line] as usize;
        let idx = (pos + self.stride - whole) % self.stride;
        let next = (idx + self.stride - 1) % self.stride;
        let off = self.offset + line * self.stride;
        let a = mem[off + idx];
        a + (mem[off + next] - a) * (d - whole as f32)
    }
    pub fn write(self, mem: &mut [f32], line: usize, x: f32) {
        let pos = mem[self.head_base + line] as usize;
        mem[self.offset + line * self.stride + pos] = if x.is_finite() { x } else { 0.0 };
        mem[self.head_base + line] = ((pos + 1) % self.stride) as f32;
    }
    pub fn allpass(self, mem: &mut [f32], line: usize, x: f32, d: f32, g: f32) -> f32 {
        // Integer tap retains unity magnitude, including lossless freeze.
        let z = self.read(mem, line, d.round());
        let v = x + g * z;
        self.write(mem, line, v);
        z - g * v
    }
}
pub(crate) fn value(p: &[f32], i: usize, default: f32) -> f32 {
    p.get(i)
        .copied()
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}
pub(crate) fn damping(mem: &mut [f32], slot: usize, x: f32, a: f32) -> f32 {
    let y = x + a * (mem[slot] - x);
    mem[slot] = y;
    y
}
pub(crate) fn rt60(delay: f32, decay: f32) -> f32 {
    (-6.907_755 * delay / decay.max(0.01)).exp()
}

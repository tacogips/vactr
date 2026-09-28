//! Original, bounded four-lane keyframe interpolation for the Frames control role.
//! The response curve is digital and does not model the upstream DAC/VCA tables.

use super::{Inp, MAX_PORTS};

pub const MAX_FRAMES: usize = 64;
pub const MAX_PAYLOADS: usize = 4;
pub const WIRE_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameData {
    pub len: u8,
    pub points: [[f32; 5]; MAX_FRAMES],
}

impl FrameData {
    pub const EMPTY: Self = Self {
        len: 0,
        points: [[0.0; 5]; MAX_FRAMES],
    };

    pub fn from_flat(xs: &[f32]) -> Result<Self, &'static str> {
        if xs.is_empty() || xs.len() % 5 != 0 || xs.len() > MAX_FRAMES * 5 {
            return Err("frames: requires 1..64 timestamp/value quadruples (stride 5)");
        }
        let mut data = Self::EMPTY;
        for (i, row) in xs.chunks_exact(5).enumerate() {
            if row
                .iter()
                .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
            {
                return Err("frames: timestamps and values must be finite within 0..1");
            }
            if i > 0 && row[0] <= data.points[i - 1][0] {
                return Err("frames: timestamps must be strictly increasing");
            }
            data.points[i].copy_from_slice(row);
        }
        data.len = u8::try_from(xs.len() / 5).map_err(|_| "frames: too many keyframes")?;
        Ok(data)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let n = usize::from(self.len);
        if n == 0 || n > MAX_FRAMES {
            return Err("frames: invalid count");
        }
        let mut prev = -1.0;
        for row in self.points.iter().take(n) {
            if row
                .iter()
                .any(|x| !x.is_finite() || !(0.0..=1.0).contains(x))
                || row[0] <= prev
            {
                return Err("frames: invalid or unsorted keyframe");
            }
            prev = row[0];
        }
        Ok(())
    }

    pub fn sample(&self, position: f32, lane: usize, ease: u8, response: f32) -> f32 {
        let n = usize::from(self.len);
        if n == 0 {
            return 0.0;
        }
        let lane = lane.min(3) + 1;
        let p = if position.is_finite() {
            position.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if p <= self.points[0][0] {
            return respond(self.points[0][lane], response);
        }
        if p >= self.points[n - 1][0] {
            return respond(self.points[n - 1][lane], response);
        }
        let hi = self.points[..n].partition_point(|row| row[0] < p);
        let lo = hi - 1;
        let a = self.points[lo];
        let b = self.points[hi];
        let t = ((p - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0);
        let t = easing(t, ease);
        respond(a[lane] + (b[lane] - a[lane]) * t, response)
    }
}

fn easing(t: f32, kind: u8) -> f32 {
    match kind.min(5) {
        0 => {
            if t < 0.5 {
                0.0
            } else {
                1.0
            }
        }
        1 => t,
        2 => t.powi(4),
        3 => 1.0 - (1.0 - t).powi(4),
        4 => 0.5 - 0.5 * (std::f32::consts::PI * t).cos(),
        _ => {
            // An original bounded quadratic bounce, not the source lookup.
            let u = (1.0 - t) * 3.0;
            (1.0 - (1.0 - t) * (u * std::f32::consts::PI).cos().abs()).clamp(0.0, 1.0)
        }
    }
}

fn respond(y: f32, response: f32) -> f32 {
    let response = if response.is_finite() {
        response.clamp(0.0, 1.0)
    } else {
        0.0
    };
    y.clamp(0.0, 1.0).powf(1.0 + 3.0 * response)
}

pub fn render(data: &FrameData, ins: &[Inp<'_>; MAX_PORTS], out: &mut [f32]) {
    for (i, y) in out.iter_mut().enumerate() {
        let selected = ins[9].at(i);
        let lane = if selected.is_finite() {
            selected.round().clamp(0.0, 3.0) as usize
        } else {
            0
        };
        let selection = ins[1 + lane].at(i);
        let ease = if selection.is_finite() {
            selection.round().clamp(0.0, 5.0) as u8
        } else {
            1
        };
        let response = ins[5 + lane].at(i);
        *y = data.sample(ins[0].at(i), lane, ease, response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_capacity_order_and_endpoints() {
        let d = FrameData::from_flat(&[0.2, 0.0, 0.0, 0.0, 0.0, 0.8, 1.0, 0.0, 0.0, 0.0]).unwrap();
        assert_eq!(d.sample(0.0, 0, 1, 0.0), 0.0);
        assert_eq!(d.sample(1.0, 0, 1, 0.0), 1.0);
        assert_eq!(d.sample(0.5, 0, 0, 0.0), 1.0);
        assert!(FrameData::from_flat(&[0.2, 0.0, 0.0, 0.0, 0.0, 0.2, 1.0, 0.0, 0.0, 0.0]).is_err());
        assert!(FrameData::from_flat(&[0.8, 0.0, 0.0, 0.0, 0.0, 0.2, 1.0, 0.0, 0.0, 0.0]).is_err());
        assert!(FrameData::from_flat(&vec![0.0; 65 * 5]).is_err());
        assert!(d.sample(f32::NAN, 0, 1, f32::NAN).is_finite());
    }

    #[test]
    fn six_easing_choices_and_per_lane_response_are_distinct() {
        let d = FrameData::from_flat(&[0.0, 0.0, 0.2, 0.4, 0.6, 1.0, 1.0, 0.8, 0.6, 0.4]).unwrap();
        let outputs: Vec<_> = (0..6).map(|ease| d.sample(0.3, 0, ease, 0.0)).collect();
        for (i, a) in outputs.iter().enumerate() {
            for b in outputs.iter().skip(i + 1) {
                assert!((a - b).abs() > 0.001, "ease {i}");
            }
        }
        assert!(d.sample(0.3, 1, 1, 0.0) > d.sample(0.3, 1, 1, 1.0));
        assert_ne!(d.sample(0.3, 0, 1, 0.0), d.sample(0.3, 3, 1, 0.0));
    }
}

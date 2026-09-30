//! Checked microsecond notation and exact frame-window composition at commit.
use super::{current, entry, Controls};
use crate::pattern::eval::num_ratio;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub frames: u64,
    pub rate: u32,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Window {
    pub start: u32,
    pub stop: u32,
    pub seconds: f64,
}
fn bad(message: &str) -> Failure {
    Failure::new(FailCode::Type, format!("sample timestamp: {message}"))
}

/// Integer milliseconds or decimal milliseconds with microsecond granularity.
/// Float32 values already rounded elsewhere cannot recover their original digits;
/// timestamp argument literals instead reach this boundary as exact ratios.
pub(crate) fn checked_micros(value: &Value) -> Result<u64, Failure> {
    let ratio = match value {
        Value::Int(v) => Ratio64::from_int(i64::from(*v)),
        Value::Int64(v) => Ratio64::from_int(*v),
        Value::Ratio(v) => *v,
        Value::Float(v) if v.is_finite() => Ratio64::from_decimal(&v.to_string())
            .ok_or_else(|| bad("number is not a representable decimal"))?,
        Value::Float64(v) if v.is_finite() => {
            if *v < 0.0 {
                return Err(bad("milliseconds cannot be negative"));
            }
            let scaled = *v * 1000.0;
            if !scaled.is_finite() || scaled >= 9_007_199_254_740_992.0 {
                return Err(bad(
                    "floating timestamp overflows exact microsecond precision",
                ));
            }
            let nearest = scaled.round();
            let tolerance = (scaled.abs() * f64::EPSILON * 4.0).max(1e-7);
            if (nearest - scaled).abs() > tolerance {
                return Err(bad("milliseconds allow at most three decimal places"));
            }
            return Ok(nearest as u64);
        }
        _ => return Err(bad("expected a finite numeric millisecond value")),
    };
    let numerator = i128::from(ratio.num())
        .checked_mul(1000)
        .ok_or_else(|| bad("milliseconds overflow"))?;
    if numerator < 0 {
        return Err(bad("milliseconds cannot be negative"));
    }
    let denominator = i128::from(ratio.den());
    if numerator % denominator != 0 {
        return Err(bad("milliseconds allow at most three decimal places"));
    }
    u64::try_from(numerator / denominator).map_err(|_| bad("milliseconds overflow microseconds"))
}
pub(super) fn present(controls: &Controls) -> bool {
    entry(controls, "start-ms").is_some() || entry(controls, "stop-ms").is_some()
}
pub(super) fn validate_values(controls: &Controls) -> Result<(), Failure> {
    for name in ["start-ms", "stop-ms"] {
        if let Some(value) = entry(controls, name) {
            checked_micros(&current(value))?;
        }
    }
    Ok(())
}
fn ceil_frame(value: Ratio64) -> Result<u32, Failure> {
    let floor = value.floor();
    let ceil = floor
        .checked_add(i64::from(value.num() % value.den() != 0))
        .ok_or_else(|| bad("frame index overflow"))?;
    u32::try_from(ceil).map_err(|_| bad("source exceeds supported u32 frame range"))
}
pub(super) fn resolve(controls: &Controls, geometry: Geometry) -> Result<Option<Window>, Failure> {
    if !present(controls) {
        return Ok(None);
    }
    if geometry.rate == 0 || geometry.frames == 0 {
        return Err(bad("source has no audio frames"));
    }
    if geometry.frames > u64::from(u32::MAX) {
        return Err(bad("source exceeds supported u32 frame range"));
    }
    let frames = Ratio64::from_int(geometry.frames as i64);
    let boundary = |name: &str, fallback: Ratio64| -> Result<Ratio64, Failure> {
        let Some(value) = entry(controls, name) else {
            return Ok(fallback);
        };
        let micros = checked_micros(&current(value))?;
        let numerator = u128::from(micros) * u128::from(geometry.rate);
        if numerator > u128::from(geometry.frames) * 1_000_000 {
            return Err(bad("boundary lies outside loaded source"));
        }
        Ratio64::new(
            i64::try_from(numerator).map_err(|_| bad("frame boundary overflow"))?,
            1_000_000,
        )
    };
    let start = boundary("start-ms", Ratio64::ZERO)?;
    let stop = boundary("stop-ms", frames)?;
    if stop <= start {
        return Err(bad("stop must be greater than start"));
    }
    let normalized = |name: &str, fallback: Ratio64| -> Result<Ratio64, Failure> {
        let Some(value) = entry(controls, name) else {
            return Ok(fallback);
        };
        let fraction = num_ratio(&current(value))
            .ok_or_else(|| bad("begin/end must be finite numeric fractions"))?;
        if fraction < Ratio64::ZERO || fraction > Ratio64::ONE {
            return Err(bad("begin/end fractions must lie in 0..1"));
        }
        Ok(fraction)
    };
    let begin = normalized("begin", Ratio64::ZERO)?;
    let end = normalized("end", Ratio64::ONE)?;
    if end <= begin {
        return Err(bad("end must be greater than begin"));
    }
    let duration = stop.checked_sub(start)?;
    let lo = start.checked_add(duration.checked_mul(begin)?)?;
    let hi = start.checked_add(duration.checked_mul(end)?)?;
    let (first, last) = (ceil_frame(lo)?, ceil_frame(hi)?);
    if last <= first {
        return Err(bad("window contains no source frames"));
    }
    // SpeedFit::Splice already includes its normalized slice fraction;
    // Fit/LoopAt likewise retain their legacy region semantics within this window.
    Ok(Some(Window {
        start: first,
        stop: last,
        seconds: duration.to_f64() / f64::from(geometry.rate),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::intern::intern_kw;
    fn controls(entries: &[(&str, Value)]) -> Controls {
        entries
            .iter()
            .map(|(n, v)| (intern_kw(n), (v.clone(), None)))
            .collect()
    }
    #[test]
    fn timestamp_numeric_notation_is_checked_integer_microseconds() {
        for (text, expected) in [
            ("0", 0),
            ("123", 123000),
            ("123.4", 123400),
            ("123.45", 123450),
            ("123.456", 123456),
            ("700000.001", 700000001),
        ] {
            assert_eq!(
                checked_micros(&Value::Ratio(Ratio64::from_decimal(text).unwrap())).unwrap(),
                expected
            );
        }
        assert_eq!(checked_micros(&Value::Int(123)).unwrap(), 123000);
        assert_eq!(checked_micros(&Value::Float64(123.456)).unwrap(), 123456);
        for bad_value in [
            Value::Int(-1),
            Value::Float(f32::NAN),
            Value::Float64(f64::INFINITY),
            Value::Int64(i64::MAX),
            Value::Ratio(Ratio64::from_decimal("123.4567").unwrap()),
            Value::Bool(true),
        ] {
            assert!(checked_micros(&bad_value).is_err());
        }
    }
    #[test]
    fn timestamp_exact_frames_omitted_bounds_and_normalized_composition() {
        for rate in [44100, 48000, 96000] {
            let g = Geometry {
                frames: u64::from(rate) * 2,
                rate,
            };
            let p = controls(&[
                (
                    "start-ms",
                    Value::Ratio(Ratio64::from_decimal("123.456").unwrap()),
                ),
                ("stop-ms", Value::Int(1000)),
                ("begin", Value::Ratio(Ratio64::new(1, 4).unwrap())),
                ("end", Value::Ratio(Ratio64::new(3, 4).unwrap())),
            ]);
            let window = resolve(&p, g).unwrap().unwrap();
            let exact_start = 123456u64 + 876544 / 4;
            let exact_stop = 123456u64 + 3 * 876544 / 4;
            assert_eq!(
                window.start,
                ((exact_start * u64::from(rate) + 999999) / 1000000) as u32
            );
            assert_eq!(
                window.stop,
                ((exact_stop * u64::from(rate) + 999999) / 1000000) as u32
            );
            assert!((window.seconds - 0.876544).abs() < 1e-12);
            assert_eq!(
                resolve(&controls(&[("stop-ms", Value::Int(1000))]), g)
                    .unwrap()
                    .unwrap()
                    .start,
                0
            );
            assert_eq!(
                resolve(&controls(&[("start-ms", Value::Int(1000))]), g)
                    .unwrap()
                    .unwrap()
                    .stop,
                2 * rate
            );
            for invalid in [
                controls(&[("start-ms", Value::Int(1000)), ("stop-ms", Value::Int(999))]),
                controls(&[("stop-ms", Value::Int(2001))]),
                controls(&[("start-ms", Value::Int(0)), ("end", Value::Int(-1))]),
            ] {
                assert!(resolve(&invalid, g).is_err());
            }
        }
        let geometry = Geometry {
            frames: 40_000_000,
            rate: 48000,
        };
        let long = resolve(
            &controls(&[(
                "start-ms",
                Value::Ratio(Ratio64::from_decimal("700000.001").unwrap()),
            )]),
            geometry,
        )
        .unwrap()
        .unwrap();
        assert_eq!(long.start, 33_600_001); // Beyond f32 integer precision.
        assert!(resolve(
            &controls(&[("start-ms", Value::Int(0))]),
            Geometry {
                frames: u64::from(u32::MAX) + 1,
                rate: 48000
            }
        )
        .is_err());
    }
}

//! Shared offline render checks for bass examples and presets.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;

pub(super) const SR: u32 = 48_000;

/// Write clamped stereo PCM16 samples under `tmp/bass` and return the WAV path.
pub(super) fn write_wav(rel: &str, left: &[f32], right: &[f32]) -> PathBuf {
    assert_eq!(
        left.len(),
        right.len(),
        "stereo channels must have equal lengths"
    );

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tmp")
        .join("bass")
        .join(format!("{rel}.wav"));
    let parent = path
        .parent()
        .unwrap_or_else(|| panic!("WAV path has no parent: {path:?}"));
    fs::create_dir_all(parent)
        .unwrap_or_else(|error| panic!("failed to create WAV directory for {path:?}: {error}"));

    let data_len = left
        .len()
        .checked_mul(4)
        .and_then(|length| u32::try_from(length).ok())
        .unwrap_or_else(|| panic!("WAV data is too large for {path:?}"));
    let riff_len = data_len
        .checked_add(36)
        .unwrap_or_else(|| panic!("WAV is too large for {path:?}"));
    let byte_rate = SR * 4;
    let mut file = File::create(&path)
        .unwrap_or_else(|error| panic!("failed to create WAV {path:?}: {error}"));

    let write_result = (|| -> io::Result<()> {
        file.write_all(b"RIFF")?;
        file.write_all(&riff_len.to_le_bytes())?;
        file.write_all(b"WAVEfmt ")?;
        file.write_all(&16_u32.to_le_bytes())?;
        file.write_all(&1_u16.to_le_bytes())?;
        file.write_all(&2_u16.to_le_bytes())?;
        file.write_all(&SR.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&4_u16.to_le_bytes())?;
        file.write_all(&16_u16.to_le_bytes())?;
        file.write_all(b"data")?;
        file.write_all(&data_len.to_le_bytes())?;
        for (&left_sample, &right_sample) in left.iter().zip(right) {
            write_pcm_sample(&mut file, left_sample)?;
            write_pcm_sample(&mut file, right_sample)?;
        }
        Ok(())
    })();
    write_result.unwrap_or_else(|error| panic!("failed to write WAV {path:?}: {error}"));
    path
}

fn write_pcm_sample(file: &mut File, sample: f32) -> io::Result<()> {
    let scaled = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
    file.write_all(&scaled.to_le_bytes())
}

/// Return the share of non-DC spectral power between 20 and 250 Hz.
pub(super) fn low_band_share(mono: &[f32]) -> f32 {
    const BINS: usize = 8192;
    const FFT_SIZE: usize = BINS * 2;
    let magnitudes = crate::dsp::offline::spectrum(mono, BINS);
    let min_bin = (20 * FFT_SIZE).div_ceil(SR as usize);
    let max_low_bin = (250 * FFT_SIZE) / SR as usize;
    let total_power: f64 = magnitudes
        .iter()
        .enumerate()
        .skip(min_bin)
        .map(|(_, magnitude)| f64::from(*magnitude).powi(2))
        .sum();
    if total_power == 0.0 {
        return 0.0;
    }
    let low_power: f64 = magnitudes
        .iter()
        .enumerate()
        .skip(min_bin)
        .take(max_low_bin + 1 - min_bin)
        .map(|(_, magnitude)| f64::from(*magnitude).powi(2))
        .sum();
    (low_power / total_power) as f32
}

pub(super) struct RenderCheck {
    pub rms: f32,
    pub peak: f32,
    pub low: f32,
    pub finite: bool,
}

/// Analyze the mono average, with peak measured across both channels.
pub(super) fn check(left: &[f32], right: &[f32]) -> RenderCheck {
    assert_eq!(
        left.len(),
        right.len(),
        "stereo channels must have equal lengths"
    );
    let mono: Vec<f32> = left
        .iter()
        .zip(right)
        .map(|(&left_sample, &right_sample)| (left_sample + right_sample) * 0.5)
        .collect();
    let peak = left
        .iter()
        .chain(right)
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    RenderCheck {
        rms: crate::dsp::offline::rms(&mono),
        peak,
        low: low_band_share(&mono),
        finite: left.iter().chain(right).all(|sample| sample.is_finite()),
    }
}

#[cfg(test)]
mod tests {
    use super::{check, low_band_share, write_wav, RenderCheck, SR};
    use std::f32::consts::TAU;
    use std::fs;

    const FRAMES: usize = 16_384;

    #[allow(clippy::cast_precision_loss)]
    fn sine(frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..FRAMES)
            .map(|frame| amplitude * (TAU * frequency * frame as f32 / SR as f32).sin())
            .collect()
    }

    #[test]
    fn low_band_share_accepts_a_100_hz_sine() {
        assert!(low_band_share(&sine(100.0, 0.5)) >= 0.95);
    }

    #[test]
    fn low_band_share_rejects_a_2_khz_sine() {
        assert!(low_band_share(&sine(2_000.0, 0.5)) <= 0.05);
    }

    #[test]
    fn write_wav_emits_a_riff_stereo_pcm_file() {
        let samples = sine(100.0, 0.5);
        let path = write_wav("selftest", &samples, &samples);
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(bytes.len(), 44 + 4 * samples.len());
        fs::remove_file(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    }

    #[test]
    fn check_reports_stereo_level_and_low_frequency_content() {
        let samples = sine(60.0, 0.5);
        let result: RenderCheck = check(&samples, &samples);
        assert!(result.finite);
        assert!(result.rms > 0.0);
        assert!((result.peak - 0.5).abs() <= 0.01);
        assert!(result.low >= 0.95);
    }
}

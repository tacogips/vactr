//! Checked streaming PCM16 writer; failed exports never replace the destination.
use crate::vm::fail::{FailCode, Failure};
type Result<T> = std::result::Result<T, Failure>;
fn failure(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::Type, message)
}
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub(super) fn header(frames: u64, rate: u32) -> Result<[u8; 44]> {
    let bytes = frames
        .checked_mul(4)
        .ok_or_else(|| failure("WAV data size overflow"))?;
    let bytes = u32::try_from(bytes).map_err(|_| failure("WAV data exceeds RIFF limit"))?;
    let riff = bytes
        .checked_add(36)
        .ok_or_else(|| failure("WAV RIFF size overflow"))?;
    let byte_rate = rate
        .checked_mul(4)
        .ok_or_else(|| failure("WAV byte rate overflow"))?;
    let mut header = [0u8; 44];
    header[..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&riff.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&1u16.to_le_bytes());
    header[22..24].copy_from_slice(&2u16.to_le_bytes());
    header[24..28].copy_from_slice(&rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&4u16.to_le_bytes());
    header[34..36].copy_from_slice(&16u16.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&bytes.to_le_bytes());
    Ok(header)
}
pub(super) struct WavOutput {
    file: BufWriter<File>,
    temporary: PathBuf,
    destination: PathBuf,
    bytes: u64,
    expected: u64,
    finished: bool,
}
impl WavOutput {
    pub fn create(destination: &Path, frames: u64, rate: u32) -> Result<Self> {
        let bytes = frames
            .checked_mul(4)
            .ok_or_else(|| failure("WAV length overflow"))?;
        let header = header(frames, rate)?;
        if let Some(parent) = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| failure(format!("cannot create output directory: {error}")))?;
        }
        let name = destination
            .file_name()
            .ok_or_else(|| failure("output must name a WAV file"))?;
        let (temporary, file) = (0..128)
            .find_map(|_| {
                let mut temp_name = name.to_os_string();
                temp_name.push(format!(
                    ".partial-{}",
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                let temporary = destination.with_file_name(temp_name);
                match OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)
                {
                    Ok(file) => Some(Ok((temporary, file))),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(error) => Some(Err(failure(format!(
                        "cannot create temporary WAV: {error}"
                    )))),
                }
            })
            .ok_or_else(|| failure("temporary WAV collision limit exhausted"))??;
        let mut output = Self {
            file: BufWriter::with_capacity(65536, file),
            temporary,
            destination: destination.to_owned(),
            bytes: 0,
            expected: bytes,
            finished: false,
        };
        output
            .file
            .write_all(&header)
            .map_err(|error| failure(format!("cannot write WAV header: {error}")))?;
        Ok(output)
    }
    pub fn write(&mut self, samples: &[f32]) -> Result<()> {
        let mut buffer = [0u8; crate::host::native::audio::MAX_BLOCK * 4];
        if samples.len() > buffer.len() / 2 || samples.len() % 2 != 0 {
            return Err(failure("invalid PCM block size"));
        }
        let added = u64::try_from(samples.len())
            .ok()
            .and_then(|n| n.checked_mul(2))
            .ok_or_else(|| failure("PCM size overflow"))?;
        if self
            .bytes
            .checked_add(added)
            .is_none_or(|n| n > self.expected)
        {
            return Err(failure("PCM exceeds declared WAV duration"));
        }
        for (index, sample) in samples.iter().copied().enumerate() {
            if !sample.is_finite() || sample.abs() > 1.0 {
                return Err(failure("PCM sample is nonfinite or outside full scale"));
            }
            let scaled = sample * if sample < 0.0 { 32768.0 } else { 32767.0 };
            let encoded = (scaled.round() as i16).to_le_bytes();
            buffer[2 * index..2 * index + 2].copy_from_slice(&encoded);
        }
        self.file
            .write_all(&buffer[..2 * samples.len()])
            .map_err(|error| failure(format!("cannot write PCM: {error}")))?;
        self.bytes = self
            .bytes
            .checked_add((2 * samples.len()) as u64)
            .ok_or_else(|| failure("PCM size overflow"))?;
        Ok(())
    }
    pub fn finish(mut self) -> Result<()> {
        if self.bytes != self.expected {
            return Err(failure(format!(
                "WAV size mismatch: {} != {}",
                self.bytes, self.expected
            )));
        }
        self.file
            .flush()
            .map_err(|error| failure(format!("cannot flush WAV: {error}")))?;
        let actual = self
            .file
            .get_ref()
            .metadata()
            .map_err(|error| failure(error.to_string()))?
            .len();
        if actual != self.expected + 44 {
            return Err(failure("WAV file length mismatch"));
        }
        std::fs::rename(&self.temporary, &self.destination)
            .map_err(|error| failure(format!("cannot finalize WAV: {error}")))?;
        self.finished = true;
        Ok(())
    }
}
impl Drop for WavOutput {
    fn drop(&mut self) {
        if !self.finished {
            let _ = std::fs::remove_file(&self.temporary);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_pcm16_header_and_riff_limits() {
        let h = header(48000, 48000).unwrap();
        assert_eq!(&h[..4], b"RIFF");
        assert_eq!(&h[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(h[4..8].try_into().unwrap()), 192036);
        assert_eq!(u32::from_le_bytes(h[28..32].try_into().unwrap()), 192000);
        assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 192000);
        assert!(header(u64::MAX, 48000).is_err());
        assert!(header(u64::from(u32::MAX) / 4, 48000).is_err());
    }
}

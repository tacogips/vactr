//! Host capabilities (design 12.7, 12.8.8).
//!
//! `CapabilitySet::require` is the one gate: the checker, instrument
//! realization and the scheduler turn any use beyond the advertised limits
//! into a `beyond-capability` diagnostic with its origin, never a crash and
//! never silent truncation.

use crate::reader::span::{FileId, Span};
use crate::types::diag::{DiagCode, Diagnostic};

/// What a host tier can do (design 12.7).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CapabilitySet {
    pub max_voices: u16,
    pub max_ir_seconds: f32,
    /// Grains per second (12.6).
    pub max_grain_density: f32,
    /// Seconds (12.6).
    pub max_grain_size: f32,
    /// The live granular capture buffer, seconds (12.6).
    pub max_capture_seconds: f32,
    pub multichannel: u8,
    pub offline_render: bool,
    pub midi_in: bool,
    pub midi_out: bool,
    pub file_access: bool,
}

/// One requested use of a capability.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cap {
    Voices(u16),
    IrSeconds(f32),
    GrainDensity(f32),
    GrainSize(f32),
    CaptureSeconds(f32),
    OfflineRender,
    MidiIn,
    MidiOut,
    FileAccess,
}

impl CapabilitySet {
    /// The browser tier: the AudioWorklet budget (MIDI stays off until the
    /// TS shell wires WebMIDI, 12.8.1).
    #[must_use]
    pub const fn browser() -> Self {
        Self {
            max_voices: 64,
            max_ir_seconds: 2.0,
            max_grain_density: 200.0,
            max_grain_size: 0.5,
            max_capture_seconds: 8.0,
            multichannel: 2,
            offline_render: false,
            midi_in: false,
            midi_out: false,
            file_access: false,
        }
    }

    /// The native tier.
    #[must_use]
    pub const fn native() -> Self {
        Self {
            max_voices: 256,
            max_ir_seconds: 10.0,
            max_grain_density: 1000.0,
            max_grain_size: 2.0,
            max_capture_seconds: 30.0,
            multichannel: 2,
            // Offline `render` through a headless native host (14.5.9).
            offline_render: true,
            midi_in: true,
            midi_out: true,
            file_access: true,
        }
    }

    /// Checks one use against this host's limits.
    ///
    /// # Errors
    /// `beyond-capability` ("not available on this host", with the limit)
    /// at `origin`; a use with no known origin is reported at an empty span
    /// of file 0.
    pub fn require(&self, cap: Cap, origin: Option<Span>) -> Result<(), Diagnostic> {
        let refused = match cap {
            Cap::Voices(n) => (n > self.max_voices).then(|| {
                format!(
                    "{n} voices are not available on this host (limit {})",
                    self.max_voices
                )
            }),
            Cap::IrSeconds(s) => over(s, self.max_ir_seconds, "an impulse response of", "s"),
            Cap::GrainDensity(d) => {
                over(d, self.max_grain_density, "a grain density of", " grains/s")
            }
            Cap::GrainSize(s) => over(s, self.max_grain_size, "a grain size of", "s"),
            Cap::CaptureSeconds(s) => over(s, self.max_capture_seconds, "a capture buffer of", "s"),
            Cap::OfflineRender => missing(
                self.offline_render,
                "offline render is not available on this host",
            ),
            Cap::MidiIn => missing(self.midi_in, "MIDI input is not available on this host"),
            Cap::MidiOut => missing(self.midi_out, "MIDI output is not available on this host"),
            Cap::FileAccess => missing(
                self.file_access,
                "file access is not available on this host",
            ),
        };
        match refused {
            None => Ok(()),
            Some(message) => Err(Diagnostic::error(
                DiagCode::BeyondCapability,
                origin.unwrap_or(Span::new(FileId::new(0), 0, 0)),
                message,
            )),
        }
    }
}

/// The refusal for a quantity above `limit` (NaN is refused too).
fn over(value: f32, limit: f32, what: &str, unit: &str) -> Option<String> {
    let within = value <= limit;
    (!within).then(|| {
        format!("{what} {value}{unit} is not available on this host (limit {limit}{unit})")
    })
}

fn missing(available: bool, message: &str) -> Option<String> {
    (!available).then(|| message.to_string())
}

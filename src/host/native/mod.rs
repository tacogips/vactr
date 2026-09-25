//! The native host tier (design 12.8.10, 16, 17), compiled only under
//! `all(feature = "host-native", not(target_arch = "wasm32"))`.
//!
//! The same core as the browser with a cpal output stream instead of the
//! worklet and lock-free SPSC rings instead of `postMessage`:
//!
//! - `audio`: `NativeAudioHost` (cpal callback running `Engine::process`,
//!   `now()` = frames rendered / sample rate) and its `AudioSide`;
//! - `tick`: the timer thread posting evaluator wakes every 5 ms;
//! - `midi`: midir input (`MidiInHost`) and output (`MidiHost`);
//! - `loader`: the WAV `SampleLoader` and `SourceLoader` confined to the
//!   configured directories.
//!
//! `std::thread`, `std::fs` and `std::time` appear only here (and in
//! `examples/`); the rest of the crate stays I/O-free for wasm32.

pub mod audio;
pub mod loader;
pub mod midi;
pub mod tap;
pub mod tick;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::rc::Rc;

pub use audio::{AudioSide, FrameClock, NativeAudioHost};
pub use loader::{parse_wav, NativeSampleLoader};
pub use midi::{parse_midi, MidiOutQueue, NativeMidiIn, NativeMidiOut};
pub use tick::{TickSource, TICK_PERIOD};

use crate::dsp::cells::AtomicCells;
use crate::host::caps::{Hosts, InstResolver, MidiHost, MidiInHost};
use crate::host::noop::NoopHost;
use crate::reader::span::{FileId, Span};
use crate::types::diag::{DiagCode, Diagnostic};

/// A `beyond-capability` host diagnostic ("not available on this host").
pub(crate) fn unavailable(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(
        DiagCode::BeyondCapability,
        Span::new(FileId::CONSOLE, 0, 0),
        message,
    )
}

/// What the native hosts open.
#[derive(Clone, Debug)]
pub struct NativeConfig {
    /// Control cells shared with the runtime (`RuntimeConfig::cell_pool`).
    pub cells: usize,
    /// Open MIDI input / output at all.
    pub midi_in: bool,
    pub midi_out: bool,
    /// A port name (exact, else substring); `None` takes the first port.
    pub midi_in_port: Option<String>,
    pub midi_out_port: Option<String>,
    /// The only directories samples and loaded sources are read from (17).
    pub sample_roots: Vec<PathBuf>,
    /// Where console-relative paths resolve (the project root).
    pub base: PathBuf,
}

impl Default for NativeConfig {
    /// 1024 cells, MIDI in and out on their first ports, and the current
    /// directory as both the base and the only root.
    fn default() -> Self {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            cells: 1024,
            midi_in: true,
            midi_out: true,
            midi_in_port: None,
            midi_out_port: None,
            sample_roots: vec![base.clone()],
            base,
        }
    }
}

/// The opened native hosts: the bundle for `Runtime::new`, plus what the
/// caller wires elsewhere.
pub struct NativeHosts {
    pub hosts: Hosts,
    /// The cells the callback reads (`Tier::Native` of the runtime).
    pub cells: AtomicCells,
    /// The audio timebase (`Runtime::tick`'s `host_now`).
    pub clock: FrameClock,
    /// The loader; a clone is the evaluator's `SourceLoader`.
    pub loader: NativeSampleLoader,
    /// Capabilities that are absent (MIDI), each "not available on this
    /// host"; their sinks are `NoopHost`.
    pub diags: Vec<Diagnostic>,
}

impl NativeHosts {
    /// Opens audio (required), MIDI in and out (optional), and the loader.
    /// OSC and render are `NoopHost` (TASK-009/010).
    ///
    /// # Errors
    /// `beyond-capability` when no audio output can be opened.
    pub fn open(cfg: &NativeConfig) -> Result<NativeHosts, Diagnostic> {
        Self::open_impl(cfg, None)
    }

    /// `open`, with the audio host's bus names resolver set before it is
    /// boxed, so named-bus taps and captures (`scope`, `spectrum`,
    /// `capture`) resolve through `names` from the first tick (14.5.9).
    ///
    /// # Errors
    /// `beyond-capability` when no audio output can be opened.
    pub fn open_with_bus_names(
        cfg: &NativeConfig,
        names: Rc<dyn InstResolver>,
    ) -> Result<NativeHosts, Diagnostic> {
        Self::open_impl(cfg, Some(names))
    }

    fn open_impl(
        cfg: &NativeConfig,
        names: Option<Rc<dyn InstResolver>>,
    ) -> Result<NativeHosts, Diagnostic> {
        let mut audio = NativeAudioHost::open(cfg)?;
        if let Some(names) = names {
            audio.set_bus_names(names);
        }
        let cells = audio.cells();
        let clock = audio.clock();
        let mut diags = Vec::new();
        let midi_in: Box<dyn MidiInHost> = if cfg.midi_in {
            match NativeMidiIn::open(cfg.midi_in_port.as_deref(), clock.clone()) {
                Ok(m) => Box::new(m),
                Err(d) => {
                    diags.push(d);
                    Box::new(NoopHost)
                }
            }
        } else {
            Box::new(NoopHost)
        };
        let midi: Box<dyn MidiHost> = if cfg.midi_out {
            match NativeMidiOut::open(cfg.midi_out_port.as_deref(), clock.clone()) {
                Ok(m) => Box::new(m),
                Err(d) => {
                    diags.push(d);
                    Box::new(NoopHost)
                }
            }
        } else {
            Box::new(NoopHost)
        };
        let loader = NativeSampleLoader::new(&cfg.sample_roots, &cfg.base);
        let hosts = Hosts {
            audio: Box::new(audio),
            midi,
            osc: Box::new(NoopHost),
            render: Box::new(NoopHost),
            midi_in,
            samples: Box::new(loader.clone()),
        };
        Ok(NativeHosts {
            hosts,
            cells,
            clock,
            loader,
            diags,
        })
    }
}

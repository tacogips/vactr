//! Single-writer, bounded-reader snapshots of final output host signals.

use std::sync::atomic::{fence, AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::host::caps::{HostSigs, MASTER_WAVEFORM_SAMPLES};

#[derive(Debug)]
pub(super) struct SharedSigs {
    sequence: AtomicU64,
    values: [AtomicU32; 9 + MASTER_WAVEFORM_SAMPLES],
    waveform: AtomicBool,
}

impl Default for SharedSigs {
    fn default() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            values: std::array::from_fn(|_| AtomicU32::new(0)),
            waveform: AtomicBool::new(false),
        }
    }
}

impl SharedSigs {
    pub(super) fn store(&self, s: &HostSigs) {
        let sequence = self.sequence.load(Ordering::Relaxed);
        self.sequence
            .store(sequence.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        self.values[0].store(s.amp.to_bits(), Ordering::Relaxed);
        for (slot, value) in self.values[1..9].iter().zip(&s.fft) {
            slot.store(value.to_bits(), Ordering::Relaxed);
        }
        if let Some(samples) = &s.waveform {
            for (slot, value) in self.values[9..].iter().zip(samples) {
                slot.store(value.to_bits(), Ordering::Relaxed);
            }
        }
        self.waveform.store(s.waveform.is_some(), Ordering::Relaxed);
        self.sequence
            .store(sequence.wrapping_add(2), Ordering::Release);
    }

    pub(super) fn load(&self) -> HostSigs {
        let get = |i: usize| f32::from_bits(self.values[i].load(Ordering::Relaxed));
        for _ in 0..4 {
            let before = self.sequence.load(Ordering::Acquire);
            if before & 1 != 0 {
                continue;
            }
            let signals = HostSigs {
                amp: get(0),
                fft: std::array::from_fn(|i| get(i + 1)),
                waveform: self
                    .waveform
                    .load(Ordering::Relaxed)
                    .then(|| std::array::from_fn(|i| get(i + 9))),
            };
            fence(Ordering::Acquire);
            if self.sequence.load(Ordering::Relaxed) == before {
                return signals;
            }
        }
        // Preserve best-effort meters; never claim a torn waveform is valid.
        HostSigs {
            amp: get(0),
            fft: std::array::from_fn(|i| get(i + 1)),
            waveform: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_waveform_transfer_is_optional_coherent_and_allocation_free() {
        let shared = SharedSigs::default();
        assert!(shared.load().waveform.is_none());
        let signals = HostSigs {
            amp: 0.25,
            fft: [0.5; 8],
            waveform: Some([0.75; MASTER_WAVEFORM_SAMPLES]),
        };
        let (loaded, allocations) = crate::dsp::alloc_probe::armed(|| {
            shared.store(&signals);
            shared.load()
        });
        assert_eq!(loaded, signals);
        assert_eq!(allocations, 0);
        shared.sequence.store(3, Ordering::Release);
        assert!(
            shared.load().waveform.is_none(),
            "bounded refusal while writer is busy"
        );
        shared.sequence.store(4, Ordering::Release);
        shared.store(&HostSigs::default());
        assert!(
            shared.load().waveform.is_none(),
            "old samples cannot survive unsupported state"
        );
    }
}

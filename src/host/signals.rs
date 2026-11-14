//! Bounded browser host-signal codec, preserving the original amp/FFT prefix.

use crate::host::caps::{HostSigs, MASTER_WAVEFORM_SAMPLES};

pub(crate) const TAG_SIGS: u8 = 0x61;
const LEGACY_LEN: usize = 1 + 9 * 4;
pub(crate) const SIGNALS_MAX_LEN: usize = LEGACY_LEN + MASTER_WAVEFORM_SAMPLES * 4;

pub(crate) fn encode_signals(s: &HostSigs, out: &mut [u8; SIGNALS_MAX_LEN]) -> usize {
    out[0] = TAG_SIGS;
    out[1..5].copy_from_slice(&s.amp.to_le_bytes());
    for (i, x) in s.fft.iter().enumerate() {
        out[5 + i * 4..9 + i * 4].copy_from_slice(&x.to_le_bytes());
    }
    let Some(waveform) = &s.waveform else {
        return LEGACY_LEN;
    };
    for (bytes, sample) in out[LEGACY_LEN..].chunks_exact_mut(4).zip(waveform) {
        bytes.copy_from_slice(&sample.to_le_bytes());
    }
    SIGNALS_MAX_LEN
}

pub(crate) fn decode_signals(record: &[u8]) -> Option<HostSigs> {
    if record.first() != Some(&TAG_SIGS) || !matches!(record.len(), LEGACY_LEN | SIGNALS_MAX_LEN) {
        return None;
    }
    let float =
        |i: usize| f32::from_le_bytes([record[i], record[i + 1], record[i + 2], record[i + 3]]);
    let waveform = (record.len() == SIGNALS_MAX_LEN)
        .then(|| std::array::from_fn(|i| float(LEGACY_LEN + i * 4)));
    Some(HostSigs {
        amp: float(1),
        fft: std::array::from_fn(|i| float(5 + i * 4)),
        waveform,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_round_trips_without_changing_the_legacy_prefix() {
        let mut s = HostSigs {
            amp: 0.25,
            fft: [0.5; 8],
            waveform: Some(std::array::from_fn(|i| i as f32 / 256.0)),
        };
        let mut bytes = [0; SIGNALS_MAX_LEN];
        assert_eq!(encode_signals(&s, &mut bytes), SIGNALS_MAX_LEN);
        assert_eq!(decode_signals(&bytes), Some(s));
        s.waveform = None;
        assert_eq!(decode_signals(&bytes[..LEGACY_LEN]), Some(s));
        assert_eq!(encode_signals(&s, &mut bytes), LEGACY_LEN);
        assert_eq!(decode_signals(&bytes[..LEGACY_LEN]), Some(s));
    }

    #[test]
    fn malformed_or_partial_extensions_do_not_decode_as_real_waveforms() {
        let mut bytes = [0; SIGNALS_MAX_LEN];
        encode_signals(&HostSigs::default(), &mut bytes);
        for len in 0..SIGNALS_MAX_LEN {
            if len != LEGACY_LEN {
                assert!(decode_signals(&bytes[..len]).is_none(), "length {len}");
            }
        }
        bytes[0] = 0;
        assert!(decode_signals(&bytes).is_none());
    }

    #[test]
    fn encoding_host_signals_allocates_nothing() {
        let signals = HostSigs {
            waveform: Some([0.5; MASTER_WAVEFORM_SAMPLES]),
            ..HostSigs::default()
        };
        let mut bytes = [0; SIGNALS_MAX_LEN];
        let (len, allocations) =
            crate::dsp::alloc_probe::armed(|| encode_signals(&signals, &mut bytes));
        assert_eq!(len, SIGNALS_MAX_LEN);
        assert_eq!(allocations, 0);
    }
}

#!/usr/bin/env python3
"""Compare Vactr's analog-percussion/fm-drum kernels with the pinned Peaks
bass/snare/hat/fm drum sources, without vendoring upstream code or samples.

Both probes trigger one hit at the source's own 48 kHz rate (Peaks' own
`peaks/io_buffer.h` block size is 4 frames; that chunking does not change
the drums' per-sample output, so both probes render one 2.5 s buffer).

Control mapping (documented here, not only in code, per the family plan):

* Frequency: the pinned `peaks/drums/{bass_drum,snare_drum,fm_drum}.h`
  `set_frequency` methods are simple linear formulas over a 128-units-per-
  semitone pitch code (`kPitchTableStart`/`kOctave` in `fm_drum.cc` confirm
  the 128-per-semitone convention; A4 = MIDI note 69 = 440 Hz is the
  standard reference). `bass_hz`, `snare_hz` and `fm_hz` below translate
  each drum's own formula to Hz for the Vactr side; this is arithmetic on
  the visible source formulas, not a value read from a generated table.
  The source high-hat has no frequency control at all (fixed 8 kHz/13 kHz
  internal resonators); Vactr's `hat-frequency` is a Vactr-only extension.
* Punch (bass), tone (bass/snare), snappy (snare) and metal (Vactr hat
  extension) share their 0..1 normalized range by naming convention; the
  source's internal resonance/lowpass curves and Vactr's are not the same
  curve, so a matching 0..1 input is a shared convention, not an equal
  perceptual step size.
* Decay has no closed-form correspondence: the source decay parameter sets
  SVF resonance (`set_decay` in each drum header), not an explicit time
  constant, while Vactr's `*-decay` is seconds. Both sides use independent
  low/medium/high representative settings, documented per scenario below.
* The source FM drum's `noise` parameter (`fm_drum.h` `set_noise`) is dual
  purpose: values at/above its midpoint mix in noise, values at/below it
  add overdrive; Vactr keeps separate `drum-noise` and `drive` controls
  (`THIRD_PARTY_NOTICES.md`, Peaks FM drum design reference). Scenarios
  hold the source `noise` parameter at each end and at the shared boundary
  to compare against a Vactr `drum-noise`-only or `drive`-only setting.
* Vactr's `pitch-sweep` and (for the FM drum) `fm-amount`'s absolute 0..12
  semitone range are Vactr extensions/rescalings: the source's auxiliary
  pitch envelope strength is an automatic function of frequency only, not
  a user control, and the source's `fm_amount` is a raw pot code with no
  documented semitone scale. `pitch-sweep` is held at its 0.5 default in
  every FM scenario; `fm-amount` uses a documented linear 0..12 rescaling
  of the source's normalized `fm_amount` control.

Metrics: RMS, peak, correlation and normalized RMS error use both signals
after onset alignment (each signal trimmed to its own first sample past
2% of its peak, then truncated to the common overlap). Spectral centroid
analyzes a 4096-sample Hann-windowed frame starting at each signal's own
aligned onset. Decay times are read from a block-RMS envelope referenced
to its own peak. Mismatches are measured gaps, not evidence of parity.
"""

import argparse
import cmath
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from compare_plaits_fm import (
    EURORACK_REVISION,
    ROOT,
    STMLIB_REVISION,
    require_clean_tracked_tree,
    revision,
    run,
)


SAMPLE_RATE = 48_000
FRAMES = 120_000
ONSET_FRACTION = 0.02
CENTROID_WINDOW = 4096
ENVELOPE_BLOCK = 240  # 5 ms at 48 kHz


def to_code16(unit):
    return max(0, min(65535, round(unit * 65535)))


def note_to_hz(note):
    return 440.0 * 2.0 ** ((note - 69.0) / 12.0)


def bass_hz(unit):
    """`peaks/drums/bass_drum.h` `set_frequency`: base note 31, +/-7 semitones."""
    frequency = to_code16(unit) - 32768
    code = (31 << 7) + ((frequency * 896) >> 15)
    return note_to_hz(code / 128.0)


def snare_hz(unit):
    """`peaks/drums/snare_drum.h` `set_frequency`: base note 52, +/-7 semitones."""
    frequency = to_code16(unit) - 32768
    code = (52 << 7) + ((frequency * 896) >> 15)
    return note_to_hz(code / 128.0)


def fm_hz(unit):
    """`peaks/drums/fm_drum.h` `set_frequency`: MIDI notes 24..96."""
    code = (24 << 7) + (((72 << 7) * to_code16(unit)) >> 16)
    return note_to_hz(code / 128.0)


# (label, source normalized [0..1] params, Vactr native params)
BASS_SCENARIOS = (
    ("low", (0.35, 0.20, 0.25, 0.20), (bass_hz(0.35), 0.20, 0.25, 0.15)),
    ("mid", (0.50, 0.50, 0.50, 0.50), (bass_hz(0.50), 0.50, 0.50, 0.40)),
    ("high", (0.75, 0.90, 0.80, 0.85), (bass_hz(0.75), 0.90, 0.80, 1.00)),
)
SNARE_SCENARIOS = (
    ("low", (0.35, 0.20, 0.20, 0.20), (snare_hz(0.35), 0.20, 0.20, 0.15)),
    ("mid", (0.50, 0.50, 0.50, 0.50), (snare_hz(0.50), 0.50, 0.50, 0.25)),
    ("high", (0.70, 0.85, 0.90, 0.85), (snare_hz(0.70), 0.85, 0.90, 0.80)),
)
# The source high-hat's Configure() takes no controls, so every scenario
# compares against the same fixed source render; only the Vactr side varies.
HAT_SOURCE_PARAMS = (0.0, 0.0, 0.0, 0.0)
HAT_SCENARIOS = (
    ("low", (2500.0, 0.20, 0.08, 0.30)),
    ("mid", (3900.0, 0.50, 0.12, 0.70)),
    ("high", (6000.0, 0.85, 0.30, 1.00)),
)
FM_SCENARIOS = (
    # noise=0.5 is the source's clean boundary (no noise mix, no overdrive).
    ("clean", (0.40, 0.30, 0.30, 0.50), (fm_hz(0.40), 0.30 * 12.0, 0.5, 0.25, 0.0, 0.0)),
    ("noisy", (0.55, 0.60, 0.50, 1.00), (fm_hz(0.55), 0.60 * 12.0, 0.5, 0.40, 0.9, 0.0)),
    ("driven", (0.65, 0.75, 0.70, 0.00), (fm_hz(0.65), 0.75 * 12.0, 0.5, 0.55, 0.0, 1.0)),
)


def parse_channel(output, label):
    lines = output.splitlines()
    if len(lines) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} rows, got {len(lines)}")
    values = [float(line) for line in lines]
    if not all(math.isfinite(value) for value in values):
        raise ValueError(f"{label}: row is not finite")
    return values


def rms(signal):
    return math.sqrt(sum(value * value for value in signal) / len(signal))


def onset_index(signal, peak):
    if peak <= 0:
        return 0
    threshold = peak * ONSET_FRACTION
    for index, value in enumerate(signal):
        if abs(value) >= threshold:
            return index
    return 0


def align(reference, candidate):
    ref_onset = onset_index(reference, max(abs(v) for v in reference))
    got_onset = onset_index(candidate, max(abs(v) for v in candidate))
    shift = got_onset - ref_onset
    if shift > 0:
        candidate = candidate[shift:]
    elif shift < 0:
        reference = reference[-shift:]
    size = min(len(reference), len(candidate))
    return reference[:size], candidate[:size], ref_onset, got_onset


def correlation(a, b):
    size = len(a)
    a_mean = sum(a) / size
    b_mean = sum(b) / size
    a_centered = [v - a_mean for v in a]
    b_centered = [v - b_mean for v in b]
    a_var = sum(v * v for v in a_centered)
    b_var = sum(v * v for v in b_centered)
    if a_var <= 0 or b_var <= 0:
        return 0.0
    covariance = sum(x * y for x, y in zip(a_centered, b_centered))
    return covariance / math.sqrt(a_var * b_var)


def normalized_rms_error(reference, candidate):
    ref_energy = sum(v * v for v in reference)
    if ref_energy <= 0:
        return None
    error_energy = sum((a - b) ** 2 for a, b in zip(reference, candidate))
    return math.sqrt(error_energy / ref_energy)


def fft(values):
    """Iterative radix-2 Cooley-Tukey FFT; `values` length must be a power of two."""
    n = len(values)
    result = [complex(value) for value in values]
    j = 0
    for i in range(1, n):
        bit = n >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j ^= bit
        if i < j:
            result[i], result[j] = result[j], result[i]
    length = 2
    while length <= n:
        angle = -2j * math.pi / length
        w_len = cmath.exp(angle)
        for start in range(0, n, length):
            w = 1 + 0j
            half = length // 2
            for k in range(half):
                u = result[start + k]
                v = result[start + k + half] * w
                result[start + k] = u + v
                result[start + k + half] = u - v
                w *= w_len
        length <<= 1
    return result


def spectral_centroid(signal, onset, sample_rate):
    window = signal[onset:onset + CENTROID_WINDOW]
    if len(window) < CENTROID_WINDOW:
        window = window + [0.0] * (CENTROID_WINDOW - len(window))
    windowed = [
        value * (0.5 - 0.5 * math.cos(2 * math.pi * i / (CENTROID_WINDOW - 1)))
        for i, value in enumerate(window)
    ]
    spectrum = fft(windowed)
    half = CENTROID_WINDOW // 2
    magnitudes = [abs(spectrum[k]) for k in range(1, half)]
    total = sum(magnitudes)
    if total <= 0:
        return 0.0
    weighted = sum(
        (k + 1) * sample_rate / CENTROID_WINDOW * magnitude
        for k, magnitude in enumerate(magnitudes)
    )
    return weighted / total


def block_envelope(signal):
    blocks = []
    for start in range(0, len(signal), ENVELOPE_BLOCK):
        block = signal[start:start + ENVELOPE_BLOCK]
        blocks.append(rms(block) if block else 0.0)
    return blocks


def decay_time_seconds(signal, sample_rate, down_db):
    envelope = block_envelope(signal)
    peak = max(envelope) if envelope else 0.0
    if peak <= 0:
        return None
    threshold = peak * (10.0 ** (-down_db / 20.0))
    peak_block = envelope.index(peak)
    for index in range(peak_block, len(envelope)):
        if all(value <= threshold for value in envelope[index:]):
            return index * ENVELOPE_BLOCK / sample_rate
    return None


def metrics(reference, candidate, sample_rate):
    ref_peak = max(abs(v) for v in reference)
    got_peak = max(abs(v) for v in candidate)
    ref_onset = onset_index(reference, ref_peak)
    got_onset = onset_index(candidate, got_peak)
    aligned_ref, aligned_got, _, _ = align(reference, candidate)
    return {
        "reference_rms": rms(reference),
        "vactr_rms": rms(candidate),
        "reference_peak": ref_peak,
        "vactr_peak": got_peak,
        "onset_frames": {"reference": ref_onset, "vactr": got_onset},
        "correlation_after_alignment": correlation(aligned_ref, aligned_got),
        "normalized_rms_error_after_alignment":
            normalized_rms_error(aligned_ref, aligned_got),
        "reference_spectral_centroid_hz":
            spectral_centroid(reference, ref_onset, sample_rate),
        "vactr_spectral_centroid_hz":
            spectral_centroid(candidate, got_onset, sample_rate),
        "reference_decay_20db_s": decay_time_seconds(reference, sample_rate, 20.0),
        "vactr_decay_20db_s": decay_time_seconds(candidate, sample_rate, 20.0),
        "reference_decay_40db_s": decay_time_seconds(reference, sample_rate, 40.0),
        "vactr_decay_40db_s": decay_time_seconds(candidate, sample_rate, 40.0),
    }


def run_upstream(executable, drum, params):
    args = [str(v) for v in params]
    return parse_channel(run([str(executable), drum, *args]), f"upstream {drum}")


def run_vactr(cargo_env, drum, params):
    args = [str(v) for v in params]
    return parse_channel(
        run(["cargo", "run", "-q", "--example", "peaks_drums_reference", "--",
             drum, *args], cwd=ROOT, env=cargo_env),
        f"vactr {drum}",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path,
                        help="separate official Eurorack checkout at the pinned revision")
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean_tracked_tree(source)
    require_clean_tracked_tree(source / "stmlib")
    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()

    with tempfile.TemporaryDirectory(prefix="vactr-peaks-drums-") as temp:
        executable = Path(temp) / "peaks_drums_reference"
        cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
        run([
            clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
            str(ROOT / "verification/peaks_drums_reference.cc"),
            str(source / "peaks/drums/bass_drum.cc"),
            str(source / "peaks/drums/snare_drum.cc"),
            str(source / "peaks/drums/high_hat.cc"),
            str(source / "peaks/drums/fm_drum.cc"),
            str(source / "peaks/resources.cc"),
            str(source / "stmlib/utils/random.cc"),
            cxx_library,
            "-o", str(executable),
        ], cwd=ROOT)
        cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")

        drums = {}

        def compare(drum_label, scenarios):
            results = []
            for label, source_params, vactr_params in scenarios:
                upstream = run_upstream(executable, drum_label, source_params)
                vactr = run_vactr(cargo_env, drum_label, vactr_params)
                results.append({
                    "label": label,
                    "source_params": source_params,
                    "vactr_params": vactr_params,
                    "metrics": metrics(upstream, vactr, SAMPLE_RATE),
                })
            drums[drum_label] = results

        compare("bass", BASS_SCENARIOS)
        compare("snare", SNARE_SCENARIOS)
        compare("fm", FM_SCENARIOS)

        hat_results = []
        hat_upstream = run_upstream(executable, "hat", HAT_SOURCE_PARAMS)
        for label, vactr_params in HAT_SCENARIOS:
            vactr = run_vactr(cargo_env, "hat", vactr_params)
            hat_results.append({
                "label": label,
                "source_params": HAT_SOURCE_PARAMS,
                "vactr_params": vactr_params,
                "metrics": metrics(hat_upstream, vactr, SAMPLE_RATE),
            })
        drums["hat"] = hat_results

    print(json.dumps({
        "scope": "raw Peaks bass/snare/hat/fm drum kernels vs. Vactr "
                 "analog-percussion (modes 0/1/2) and fm-drum; single "
                 "triggered hit each, no LPG, sequencer or .vact host",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE,
        "frames_compared": FRAMES,
        "onset_fraction": ONSET_FRACTION,
        "spectral_centroid_window_frames": CENTROID_WINDOW,
        "drums": drums,
    }, indent=2))


if __name__ == "__main__":
    main()

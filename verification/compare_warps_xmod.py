#!/usr/bin/env python3
"""Compare raw Warps XMOD positions with Vactr's raw dual-mod kernel."""

import argparse
import json
import math
import os
from pathlib import Path
import struct
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


SAMPLE_RATE = 96_000
BLOCK_FRAMES = 60
FRAMES = 12_000
WARMUP = 1_200
NOTE = 48.0
EXTERNAL_CARRIER_HZ = 220.0
MODULATOR_HZ = 137.0
POSITIONS = tuple(index * 0.5 for index in range(11))
TIMBRES = (0.1, 0.5, 0.9)
DRIVES = ((0.4, 0.6), (0.9, 0.7))
SHAPES = (0, 1, 2, 3)


def run_checked(argv, *, cwd=None, env=None):
    return subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=True,
        capture_output=True,
        text=True,
        timeout=180,
    ).stdout


def generate_input(path):
    """Write a common deterministic external sine/modulator saw input file."""
    values = bytearray()
    for frame in range(FRAMES):
        time = frame / SAMPLE_RATE
        carrier = 0.48 * math.sin(math.tau * EXTERNAL_CARRIER_HZ * time)
        phase = (MODULATOR_HZ * time) % 1.0
        modulator = 0.58 * (2.0 * phase - 1.0)
        values.extend(struct.pack("<ff", carrier, modulator))
    path.write_bytes(values)


def parse_channels(output, label):
    lines = output.splitlines()
    if len(lines) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} rows, got {len(lines)}")
    main = []
    aux = []
    for number, line in enumerate(lines, 1):
        fields = line.split()
        if len(fields) != 2:
            raise ValueError(f"{label}: row {number} is not a channel pair")
        a, b = map(float, fields)
        if not math.isfinite(a) or not math.isfinite(b):
            raise ValueError(f"{label}: row {number} is not finite")
        main.append(a)
        aux.append(b)
    return main, aux


def estimate_lag(reference, candidate):
    """Estimate candidate delay in samples, positive when candidate lags."""
    best_lag = 0
    best_score = -1.0
    stride = 4
    for lag in range(-48, 49):
        start = max(WARMUP, WARMUP - lag)
        end = min(FRAMES, FRAMES - lag)
        cross = 0.0
        ref_energy = 0.0
        got_energy = 0.0
        for index in range(start, end, stride):
            a = reference[index]
            b = candidate[index + lag]
            cross += a * b
            ref_energy += a * a
            got_energy += b * b
        denom = math.sqrt(ref_energy * got_energy)
        score = abs(cross / denom) if denom else 0.0
        if score > best_score:
            best_lag = lag
            best_score = score
    return best_lag


def fft_power(samples, size=8192):
    """Small radix-2 FFT used to keep the probe dependency-free."""
    windowed = []
    count = min(len(samples), size)
    for index in range(size):
        if index < count:
            hann = 0.5 - 0.5 * math.cos(math.tau * index / max(count - 1, 1))
            windowed.append(complex(samples[index] * hann, 0.0))
        else:
            windowed.append(0j)
    j = 0
    for i in range(1, size):
        bit = size >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j ^= bit
        if i < j:
            windowed[i], windowed[j] = windowed[j], windowed[i]
    length = 2
    while length <= size:
        angle = -math.tau / length
        root = complex(math.cos(angle), math.sin(angle))
        for start in range(0, size, length):
            factor = 1 + 0j
            half = length // 2
            for offset in range(half):
                even = windowed[start + offset]
                odd = windowed[start + offset + half] * factor
                windowed[start + offset] = even + odd
                windowed[start + offset + half] = even - odd
                factor *= root
        length *= 2
    return [value.real * value.real + value.imag * value.imag for value in windowed[:size // 2 + 1]]


def spectral_metrics(samples, carrier_hz):
    samples = samples[WARMUP:]
    power = fft_power(samples)
    bin_hz = SAMPLE_RATE / 8192
    total = sum(power[1:])
    if total <= 0.0:
        return {"centroid_hz": 0.0, "harmonic_energy_ratio": 0.0,
                "intermodulation_energy_ratio": 0.0}
    centroid = sum(index * bin_hz * energy for index, energy in enumerate(power[1:], 1)) / total

    def band_bins(frequency):
        center = int(round(frequency / bin_hz))
        return set(range(max(1, center - 1), min(len(power), center + 2)))

    harmonic_bins = set()
    for multiple in range(1, int((SAMPLE_RATE / 2) / carrier_hz) + 1):
        harmonic_bins.update(band_bins(multiple * carrier_hz))
    intermod_bins = set()
    mod_hz = MODULATOR_HZ
    nyquist = SAMPLE_RATE / 2
    for carrier_multiple in range(1, 7):
        for mod_multiple in range(1, 7):
            for sign in (-1, 1):
                frequency = abs(carrier_multiple * carrier_hz + sign * mod_multiple * mod_hz)
                if 0 < frequency < nyquist:
                    intermod_bins.update(band_bins(frequency))
    intermod_bins.difference_update(harmonic_bins)
    harmonic = sum(power[index] for index in harmonic_bins)
    intermod = sum(power[index] for index in intermod_bins)
    return {
        "centroid_hz": centroid,
        "harmonic_energy_ratio": harmonic / total,
        "intermodulation_energy_ratio": intermod / total,
    }


def channel_metrics(reference, candidate, carrier_hz):
    lag = estimate_lag(reference, candidate)
    start = max(WARMUP, WARMUP + lag)
    end = min(FRAMES, FRAMES + lag)
    ref = reference[start:end]
    got = candidate[start + lag:end + lag]
    size = len(ref)
    ref_mean = sum(ref) / size
    got_mean = sum(got) / size
    ref_energy = sum(value * value for value in ref)
    got_energy = sum(value * value for value in got)
    error_energy = sum((a - b) ** 2 for a, b in zip(ref, got))
    ref_centered = [value - ref_mean for value in ref]
    got_centered = [value - got_mean for value in got]
    covariance = sum(a * b for a, b in zip(ref_centered, got_centered))
    ref_variance = sum(value * value for value in ref_centered)
    got_variance = sum(value * value for value in got_centered)
    correlation = (covariance / math.sqrt(ref_variance * got_variance)
                   if ref_variance > 0.0 and got_variance > 0.0 else 0.0)
    return {
        "estimated_latency_samples": lag,
        "estimated_latency_ms": lag * 1000.0 / SAMPLE_RATE,
        "reference_rms": math.sqrt(ref_energy / size),
        "vactr_rms": math.sqrt(got_energy / size),
        "correlation": correlation,
        "normalized_rms_error": math.sqrt(error_energy / ref_energy) if ref_energy else 0.0,
        "reference_spectrum": spectral_metrics(reference, carrier_hz),
        "vactr_spectrum": spectral_metrics(candidate, carrier_hz),
    }


def compile_reference(source, executable):
    clang = run_checked(["mise", "which", "clang"], cwd=ROOT).strip()
    cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
    files = [
        ROOT / "verification/warps_xmod_reference.cc",
        source / "warps/dsp/modulator.cc",
        source / "warps/dsp/oscillator.cc",
        source / "warps/dsp/vocoder.cc",
        source / "warps/dsp/filter_bank.cc",
        source / "warps/resources.cc",
        source / "stmlib/dsp/units.cc",
        source / "stmlib/utils/random.cc",
    ]
    run_checked([
        clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
        *(str(path) for path in files), cxx_library, "-o", str(executable),
    ], cwd=ROOT)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean_tracked_tree(source)
    require_clean_tracked_tree(source / "stmlib")

    cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")
    scenarios = []
    aggregate = {position: [] for position in POSITIONS}
    with tempfile.TemporaryDirectory(prefix="vactr-warps-xmod-") as temporary:
        temp = Path(temporary)
        executable = temp / "warps_xmod_reference"
        input_path = temp / "stereo_input.f32"
        generate_input(input_path)
        compile_reference(source, executable)
        for position in POSITIONS:
            for timbre in TIMBRES:
                for carrier_drive, modulator_drive in DRIVES:
                    for shape in SHAPES:
                        carrier_hz = (EXTERNAL_CARRIER_HZ if shape == 0 else
                                      440.0 * 2.0 ** ((NOTE - 69.0) / 12.0))
                        source_args = [
                            str(position), str(timbre), str(carrier_drive),
                            str(modulator_drive), str(shape), str(NOTE),
                            str(input_path), str(FRAMES),
                        ]
                        vactr_args = [
                            "cargo", "run", "-q", "--example", "warps_xmod_reference", "--",
                            str(position), str(timbre), str(carrier_drive),
                            str(modulator_drive), str(shape), str(carrier_hz),
                            str(input_path), str(FRAMES),
                        ]
                        upstream = parse_channels(
                            run_checked([str(executable), *source_args]), "upstream")
                        vactr = parse_channels(
                            run_checked(vactr_args, cwd=ROOT, env=cargo_env), "vactr")
                        metrics = {
                            "main": channel_metrics(upstream[0], vactr[0], carrier_hz),
                            "aux": channel_metrics(upstream[1], vactr[1], carrier_hz),
                        }
                        aggregate[position].append(metrics)
                        scenarios.append({
                            "algorithm_position": position,
                            "source_modulation_algorithm": position / 8.0,
                            "timbre": timbre,
                            "carrier_drive": carrier_drive,
                            "modulator_drive": modulator_drive,
                            "carrier_shape": shape,
                            "carrier_role": "external" if shape == 0 else "internal",
                            **metrics,
                        })

    classifications = []
    for position, rows in aggregate.items():
        values = [pair["main"]["normalized_rms_error"] for pair in rows]
        correlations = [pair["main"]["correlation"] for pair in rows]
        mean_error = sum(values) / len(values)
        mean_correlation = sum(correlations) / len(correlations)
        if position in (0.5, 1.0, 1.5):
            classification = "not comparable"
            reason = "This point includes the authored fold adaptation, so the source fold contribution is not comparable."
        elif mean_error <= 0.10 and mean_correlation >= 0.95:
            classification = "close"
            reason = "Aggregate stereo error and correlation meet the probe's numeric-close rule."
        else:
            classification = "measured gap"
            reason = "The stage is mapped, but measured stereo output exceeds the numeric-close rule."
        classifications.append({
            "algorithm_position": position,
            "classification": classification,
            "mean_normalized_rms_error": mean_error,
            "mean_correlation": mean_correlation,
            "scenario_count": len(rows),
            "reason": reason,
        })

    print(json.dumps({
        "scope": "raw Warps Modulator::Process XMOD versus raw Vactr cross_mod kernel",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE,
        "block_frames": BLOCK_FRAMES,
        "frames": FRAMES,
        "warmup_frames": WARMUP,
        "input": "Python-generated 220 Hz carrier sine and 137 Hz noise-free saw, same interleaved float32 input for both probes",
        "mapping": "source algorithm position / 8; Vactr public algorithm position unchanged; drives, timbre, and carrier shape passed directly",
        "classification_basis": "Main output represents the algorithm response. Auxiliary metrics are reported separately because Warps applies fixed half-gain when converting every aux output to signed 16-bit: internal carriers provide a full-scale oscillator, while external-carrier aux is the mean of two inputs.",
        "close_rule": "mean main-channel NRMSE <= 0.10 and mean main-channel correlation >= 0.95; harmonic and intermodulation bins are disjoint; this is a measurement label, not parity certification",
        "classifications": classifications,
        "scenarios": scenarios,
    }, indent=2))


if __name__ == "__main__":
    main()

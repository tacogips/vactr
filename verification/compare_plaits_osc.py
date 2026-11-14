#!/usr/bin/env python3
"""Measure raw Plaits oscillator-family kernels against Vactr kernels."""

import argparse
import cmath
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
EURORACK_REVISION = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4"
STMLIB_REVISION = "e3bd7c9cc00e4364166f9905c0509b6ffd0535ec"
SAMPLE_RATE = 48000
BLOCK_FRAMES = 24
BLOCKS = 3000
FRAMES = BLOCK_FRAMES * BLOCKS
WARMUP = BLOCK_FRAMES
SPECTRAL_WINDOW = 2048
POSITIONS = (0, 1, 7, 8, 9, 11, 12)
SCENARIOS = (
    (0.1, 0.25, 0.2, 0.8, 1.0),
    (0.5, 0.5, 0.5, 0.8, 1.0),
    (0.9, 0.75, 0.8, 0.8, 1.0),
    (0.5, 0.5, 0.5, 0.3, 0.0),
)
ENGINE_CC = (
    "plaits/dsp/engine2/virtual_analog_vcf_engine.cc",
    "plaits/dsp/engine2/phase_distortion_engine.cc",
    "plaits/dsp/engine2/chiptune_engine.cc",
    "plaits/dsp/engine/virtual_analog_engine.cc",
    "plaits/dsp/engine/waveshaping_engine.cc",
    "plaits/dsp/engine/grain_engine.cc",
    "plaits/dsp/engine/additive_engine.cc",
    "plaits/dsp/chords/chord_bank.cc",
    "stmlib/utils/random.cc",
)


def run(argv, *, cwd=None, env=None, timeout=180):
    completed = subprocess.run(argv, cwd=cwd, env=env, check=False,
                               capture_output=True, text=True, timeout=timeout)
    if completed.returncode:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {argv!r}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed.stdout


def revision(path):
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).strip()


def require_clean_tracked_tree(path):
    if run(["git", "-C", str(path), "status", "--porcelain",
            "--untracked-files=no"]).strip():
        raise ValueError(f"reference checkout has modified tracked files: {path}")


def parse_channels(output, label):
    lines = output.splitlines()
    if len(lines) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} rows, got {len(lines)}")
    channels = ([], [])
    for row, line in enumerate(lines, 1):
        fields = line.split()
        if len(fields) != 2:
            raise ValueError(f"{label}: row {row} is not a channel pair")
        values = tuple(float(field) for field in fields)
        if not all(math.isfinite(value) for value in values):
            raise ValueError(f"{label}: row {row} is nonfinite")
        channels[0].append(values[0])
        channels[1].append(values[1])
    return channels


def fft(values):
    """In-place radix-2 complex FFT, used for the small spectral metrics."""
    data = [complex(value) for value in values]
    n = len(data)
    j = 0
    for i in range(1, n):
        bit = n >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j ^= bit
        if i < j:
            data[i], data[j] = data[j], data[i]
    length = 2
    while length <= n:
        step = cmath.exp(-2j * math.pi / length)
        half = length >> 1
        for start in range(0, n, length):
            factor = 1.0 + 0.0j
            for offset in range(half):
                even = data[start + offset]
                odd = factor * data[start + offset + half]
                data[start + offset] = even + odd
                data[start + offset + half] = even - odd
                factor *= step
        length <<= 1
    return data


def inverse_fft(values):
    conjugated = [value.conjugate() for value in values]
    transformed = fft(conjugated)
    size = len(values)
    return [value.conjugate().real / size for value in transformed]


def spectral_metrics(samples):
    size = min(SPECTRAL_WINDOW, len(samples))
    segment = samples[-size:]
    if size < 2:
        return {"spectral_centroid_hz": None, "fundamental_hz": None}
    windowed = [x * (0.5 - 0.5 * math.cos(2.0 * math.pi * i / (size - 1)))
                for i, x in enumerate(segment)]
    spectrum = fft(windowed)
    powers = [abs(value) ** 2 for value in spectrum[: size // 2 + 1]]
    weighted = sum(i * SAMPLE_RATE / size * power
                   for i, power in enumerate(powers[1:], 1))
    total = sum(powers[1:])
    centroid = weighted / total if total > 1.0e-24 else None

    # FFT autocorrelation estimates the lowest strong periodic component,
    # avoiding a full quadratic search across the 24-frame comparison.
    padded = [complex(value, 0.0) for value in windowed]
    padded.extend([0j] * size)
    correlation_spectrum = fft([value.real for value in padded])
    correlation = inverse_fft([value * value.conjugate()
                               for value in correlation_spectrum])
    minimum_lag = max(2, SAMPLE_RATE // 2000)
    maximum_lag = min(size - 2, SAMPLE_RATE // 35)
    candidates = []
    for lag in range(minimum_lag + 1, maximum_lag):
        if correlation[lag] >= correlation[lag - 1] and correlation[lag] > correlation[lag + 1]:
            candidates.append((correlation[lag] / max(correlation[0], 1.0e-24), lag))
    candidates = [candidate for candidate in candidates if candidate[0] > 0.12]
    if candidates:
        strength, lag = max(candidates, key=lambda item: (item[0], -item[1]))
        left, center, right = correlation[lag - 1:lag + 2]
        denominator = left - 2.0 * center + right
        fraction = (0.5 * (left - right) / denominator
                    if abs(denominator) > 1.0e-24 else 0.0)
        fundamental = SAMPLE_RATE / (lag + fraction) if strength > 0 else None
    else:
        fundamental = None
    return {"spectral_centroid_hz": centroid, "fundamental_hz": fundamental}


def correlation(a, b):
    size = min(len(a), len(b))
    mean_a = sum(a[:size]) / size
    mean_b = sum(b[:size]) / size
    centered_a = [x - mean_a for x in a[:size]]
    centered_b = [x - mean_b for x in b[:size]]
    var_a = sum(x * x for x in centered_a)
    var_b = sum(x * x for x in centered_b)
    if var_a <= 0.0 or var_b <= 0.0:
        return None
    return sum(x * y for x, y in zip(centered_a, centered_b)) / math.sqrt(var_a * var_b)


def pair_metrics(reference, candidate):
    reference = reference[WARMUP:]
    candidate = candidate[WARMUP:]
    size = min(len(reference), len(candidate))
    reference = reference[:size]
    candidate = candidate[:size]

    def raw_metrics(a, b):
        energy = sum(x * x for x in a)
        got_energy = sum(x * x for x in b)
        err_energy = sum((x - y) ** 2 for x, y in zip(a, b))
        return {
            "reference_rms": math.sqrt(energy / len(a)),
            "vactr_rms": math.sqrt(got_energy / len(b)),
            "correlation": correlation(a, b),
            "normalized_rms_error": math.sqrt(err_energy / energy) if energy else None,
        }

    base = raw_metrics(reference, candidate)
    period = pair_metrics_period(reference)
    max_lag = min(period, size // 4)
    best = (None, 0)
    window = min(size, 2048)
    for lag in range(-max_lag, max_lag + 1):
        if lag >= 0:
            left, right = reference[lag:window], candidate[:window - lag]
        else:
            left, right = reference[:window + lag], candidate[-lag:window]
        score = correlation(left, right)
        if score is not None and (best[0] is None or score > best[0]):
            best = (score, lag)
    lag = best[1]
    if lag >= 0:
        aligned_ref, aligned_got = reference[lag:], candidate[:size - lag]
    else:
        aligned_ref, aligned_got = reference[:size + lag], candidate[-lag:]
    aligned = raw_metrics(aligned_ref, aligned_got)
    return {
        "after_warmup": base,
        "best_integer_lag_samples_within_one_period": lag,
        "after_best_lag_alignment": aligned,
        "reference_spectrum": spectral_metrics(reference),
        "vactr_spectrum": spectral_metrics(candidate),
    }


def pair_metrics_period(samples):
    spectral = spectral_metrics(samples)
    fundamental = spectral["fundamental_hz"]
    if fundamental is None or fundamental <= 0:
        return 1
    return max(1, int(math.ceil(SAMPLE_RATE / fundamental)))


MAPS = {
    0: "Hz->va_filter source[0]/filter[1]; morph->source[1]; timbre->filter[2]; harmonics->filter[3] (filter-harmonics); mode selects LP/HP. Accent/trigger have no raw-kernel input.",
    1: "Hz->phase[0]; harmonics->phase-harmonics[1] using original equal-step ratio; timbre[2]; morph[3]; mode selects synchronized main/free-running aux. Accent/trigger unused.",
    7: "Hz->chip[0]; harmonics->chip-chord[1]; timbre[2]; morph->chip shape[3]; trigger->chip-clocked[5]; mode selects chord/arpeggio main versus bass aux. chip-rate fixed at 8; accent unused. External source trigger clock differs from Vactr internal clock.",
    8: "Hz->analog[0]; harmonics->analog-detune[1] (including authored five-interval map); timbre[2]; morph[3]; mode selects main/sync-difference aux. Accent/trigger unused.",
    9: "Hz->shape[0]; harmonics->shape-harmonics[1]; timbre[2]; morph[3]; mode selects folded main/overtone aux. Accent/trigger unused.",
    11: "Hz->grain[0]; harmonics->grain-harmonics[1]; timbre[2]; morph[3]; mode selects two-grainlet main/Z aux. Accent/trigger unused; event reset is outside raw kernel.",
    12: "Hz->spectrum[0]; source timbre centroid->Vactr timbre[1]; morph slope->morph[2]; harmonics bump->spectrum-bumps[3]; mode selects 24-partial main/eight-organ-stop aux. Accent/trigger unused.",
}

GAP_REASONS = {
    0: "The low-pass level and auxiliary spectrum differ; Vactr's analytic oscillator and filters do not match the upstream SVF stages.",
    1: "The original equal-step ratio map and analytic two-substep oscillator differ from the curated source quantizer and source oscillator/downsampling behavior.",
    8: "The analytic variable oscillators and sync-difference auxiliary retain waveform and phase gaps relative to the source BLEP oscillators.",
    9: "The independent fold/overtone equations and slope antialiasing differ from the generated source waveshaper behavior.",
    11: "The grainlet main aligns closely in the center case, while auxiliary Z/high-pass behavior and other settings remain outside the close threshold.",
    12: "Centroid and level track closely, while source amplitude-state recurrence and oscillator phase leave waveform error.",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path,
                        help="separate pinned Eurorack checkout")
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean_tracked_tree(source)
    require_clean_tracked_tree(source / "stmlib")
    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()
    with tempfile.TemporaryDirectory(prefix="vactr-plaits-osc-") as temp:
        executable = Path(temp) / "plaits_osc_reference"
        cxx = "-lc++" if sys.platform == "darwin" else "-lstdc++"
        run([clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
             str(ROOT / "verification/plaits_osc_reference.cc"),
             *[str(source / path) for path in ENGINE_CC],
             str(source / "plaits/resources.cc"),
             str(source / "stmlib/dsp/units.cc"), cxx, "-o", str(executable)], cwd=ROOT)
        env = dict(os.environ, CARGO_TERM_QUIET="true")
        results = []
        for position in POSITIONS:
            for index, (harmonics, timbre, morph, accent, trigger) in enumerate(SCENARIOS):
                controls = [str(value) for value in (harmonics, timbre, morph, accent, trigger)]
                source_samples = parse_channels(run([str(executable), str(position), *controls]),
                                                f"upstream position {position}")
                vactr_samples = parse_channels(run(["cargo", "run", "-q", "--example",
                                                     "plaits_osc_reference", "--",
                                                     str(position), "69", *controls], cwd=ROOT, env=env),
                                                f"Vactr position {position}")
                results.append({
                    "position": position, "scenario": index,
                    "harmonics": harmonics, "timbre": timbre, "morph": morph,
                    "accent": accent, "trigger": trigger,
                    "main": pair_metrics(source_samples[0], vactr_samples[0]),
                    "aux": pair_metrics(source_samples[1], vactr_samples[1]),
                })
    summary = {}
    for position in POSITIONS:
        rows = [row for row in results if row["position"] == position]
        worst_error = max(row[channel]["after_best_lag_alignment"]["normalized_rms_error"]
                          for row in rows for channel in ("main", "aux")
                          if row[channel]["after_best_lag_alignment"]["normalized_rms_error"] is not None)
        best_corr = min(row[channel]["after_best_lag_alignment"]["correlation"]
                        for row in rows for channel in ("main", "aux")
                        if row[channel]["after_best_lag_alignment"]["correlation"] is not None)
        if position == 7:
            classification = "not comparable"
            reason = "Metrics are reported, but source external-trigger arpeggiation and Vactr's internal chip clock have no one-to-one timing/control contract."
        else:
            classification = ("close" if worst_error < 0.1 and best_corr >= 0.99
                              else "measured gap")
            reason = ("Aligned waveform metrics meet the documented close threshold."
                      if classification == "close" else GAP_REASONS[position])
        summary[position] = {"classification": classification, "reason": reason}
    print(json.dumps({
        "scope": "raw oscillator-family engine/kernel comparison only; no Plaits voice, LPG, host or event path",
        "upstream_revision": EURORACK_REVISION, "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE, "block_frames": BLOCK_FRAMES,
        "frames": FRAMES, "warmup_frames": WARMUP,
        "note": 69,
        "vactr_hz": 440.0 * SAMPLE_RATE / 47872.34,
        "close_criterion": "every scenario and channel has lag-aligned normalized RMS error < 0.10 and correlation >= 0.99",
        "fundamental_estimator": "parabolically interpolated peak in FFT-derived autocorrelation",
        "control_mappings": MAPS, "classifications": summary,
        "scenarios": results,
    }, indent=2))


if __name__ == "__main__":
    main()

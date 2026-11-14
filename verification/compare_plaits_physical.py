#!/usr/bin/env python3
"""Compare raw Plaits positions 16-20 with Vactr kernels at firmware rate."""

import argparse
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
MAX_LAG = 32
SPECTRAL_WINDOW = 4096
ENVELOPE_WINDOW = 240
ENGINES = ("swarm", "noise", "particle", "string", "modal")
SCENARIOS = (
    (0.1, 0.25, 0.2, 1.0),
    (0.5, 0.5, 0.5, 1.0),
    (0.9, 0.75, 0.8, 1.0),
    (0.5, 0.5, 0.5, 0.3),
)
BANDS_HZ = (0, 125, 250, 500, 1000, 2000, 4000, 8000, 12000, 24000)


def run(argv, *, cwd=None, env=None, timeout=180):
    result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True,
                           text=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(
            f"command failed ({result.returncode}): {argv[0]}\n{result.stderr}"
        )
    return result.stdout


def revision(path):
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).strip()


def require_clean_tracked_tree(path):
    dirty = run(["git", "-C", str(path), "status", "--porcelain",
                 "--untracked-files=no"]).strip()
    if dirty:
        raise ValueError(f"reference checkout has modified tracked files: {path}")


def parse_channels(output, label):
    lines = output.splitlines()
    if len(lines) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} rows, got {len(lines)}")
    channels = ([], [])
    for number, line in enumerate(lines, 1):
        fields = line.split()
        if len(fields) != 2:
            raise ValueError(f"{label}: row {number} is not a channel pair")
        values = tuple(float(value) for value in fields)
        if not all(math.isfinite(value) for value in values):
            raise ValueError(f"{label}: row {number} is not finite")
        channels[0].append(values[0])
        channels[1].append(values[1])
    return channels


def rms(samples):
    return math.sqrt(sum(value * value for value in samples) / len(samples)) if samples else 0.0


def correlation(left, right):
    size = min(len(left), len(right))
    if size == 0:
        return None
    left_mean = sum(left[:size]) / size
    right_mean = sum(right[:size]) / size
    a = [x - left_mean for x in left[:size]]
    b = [x - right_mean for x in right[:size]]
    aa = sum(x * x for x in a)
    bb = sum(x * x for x in b)
    return sum(x * y for x, y in zip(a, b)) / math.sqrt(aa * bb) if aa and bb else None


def aligned_pair(reference, candidate):
    reference_window = reference[WARMUP:WARMUP + SPECTRAL_WINDOW]
    candidate_window = candidate[WARMUP:WARMUP + SPECTRAL_WINDOW]
    best_lag = 0
    best_score = -math.inf
    for lag in range(-MAX_LAG, MAX_LAG + 1):
        if lag >= 0:
            left = reference_window[lag:]
            right = candidate_window[:len(candidate_window) - lag or None]
        else:
            left = reference_window[:len(reference_window) + lag]
            right = candidate_window[-lag:]
        score = sum(a * b for a, b in zip(left, right))
        if score > best_score:
            best_lag, best_score = lag, score
    if best_lag >= 0:
        return (reference_window[best_lag:], candidate_window[:len(candidate_window) - best_lag or None], best_lag)
    return (reference_window[:len(reference_window) + best_lag], candidate_window[-best_lag:], best_lag)


def fft_power(samples):
    n = 1 << (min(len(samples), SPECTRAL_WINDOW).bit_length() - 1)
    if n < 8:
        return []
    data = [0j] * n
    for i in range(n):
        hann = 0.5 - 0.5 * math.cos(2.0 * math.pi * i / (n - 1))
        data[i] = complex(samples[i] * hann, 0.0)
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
        angle = -2.0 * math.pi / length
        root = complex(math.cos(angle), math.sin(angle))
        for start in range(0, n, length):
            factor = 1.0 + 0.0j
            half = length // 2
            for offset in range(half):
                even = data[start + offset]
                odd = factor * data[start + offset + half]
                data[start + offset] = even + odd
                data[start + offset + half] = even - odd
                factor *= root
        length *= 2
    return [abs(value) ** 2 for value in data[:n // 2 + 1]]


def spectral_metrics(samples):
    data = samples[WARMUP:WARMUP + SPECTRAL_WINDOW]
    powers = fft_power(data)
    if not powers:
        return {"centroid_hz": None, "dominant_peak_hz": None,
                "resonance_peak_hz": None, "octave_band_energy_ratio": {}}
    n = (len(powers) - 1) * 2
    frequencies = [index * SAMPLE_RATE / n for index in range(len(powers))]
    total_power = sum(powers[1:])
    centroid = sum(f * p for f, p in zip(frequencies[1:], powers[1:])) / total_power if total_power else None
    peaks = [(p, f) for f, p in zip(powers[1:], frequencies[1:]) if 20 <= f <= 20000]
    dominant = max(peaks)[1] if peaks else None
    resonances = [(p, f) for p, f in peaks if 40 <= f <= 4000]
    resonance = max(resonances)[1] if resonances else None
    bands = {}
    for low, high in zip(BANDS_HZ, BANDS_HZ[1:]):
        energy = sum(p for p, f in zip(powers, frequencies) if low <= f < high)
        bands[f"{low}-{high}Hz"] = energy / total_power if total_power else None
    return {"centroid_hz": centroid, "dominant_peak_hz": dominant,
            "resonance_peak_hz": resonance, "octave_band_energy_ratio": bands}


def decay_time_ms(samples, drop_db):
    envelopes = [rms(samples[i:i + ENVELOPE_WINDOW])
                 for i in range(0, len(samples), ENVELOPE_WINDOW)]
    peak_value = max(envelopes, default=0.0)
    if peak_value <= 0.0:
        return None
    peak_index = envelopes.index(peak_value)
    threshold = peak_value * 10.0 ** (-drop_db / 20.0)
    for index in range(peak_index, len(envelopes) - 2):
        if all(value <= threshold for value in envelopes[index:index + 3]):
            return (index - peak_index) * ENVELOPE_WINDOW * 1000.0 / SAMPLE_RATE
    return None


def channel_metrics(reference, candidate):
    reference_run = reference[WARMUP:]
    candidate_run = candidate[WARMUP:]
    aligned_ref, aligned_got, lag = aligned_pair(reference, candidate)
    ref_energy = sum(value * value for value in aligned_ref)
    error_energy = sum((a - b) ** 2 for a, b in zip(aligned_ref, aligned_got))
    return {
        "reference_rms": rms(reference_run),
        "vactr_rms": rms(candidate_run),
        "correlation_early_window": correlation(aligned_ref, aligned_got),
        "normalized_rms_error_early_window": math.sqrt(error_energy / ref_energy) if ref_energy else None,
        "onset_lag_samples": lag,
        "reference_spectral": spectral_metrics(reference),
        "vactr_spectral": spectral_metrics(candidate),
        "reference_decay_20db_ms": decay_time_ms(reference, 20.0),
        "vactr_decay_20db_ms": decay_time_ms(candidate, 20.0),
        "reference_decay_40db_ms": decay_time_ms(reference, 40.0),
        "vactr_decay_40db_ms": decay_time_ms(candidate, 40.0),
    }


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
    cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
    engines = [f"{source}/plaits/dsp/engine/{name}_engine.cc" for name in ENGINES]
    physical_sources = [
        source / "plaits/dsp/physical_modelling/modal_voice.cc",
        source / "plaits/dsp/physical_modelling/resonator.cc",
        source / "plaits/dsp/physical_modelling/string_voice.cc",
        source / "plaits/dsp/physical_modelling/string.cc",
    ]
    with tempfile.TemporaryDirectory(prefix="vactr-plaits-physical-") as temp:
        executable = Path(temp) / "plaits_physical_reference"
        run([clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
             str(ROOT / "verification/plaits_physical_reference.cc"), *engines,
             *(str(path) for path in physical_sources),
             str(source / "plaits/resources.cc"),
             str(source / "stmlib/dsp/units.cc"),
             str(source / "stmlib/utils/random.cc"), cxx_library,
             "-o", str(executable)], cwd=ROOT)
        cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")
        results = []
        for engine in ENGINES:
            for harmonics, timbre, morph, accent in SCENARIOS:
                controls = [str(value) for value in (harmonics, timbre, morph, accent)]
                source_audio = parse_channels(
                    run([str(executable), engine, *controls]), f"upstream {engine}" )
                vactr_audio = parse_channels(run(
                    ["cargo", "run", "-q", "--example", "plaits_physical_reference", "--",
                     engine, "69", *controls], cwd=ROOT, env=cargo_env, timeout=300), f"Vactr {engine}")
                results.append({
                    "position": {"swarm": 16, "noise": 17, "particle": 18,
                                 "string": 19, "modal": 20}[engine],
                    "engine": engine,
                    "harmonics": harmonics,
                    "timbre": timbre,
                    "morph": morph,
                    "accent": accent,
                    "main": channel_metrics(source_audio[0], vactr_audio[0]),
                    "aux": channel_metrics(source_audio[1], vactr_audio[1]),
                })
    print(json.dumps({
        "scope": "raw Plaits engine versus raw Vactr kernel; no firmware voice, LPG, host graph or imported upstream artifacts",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE,
        "block_frames": BLOCK_FRAMES,
        "warmup_frames": WARMUP,
        "frames_per_scenario": FRAMES,
        "trigger": "one rising edge on first block, low thereafter",
        "randomness_note": "Source global and Vactr per-voice random streams differ; sample correlation is descriptive, while distributional metrics are primary for stochastic engines.",
        "scenarios": results,
    }, indent=2))


if __name__ == "__main__":
    main()

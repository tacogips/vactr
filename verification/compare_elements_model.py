#!/usr/bin/env python3
"""Compare Elements Part exciter/resonator scenarios with Vactr's kernel.

The upstream source and its aggregate resources are compiled only in a
temporary directory for this local probe. Strike/blow cases depend on
resource samples and are classified as replacements, while bow-only cases
are the fair sample-independent comparisons.
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


ROOT = Path(__file__).resolve().parent.parent
EURORACK_REVISION = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4"
STMLIB_REVISION = "e3bd7c9cc00e4364166f9905c0509b6ffd0535ec"
SAMPLE_RATE = 32_000
BLOCK_FRAMES = 16
BLOCKS = 6_000
FRAMES = BLOCK_FRAMES * BLOCKS
MAX_ONSET_LAG = 32
SPECTRAL_WINDOW = 4096
ENVELOPE_WINDOW = 16
MODELS = ("modal", "string", "strings")
SCENARIOS = ("bow-warm", "bow-bright", "bow-muted", "sample-blow", "sample-strike")
CONTROL_NAMES = (
    "ex-env-shape", "ex-bow-level", "ex-bow-timbre", "ex-blow-level",
    "ex-blow-meta", "ex-blow-timbre", "ex-strike-level", "ex-strike-meta",
    "ex-strike-timbre", "ex-signature", "ex-geometry", "ex-brightness",
    "ex-damping", "ex-position", "ex-res-mod-frequency", "ex-res-mod-offset",
    "ex-reverb-diffusion", "ex-reverb-lp", "ex-space", "ex-modulation-frequency",
)
PATCH_FIELDS = (
    "ex-env-shape", "ex-bow-level", "ex-bow-timbre", "ex-blow-level",
    "ex-blow-meta", "ex-blow-timbre", "ex-strike-level", "ex-strike-meta",
    "ex-strike-timbre", "ex-signature", "ex-geometry", "ex-brightness",
    "ex-damping", "ex-position", "ex-res-mod-frequency", "ex-res-mod-offset",
    "ex-reverb-diffusion", "ex-reverb-lp", "ex-space", "ex-modulation-frequency",
)
PATCHES = {
    "bow-warm": (0.50, 0.72, 0.18, 0, 0.45, 0.35, 0, 0.50, 0.50, 0.52, 0.30, 0.38, 0.30, 0.28, 0.20, 0.18, 0.42, 0.55, 0.35, 0.25),
    "bow-bright": (0.35, 0.80, 0.78, 0, 0.55, 0.78, 0, 0.35, 0.72, 0.78, 0.74, 0.82, 0.24, 0.62, 0.58, 0.42, 0.70, 0.62, 0.72, 0.48),
    "bow-muted": (0.72, 0.55, 0.42, 0, 0.25, 0.22, 0, 0.70, 0.24, 0.35, 0.18, 0.24, 0.78, 0.42, 0.12, 0.08, 0.25, 0.82, 0.20, 0.10),
    "sample-blow": (0.50, 0, 0.50, 0.68, 0.58, 0.62, 0, 0.50, 0.50, 0.55, 0.48, 0.60, 0.42, 0.40, 0.32, 0.24, 0.50, 0.58, 0.42, 0.30),
    "sample-strike": (0.34, 0, 0.50, 0, 0.50, 0.50, 0.76, 0.66, 0.74, 0.62, 0.56, 0.72, 0.36, 0.58, 0.26, 0.20, 0.52, 0.54, 0.48, 0.24),
}


def run(argv, *, cwd=None, env=None, timeout=180):
    return subprocess.run(argv, cwd=cwd, env=env, check=True,
                          capture_output=True, text=True, timeout=timeout).stdout


def revision(path):
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).strip()


def require_clean_tracked_tree(path):
    if run(["git", "-C", str(path), "status", "--porcelain",
            "--untracked-files=no"]).strip():
        raise ValueError(f"reference checkout has modified tracked files: {path}")


def parse_pairs(output, label):
    main = []
    aux = []
    for number, line in enumerate(output.splitlines(), 1):
        fields = line.split()
        if len(fields) != 2:
            raise ValueError(f"{label}: row {number} is not a main/aux pair")
        left, right = map(float, fields)
        if not math.isfinite(left) or not math.isfinite(right):
            raise ValueError(f"{label}: row {number} contains a nonfinite sample")
        main.append(left)
        aux.append(right)
    if len(main) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} frames, got {len(main)}")
    return {"main": main, "aux": aux}


def parse_vactr(output, model, scenario):
    lines = output.splitlines()
    if not lines:
        raise ValueError(f"{model}/{scenario}: Vactr probe returned no output")
    metadata = json.loads(lines[0])
    expected = {"type": "metadata", "family": "elements-model", "model": model,
                "scenario": scenario, "sample_rate": SAMPLE_RATE,
                "block_frames": BLOCK_FRAMES, "frames": FRAMES}
    for key, value in expected.items():
        if metadata.get(key) != value:
            raise ValueError(f"{model}/{scenario}: metadata {key}={metadata.get(key)!r}, expected {value!r}")
    if set(metadata.get("controls", {})) != set(CONTROL_NAMES):
        raise ValueError(f"{model}/{scenario}: metadata has incomplete ex-* controls")
    channels = {"main": [], "aux": []}
    for number, line in enumerate(lines[1:], 1):
        sample = json.loads(line)
        if sample.get("type") != "sample" or sample.get("frame") != number - 1:
            raise ValueError(f"{model}/{scenario}: invalid sample row {number}")
        for channel in channels:
            value = float(sample[channel])
            if not math.isfinite(value):
                raise ValueError(f"{model}/{scenario}: {channel} contains a nonfinite sample")
            channels[channel].append(value)
    if any(len(values) != FRAMES for values in channels.values()):
        raise ValueError(f"{model}/{scenario}: expected {FRAMES} Vactr sample rows")
    return metadata, channels


def rms(samples):
    return math.sqrt(sum(value * value for value in samples) / len(samples))


def best_onset_lag(reference, candidate):
    window = min(4096, len(reference), len(candidate))
    best_lag = 0
    best_score = None
    for lag in range(-MAX_ONSET_LAG, MAX_ONSET_LAG + 1):
        if lag >= 0:
            ref = reference[lag:window]
            cand = candidate[:len(ref)]
        else:
            cand = candidate[-lag:window]
            ref = reference[:len(cand)]
        score = sum(a * b for a, b in zip(ref, cand))
        if best_score is None or score > best_score:
            best_lag, best_score = lag, score
    return best_lag


def align(reference, candidate, lag):
    if lag >= 0:
        return reference[lag:], candidate[:len(reference) - lag]
    return reference[:len(reference) + lag], candidate[-lag:]


def correlation(reference, candidate):
    size = len(reference)
    mean_ref = sum(reference) / size
    mean_got = sum(candidate) / size
    covariance = sum((a - mean_ref) * (b - mean_got)
                     for a, b in zip(reference, candidate))
    ref_energy = sum((value - mean_ref) ** 2 for value in reference)
    got_energy = sum((value - mean_got) ** 2 for value in candidate)
    if ref_energy == 0 or got_energy == 0:
        return None
    return covariance / math.sqrt(ref_energy * got_energy)


def normalized_rms_error(reference, candidate):
    denominator = sum(value * value for value in reference)
    if denominator <= 0:
        return None
    error = sum((a - b) ** 2 for a, b in zip(reference, candidate))
    return math.sqrt(error / denominator)


def fft(values):
    size = len(values)
    data = [complex(value, 0.0) for value in values]
    j = 0
    for i in range(1, size):
        bit = size >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j ^= bit
        if i < j:
            data[i], data[j] = data[j], data[i]
    length = 2
    while length <= size:
        root = cmath.exp(-2j * math.pi / length)
        for start in range(0, size, length):
            factor = 1 + 0j
            half = length // 2
            for offset in range(half):
                even = data[start + offset]
                odd = factor * data[start + offset + half]
                data[start + offset] = even + odd
                data[start + offset + half] = even - odd
                factor *= root
        length <<= 1
    return data


def spectrum(samples):
    size = min(SPECTRAL_WINDOW, len(samples))
    windowed = [samples[i] * (0.5 - 0.5 * math.cos(2 * math.pi * i / (size - 1)))
                for i in range(size)]
    bins = fft(windowed)[:size // 2 + 1]
    magnitudes = [abs(value) for value in bins]
    powers = [value * value for value in magnitudes]
    weighted = sum((index * SAMPLE_RATE / size) * value
                   for index, value in enumerate(magnitudes[1:], 1))
    total_mag = sum(magnitudes[1:])
    band_edges = (31.25, 62.5, 125, 250, 500, 1000, 2000, 4000, 8000, 16000)
    total_power = sum(powers[1:])
    bands = {}
    for low, high in zip(band_edges, band_edges[1:]):
        first = max(1, math.ceil(low * size / SAMPLE_RATE))
        last = min(len(powers), math.ceil(high * size / SAMPLE_RATE))
        energy = sum(powers[first:last])
        bands[f"{low:g}-{high:g}Hz"] = energy / total_power if total_power else None
    return {"centroid_hz": weighted / total_mag if total_mag else None,
            "octave_band_power_ratios": bands}


def decay_ms(samples, drop_db):
    levels = [rms(samples[start:start + ENVELOPE_WINDOW])
              for start in range(0, len(samples), ENVELOPE_WINDOW)]
    peak = max(levels, default=0.0)
    if peak <= 0:
        return None
    peak_index = levels.index(peak)
    threshold = peak * 10 ** (-drop_db / 20)
    for index in range(peak_index, len(levels) - 2):
        if all(value <= threshold for value in levels[index:index + 3]):
            return (index - peak_index) * ENVELOPE_WINDOW * 1000 / SAMPLE_RATE
    return None


def fundamental_hz(samples):
    # Normalized autocorrelation peak around the fixed C4 note, including the
    # octave below used by Elements' five-string chord set.
    window = samples[:min(8192, len(samples))]
    if len(window) < 64:
        return None
    max_lag = min(int(SAMPLE_RATE / 55), len(window) // 2)
    min_lag = max(2, int(SAMPLE_RATE / 530))
    best_lag = None
    best_score = -1.0
    for lag in range(min_lag, max_lag + 1):
        a = window[:-lag]
        b = window[lag:]
        numerator = sum(x * y for x, y in zip(a, b))
        denominator = math.sqrt(sum(x * x for x in a) * sum(y * y for y in b))
        score = numerator / denominator if denominator else 0.0
        if score > best_score:
            best_lag, best_score = lag, score
    return SAMPLE_RATE / best_lag if best_lag else None


def channel_metrics(reference, candidate):
    lag = best_onset_lag(reference, candidate)
    aligned_ref, aligned_got = align(reference, candidate, lag)
    return {
        "reference_rms": rms(reference),
        "vactr_rms": rms(candidate),
        "correlation_after_onset_alignment": correlation(aligned_ref, aligned_got),
        "onset_lag_samples": lag,
        "normalized_rms_error_after_onset_alignment": normalized_rms_error(aligned_ref, aligned_got),
        "reference_spectral": spectrum(reference),
        "vactr_spectral": spectrum(candidate),
        "reference_decay_20db_ms": decay_ms(reference, 20),
        "vactr_decay_20db_ms": decay_ms(candidate, 20),
        "reference_decay_40db_ms": decay_ms(reference, 40),
        "vactr_decay_40db_ms": decay_ms(candidate, 40),
        "reference_fundamental_hz": fundamental_hz(aligned_ref),
        "vactr_fundamental_hz": fundamental_hz(aligned_got),
    }


def classify(scenario, main, aux):
    if scenario.startswith("sample-"):
        return "replacement"
    values = (main, aux)
    if any(metric["reference_rms"] < 1e-9 or metric["vactr_rms"] < 1e-9
           for metric in values):
        return "not comparable"
    if all((metric["correlation_after_onset_alignment"] or 0) >= 0.99 and
           (metric["normalized_rms_error_after_onset_alignment"] or 1) <= 0.1
           for metric in values):
        return "close"
    return "measured gap"


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

    with tempfile.TemporaryDirectory(prefix="vactr-elements-model-") as temp:
        executable = Path(temp) / "elements_model_reference"
        cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
        compile_sources = (
            "elements/dsp/part.cc", "elements/dsp/voice.cc",
            "elements/dsp/exciter.cc", "elements/dsp/resonator.cc",
            "elements/dsp/multistage_envelope.cc", "elements/dsp/string.cc",
            "elements/dsp/tube.cc",
            "elements/dsp/ominous_voice.cc", "elements/resources.cc",
            "stmlib/dsp/units.cc", "stmlib/utils/random.cc",
        )
        run([clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
             str(ROOT / "verification/elements_model_reference.cc"),
             *[str(source / path) for path in compile_sources], cxx_library,
             "-o", str(executable)], cwd=ROOT)

        cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")
        run(["cargo", "build", "-q", "--example", "elements_model_reference"],
            cwd=ROOT, env=cargo_env)
        vactr_executable = ROOT / "target/debug/examples/elements_model_reference"
        results = []
        for model in MODELS:
            for scenario in SCENARIOS:
                vactr_output = run([str(vactr_executable), model, scenario], cwd=ROOT)
                metadata, vactr = parse_vactr(vactr_output, model, scenario)
                controls = metadata["controls"]
                if [controls[name] for name in PATCH_FIELDS] != list(PATCHES[scenario]):
                    raise ValueError(f"{model}/{scenario}: Rust and Python control maps differ")
                upstream = parse_pairs(
                    run([str(executable), model,
                         *[str(value) for value in PATCHES[scenario]]]),
                    f"{model}/{scenario} upstream")
                channel_results = {name: channel_metrics(upstream[name], vactr[name])
                                   for name in ("main", "aux")}
                classification = classify(scenario, channel_results["main"], channel_results["aux"])
                results.append({
                    "model": model.upper(),
                    "scenario": scenario,
                    "classification": classification,
                    "resource_note": ("replacement - sample-dependent scenario; upstream resources compiled only in the temporary local reference"
                                      if scenario.startswith("sample-") else "bow-only; blow and strike levels are zero"),
                    "sample_rate_hz": SAMPLE_RATE,
                    "block_frames": BLOCK_FRAMES,
                    "fixed_note": metadata["note"],
                    "fixed_gate": metadata["gate"],
                    "controls": controls,
                    "channels": channel_results,
                })
    print(json.dumps({
        "scope": "Elements Part::Process raw main/aux outputs versus the Vactr exciter-core kernel",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE,
        "block_frames": BLOCK_FRAMES,
        "frames": FRAMES,
        "comparison_limit": "Kernel boundary only; measured gaps are not parity. Sample scenarios are replacements and bow-only scenarios are the sample-independent comparison.",
        "results": results,
    }, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Compare raw Braids macro-oscillator kernels for all 47 accessible shapes.

This does not vendor upstream code or samples. It compiles a small original
C++ driver that #includes the pinned upstream Eurorack Braids sources from a
separate checkout named on the command line, and compares its output against
Vactr's own `examples/braids_shapes_reference.rs` raw-kernel probe. Neither
process is linked into Vactr's library, binary, or browser build, and no
upstream object, table, or audio sample is copied into this repository.

Metrics are per shape/scenario: RMS (both sides), correlation, normalized
RMS error after a one-block warm-up, an approximate Goertzel-bin spectral
centroid, and a coarse-to-fine autocorrelation fundamental estimate (None
when the estimate's confidence is too low to trust, e.g. noise-like output).
Shapes are classified close / measured gap / replacement / not comparable;
positions 37-40 are always "replacement" because their Vactr kernels are
original procedural substitutes for the upstream wave-bank/map/line/chord
data path, not adaptations expected to track it numerically.
"""

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
SAMPLE_RATE_HZ = 96_000.0
BLOCK_FRAMES = 24
BLOCKS = 200
FRAMES = BLOCK_FRAMES * BLOCKS
WARMUP = BLOCK_FRAMES
# (timbre, color) scenarios shared by every shape.
SCENARIOS = (
    (0.15, 0.25),
    (0.50, 0.50),
    (0.85, 0.75),
)
# MIDI note 60, matching the pinned firmware's own default quantizer target
# expression "(60 + settings.quantizer_root()) << 7" (root 0) in braids.cc:
# 60 << 7 == 7680 in the source's 128-units-per-semitone pitch code.
MIDI_NOTE = 60
PITCH_CODE = MIDI_NOTE << 7

# Published registration order from `braids/settings.h`, mirrored in
# `src/dsp/ported/braids.rs`.
SHAPE_NAMES = (
    "CSAW", "MORPH", "SAW_SQUARE", "SINE_TRIANGLE", "BUZZ",
    "SQUARE_SUB", "SAW_SUB", "SQUARE_SYNC", "SAW_SYNC",
    "TRIPLE_SAW", "TRIPLE_SQUARE", "TRIPLE_TRIANGLE", "TRIPLE_SINE",
    "TRIPLE_RING_MOD", "SAW_SWARM", "SAW_COMB", "TOY",
    "DIGITAL_FILTER_LP", "DIGITAL_FILTER_PK", "DIGITAL_FILTER_BP",
    "DIGITAL_FILTER_HP", "VOSIM", "VOWEL", "VOWEL_FOF", "HARMONICS",
    "FM", "FEEDBACK_FM", "CHAOTIC_FEEDBACK_FM",
    "PLUCKED", "BOWED", "BLOWN", "FLUTED",
    "STRUCK_BELL", "STRUCK_DRUM",
    "KICK", "CYMBAL", "SNARE",
    "WAVETABLES", "WAVE_MAP", "WAVE_LINE", "WAVE_PARAPHONIC",
    "FILTERED_NOISE", "TWIN_PEAKS_NOISE", "CLOCKED_NOISE",
    "GRANULAR_CLOUD", "PARTICLE_NOISE", "DIGITAL_MODULATION",
)
assert len(SHAPE_NAMES) == 47

# Positions whose Vactr kernel is an original procedural replacement for the
# upstream wave-bank/map/line/chord data path (WAVETABLES, WAVE_MAP,
# WAVE_LINE, WAVE_PARAPHONIC). Never expected to track the reference.
REPLACEMENT_POSITIONS = frozenset({37, 38, 39, 40})

CLOSE_CORRELATION = 0.9
CLOSE_NRMSE = 0.35
FUNDAMENTAL_CONFIDENCE = 0.2
FUNDAMENTAL_MIN_HZ = 40.0
FUNDAMENTAL_MAX_HZ = 2000.0
CENTROID_MIN_HZ = 20.0
CENTROID_MAX_HZ = 20_000.0
CENTROID_BINS = 60
AUTOCORR_WINDOW = 3000


def run(argv, *, cwd=None, env=None, timeout=120):
    return subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=True,
        capture_output=True,
        text=True,
        timeout=timeout,
    ).stdout


def revision(path):
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).strip()


def require_clean_tracked_tree(path):
    if run(["git", "-C", str(path), "status", "--porcelain",
            "--untracked-files=no"]).strip():
        raise ValueError(f"reference checkout has modified tracked files: {path}")


def parse_samples(output, label):
    lines = output.splitlines()
    if len(lines) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} rows, got {len(lines)}")
    values = [float(line) for line in lines]
    if not all(math.isfinite(value) for value in values):
        raise ValueError(f"{label}: output is not finite")
    return values


def goertzel_power(window, sr, freq):
    n = len(window)
    k = int(0.5 + n * freq / sr)
    if k <= 0 or k >= n // 2:
        return 0.0
    omega = 2.0 * math.pi * k / n
    coeff = 2.0 * math.cos(omega)
    s1 = s2 = 0.0
    for sample in window:
        s0 = sample + coeff * s1 - s2
        s2 = s1
        s1 = s0
    power = s1 * s1 + s2 * s2 - coeff * s1 * s2
    return max(power, 0.0)


def spectral_centroid(signal, sr):
    """Approximate spectral centroid from log-spaced Goertzel bins.

    This is a bounded-cost approximation (fixed bin count), not a
    full-resolution FFT spectrum; it is precise enough to compare gross
    brightness between the reference and Vactr renders.
    """
    window_len = min(4096, len(signal))
    tail = signal[-window_len:]
    windowed = [
        sample * (0.5 - 0.5 * math.cos(2.0 * math.pi * i / (window_len - 1)))
        for i, sample in enumerate(tail)
    ]
    log_min = math.log(CENTROID_MIN_HZ)
    log_max = math.log(min(CENTROID_MAX_HZ, sr * 0.5 * 0.98))
    weighted = 0.0
    total = 0.0
    for step in range(CENTROID_BINS):
        freq = math.exp(log_min + (log_max - log_min) * step / (CENTROID_BINS - 1))
        magnitude = math.sqrt(goertzel_power(windowed, sr, freq))
        weighted += freq * magnitude
        total += magnitude
    if total <= 0.0:
        return None
    return weighted / total


def _normalized_autocorrelation(window, lag):
    n = len(window)
    if lag >= n:
        return 0.0
    return sum(a * b for a, b in zip(window[: n - lag], window[lag:]))


def fundamental_hz(signal, sr):
    """Coarse-to-fine normalized-autocorrelation pitch estimate.

    Returns (frequency_hz, confidence) or None if no lag in the searched
    range clears the confidence floor (e.g. noise-like or unpitched output).
    """
    window = signal[-AUTOCORR_WINDOW:] if len(signal) > AUTOCORR_WINDOW else signal
    mean = sum(window) / len(window)
    window = [value - mean for value in window]
    energy0 = sum(value * value for value in window)
    if energy0 <= 0.0:
        return None
    min_lag = max(1, int(sr / FUNDAMENTAL_MAX_HZ))
    max_lag = min(len(window) - 1, int(sr / FUNDAMENTAL_MIN_HZ))
    if max_lag <= min_lag:
        return None
    coarse_step = 4
    best_lag, best_score = None, 0.0
    for lag in range(min_lag, max_lag + 1, coarse_step):
        score = _normalized_autocorrelation(window, lag) / energy0
        if score > best_score:
            best_score, best_lag = score, lag
    if best_lag is None:
        return None
    refine_lo = max(min_lag, best_lag - coarse_step)
    refine_hi = min(max_lag, best_lag + coarse_step)
    for lag in range(refine_lo, refine_hi + 1):
        score = _normalized_autocorrelation(window, lag) / energy0
        if score > best_score:
            best_score, best_lag = score, lag
    if best_score < FUNDAMENTAL_CONFIDENCE:
        return None
    # A peak sitting at the very edge of the searched lag range is often a
    # boundary artifact (the true peak, if any, lies outside the searched
    # 40-2000 Hz band) rather than a genuine fundamental; require much
    # higher confidence before trusting it.
    at_boundary = best_lag <= min_lag + coarse_step or best_lag >= max_lag - coarse_step
    if at_boundary and best_score < 0.6:
        return None
    return (sr / best_lag, best_score)


def metrics(reference, candidate, sr):
    reference = reference[WARMUP:]
    candidate = candidate[WARMUP:]
    size = len(reference)
    ref_mean = sum(reference) / size
    got_mean = sum(candidate) / size
    ref_energy = sum(value * value for value in reference)
    got_energy = sum(value * value for value in candidate)
    error_energy = sum((a - b) ** 2 for a, b in zip(reference, candidate))
    ref_centered = [value - ref_mean for value in reference]
    got_centered = [value - got_mean for value in candidate]
    covariance = sum(a * b for a, b in zip(ref_centered, got_centered))
    ref_variance = sum(value * value for value in ref_centered)
    got_variance = sum(value * value for value in got_centered)
    if ref_energy <= 0 or ref_variance <= 0 or got_variance <= 0:
        raise ValueError("comparison output has zero reference or candidate variance")
    ref_fundamental = fundamental_hz(reference, sr)
    got_fundamental = fundamental_hz(candidate, sr)
    return {
        "reference_rms": math.sqrt(ref_energy / size),
        "vactr_rms": math.sqrt(got_energy / size),
        "correlation": covariance / math.sqrt(ref_variance * got_variance),
        "normalized_rms_error": math.sqrt(error_energy / ref_energy),
        "reference_spectral_centroid_hz": spectral_centroid(reference, sr),
        "vactr_spectral_centroid_hz": spectral_centroid(candidate, sr),
        "reference_fundamental_hz": ref_fundamental[0] if ref_fundamental else None,
        "reference_fundamental_confidence": ref_fundamental[1] if ref_fundamental else None,
        "vactr_fundamental_hz": got_fundamental[0] if got_fundamental else None,
        "vactr_fundamental_confidence": got_fundamental[1] if got_fundamental else None,
    }


def classify(position, scenario_results):
    if position in REPLACEMENT_POSITIONS:
        return (
            "replacement",
            "Vactr kernel is an original procedural replacement for the "
            "upstream wave-bank/map/line/chord data path; not expected to "
            "match by design.",
        )
    ok = [row for row in scenario_results if row.get("metrics") is not None]
    if not ok:
        reasons = "; ".join(row["error"] for row in scenario_results)
        return ("not comparable", f"no scenario produced comparable output: {reasons}")
    correlations = sorted(row["metrics"]["correlation"] for row in ok)
    nrmses = sorted(row["metrics"]["normalized_rms_error"] for row in ok)
    median_correlation = correlations[len(correlations) // 2]
    median_nrmse = nrmses[len(nrmses) // 2]
    partial = "" if len(ok) == len(scenario_results) else (
        f" ({len(ok)}/{len(scenario_results)} scenarios comparable)"
    )
    reason = (
        f"median correlation {median_correlation:.3f}, "
        f"median normalized RMS error {median_nrmse:.3f} across scenarios{partial}"
    )
    if median_correlation >= CLOSE_CORRELATION and median_nrmse <= CLOSE_NRMSE:
        return ("close", reason)
    return ("measured gap", reason)


def compile_reference(clang, source, temp_dir):
    executable = Path(temp_dir) / "braids_shapes_reference"
    cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
    run([
        clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
        str(ROOT / "verification/braids_shapes_reference.cc"),
        str(source / "braids/macro_oscillator.cc"),
        str(source / "braids/analog_oscillator.cc"),
        str(source / "braids/digital_oscillator.cc"),
        str(source / "braids/resources.cc"),
        str(source / "stmlib/utils/random.cc"),
        cxx_library,
        "-o", str(executable),
    ], cwd=ROOT, timeout=180)
    return executable


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path,
                        help="separate official Eurorack checkout at the pinned revision")
    parser.add_argument("--positions", default=None,
                        help="comma-separated subset of shape positions (default: all 47)")
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean_tracked_tree(source)
    require_clean_tracked_tree(source / "stmlib")
    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()
    positions = (
        [int(value) for value in args.positions.split(",")]
        if args.positions else list(range(47))
    )

    cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")
    shapes = []
    with tempfile.TemporaryDirectory(prefix="vactr-braids-shapes-") as temp:
        executable = compile_reference(clang, source, temp)
        for position in positions:
            scenario_results = []
            for timbre, color in SCENARIOS:
                controls = [str(position), str(timbre), str(color)]
                entry = {"timbre": timbre, "color": color}
                try:
                    upstream = parse_samples(
                        run([str(executable), *controls], timeout=60), "upstream")
                    vactr = parse_samples(run(
                        ["cargo", "run", "-q", "--example", "braids_shapes_reference",
                         "--", *controls],
                        cwd=ROOT, env=cargo_env, timeout=60,
                    ), "vactr")
                    entry["metrics"] = metrics(upstream, vactr, SAMPLE_RATE_HZ)
                except (ValueError, subprocess.CalledProcessError,
                        subprocess.TimeoutExpired) as error:
                    entry["error"] = str(error)
                scenario_results.append(entry)
            classification, reason = classify(position, scenario_results)
            shapes.append({
                "position": position,
                "source_name": SHAPE_NAMES[position],
                "classification": classification,
                "reason": reason,
                "upstream_wave_assets": position in REPLACEMENT_POSITIONS,
                "scenarios": scenario_results,
            })

    print(json.dumps({
        "scope": "raw Braids macro-oscillator family kernels; no envelope, "
                 "trigger, LPG, voice or .vact host",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE_HZ,
        "block_frames": BLOCK_FRAMES,
        "warmup_frames": WARMUP,
        "frames_compared": FRAMES - WARMUP,
        "pitch": {
            "midi_note": MIDI_NOTE,
            "braids_pitch_code": PITCH_CODE,
            "vactr_hz": 440.0 * 2.0 ** ((MIDI_NOTE - 69) / 12.0),
        },
        "scenarios": [{"timbre": t, "color": c} for t, c in SCENARIOS],
        "classification_thresholds": {
            "close_min_correlation": CLOSE_CORRELATION,
            "close_max_normalized_rms_error": CLOSE_NRMSE,
            "fundamental_confidence_floor": FUNDAMENTAL_CONFIDENCE,
        },
        "shapes": shapes,
    }, indent=2))


if __name__ == "__main__":
    main()

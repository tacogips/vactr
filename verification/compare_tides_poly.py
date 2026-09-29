#!/usr/bin/env python3
"""Compare raw Tides1 function and Tides2 four-lane kernels with pinned DSP.

The upstream checkout is read-only and is compiled only into a temporary
local executable. The Vactr probe is an independent Rust example; no upstream
code, generated resource, or audio output is stored in this repository.
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
SAMPLE_RATE = 48000.0
BLOCK_FRAMES = 24
FRAMES = 24000
WARMUP = BLOCK_FRAMES * 2
MAX_LAG = 256

# Shared baseline and two additional control points for looping Tides2.
BASE = {"frequency_hz": 5.0, "pw": 0.5, "shape": 0.5,
        "smoothness": 0.35, "shift": 0.5}
LOOP_SETTINGS = (
    BASE,
    {"frequency_hz": 5.0, "pw": 0.36, "shape": 0.25,
     "smoothness": 0.75, "shift": 0.2},
    {"frequency_hz": 5.0, "pw": 0.68, "shape": 0.8,
     "smoothness": 0.15, "shift": 0.8},
)
MODE_NAMES = ("AD", "LOOPING", "AR")
OUTPUT_NAMES = ("GATES", "AMPLITUDE", "PHASE", "FREQUENCY")


def run(argv, *, cwd=None, env=None, timeout=180):
    return subprocess.run(argv, cwd=cwd, env=env, check=True,
                          capture_output=True, text=True,
                          timeout=timeout).stdout


def revision(path):
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).strip()


def require_clean(path):
    if run(["git", "-C", str(path), "status", "--porcelain",
            "--untracked-files=no"]).strip():
        raise ValueError(f"reference checkout has modified tracked files: {path}")


def parse_lanes(output, label, lanes=4):
    rows = []
    for line_number, line in enumerate(output.splitlines(), 1):
        fields = line.split()
        if len(fields) != lanes:
            raise ValueError(f"{label}: row {line_number} expected {lanes} lanes")
        row = [float(value) for value in fields]
        if not all(math.isfinite(value) for value in row):
            raise ValueError(f"{label}: nonfinite value on row {line_number}")
        rows.append(row)
    if len(rows) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} frames, got {len(rows)}")
    return [[row[lane] for row in rows] for lane in range(lanes)]


def rms(signal):
    return math.sqrt(sum(x * x for x in signal) / len(signal)) if signal else 0.0


def correlation(a, b):
    n = min(len(a), len(b))
    ma, mb = sum(a[:n]) / n, sum(b[:n]) / n
    ac = [x - ma for x in a[:n]]
    bc = [x - mb for x in b[:n]]
    ea, eb = sum(x * x for x in ac), sum(x * x for x in bc)
    if ea <= 1e-20 or eb <= 1e-20:
        return None
    return sum(x * y for x, y in zip(ac, bc)) / math.sqrt(ea * eb)


def shifted(a, b, lag):
    if lag >= 0:
        return a[lag:], b[:len(a) - lag]
    return a[:len(a) + lag], b[-lag:]


def best_phase_lag(a, b):
    a, b = a[WARMUP:], b[WARMUP:]
    window = min(2048, len(a), len(b))
    a, b = a[:window], b[:window]
    mean_a, mean_b = sum(a) / window, sum(b) / window
    a = [value - mean_a for value in a]
    b = [value - mean_b for value in b]
    limit = min(MAX_LAG, window // 4)
    coarse = []
    for lag in range(-limit, limit + 1, 8):
        if lag >= 0:
            score = sum(x * y for x, y in zip(a[lag:], b[:window - lag]))
        else:
            score = sum(x * y for x, y in zip(a[:window + lag], b[-lag:]))
        coarse.append((lag, score))
    best_lag = max(coarse, key=lambda item: item[1])[0]
    best_score = -float("inf")
    for lag in range(max(-limit, best_lag - 7), min(limit, best_lag + 7) + 1):
        if lag >= 0:
            score = sum(x * y for x, y in zip(a[lag:], b[:window - lag]))
        else:
            score = sum(x * y for x, y in zip(a[:window + lag], b[-lag:]))
        if score > best_score:
            best_lag, best_score = lag, score
    return best_lag


def nrmse_aligned(a, b, lag):
    x, y = shifted(a[WARMUP:], b[WARMUP:], lag)
    n = min(len(x), len(y))
    denominator = sum(value * value for value in x[:n])
    if denominator <= 1e-20:
        return None
    return math.sqrt(sum((u - v) ** 2 for u, v in zip(x[:n], y[:n])) /
                     denominator)


def period_estimate(signal):
    signal = signal[WARMUP:]
    # Average the fixed 16-sample Tides1 render granularity before estimating
    # cycles; otherwise its block-edge artifacts can look like high-frequency
    # crossings in the output.
    width = 16
    signal = [sum(signal[start:start + width]) / width
              for start in range(0, len(signal) - width + 1, width)]
    if len(signal) < 64:
        return {"period_samples": None, "frequency_hz": None}
    mean = sum(signal) / len(signal)
    low, high = min(signal), max(signal)
    span = high - low
    if span < 1e-8:
        return {"period_samples": None, "frequency_hz": None}
    threshold = low + 0.8 * span
    peaks = [index for index in range(1, len(signal) - 1)
             if signal[index] >= threshold and
             signal[index] >= signal[index - 1] and
             signal[index] > signal[index + 1]]
    peak_periods = [b - a for a, b in zip(peaks, peaks[1:])
                    if 2 <= b - a <= len(signal) // 2]
    if len(peak_periods) >= 2:
        peak_periods.sort()
        period = peak_periods[len(peak_periods) // 2]
        return {"period_samples": period * width,
                "frequency_hz": SAMPLE_RATE / (period * width)}
    crossings = [index for index in range(1, len(signal))
                 if signal[index - 1] <= mean < signal[index]]
    if len(crossings) < 2:
        return {"period_samples": None, "frequency_hz": None}
    periods = [b - a for a, b in zip(crossings, crossings[1:])
               if 2 <= b - a <= len(signal) // 2]
    if not periods:
        return {"period_samples": None, "frequency_hz": None}
    periods.sort()
    period = periods[len(periods) // 2]
    return {"period_samples": period * width,
            "frequency_hz": SAMPLE_RATE / (period * width)}


def crossing_time(signal, rising):
    signal = signal[WARMUP:]
    if not signal:
        return None
    low, high = min(signal), max(signal)
    span = high - low
    if span < 1e-8:
        return None
    first = 0.1 if rising else 0.9
    second = 0.9 if rising else 0.1
    a, b = low + span * first, low + span * second
    start = None
    for index in range(1, len(signal)):
        crossed = ((signal[index - 1] < a <= signal[index]) if rising
                   else (signal[index - 1] > a >= signal[index]))
        if start is None and crossed:
            start = index
        if start is not None:
            done = ((signal[index - 1] < b <= signal[index]) if rising
                    else (signal[index - 1] > b >= signal[index]))
            if done:
                return (index - start) / SAMPLE_RATE * 1000.0
    return None


def lane_metrics(reference, candidate):
    a, b = reference[WARMUP:], candidate[WARMUP:]
    lag = best_phase_lag(reference, candidate)
    return {
        "reference_rms": rms(a), "vactr_rms": rms(b),
        "correlation": correlation(a, b), "phase_alignment_lag_samples": lag,
        "normalized_rms_error_after_phase_alignment": nrmse_aligned(reference, candidate, lag),
        "reference_peak": max(a), "reference_trough": min(a),
        "vactr_peak": max(b), "vactr_trough": min(b),
        "reference_period": period_estimate(reference),
        "vactr_period": period_estimate(candidate),
        "reference_rise_10_90_ms": crossing_time(reference, True),
        "vactr_rise_10_90_ms": crossing_time(candidate, True),
        "reference_fall_90_10_ms": crossing_time(reference, False),
        "vactr_fall_90_10_ms": crossing_time(candidate, False),
    }


def classify(metrics):
    # Thresholds are descriptive only; they never imply SourcePort coverage.
    usable = [m for m in metrics if m["correlation"] is not None and
              m["normalized_rms_error_after_phase_alignment"] is not None]
    if usable and all(m["correlation"] >= 0.90 and
                      m["normalized_rms_error_after_phase_alignment"] <= 0.35
                      for m in usable):
        return "close"
    if not usable:
        return "not comparable"
    return "measured gap"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean(source)
    require_clean(source / "stmlib")
    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()
    cxx_lib = "-lc++" if sys.platform == "darwin" else "-lstdc++"
    env = dict(os.environ, CARGO_TERM_QUIET="true")
    run(["cargo", "build", "-q", "--example", "tides_poly_reference"],
        cwd=ROOT, env=env)
    probe = ROOT / "target/debug/examples/tides_poly_reference"
    results = []
    with tempfile.TemporaryDirectory(prefix="vactr-tides-poly-") as tmp:
        binary2 = Path(tmp) / "tides2_reference"
        binary1 = Path(tmp) / "tides1_reference"
        driver = str(ROOT / "verification/tides_poly_reference.cc")
        units = str(source / "stmlib/dsp/units.cc")
        run([clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
             driver, str(source / "tides2/poly_slope_generator.cc"),
             str(source / "tides2/resources.cc"), units, cxx_lib,
             "-o", str(binary2)], cwd=ROOT)
        run([clang, "-std=c++11", "-DTEST", "-DTIDES1_PROBE", "-O2", "-I",
             str(source), driver, str(source / "tides/generator.cc"),
             str(source / "tides/resources.cc"), units, cxx_lib,
             "-o", str(binary1)], cwd=ROOT)
        for mode, mode_name in enumerate(MODE_NAMES):
            for output, output_name in enumerate(OUTPUT_NAMES):
                for range_id, range_name in enumerate(("control", "audio")):
                    settings_list = LOOP_SETTINGS if mode == 1 else (BASE,)
                    setting_results = []
                    for setting_index, setting in enumerate(settings_list):
                        config = [str(setting[key]) for key in
                                  ("frequency_hz", "pw", "shape", "smoothness", "shift")]
                        upstream = parse_lanes(run(
                            [str(binary2), "tides2", str(mode), str(output),
                             str(range_id), *config]), "Tides2 upstream")
                        vactr = parse_lanes(run(
                            [str(probe), str(mode), str(output), str(range_id),
                             str(setting["frequency_hz"]), str(setting["pw"]),
                             str(setting["shape"]), str(setting["smoothness"]),
                             str(setting["shift"]), str(FRAMES)], cwd=ROOT, env=env),
                            "Tides2 Vactr")
                        lane_values = [lane_metrics(upstream[i], vactr[i])
                                       for i in range(4)]
                        setting_results.append({
                            "setting_index": setting_index, "settings": setting,
                            "lanes": lane_values})
                    combination_metrics = [entry["lanes"][lane]
                                           for entry in setting_results
                                           for lane in range(4)]
                    results.append({
                        "generation": 2, "mode": mode_name,
                        "output_mode": output_name, "range": range_name,
                        "classification": classify(combination_metrics),
                        "settings": setting_results})

        tides1 = []
        tides1_frequency = 12.0  # Tides1 high range bottoms out near 8.18 Hz.
        for mode, mode_name in enumerate(MODE_NAMES):
            args1 = ["tides1", str(mode), str(tides1_frequency),
                     "0.5", "0.5", "0.5"]
            upstream = parse_lanes(run([str(binary1), *args1], cwd=ROOT),
                                   "Tides1 upstream", lanes=2)
            vactr = parse_lanes(run(
                [str(probe), "tides1", str(mode), str(tides1_frequency),
                 "0.5", "0.5", "0.5",
                 str(FRAMES)], cwd=ROOT, env=env), "Tides1 Vactr", lanes=2)
            tides1_metrics = [lane_metrics(upstream[i], vactr[i])
                              for i in range(2)]
            tides1.append({
                "mode": mode_name,
                "frequency_hz": tides1_frequency,
                "classification": classify(tides1_metrics),
                "role_note": "unipolar/bipolar source samples are compared to Vactr tidal_function main lanes; control-role template selector mapping is not a direct simultaneous-channel match",
                "lanes": tides1_metrics})

    print(json.dumps({
        "scope": "raw kernels only; Tides2 four lanes, source output modes 2/3 rendered at half rate and duplicated; Tides1 source default WAVETABLE_HACK-disabled unipolar/bipolar roles; no host graph or .vact event path",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE, "block_frames": BLOCK_FRAMES,
        "frames_compared_after_warmup": FRAMES - WARMUP,
        "tides2_baseline": BASE,
        "tides2_combinations": results,
        "tides1": tides1,
    }, indent=2))


if __name__ == "__main__":
    main()

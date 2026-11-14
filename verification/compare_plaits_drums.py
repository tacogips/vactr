#!/usr/bin/env python3
"""Compare raw dual bass-drum, snare-drum and hi-hat kernels (Plaits
positions 21-23) against a separate pinned checkout, without vendoring
upstream code or samples."""

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
MAX_ONSET_LAG = 32
ENVELOPE_WINDOW = 64
SPECTRAL_WINDOW = 512

ENGINES = ("kick", "snare", "hihat")

# (harmonics, timbre, morph, accent) shared by every engine. The fourth
# scenario holds harmonics/timbre/morph at the common 0.5 point and varies
# only accent, as a fourth "accent variation" scenario.
SCENARIOS = (
    (0.1, 0.25, 0.2, 1.0),
    (0.5, 0.5, 0.5, 1.0),
    (0.9, 0.75, 0.8, 1.0),
    (0.5, 0.5, 0.5, 0.3),
)


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
        values = [float(value) for value in fields]
        if not all(math.isfinite(value) for value in values):
            raise ValueError(f"{label}: row {number} is not finite")
        main.append(values[0])
        aux.append(values[1])
    return (main, aux)


def rms(samples):
    return math.sqrt(sum(x * x for x in samples) / len(samples))


def peak(samples):
    return max(abs(x) for x in samples)


def correlation(reference, candidate):
    size = len(reference)
    ref_mean = sum(reference) / size
    got_mean = sum(candidate) / size
    ref_centered = [x - ref_mean for x in reference]
    got_centered = [x - got_mean for x in candidate]
    covariance = sum(a * b for a, b in zip(ref_centered, got_centered))
    ref_var = sum(x * x for x in ref_centered)
    got_var = sum(x * x for x in got_centered)
    if ref_var <= 0 or got_var <= 0:
        return None
    return covariance / math.sqrt(ref_var * got_var)


def best_onset_lag(reference, candidate, max_lag):
    """Search +/- max_lag samples for the candidate shift with the highest
    cross-correlation against the reference onset, so the error metric below
    compares aligned onsets rather than a fixed-zero-lag pair."""
    window = min(len(reference), len(candidate), 4096)
    ref_window = reference[:window]
    best_lag = 0
    best_score = None
    for lag in range(-max_lag, max_lag + 1):
        if lag >= 0:
            ref_part = ref_window[lag:]
            cand_part = candidate[: len(ref_part)]
        else:
            cand_part = candidate[-lag : -lag + window]
            ref_part = ref_window[: len(cand_part)]
        if len(ref_part) < window // 2:
            continue
        score = sum(a * b for a, b in zip(ref_part, cand_part))
        if best_score is None or score > best_score:
            best_score = score
            best_lag = lag
    return best_lag


def normalized_rms_error(reference, candidate, lag):
    if lag >= 0:
        ref_part = reference[lag:]
        cand_part = candidate[: len(ref_part)]
    else:
        cand_part = candidate[-lag:]
        ref_part = reference[: len(cand_part)]
    size = min(len(ref_part), len(cand_part))
    ref_part = ref_part[:size]
    cand_part = cand_part[:size]
    ref_energy = sum(x * x for x in ref_part)
    error_energy = sum((a - b) ** 2 for a, b in zip(ref_part, cand_part))
    if ref_energy <= 0:
        return None
    return math.sqrt(error_energy / ref_energy)


def spectral_centroid(samples, sample_rate, window):
    """Magnitude-weighted mean frequency of a Hann-windowed naive DFT over
    the first `window` samples (the onset and early decay)."""
    n = min(window, len(samples))
    if n < 8:
        return None
    data = samples[:n]
    windowed = [
        value * (0.5 - 0.5 * math.cos(2.0 * math.pi * i / (n - 1)))
        for i, value in enumerate(data)
    ]
    weighted = 0.0
    total = 0.0
    for k in range(1, n // 2):
        angle_step = -2.0 * math.pi * k / n
        real = 0.0
        imag = 0.0
        for i, value in enumerate(windowed):
            angle = angle_step * i
            real += value * math.cos(angle)
            imag += value * math.sin(angle)
        magnitude = math.hypot(real, imag)
        freq = k * sample_rate / n
        weighted += freq * magnitude
        total += magnitude
    if total <= 0:
        return None
    return weighted / total


def envelope_blocks(samples, window):
    blocks = []
    for start in range(0, len(samples), window):
        chunk = samples[start : start + window]
        blocks.append(rms(chunk) if chunk else 0.0)
    return blocks


def decay_time_ms(samples, sample_rate, window, drop_db):
    """Milliseconds from the envelope peak until it stays at or below
    `drop_db` below that peak; None if the window never reaches it."""
    blocks = envelope_blocks(samples, window)
    peak_value = max(blocks) if blocks else 0.0
    if peak_value <= 0:
        return None
    threshold = peak_value * (10.0 ** (-drop_db / 20.0))
    peak_index = blocks.index(peak_value)
    for index in range(peak_index, len(blocks)):
        if blocks[index] <= threshold and all(
            value <= threshold for value in blocks[index : index + 3]
        ):
            return (index - peak_index) * window / sample_rate * 1000.0
    return None


def channel_metrics(reference, candidate):
    lag = best_onset_lag(reference, candidate, MAX_ONSET_LAG)
    return {
        "reference_rms": rms(reference),
        "vactr_rms": rms(candidate),
        "reference_peak": peak(reference),
        "vactr_peak": peak(candidate),
        "correlation": correlation(reference, candidate),
        "onset_lag_samples": lag,
        "normalized_rms_error": normalized_rms_error(reference, candidate, lag),
        "reference_spectral_centroid_hz": spectral_centroid(
            reference, SAMPLE_RATE, SPECTRAL_WINDOW
        ),
        "vactr_spectral_centroid_hz": spectral_centroid(
            candidate, SAMPLE_RATE, SPECTRAL_WINDOW
        ),
        "reference_decay_20db_ms": decay_time_ms(
            reference, SAMPLE_RATE, ENVELOPE_WINDOW, 20.0
        ),
        "vactr_decay_20db_ms": decay_time_ms(
            candidate, SAMPLE_RATE, ENVELOPE_WINDOW, 20.0
        ),
        "reference_decay_40db_ms": decay_time_ms(
            reference, SAMPLE_RATE, ENVELOPE_WINDOW, 40.0
        ),
        "vactr_decay_40db_ms": decay_time_ms(
            candidate, SAMPLE_RATE, ENVELOPE_WINDOW, 40.0
        ),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        required=True,
        type=Path,
        help="separate official Eurorack checkout at the pinned revision",
    )
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean_tracked_tree(source)
    require_clean_tracked_tree(source / "stmlib")
    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()

    with tempfile.TemporaryDirectory(prefix="vactr-plaits-drums-") as temp:
        executable = Path(temp) / "plaits_drums_reference"
        cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
        run(
            [
                clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
                str(ROOT / "verification/plaits_drums_reference.cc"),
                str(source / "plaits/dsp/engine/bass_drum_engine.cc"),
                str(source / "plaits/dsp/engine/snare_drum_engine.cc"),
                str(source / "plaits/dsp/engine/hi_hat_engine.cc"),
                str(source / "plaits/resources.cc"),
                str(source / "stmlib/dsp/units.cc"),
                str(source / "stmlib/utils/random.cc"),
                cxx_library,
                "-o", str(executable),
            ],
            cwd=ROOT,
        )
        cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")
        engines = []
        for engine in ENGINES:
            comparisons = []
            for harmonics, timbre, morph, accent in SCENARIOS:
                controls = [str(harmonics), str(timbre), str(morph), str(accent)]
                upstream = parse_channels(
                    run([str(executable), engine, *controls]), f"{engine} upstream"
                )
                vactr = parse_channels(
                    run(
                        [
                            "cargo", "run", "-q", "--example", "plaits_drums_reference",
                            "--", engine, *controls,
                        ],
                        cwd=ROOT,
                        env=cargo_env,
                    ),
                    f"{engine} vactr",
                )
                comparisons.append(
                    {
                        "harmonics": harmonics,
                        "timbre": timbre,
                        "morph": morph,
                        "accent": accent,
                        "main": channel_metrics(upstream[0], vactr[0]),
                        "aux": channel_metrics(upstream[1], vactr[1]),
                    }
                )
            engines.append({"engine": engine, "scenarios": comparisons})
    print(
        json.dumps(
            {
                "scope": (
                    "raw Plaits position 21/22/23 dual kick/snare/hi-hat "
                    "main/aux kernels; no LPG, voice or .vact host; a single "
                    "trigger at the first sample of each run"
                ),
                "upstream_revision": EURORACK_REVISION,
                "stmlib_revision": STMLIB_REVISION,
                "sample_rate_hz": SAMPLE_RATE,
                "block_frames": BLOCK_FRAMES,
                "frames_compared": FRAMES,
                "engines": engines,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

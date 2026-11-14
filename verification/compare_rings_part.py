#!/usr/bin/env python3
"""Compare raw Rings Part/StringSynthPart kernels without vendoring upstream
code or samples.

Drives the pinned upstream `rings::Part` (six `ResonatorModel` values) and
`rings::StringSynthPart` from a separate local checkout, and Vactr's
`resonator-part-core` / `string-choir-core` kernels, at the source rate and
block size (48 kHz, 24-sample blocks; see `rings/dsp/dsp.h`), with polyphony
fixed at 1, an internal strum/exciter trigger firing once at the start of the
render, and a fixed note. This is a raw-kernel probe: no `.vact` event, host
callback, LPG, voice-stealing, external-excitation routing, or browser
pathway is exercised. Vactr's kernel is an event-local, single-voice
adaptation, not a port of the source's shared, polyphonic `Part` state, so
gaps against the pinned source are expected; see
`impl-plans/active/modular-rings-resonator.md` and
`THIRD_PARTY_NOTICES.md#rings-part-architectural-reference`.
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

BLOCK = 24  # rings::kMaxBlockSize
BLOCKS = 4000  # 2.0 s at 48 kHz
FRAMES = BLOCK * BLOCKS
SAMPLE_RATE = 48000

# structure, brightness, damping, position
SCENARIOS = (
    (0.15, 0.25, 0.20, 0.30),
    (0.50, 0.50, 0.50, 0.50),
    (0.85, 0.75, 0.80, 0.70),
)

TARGETS = (
    ("0", "MODAL_RESONATOR"),
    ("1", "SYMPATHETIC_STRING"),
    ("2", "STRING"),
    ("3", "FM_VOICE"),
    ("4", "SYMPATHETIC_STRING_QUANTIZED"),
    ("5", "STRING_AND_REVERB"),
    ("stringsynth", "STRING_SYNTH_PART"),
)

# Onset detection: first sample whose magnitude reaches this fraction of the
# channel's peak absolute value.
ONSET_FRACTION = 0.05

# Fundamental estimate via time-domain autocorrelation.
AUTOCORR_WINDOW = 2048
AUTOCORR_MIN_HZ = 30.0
AUTOCORR_MAX_HZ = 1500.0

# Spectral centroid via a windowed, direct (non-FFT) DFT; no numpy dependency.
CENTROID_WINDOW = 1024

# Envelope for -20/-40 dB decay times: block RMS at this chunk size.
DECAY_CHUNK = 64

# Classification thresholds, averaged over every scenario/channel pair for a
# target. These are coarse triage bands, not a fidelity certification.
CLOSE_CORRELATION = 0.6
CLOSE_ERROR = 0.8
GAP_CORRELATION = 0.05
GAP_ERROR = 3.0


def run(argv, *, cwd=None, env=None, timeout=180):
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


def onset_index(signal, fraction=ONSET_FRACTION):
    peak = max((abs(value) for value in signal), default=0.0)
    if peak <= 0.0:
        return 0
    threshold = peak * fraction
    for index, value in enumerate(signal):
        if abs(value) >= threshold:
            return index
    return 0


def align_from_onset(reference, candidate):
    """Drop each channel's pre-onset lead-in, then trim to a common length."""
    ref_onset = onset_index(reference)
    cand_onset = onset_index(candidate)
    aligned_ref = reference[ref_onset:]
    aligned_cand = candidate[cand_onset:]
    size = min(len(aligned_ref), len(aligned_cand))
    return aligned_ref[:size], aligned_cand[:size], ref_onset, cand_onset


def level_metrics(reference, candidate):
    size = len(reference)
    if size == 0:
        return None
    ref_energy = sum(value * value for value in reference)
    got_energy = sum(value * value for value in candidate)
    error_energy = sum((a - b) ** 2 for a, b in zip(reference, candidate))
    ref_mean = sum(reference) / size
    got_mean = sum(candidate) / size
    ref_centered = [value - ref_mean for value in reference]
    got_centered = [value - got_mean for value in candidate]
    ref_variance = sum(value * value for value in ref_centered)
    got_variance = sum(value * value for value in got_centered)
    if ref_energy <= 0.0 or ref_variance <= 0.0 or got_variance <= 0.0:
        return {
            "reference_rms": math.sqrt(ref_energy / size),
            "vactr_rms": math.sqrt(got_energy / size),
            "correlation": None,
            "normalized_rms_error": None,
            "note": "zero reference or candidate variance; correlation undefined",
        }
    covariance = sum(a * b for a, b in zip(ref_centered, got_centered))
    return {
        "reference_rms": math.sqrt(ref_energy / size),
        "vactr_rms": math.sqrt(got_energy / size),
        "correlation": covariance / math.sqrt(ref_variance * got_variance),
        "normalized_rms_error": math.sqrt(error_energy / ref_energy),
    }


def spectral_centroid(signal, sample_rate=SAMPLE_RATE, window=CENTROID_WINDOW):
    n = min(window, len(signal))
    if n < 8:
        return None
    frame = signal[:n]
    windowed = [
        value * (0.5 - 0.5 * math.cos(2.0 * math.pi * i / (n - 1)))
        for i, value in enumerate(frame)
    ]
    max_bin = n // 2
    weighted = 0.0
    total = 0.0
    for k in range(1, max_bin):
        step = 2.0 * math.pi * k / n
        real = 0.0
        imag = 0.0
        for i, value in enumerate(windowed):
            angle = step * i
            real += value * math.cos(angle)
            imag -= value * math.sin(angle)
        magnitude = math.hypot(real, imag)
        weighted += (k * sample_rate / n) * magnitude
        total += magnitude
    return weighted / total if total > 0.0 else None


def fundamental_estimate(
    signal,
    sample_rate=SAMPLE_RATE,
    window=AUTOCORR_WINDOW,
    min_hz=AUTOCORR_MIN_HZ,
    max_hz=AUTOCORR_MAX_HZ,
):
    n = min(window, len(signal))
    if n < 32:
        return None
    frame = signal[:n]
    mean = sum(frame) / n
    centered = [value - mean for value in frame]
    energy0 = sum(value * value for value in centered)
    if energy0 <= 0.0:
        return None
    min_lag = max(1, int(sample_rate / max_hz))
    max_lag = min(n - 1, int(sample_rate / min_hz))
    best_lag = None
    best_value = 0.0
    for lag in range(min_lag, max_lag + 1):
        value = sum(
            centered[i] * centered[i + lag] for i in range(n - lag)
        )
        if value > best_value:
            best_value = value
            best_lag = lag
    if best_lag is None or best_value <= 0.05 * energy0:
        return None
    return sample_rate / best_lag


def decay_times(signal, sample_rate=SAMPLE_RATE, chunk=DECAY_CHUNK):
    n = len(signal)
    if n < chunk:
        return None, None
    envelope = []
    for start in range(0, n - chunk + 1, chunk):
        block = signal[start:start + chunk]
        envelope.append(math.sqrt(sum(value * value for value in block) / chunk))
    peak = max(envelope, default=0.0)
    if peak <= 0.0:
        return None, None
    peak_index = envelope.index(peak)
    target_20 = peak * (10.0 ** (-20.0 / 20.0))
    target_40 = peak * (10.0 ** (-40.0 / 20.0))
    time_20 = None
    time_40 = None
    for index in range(peak_index, len(envelope)):
        if time_20 is None and envelope[index] <= target_20:
            time_20 = index * chunk / sample_rate
        if time_40 is None and envelope[index] <= target_40:
            time_40 = index * chunk / sample_rate
            break
    return time_20, time_40


def channel_report(reference, candidate):
    aligned_ref, aligned_cand, ref_onset, cand_onset = align_from_onset(
        reference, candidate)
    metrics = level_metrics(aligned_ref, aligned_cand)
    ref_t20, ref_t40 = decay_times(aligned_ref)
    cand_t20, cand_t40 = decay_times(aligned_cand)
    return {
        "reference_onset_sample": ref_onset,
        "vactr_onset_sample": cand_onset,
        "aligned_frames_compared": len(aligned_ref),
        "level": metrics,
        "reference_spectral_centroid_hz": spectral_centroid(aligned_ref),
        "vactr_spectral_centroid_hz": spectral_centroid(aligned_cand),
        "reference_fundamental_hz": fundamental_estimate(aligned_ref),
        "vactr_fundamental_hz": fundamental_estimate(aligned_cand),
        "reference_decay_20db_s": ref_t20,
        "reference_decay_40db_s": ref_t40,
        "vactr_decay_20db_s": cand_t20,
        "vactr_decay_40db_s": cand_t40,
    }


def classify(scenarios):
    samples = []
    for scenario in scenarios:
        for channel in ("main", "aux"):
            level = scenario[channel]["level"]
            if level is None or level.get("correlation") is None:
                continue
            samples.append((level["correlation"], level["normalized_rms_error"]))
    if not samples:
        return "not comparable"
    avg_correlation = sum(c for c, _ in samples) / len(samples)
    avg_error = sum(e for _, e in samples) / len(samples)
    if avg_correlation >= CLOSE_CORRELATION and avg_error <= CLOSE_ERROR:
        return "close"
    if avg_correlation > GAP_CORRELATION or avg_error < GAP_ERROR:
        return "measured gap"
    return "not comparable"


def build_probe(source, temp_dir):
    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()
    executable = Path(temp_dir) / "rings_part_reference"
    cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
    sources = [
        str(ROOT / "verification/rings_part_reference.cc"),
        str(source / "rings/dsp/part.cc"),
        str(source / "rings/dsp/resonator.cc"),
        str(source / "rings/dsp/string.cc"),
        str(source / "rings/dsp/fm_voice.cc"),
        str(source / "rings/dsp/string_synth_part.cc"),
        str(source / "rings/resources.cc"),
        str(source / "stmlib/dsp/units.cc"),
        str(source / "stmlib/utils/random.cc"),
    ]
    run([
        clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
        *sources,
        cxx_library,
        "-o", str(executable),
    ], cwd=ROOT, timeout=300)
    return executable


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

    cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")

    with tempfile.TemporaryDirectory(prefix="vactr-rings-part-") as temp:
        executable = build_probe(source, temp)
        report = []
        for target, label in TARGETS:
            scenarios = []
            for structure, brightness, damping, position in SCENARIOS:
                controls = [str(structure), str(brightness), str(damping), str(position)]
                upstream = parse_channels(
                    run([str(executable), target, *controls], timeout=120),
                    f"upstream/{label}")
                vactr = parse_channels(run(
                    ["cargo", "run", "-q", "--example", "rings_part_reference",
                     "--", target, *controls],
                    cwd=ROOT, env=cargo_env, timeout=120,
                ), f"vactr/{label}")
                scenarios.append({
                    "structure": structure,
                    "brightness": brightness,
                    "damping": damping,
                    "position": position,
                    "main": channel_report(upstream[0], vactr[0]),
                    "aux": channel_report(upstream[1], vactr[1]),
                })
            report.append({
                "target": target,
                "label": label,
                "scenarios": scenarios,
                "classification": classify(scenarios),
            })

    print(json.dumps({
        "scope": (
            "raw rings::Part (polyphony 1, internal strum/exciter, fixed "
            "note) and rings::StringSynthPart kernels versus Vactr "
            "resonator-part-core/string-choir-core; no .vact event, host "
            "callback, LPG, voice-stealing, external-excitation routing, "
            "or browser pathway"
        ),
        "adaptation_note": (
            "Vactr is an event-local, single-voice adaptation, not a port "
            "of the source's shared polyphonic Part state; gaps against "
            "the pinned source are an expected outcome of this comparison, "
            "not a test failure"
        ),
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": SAMPLE_RATE,
        "block_frames": BLOCK,
        "frames_compared_before_alignment": FRAMES,
        "onset_fraction": ONSET_FRACTION,
        "targets": report,
    }, indent=2))


if __name__ == "__main__":
    main()

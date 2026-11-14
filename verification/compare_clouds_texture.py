#!/usr/bin/env python3
"""Compare four Vactr Clouds texture adaptations with pinned Clouds DSP."""

import json
import math
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parent.parent
EURORACK_REVISION = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4"
STMLIB_REVISION = "e3bd7c9cc00e4364166f9905c0509b6ffd0535ec"
RATE = 32_000
BLOCK = 32
FRAMES = RATE * 3
MODES = ("granular", "stretch", "loop", "spectral")
SCENARIOS = (
    {"position": .15, "size": .18, "pitch_st": -7, "density": .25,
     "texture": .22, "dry_wet": .70, "stereo_spread": 0, "feedback": .15, "reverb": .10},
    {"position": .50, "size": .52, "pitch_st": 0, "density": .50,
     "texture": .50, "dry_wet": .80, "stereo_spread": .50, "feedback": .45, "reverb": .35},
    {"position": .82, "size": .86, "pitch_st": 7, "density": .78,
     "texture": .82, "dry_wet": .90, "stereo_spread": 1, "feedback": .72, "reverb": .65},
)


def run(argv, *, cwd=None, env=None, stdin=None):
    try:
        return subprocess.run(argv, cwd=cwd, env=env, stdin=stdin, check=True,
                              capture_output=True, text=True, timeout=300).stdout
    except subprocess.CalledProcessError as error:
        raise RuntimeError(
            f"command failed ({error.returncode}): {' '.join(argv)}\n"
            f"stdout:\n{error.stdout}\nstderr:\n{error.stderr}"
        ) from error


def revision(path):
    return run(["git", "-C", str(path), "rev-parse", "HEAD"]).strip()


def require_clean(path):
    dirty = run(["git", "-C", str(path), "status", "--porcelain",
                 "--untracked-files=no"]).strip()
    if dirty:
        raise ValueError(f"reference checkout has modified tracked files: {path}")


def parse_stereo(output, label):
    rows = output.splitlines()
    if len(rows) != FRAMES:
        raise ValueError(f"{label}: expected {FRAMES} stereo rows, got {len(rows)}")
    channels = ([], [])
    for index, row in enumerate(rows, 1):
        fields = row.split()
        if len(fields) != 2:
            raise ValueError(f"{label}: row {index} is not a stereo pair")
        left, right = map(float, fields)
        if not math.isfinite(left) or not math.isfinite(right):
            raise ValueError(f"{label}: nonfinite output at row {index}")
        channels[0].append(left)
        channels[1].append(right)
    return channels


def input_samples():
    """Two distinct stereo tone mixtures plus a shared deterministic click train."""
    result = [[], []]
    for n in range(FRAMES):
        t = n / RATE
        if t < 2.0:
            click = 0.52 if n % 4000 < 3 else 0.0
            left = .24 * math.sin(2 * math.pi * 220 * t) + .12 * math.sin(2 * math.pi * 997 * t) + click
            right = .21 * math.sin(2 * math.pi * 330 * t + .31) + .11 * math.sin(2 * math.pi * 997 * t + .7) - click
        else:
            left = right = 0.0
        result[0].append(max(-.95, min(.95, left)))
        result[1].append(max(-.95, min(.95, right)))
    return result


def write_input(path, channels):
    with path.open("w", encoding="ascii") as stream:
        for left, right in zip(*channels):
            stream.write(f"{left:.9g} {right:.9g}\n")


def correlation(a, b):
    # Long-run correlations use a regular 16:1 decimation to keep this
    # dependency-free harness fast; the RMS and spectrum metrics use full data.
    if len(a) > 8192:
        a = a[::16]
        b = b[::16]
    count = len(a)
    ma, mb = sum(a) / count, sum(b) / count
    aa = sum((x - ma) ** 2 for x in a)
    bb = sum((x - mb) ** 2 for x in b)
    if aa <= 1e-24 or bb <= 1e-24:
        return None
    return sum((x - ma) * (y - mb) for x, y in zip(a, b)) / math.sqrt(aa * bb)


def fft_power(values):
    """In-place iterative radix-2 FFT power spectrum, standard library only."""
    data = [complex(value, 0.0) for value in values]
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
        angle = -2.0 * math.pi / length
        root = complex(math.cos(angle), math.sin(angle))
        half = length // 2
        for start in range(0, n, length):
            factor = 1 + 0j
            for offset in range(half):
                even = data[start + offset]
                odd = data[start + offset + half] * factor
                data[start + offset] = even + odd
                data[start + offset + half] = even - odd
                factor *= root
        length *= 2
    return [value.real * value.real + value.imag * value.imag for value in data[:n // 2 + 1]]


def spectral_metrics(signal):
    # Analyze a 4096-sample window decimated by eight (4 kHz analysis rate).
    stride = 8
    values = signal[4096:4096 + 4096 * stride:stride]
    mean = sum(values) / len(values)
    values = [(value - mean) * (0.5 - 0.5 * math.cos(2 * math.pi * i / (len(values) - 1)))
              for i, value in enumerate(values)]
    power = fft_power(values)
    sample_rate = RATE / stride
    frequencies = [i * sample_rate / 4096 for i in range(len(power))]
    total = sum(power)
    centroid = sum(f * p for f, p in zip(frequencies, power)) / total if total else 0.0
    bands = {}
    for low, high in ((31.25, 62.5), (62.5, 125), (125, 250), (250, 500),
                      (500, 1000), (1000, 2000)):
        energy = sum(p for f, p in zip(frequencies, power) if low <= f < high)
        bands[f"{int(low)}-{int(high)}Hz"] = energy / total if total else 0.0
    return {"spectral_centroid_hz": centroid, "octave_band_energy_ratio": bands}


def tail_decay(signal):
    # Input is silent after t=2s. Find the first 20 ms RMS window 40 dB below
    # the largest post-input window; report null if the tail does not reach it.
    start = 2 * RATE
    width = RATE // 50
    levels = []
    for at in range(start, len(signal) - width + 1, width):
        chunk = signal[at:at + width]
        levels.append(math.sqrt(sum(x * x for x in chunk) / width))
    peak = max(levels, default=0.0)
    for index, value in enumerate(levels):
        if peak > 0 and value <= peak * .01:
            return index * width / RATE
    return None


def lag_estimate(signal, dry):
    # Coarse normalized cross-correlation at 2 kHz with a +/-64 ms search.
    stride = 16
    start = 256
    count = 256
    a = signal[start * stride:(start + count) * stride:stride]
    b = dry[start * stride:(start + count) * stride:stride]
    best = (float("-inf"), 0)
    for lag in range(-128, 129):
        pairs = [(a[i], b[i + lag]) for i in range(count)
                 if 0 <= i + lag < count]
        score = correlation([x for x, _ in pairs], [y for _, y in pairs])
        if score is not None and abs(score) > best[0]:
            best = (abs(score), lag)
    return best[1] * stride * 1000.0 / RATE


def channel_metrics(output, dry, input_channels):
    warm = RATE // 4
    compared = output[warm:2 * RATE]
    rms = math.sqrt(sum(x * x for x in compared) / len(compared))
    dry_part = dry[warm:2 * RATE]
    dry_rms = math.sqrt(sum(x * x for x in dry_part) / len(dry_part))
    rms_dry = math.sqrt(sum(x * x for x in input_channels[warm:2 * RATE]) /
                        (len(input_channels[warm:2 * RATE])))
    return {
        "rms": rms,
        "dry_input_rms": rms_dry,
        "output_to_dry_rms_ratio": rms / dry_rms if dry_rms else None,
        "dry_input_correlation": correlation(compared, dry_part),
        "lag_estimate_ms": lag_estimate(output, dry),
        "feedback_tail_decay_to_minus_40db_s": tail_decay(output),
        **spectral_metrics(output),
    }


def compare_metrics(reference, candidate, dry):
    result = {}
    for label, ref, got, dry_channel in zip(("left", "right"), reference,
                                             candidate, dry):
        warm = RATE // 4
        a = ref[warm:2 * RATE]
        b = got[warm:2 * RATE]
        ref_rms = math.sqrt(sum(x * x for x in a) / len(a))
        got_rms = math.sqrt(sum(x * x for x in b) / len(b))
        err = math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)) / len(a))
        result[label] = {
            "rms": {"clouds": ref_rms, "vactr": got_rms},
            "correlation": correlation(a, b),
            "normalized_rms_error": err / ref_rms if ref_rms else None,
            "clouds_properties": channel_metrics(ref, dry_channel, dry_channel),
            "vactr_properties": channel_metrics(got, dry_channel, dry_channel),
        }
    ref_width = math.sqrt(sum((x - y) ** 2 for x, y in zip(reference[0], reference[1])) / FRAMES)
    got_width = math.sqrt(sum((x - y) ** 2 for x, y in zip(candidate[0], candidate[1])) / FRAMES)
    return {
        "channels": result,
        "stereo_correlation": {"clouds": correlation(reference[0], reference[1]),
                               "vactr": correlation(candidate[0], candidate[1])},
        "stereo_width_rms": {"clouds": ref_width, "vactr": got_width},
    }


def mode_classification(cases):
    correlations = [channel["correlation"] for case in cases
                    for channel in case["channels"].values()
                    if channel["correlation"] is not None]
    errors = [channel["normalized_rms_error"] for case in cases
              for channel in case["channels"].values()
              if channel["normalized_rms_error"] is not None]
    active = sum(channel["rms"]["clouds"] > 1.0e-7 and
                 channel["rms"]["vactr"] > 1.0e-7
                 for case in cases for channel in case["channels"].values())
    if active < len(cases) * 2:
        label = "not comparable"
        reason = "one implementation is silent in too many channel/scenario runs"
    elif correlations and errors and median(correlations) >= .95 and median(errors) <= .20:
        label = "close"
        reason = "median channel correlation >= 0.95 and median normalized RMS error <= 0.20"
    else:
        label = "measured gap"
        reason = "outputs are measurable; at least one close-response threshold is not met"
    return {
        "classification": label,
        "reason": reason,
        "median_channel_correlation": median(correlations) if correlations else None,
        "median_normalized_rms_error": median(errors) if errors else None,
        "scenario_freeze_quality_runs": len(cases),
    }


def median(values):
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) * .5


def main():
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path,
                        help="separate pinned Eurorack checkout")
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean(source)
    require_clean(source / "stmlib")

    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()
    cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")
    run(["cargo", "build", "--quiet", "--example", "clouds_texture_reference"],
        cwd=ROOT, env=cargo_env)
    with tempfile.TemporaryDirectory(prefix="vactr-clouds-texture-") as temp_name:
        temp = Path(temp_name)
        executable = temp / "clouds_texture_reference"
        input_path = temp / "input.txt"
        write_input(input_path, input_samples())
        library = "-lc++" if os.uname().sysname == "Darwin" else "-lstdc++"
        run([
            clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
            str(ROOT / "verification/clouds_texture_reference.cc"),
            str(source / "clouds/dsp/granular_processor.cc"),
            str(source / "clouds/dsp/correlator.cc"),
            str(source / "clouds/dsp/pvoc/stft.cc"),
            str(source / "clouds/dsp/pvoc/frame_transformation.cc"),
            str(source / "clouds/dsp/pvoc/phase_vocoder.cc"),
            str(source / "clouds/dsp/mu_law.cc"),
            str(source / "clouds/resources.cc"),
            str(source / "stmlib/dsp/atan.cc"),
            str(source / "stmlib/dsp/units.cc"),
            str(source / "stmlib/utils/random.cc"), library,
            "-o", str(executable),
        ], cwd=ROOT)

        outputs = []
        with input_path.open(encoding="ascii") as input_stream:
            dry = input_samples()
        vactr_executable = ROOT / "target/debug/examples/clouds_texture_reference"
        for mode_index, mode in enumerate(MODES):
            for scenario_index, controls in enumerate(SCENARIOS):
                for freeze in (0, 1):
                    for quality in range(4):
                        argv = [str(mode_index), str(scenario_index), str(freeze), str(quality)]
                        with input_path.open(encoding="ascii") as input_stream:
                            upstream = parse_stereo(run([str(executable), *argv],
                                                       cwd=ROOT, stdin=input_stream), "Clouds")
                        with input_path.open(encoding="ascii") as input_stream:
                            vactr_mode = "grain" if mode == "granular" else mode
                            vactr = parse_stereo(run([str(vactr_executable), vactr_mode,
                                                      str(scenario_index), str(freeze),
                                                      str(quality)], cwd=ROOT, env=cargo_env,
                                                     stdin=input_stream), "Vactr")
                        outputs.append({
                            "mode": mode,
                            "scenario": scenario_index,
                            "controls": controls,
                            "freeze": bool(freeze),
                            "quality": quality,
                            **compare_metrics(upstream, vactr, dry),
                        })
                        print(f"compared {len(outputs)}/96: {mode} scenario={scenario_index} freeze={freeze} quality={quality}",
                              file=__import__("sys").stderr, flush=True)
        mode_summaries = [
            {"mode": mode, **mode_classification(
                [case for case in outputs if case["mode"] == mode])}
            for mode in MODES
        ]
        print(json.dumps({
            "scope": "raw Clouds GranularProcessor vs direct Vactr texture kernels with bus dry/wet mix; not full firmware/codec parity",
            "upstream_revision": EURORACK_REVISION,
            "stmlib_revision": STMLIB_REVISION,
            "sample_rate_hz": RATE,
            "block_frames": BLOCK,
            "frames": FRAMES,
            "input": "3 seconds: two distinct stereo sine mixtures plus a shared click train for the first 2 seconds, then silence for tail measurement; Python writes one input file reused by both probes",
            "rate_and_mix_mapping": "Clouds receives normalized Python float input quantized to signed ShortFrame at 32 kHz and processes 32-frame blocks; quality 2/3 enables its 16 kHz SRC path. Vactr kernels run directly at 32 kHz in 32-frame blocks; its low-fidelity host-rate hold/quantizer remains an adaptation. Source uses equal-power dry/wet LUT; Vactr probe wraps the raw wet kernel in FxUnit's linear dry + (wet-dry)*mix bus blend.",
            "freeze_transition_seconds": 1.0,
            "quality_bits": {"0": "stereo/high", "1": "mono/high", "2": "stereo/low", "3": "mono/low"},
            "randomness_note": "Granular scheduling and several adaptations are stochastic; per-run correlation is descriptive, while distribution metrics are also reported.",
            "classification_thresholds": "close means median channel correlation >= 0.95 and median normalized RMS error <= 0.20; measured gap means both are measurable but threshold is missed; not comparable means either side is silent in over half of channel/scenario runs. These labels summarize this probe only.",
            "mode_classifications": mode_summaries,
            "comparisons": outputs,
        }, indent=2))


if __name__ == "__main__":
    main()

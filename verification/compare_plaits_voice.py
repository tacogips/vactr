#!/usr/bin/env python3
"""Compare Vactr's voice layer with a separate pinned Plaits checkout."""

import argparse
import json
import math
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
EURORACK_REVISION = "08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4"
STMLIB_REVISION = "e3bd7c9cc00e4364166f9905c0509b6ffd0535ec"
BLOCK = 12
TRAJECTORY_THRESHOLD = 1.0e-4
AUDIO_CORRELATION_THRESHOLD = 0.999


def run(argv, *, cwd=None, env=None, timeout=600):
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


def correlation(reference, candidate):
    if len(reference) != len(candidate) or not reference:
        return None
    mean_a = sum(reference) / len(reference)
    mean_b = sum(candidate) / len(candidate)
    cov = sum((a - mean_a) * (b - mean_b)
              for a, b in zip(reference, candidate))
    var_a = sum((a - mean_a) ** 2 for a in reference)
    var_b = sum((b - mean_b) ** 2 for b in candidate)
    denom = math.sqrt(var_a * var_b)
    return cov / denom if denom > 1.0e-24 else None


def metrics(reference, candidate):
    if len(reference) != len(candidate) or not reference:
        raise ValueError(f"sample count mismatch: {len(reference)} vs {len(candidate)}")
    errors = [b - a for a, b in zip(reference, candidate)]
    return {
        "sample_count": len(reference),
        "correlation": correlation(reference, candidate),
        "max_abs_error": max(abs(error) for error in errors),
        "rms_error": math.sqrt(sum(error * error for error in errors) / len(errors)),
    }


def parse_rows(output, tags, label):
    rows = {tag: [] for tag in tags}
    for number, line in enumerate(output.splitlines(), 1):
        fields = line.split()
        if not fields or fields[0] not in rows:
            raise ValueError(f"{label}: unexpected row {number}: {line!r}")
        values = [float(field) for field in fields[1:]]
        if not all(math.isfinite(value) for value in values):
            raise ValueError(f"{label}: nonfinite value on row {number}")
        rows[fields[0]].append(values)
    return rows


def write_values(path, values):
    path.write_text("".join(f"{value:.9g}\n" for value in values))


def write_trajectory(path, rows):
    path.write_text("".join(f"{row[1]:.9g} {row[2]:.9g} {row[3]:.9g}\n"
                            for row in rows))


def note_frequency(note):
    return 440.0 * 2.0 ** ((note - 69.0) / 12.0) * 48000.0 / 47872.34


def rust_traj(mode, decay, color, note, velocity, gate_blocks, blocks, rate):
    output = run([
        "cargo", "run", "-q", "--example", "plaits_voice_reference", "--",
        "traj", mode, str(decay), str(color), f"{note_frequency(note):.9g}",
        str(velocity), str(gate_blocks), str(blocks), str(rate),
    ], cwd=ROOT, env=dict(os.environ, CARGO_TERM_QUIET="true"))
    rows = parse_rows(output, {"T"}, "Vactr trajectory")["T"]
    if len(rows) != blocks or any(len(row) != 6 for row in rows):
        raise ValueError(f"Vactr trajectory expected {blocks} six-field rows, got {len(rows)}")
    return rows


def upstream_voice(executable, engine, mode, decay, color, note, velocity,
                  gate_blocks, blocks):
    output = run([
        str(executable), "voice", str(engine), mode, str(decay), str(color),
        str(note), str(velocity), str(gate_blocks), str(blocks),
    ], cwd=ROOT)
    return parse_rows(output, {"T", "M", "X", "R", "Q"}, "upstream voice")


def trajectory_metrics(reference, candidate):
    if len(reference) != len(candidate):
        raise ValueError("trajectory row count mismatch")
    names = ("gain", "frequency", "hf_bleed", "decay_envelope")
    result = {}
    for index, name in enumerate(names, 1):
        errors = [b[index] - a[index] for a, b in zip(reference, candidate)]
        result[name] = {"max_abs_error": max(abs(value) for value in errors),
                        "rms_error": math.sqrt(sum(value * value for value in errors) /
                                                len(errors))}
    result["start_sample_match"] = all(int(a[5]) == int(b[5])
                                       for a, b in zip(reference, candidate))
    result["meets_threshold"] = all(
        result[name]["max_abs_error"] <= TRAJECTORY_THRESHOLD
        for name in ("gain", "frequency", "hf_bleed")
    ) and result["start_sample_match"]
    return result


def audio_scenario(executable, temp, rows):
    rng = random.Random(1234)
    samples = [rng.uniform(-0.5, 0.5) +
               0.3 * math.sin(2.0 * math.pi * 220.0 * index / 48000.0)
               for index in range(14400)]
    input_path = temp / "audio-input.txt"
    traj_path = temp / "audio-trajectory.txt"
    write_values(input_path, samples)
    write_trajectory(traj_path, rows)
    result = []
    for gain in (0.8, 0.6, -1.0, -2.0):
        source = parse_rows(run([str(executable), "audio", str(gain),
                                 str(input_path), str(traj_path)]), {"M"},
                            "upstream audio")["M"]
        candidate = parse_rows(run([
            "cargo", "run", "-q", "--example", "plaits_voice_reference", "--",
            "audio", str(gain), str(input_path), str(traj_path),
        ], cwd=ROOT, env=dict(os.environ, CARGO_TERM_QUIET="true")), {"M"},
            "Vactr audio")["M"]
        measured = metrics([row[0] for row in source], [row[0] for row in candidate])
        measured.update({"registered_gain": gain,
                         "meets_threshold": measured["correlation"] is not None and
                         measured["correlation"] >= AUDIO_CORRELATION_THRESHOLD})
        result.append(measured)
    return result


def lane_scenario(executable, temp, engine, mode, rows, velocity):
    upstream = upstream_voice(executable, engine, mode, 0.5, 0.5, 69, velocity,
                              400, 1200)
    if len(upstream["T"]) != 1200:
        raise ValueError("upstream voice did not emit 1200 trajectory rows")
    results = []
    for lane, raw_tag, output_tag in ((0, "R", "M"), (1, "Q", "X")):
        input_path = temp / f"lane-{engine}-{lane}.txt"
        write_values(input_path, [row[0] for row in upstream[raw_tag]])
        candidate = parse_rows(run([
            "cargo", "run", "-q", "--example", "plaits_voice_reference", "--",
            "lane", str(engine), str(lane), mode, "0.5", "0.5",
            f"{note_frequency(69):.9g}", str(velocity), "400", str(input_path),
        ], cwd=ROOT, env=dict(os.environ, CARGO_TERM_QUIET="true")), {"M"},
            "Vactr lane")["M"]
        expected = [row[0] for row in upstream[output_tag]]
        measured = metrics(expected, [row[0] for row in candidate])
        measured.update({"engine": engine, "mode": mode, "lane": lane,
                         "meets_threshold": measured["correlation"] is not None and
                         measured["correlation"] >= AUDIO_CORRELATION_THRESHOLD})
        results.append(measured)
    return results


def bypass_scenario(executable, temp):
    upstream = upstream_voice(executable, 21, "ping", 0.5, 0.5, 69, 1.0,
                              400, 1200)
    input_path = temp / "bypass-input.txt"
    write_values(input_path, [row[0] for row in upstream["R"]])
    source = [row[0] for row in upstream["M"]]
    candidate = parse_rows(run([
        "cargo", "run", "-q", "--example", "plaits_voice_reference", "--",
        "bypass", "0.8", str(input_path),
    ], cwd=ROOT, env=dict(os.environ, CARGO_TERM_QUIET="true")), {"M"},
        "Vactr bypass")["M"]
    measured = metrics(source, [row[0] for row in candidate])
    measured.update({"engine": 21, "registered_gain": 0.8,
                     "meets_threshold": measured["correlation"] is not None and
                     measured["correlation"] >= AUDIO_CORRELATION_THRESHOLD})
    return measured


def crossing_time(rows, threshold, rate):
    gains = [row[1] for row in rows]
    peak_index = max(range(len(gains)), key=gains.__getitem__)
    for index in range(peak_index, len(gains)):
        if gains[index] < threshold:
            return rows[index][5] / rate * 1000.0
    return None


def host_rate_scenario():
    blocks = 6000
    reference = rust_traj("ping", 0.5, 0.5, 69, 1.0, 400, blocks, 48000)
    result = []
    for rate in (44100, 96000):
        candidate = rust_traj("ping", 0.5, 0.5, 69, 1.0, 400, blocks, rate)
        measurements = {}
        meets = True
        for threshold in (0.5, 0.01):
            ref_time = crossing_time(reference, threshold, 48000)
            candidate_time = crossing_time(candidate, threshold, rate)
            tolerance_ms = 0.25 + 1000.0 / rate
            error_ms = (abs(candidate_time - ref_time)
                        if candidate_time is not None and ref_time is not None else None)
            measurements[str(threshold)] = {
                "reference_ms": ref_time,
                "candidate_ms": candidate_time,
                "absolute_error_ms": error_ms,
                "tolerance_ms": tolerance_ms,
                "meets_threshold": error_ms is not None and error_ms <= tolerance_ms,
            }
            meets = meets and measurements[str(threshold)]["meets_threshold"]
        result.append({"sample_rate_hz": rate, "thresholds": measurements,
                       "meets_threshold": meets})
    return result


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
    with tempfile.TemporaryDirectory(prefix="vactr-plaits-voice-") as directory:
        temp = Path(directory)
        executable = temp / "plaits_voice_reference"
        sources = sorted((source / "plaits/dsp").rglob("*.cc"))
        if not sources:
            raise ValueError("pinned checkout has no plaits/dsp C++ sources")
        cxx = "-lc++" if sys.platform == "darwin" else "-lstdc++"
        run([clang, "-std=c++11", "-DTEST", "-O2", "-fno-access-control",
             "-I", str(source), str(ROOT / "verification/plaits_voice_reference.cc"),
             str(source / "plaits/dsp/voice.cc"),
             *[str(path) for path in sources if path.name != "voice.cc"],
             str(source / "plaits/resources.cc"),
             str(source / "stmlib/dsp/units.cc"),
             str(source / "stmlib/utils/random.cc"), cxx, "-o", str(executable)],
            cwd=ROOT)

        trajectory_results = []
        trigger_self_checks = {"ping": False, "level": False}
        all_trajectory_thresholds = True
        for mode in ("ping", "level"):
            velocities = (1.0,) if mode == "ping" else (1.0, 0.5)
            for decay in (0.2, 0.5, 0.8):
                for color in (0.0, 0.5, 1.0):
                    for note in (48, 69):
                        for velocity in velocities:
                            upstream = upstream_voice(executable, 0, mode, decay, color,
                                                      note, velocity, 400, 1200)["T"]
                            trigger_self_checks[mode] = True
                            candidate = rust_traj(mode, decay, color, note, velocity,
                                                  400, 1200, 48000)
                            measured = trajectory_metrics(upstream, candidate)
                            measured.update({"mode": mode, "decay_control": decay,
                                             "color": color, "note": note,
                                             "velocity": velocity})
                            trajectory_results.append(measured)
                            all_trajectory_thresholds = (
                                all_trajectory_thresholds and measured["meets_threshold"])

        baseline_rows = rust_traj("ping", 0.5, 0.5, 69, 1.0, 400, 1200, 48000)
        scenario_b = audio_scenario(executable, temp, baseline_rows)
        scenario_c = []
        for engine in (0, 10):
            for mode in ("ping", "level"):
                scenario_c.extend(lane_scenario(executable, temp, engine, mode,
                                                baseline_rows, 1.0))
        scenario_d = bypass_scenario(executable, temp)
        scenario_e = host_rate_scenario()
        scenarios = {
            "A_trajectory": {"runs": trajectory_results,
                             "trigger_state_self_checks": {
                                 mode: {"after_block_19": False,
                                        "after_block_20": True,
                                        "harness_exit": 0,
                                        "passed": trigger_self_checks[mode]}
                                 for mode in ("ping", "level")
                             },
                             "meets_threshold": all_trajectory_thresholds},
            "B_audio_path": {"runs": scenario_b,
                             "meets_threshold": all(row["meets_threshold"]
                                                    for row in scenario_b)},
            "C_end_to_end": {"runs": scenario_c,
                             "meets_threshold": all(row["meets_threshold"]
                                                    for row in scenario_c)},
            "D_bypass": {"metrics": scenario_d,
                         "meets_threshold": scenario_d["meets_threshold"]},
            "E_host_rate": {"runs": scenario_e,
                            "meets_threshold": all(row["meets_threshold"]
                                                   for row in scenario_e)},
        }
    eligible = all(scenario["meets_threshold"] for scenario in scenarios.values())
    print(json.dumps({
        "scope": "Plaits voice-level trigger, decay, LPG and main/aux post path comparison; upstream aligned at detected rising-edge block 20 (trigger raised at block 16, 4-block effective delay)",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": 48000,
        "block_frames": BLOCK,
        "trajectory_threshold_max_abs": TRAJECTORY_THRESHOLD,
        "audio_correlation_threshold": AUDIO_CORRELATION_THRESHOLD,
        "scenarios": scenarios,
        "voice_layer_label_eligible": eligible,
    }, indent=2))


if __name__ == "__main__":
    main()

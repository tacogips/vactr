#!/usr/bin/env python3
"""Compare two raw FM kernels without vendoring upstream code or samples."""

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
FRAMES = 24 * 100
WARMUP = 24
SCENARIOS = (
    (0.1, 0.25, 0.2),
    (0.5, 0.5, 0.5),
    (0.9, 0.75, 0.8),
)


def run(argv, *, cwd=None, env=None):
    return subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
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


def metrics(reference, candidate):
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
        raise ValueError("comparison output has zero reference or variance")
    return {
        "reference_rms": math.sqrt(ref_energy / size),
        "vactr_rms": math.sqrt(got_energy / size),
        "correlation": covariance / math.sqrt(ref_variance * got_variance),
        "normalized_rms_error": math.sqrt(error_energy / ref_energy),
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

    with tempfile.TemporaryDirectory(prefix="vactr-plaits-fm-") as temp:
        executable = Path(temp) / "plaits_fm_reference"
        cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
        run([
            clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
            str(ROOT / "verification/plaits_fm_reference.cc"),
            str(source / "plaits/dsp/engine/fm_engine.cc"),
            str(source / "plaits/resources.cc"),
            str(source / "stmlib/dsp/units.cc"),
            cxx_library,
            "-o", str(executable),
        ], cwd=ROOT)
        cargo_env = dict(os.environ, CARGO_TERM_QUIET="true")
        comparisons = []
        for harmonics, timbre, morph in SCENARIOS:
            controls = [str(harmonics), str(timbre), str(morph)]
            upstream = parse_channels(
                run([str(executable), *controls]), "upstream")
            vactr = parse_channels(run(
                ["cargo", "run", "-q", "--example", "fm_pair_reference", "--", *controls],
                cwd=ROOT, env=cargo_env,
            ), "vactr")
            comparisons.append({
                "harmonics": harmonics,
                "timbre": timbre,
                "morph": morph,
                "main": metrics(upstream[0], vactr[0]),
                "aux": metrics(upstream[1], vactr[1]),
            })
    print(json.dumps({
        "scope": "raw Plaits position-10 FM carrier/sub kernels; no LPG, trigger, voice or .vact host",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": 48000,
        "block_frames": 24,
        "warmup_frames": WARMUP,
        "frames_compared": FRAMES - WARMUP,
        "scenarios": comparisons,
    }, indent=2))


if __name__ == "__main__":
    main()

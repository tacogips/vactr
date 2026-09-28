#!/usr/bin/env python3
"""Measure a pinned Warps vocoder in a separate checkout without vendoring it."""

import argparse
import json
import math
from pathlib import Path
import sys
import tempfile

from compare_plaits_fm import (
    EURORACK_REVISION,
    ROOT,
    STMLIB_REVISION,
    require_clean_tracked_tree,
    revision,
    run,
)


FRAMES = 60 * 100
SCENARIOS = (
    (0.1, 0.25, 100, 0.1),
    (0.5, 0.5, 100, 0.5),
    (0.9, 0.75, 100, 0.9),
    (0.1, 0.5, 50, 0.1),
    (0.9, 0.5, 50, 0.9),
    (0.5, 0.5, 50, 1.0),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    args = parser.parse_args()
    source = args.source.resolve()
    if revision(source) != EURORACK_REVISION:
        raise ValueError(f"Eurorack checkout must be {EURORACK_REVISION}")
    if revision(source / "stmlib") != STMLIB_REVISION:
        raise ValueError(f"stmlib checkout must be {STMLIB_REVISION}")
    require_clean_tracked_tree(source)
    require_clean_tracked_tree(source / "stmlib")
    clang = run(["mise", "which", "clang"], cwd=ROOT).strip()

    with tempfile.TemporaryDirectory(prefix="vactr-warps-vocoder-") as temp:
        executable = Path(temp) / "warps_vocoder_reference"
        filter_resource = Path(temp) / "warps_filter_bank_reference.cc"
        resource_text = (source / "warps/resources.cc").read_text()
        filter_prefix, marker, _ = resource_text.partition("\nconst float lut_sin[] = {")
        if not marker or "const float* filter_bank_table[]" not in filter_prefix:
            raise ValueError("pinned filter-bank resource boundary changed")
        filter_resource.write_text(filter_prefix + "\n}  // namespace warps\n")
        cxx_library = "-lc++" if sys.platform == "darwin" else "-lstdc++"
        run([
            clang, "-std=c++11", "-DTEST", "-O2", "-I", str(source),
            str(ROOT / "verification/warps_vocoder_reference.cc"),
            str(source / "warps/dsp/vocoder.cc"),
            str(source / "warps/dsp/filter_bank.cc"),
            str(filter_resource),
            str(source / "stmlib/dsp/units.cc"),
            cxx_library, "-o", str(executable),
        ], cwd=ROOT)
        measurements = []
        for release, formant, gate_block, after_release in SCENARIOS:
            output = run([
                str(executable), str(release), str(formant), str(gate_block),
                str(after_release),
            ])
            samples = [float(line) for line in output.splitlines()]
            if len(samples) != FRAMES or not all(map(math.isfinite, samples)):
                raise ValueError("reference output must contain 6,000 finite samples")
            late = samples[60 * 20:]
            measurements.append({
                "release": release,
                "formant": formant,
                "gate_block": gate_block,
                "after_release": after_release,
                "rms": math.sqrt(sum(x * x for x in samples) / FRAMES),
                "late_rms": math.sqrt(sum(x * x for x in late) / len(late)),
                "tail_rms": math.sqrt(sum(x * x for x in samples[60 * 75:]) / (60 * 25)),
                "min": min(samples),
                "max": max(samples),
                "sample_points": {
                    str(index): samples[index]
                    for index in (200, 300, 600, 1200, 3000, 4500, 5999)
                } if (release, formant, gate_block) == (0.5, 0.5, 100) else {},
            })
    print(json.dumps({
        "scope": "raw source vocoder only; no modulator, carrier generator or Vactr host",
        "upstream_revision": EURORACK_REVISION,
        "stmlib_revision": STMLIB_REVISION,
        "sample_rate_hz": 96000,
        "block_frames": 60,
        "frames": FRAMES,
        "measurements": measurements,
    }, indent=2))


if __name__ == "__main__":
    main()

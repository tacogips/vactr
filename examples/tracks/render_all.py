#!/usr/bin/env python3
"""Export and audit the original collection with Vactr's production renderer."""
import argparse
import concurrent.futures
import hashlib
import html
import json
from pathlib import Path
import subprocess
import wave

ROOT = Path(__file__).resolve().parents[2]
CATALOG = json.loads((ROOT / "examples/tracks/catalog.json").read_text())


def export(track, destination, renderer):
    score = ROOT / "examples/tracks" / track["g"] / (track["slug"] + ".vact")
    output = destination / track["g"] / (track["slug"] + ".wav")
    output.parent.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        [str(renderer), str(score), str(output), "--cycles", "64", "--tail-seconds", "8"],
        cwd=ROOT, capture_output=True, text=True, check=False,
    )
    if result.returncode:
        raise RuntimeError(f"{track['title']}: {result.stdout} {result.stderr}")
    metrics = json.loads(result.stderr.strip().splitlines()[-1])
    if metrics["cycles"] != 64 or metrics["sample_rate"] != 48000:
        raise RuntimeError(f"Unexpected duration/rate: {track['title']}")
    if not 1e-4 < metrics["rms"] < 1 or not 0 < metrics["peak"] < 1:
        raise RuntimeError(f"Silent/clipping audio: {track['title']}")
    if len(metrics["sections"]) != 8 or any(s["rms"] < 1e-5 for s in metrics["sections"]):
        raise RuntimeError(f"Missing arrangement section: {track['title']}")
    if metrics["sections"][-1]["rms"] >= max(s["rms"] for s in metrics["sections"]) * 0.75:
        raise RuntimeError(f"Ending did not recede: {track['title']}")
    preview = destination / "previews" / track["g"] / (track["slug"] + ".wav")
    preview.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(output), "rb") as source:
        if (source.getnchannels(), source.getsampwidth(), source.getframerate(), source.getnframes()) != (2, 2, 48000, metrics["frames"]):
            raise RuntimeError(f"Invalid WAV header: {track['title']}")
        expected = round((64 * 240 / track["bpm"] + 8) * 48000)
        if abs(metrics["frames"] - expected) > 1:
            raise RuntimeError(f"Wrong musical duration: {track['title']}")
        source.setpos(round(32 * 240 / track["bpm"] * 48000))
        with wave.open(str(preview), "wb") as clip:
            clip.setparams(source.getparams())
            clip.writeframes(source.readframes(24 * 48000))
    metrics.update(track)
    metrics["score_sha256"] = hashlib.sha256(score.read_bytes()).hexdigest()
    metrics["wav_bytes"] = output.stat().st_size
    metrics["preview"] = str(preview)
    print(f"PASS {track['g']:7s} {track['title']:18s} {metrics['duration_seconds']:.1f}s peak={metrics['peak']:.3f} RMS={metrics['rms']:.4f}", flush=True)
    return metrics


def playlist(destination, metrics):
    cards = []
    for genre in dict.fromkeys(track["g"] for track in metrics):
        cards.append(f"<h2>{html.escape(genre.title())}</h2><section>")
        for track in metrics:
            if track["g"] != genre:
                continue
            path = f"{genre}/{track['slug']}.wav"
            minutes, seconds = divmod(round(track["duration_seconds"]), 60)
            cards.append(f"<article><h3>{html.escape(track['title'])}</h3><p>{track['bpm']} BPM · {html.escape(track['key'])} · {minutes}:{seconds:02}</p><p>{html.escape(track['desc'])}</p><label>24-second refrain preview</label><audio controls preload='none' src='previews/{path}'></audio><details><summary>Full song</summary><audio controls preload='none' src='{path}'></audio></details><a href='{path}' download>Download WAV</a></article>")
        cards.append("</section>")
    page = """<!doctype html><html lang='en'><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Vactr — Original Track Collection</title><style>body{background:#10151d;color:#e9eef5;font:16px system-ui;max-width:1100px;margin:auto;padding:32px}h1{font-size:36px}h2{margin-top:40px;text-transform:capitalize;color:#8bd6ca}section{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:18px}article{padding:22px;border:1px solid #344152;border-radius:14px;background:#19222e}h3{margin-top:0}p{color:#b7c7d8;line-height:1.5}audio{width:100%;margin:12px 0}label,summary{font-size:14px;color:#c5d9e6}a{color:#8bd6ca}details{margin-bottom:14px}</style><h1>Original Track Collection</h1><p>Original instrumental tracks with synthesized voices and a licensed break recreation. 64-bar arrangements and natural reverb tails. Stereo 48 kHz / 16-bit WAV. Preview begins at the refrain; expand to play the complete song.</p>""" + "".join(cards) + "</html>"
    (destination / "index.html").write_text(page)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "tmp/genre-tracks")
    parser.add_argument("--workers", type=int, default=2)
    args = parser.parse_args()
    if not 1 <= args.workers <= 4:
        parser.error("workers must be between 1 and 4")
    renderer = ROOT / "target/release/examples/render_track"
    if not renderer.is_file():
        parser.error("Build first: CARGO_TERM_QUIET=true mise exec -- cargo build --release --example render_track")
    destination = args.output.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(export, track, destination, renderer) for track in CATALOG]
        metrics = [future.result() for future in futures]
    (destination / "metrics.json").write_text(json.dumps(metrics, indent=2) + "\n")
    playlist(destination, metrics)
    print(f"Verified {len(metrics)} tracks. Playlist: {destination / 'index.html'}", flush=True)


if __name__ == "__main__":
    main()

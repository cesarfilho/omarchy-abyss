#!/usr/bin/env python3
"""Renders the README animations with `abyss snapshot --ppm` (no terminal, nothing on your screen).

    scripts/make-gifs.py [name ...]      # default: all of them; needs ffmpeg and a release build
"""
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target/release/abyss"
OUT = ROOT / "docs/screenshots"

# name: (zone, reef, start epoch seconds, seconds, fps, extra snapshot args)
GIFS = {
    "swim": (0.5, 12, 1700000540, 12, 10, []),
    "shark": (1.0, 10, 1699999956, 28, 10, []),
    "titanic": (4.2, 14, 1700000500, 10, 10, ["--landmark", "titanic"]),
}
SIZE = "640x360"


def render(args):
    path, zone, reef, t, extra = args
    subprocess.run([BIN, "snapshot", str(zone), str(reef), "--size", SIZE, "--ppm", str(path), "--t", f"{t:.3f}", *extra], check=True)


def make(name):
    zone, reef, t0, secs, fps, extra = GIFS[name]
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        jobs = [(tmp / f"f{i}.ppm", zone, reef, t0 + i / fps, extra) for i in range(secs * fps)]
        with ThreadPoolExecutor(max_workers=6) as pool:
            list(pool.map(render, jobs))
        pal = tmp / "pal.png"
        src = ["-framerate", str(fps), "-i", str(tmp / "f%d.ppm")]
        subprocess.run(["ffmpeg", "-loglevel", "error", "-y", *src, "-vf", "palettegen=max_colors=96:stats_mode=diff", str(pal)], check=True)
        out = OUT / f"{name}.gif"
        subprocess.run(["ffmpeg", "-loglevel", "error", "-y", *src, "-i", str(pal), "-lavfi", "paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle", str(out)], check=True)
        print(f"{out.relative_to(ROOT)}  {out.stat().st_size / 1e6:.1f} MB")


if __name__ == "__main__":
    for n in sys.argv[1:] or GIFS:
        make(n)

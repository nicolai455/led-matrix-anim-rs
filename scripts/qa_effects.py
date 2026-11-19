#!/usr/bin/env python3
"""Objective QA for the rendered effect GIFs.

Vision review is subjective and slow; this measures the artifacts directly and
answers the questions that actually decide "is this effect broken":

* Is anything visible at all (non-black fraction)?
* Does it animate (frame-to-frame difference)?
* Are the colours plausible for the name (sorted, not one flat colour where
  variety is expected)?
* Is the content confined to a corner / a thin band (a signature of broken
  geometry maths that renders into one quadrant)?
* Does the GIF loop (first frame close to last)?

Outputs a machine-readable JSON next to a markdown table, and exits non-zero if
any effect is objectively broken.

Usage: python3 scripts/qa_effects.py [--gifs DIR] [--out DIR]
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import dataclass, asdict

from PIL import Image, ImageSequence

# Effects that are static or nearly so by design: identical frames are expected.
STATIC_BY_DESIGN = {"TestPattern", "Solid", "TextStatic"}
# Pass-through effect: black without an external LFX/audio source.
KNOWN_BLACK = {"Frames"}
# Effects whose whole point is a single solid flash (a strobe can be all-white).
STROBE = {"stroboscope", "strobe_blinder", "police_strobe"}
# Effects that legitimately occupy a thin band rather than the whole panel.
BANDED = {"TextStatic", "TextScroll", "text_effects", "freq_bars", "spectrogram",
          "audio_waveform", "equalizer_city", "radial_spectrum"}

# Below this share of lit pixels an effect is "effectively black".
MIN_LIT_FRACTION = 0.002
# Frame-to-frame difference below this means "not animating" (0..255 mean abs).
MIN_MOTION = 0.35


@dataclass
class Verdict:
    index: int
    id: str
    name: str
    status: str          # ok | check | broken
    frames: int
    lit_fraction: float  # share of pixels lit in the brightest frame
    motion: float        # mean abs frame delta
    distinct_colors: int
    bbox_fill: float     # lit-area / bbox-area of the brightest frame
    bbox_cover: float    # bbox area / panel area
    loops: bool
    observation: str
    action: str


def analyse(path: str) -> dict:
    with Image.open(path) as image:
        frames = [frame.convert("RGB").copy() for frame in ImageSequence.Iterator(image)]

    if not frames:
        return {"frames": 0, "lit_fraction": 0.0, "motion": 0.0, "distinct_colors": 0,
                "bbox_fill": 0.0, "bbox_cover": 0.0, "loops": False}

    width, height = frames[0].size
    panel_pixels = width * height

    def luma(frame: Image.Image) -> bytes:
        return frame.convert("L").tobytes()

    luma_frames = [luma(frame) for frame in frames]

    # Brightest frame by lit-pixel count.
    def lit_of(data: bytes) -> int:
        return sum(1 for value in data if value > 8)

    lit_counts = [lit_of(data) for data in luma_frames]
    brightest = max(range(len(frames)), key=lambda i: lit_counts[i])
    lit_fraction = lit_counts[brightest] / panel_pixels

    # Motion: mean absolute difference between consecutive frames.
    deltas = []
    for a, b in zip(luma_frames, luma_frames[1:]):
        total = sum(abs(x - y) for x, y in zip(a, b))
        deltas.append(total / panel_pixels)
    motion = sum(deltas) / len(deltas) if deltas else 0.0

    # Distinct colours across the whole clip (RGB triples).
    colors = set()
    for frame in frames[:: max(1, len(frames) // 12)]:
        colors.update(frame.getdata())  # pyright: ignore[reportArgumentType]

    # Bounding box of lit pixels in the brightest frame.
    bright = frames[brightest].convert("L")
    mask = bright.point(lambda v: 255 if int(v) > 8 else 0)
    mask = mask.convert("L")  # pyright: ignore[reportArgumentType]
    bbox = mask.getbbox()
    if bbox:
        bx0, by0, bx1, by1 = bbox
        bbox_area = max(1, (bx1 - bx0) * (by1 - by0))
        bbox_fill = lit_counts[brightest] / bbox_area
        bbox_cover = bbox_area / panel_pixels
    else:
        bbox_fill = 0.0
        bbox_cover = 0.0

    # Loop continuity: the first frame should resemble the last for a seamless loop.
    first, last = luma_frames[0], luma_frames[-1]
    loop_delta = sum(abs(x - y) for x, y in zip(first, last)) / panel_pixels
    loops = loop_delta < 40.0

    return {
        "frames": len(frames),
        "lit_fraction": round(lit_fraction, 4),
        "motion": round(motion, 3),
        "distinct_colors": len(colors),
        "bbox_fill": round(bbox_fill, 3),
        "bbox_cover": round(bbox_cover, 3),
        "loops": loops,
    }


def verdict(index: int, effect_id: str, name: str, stats: dict) -> Verdict:
    lit = stats["lit_fraction"]
    motion = stats["motion"]
    colors = stats["distinct_colors"]
    fill = stats["bbox_fill"]
    cover = stats["bbox_cover"]

    notes: list[str] = []
    status = "ok"
    action = "none"

    if effect_id in KNOWN_BLACK:
        if lit > 0.002:
            notes.append("expected black without LFX source but renders content")
        else:
            status = "ok"
            notes.append("black by design (needs external LFX source)")
            action = "none"
        return Verdict(index, effect_id, name, status, stats["frames"], lit, motion, colors,
                       fill, cover, stats["loops"], "; ".join(notes) or "as documented", action)

    # Effectively black.
    if lit < MIN_LIT_FRACTION:
        return Verdict(index, effect_id, name, "broken", stats["frames"], lit, motion, colors,
                       fill, cover, stats["loops"],
                       f"effectively black ({lit*100:.2f}% lit)", "investigate render path")

    # No animation and not static by design.
    if motion < MIN_MOTION and effect_id not in STATIC_BY_DESIGN:
        status = "check"
        notes.append(f"barely moves (delta {motion:.2f})")
        action = "verify animation is intended"

    # Content trapped in a small corner: a classic broken-geometry signature.
    if cover < 0.20 and effect_id not in STROBE and effect_id not in BANDED:
        status = "broken" if cover < 0.10 else "check"
        notes.append(f"content confined to {cover*100:.0f}% of panel")
        action = "check effect coordinate maths"

    # A single flat colour where the effect should show structure.
    if colors <= 1 and effect_id not in STATIC_BY_DESIGN and effect_id not in STROBE:
        status = "broken"
        notes.append("single flat colour")
        action = "check colour/palette generation"

    # Full-panel solid white isn't a strobe problem, but a flat 100% panel is.
    if lit > 0.98 and colors <= 2 and effect_id not in STROBE and effect_id not in STATIC_BY_DESIGN:
        status = "broken"
        notes.append("whole panel flooded with one colour")
        action = "check fill/alpha logic"

    # NOTE: a 4-second slice of a continuous animation is not expected to loop
    # seamlessly, so the seam is reported for information only and never changes
    # the status. Only a periodic effect should be judged on it.

    return Verdict(index, effect_id, name, status, stats["frames"], lit, motion, colors,
                   fill, cover, stats["loops"],
                   "; ".join(notes) if notes else "visible, animated, structured", action)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--gifs", default="docs/effects/gifs")
    parser.add_argument("--summary", default="docs/effects/summary.json")
    parser.add_argument("--out", default=".hermes/qa")
    args = parser.parse_args()

    with open(args.summary) as handle:
        summary = json.load(handle)
    effects = summary["effects"]

    os.makedirs(args.out, exist_ok=True)
    verdicts: list[Verdict] = []
    for row in effects:
        stem = f"{row['index'] + 1:03d}-{row['id']}"
        path = os.path.join(args.gifs, f"{stem}.gif")
        if not os.path.exists(path):
            verdicts.append(Verdict(row["index"], row["id"], row["name"], "broken", 0, 0.0, 0.0,
                                    0, 0.0, 0.0, False, "GIF missing", "render the artifact"))
            continue
        verdicts.append(verdict(row["index"], row["id"], row["name"], analyse(path)))
        print(f"[{row['index']+1:>3}/{len(effects)}] {row['id']:<22} {verdicts[-1].status:<6} "
              f"lit={verdicts[-1].lit_fraction*100:5.1f}% motion={verdicts[-1].motion:6.2f} "
              f"colors={verdicts[-1].distinct_colors:>3}", flush=True)

    with open(os.path.join(args.out, "measurements.json"), "w") as handle:
        json.dump([asdict(v) for v in verdicts], handle, indent=1)

    lines = ["| id | status | lit | motion | colors | panel cover | observation | action |",
             "|---|---|---|---|---|---|---|---|"]
    for v in verdicts:
        lines.append(
            f"| {v.id} | {v.status} | {v.lit_fraction*100:.1f}% | {v.motion:.2f} | "
            f"{v.distinct_colors} | {v.bbox_cover*100:.0f}% | {v.observation} | {v.action} |"
        )
    with open(os.path.join(args.out, "measurements.md"), "w") as handle:
        handle.write("\n".join(lines) + "\n")

    ok = sum(1 for v in verdicts if v.status == "ok")
    check = sum(1 for v in verdicts if v.status == "check")
    broken = sum(1 for v in verdicts if v.status == "broken")
    print(f"\ntotal {len(verdicts)}: {ok} ok, {check} check, {broken} broken")
    for v in verdicts:
        if v.status != "ok":
            print(f"  {v.status:<6} {v.id:<22} {v.observation}")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())

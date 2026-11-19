# matrix-gif

Renders every built-in LED matrix effect to an animated GIF and a 2x3 PNG
contact sheet, and writes a JSON summary of what was produced.

The binary drives the real `matrix_core::Engine` with `builtin_effects()`, so
the artifacts always match what the panel shows — no parallel rendering path.

## Usage

```bash
# All effects -> docs/effects/gifs/<index>-<id>.gif + summary.json
cargo run --release -p matrix-gif -- --all \
    --out docs/effects/gifs --summary docs/effects/summary.json

# Contact sheets -> docs/effects/contact-sheets/<index>-<id>.png
cargo run --release -p matrix-gif -- contact-sheets --all \
    --out docs/effects/contact-sheets --summary docs/effects/contact-sheets-summary.json

# Single effects, shorter clip, other scale
cargo run --release -p matrix-gif -- --effect fire --effect plasma \
    --seconds 2.5 --fps 30 --scale 8 --out /tmp/gifs
```

## Flags

| Flag | Default | Meaning |
| --- | --- | --- |
| `--out <dir>` | `docs/effects/gifs` | Output directory (`contact-sheets`: `docs/effects/contact-sheets`) |
| `--all` | off | Render every effect (also the default when no `--effect` is given) |
| `--effect <id>` | — | Effect id or name, repeatable; rendered in catalog order |
| `--seconds <f>` | `4.0` | Clip length |
| `--fps <n>` | `25` | Frames per second |
| `--scale <n>` | `5` (sheets: `4`) | Nearest-neighbour upscale of each 32x32 frame |
| `--audio synthetic\|off` | `synthetic` | Audio feed for reactive effects |
| `--summary <path>` | — | Write a JSON summary here |

## Rendering contract

* 60 warmup frames are rendered and discarded so particle/decay effects reach a
  steady state before recording starts.
* Frame `i` is rendered at exactly `t = i * 1000 / fps` ms with
  `matrix_core::synthetic_audio(t)` (or a silent snapshot with `--audio off`),
  so runs are reproducible byte-for-byte.
* Engine state is set through the public command API:
  `light {"state":"ON","effect":"<id>","brightness":255}`.
* Frames are RGBA, upscaled with nearest-neighbour and encoded with
  `GifEncoder` + `Repeat::Infinite` at `1000/fps` ms per frame.
* Files are written to `<name>.part` and renamed on success, so an aborted run
  never leaves a truncated GIF/PNG behind.
* If an effect panics while rendering, the panic is caught per frame, recorded
  in the summary as `{"error": "..."}` and the run continues with the next
  effect (the engine is rebuilt to discard partially updated state).

## Summary format

```json
{
  "generated_by": "matrix-gif",
  "fps": 25, "seconds": 4.0, "scale": 5, "audio": "synthetic",
  "warmup_frames": 60, "total_effects": 82, "rendered": 82, "failed": 0,
  "total_bytes": 32952045,
  "effects": [
    {
      "index": 6, "id": "fire", "name": "Fire",
      "frames": 100, "fps": 25, "seconds": 4.0,
      "max_luma": 255, "mean_luma": 82.95,
      "nonblack_frames": 100, "distinct_colors_max": 404,
      "has_parameters": true, "audio_reactive": true,
      "bytes": 549742
    }
  ]
}
```

`max_luma` / `mean_luma` use Rec. 709 integer weights over all pixels of all
recorded frames; `distinct_colors_max` is the largest number of distinct RGB
triples in any single frame.

## Contact sheets

Six samples per effect at 10/30/50/70/85/95% of the clip, laid out as a 2x3
grid. The title and cell labels are drawn with the shared 5x7 bitmap font
(`matrix_effects::text::draw_char`) and scaled with nearest-neighbour blocks;
labels shrink or truncate rather than overlapping neighbouring cells.

## Notes

* `Frames` renders black without an attached LFX/LedFx source — that is its
  default state, not a rendering failure. It appears in `black_or_empty`
  checks but carries no `error` field.
* The crate only depends on local workspace crates plus `image` (gif/png),
  `clap`, `serde` and `serde_json`; it runs fully offline once built.

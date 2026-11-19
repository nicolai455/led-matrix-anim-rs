# Effekt-Galerie (GIFs & Kontaktblätter)

Alle 82 eingebauten Effekte, gerendert vom echten `matrix_core::Engine` über
[`crates/matrix-gif`](../../crates/matrix-gif/README.md). Keine Hardware, keine
Screenshots — dieselben Frames, die das Panel anzeigt.

| Artefakt | Pfad | Inhalt |
| --- | --- | --- |
| Animationen | `gifs/NNN-<id>.gif` | 100 Frames, 4 s @ 25 fps, 160×160 px, Loop unendlich |
| Kontaktblätter | `contact-sheets/NNN-<id>.png` | 2×3-Raster aus 10/30/50/70/85/95 % der Laufzeit |
| Statistik | [`summary.json`](summary.json) | Luma, Nicht-Schwarz-Frames, Farbanzahl, Bytes pro Effekt |
| Statistik Kontaktblätter | [`contact-sheets-summary.json`](contact-sheets-summary.json) | gleiche Felder für die PNGs |

## Reproduzieren

```bash
cargo run --release -p matrix-gif -- --all \
    --out docs/effects/gifs --summary docs/effects/summary.json

cargo run --release -p matrix-gif -- contact-sheets --all \
    --out docs/effects/contact-sheets --summary docs/effects/contact-sheets-summary.json
```

Beide Läufe sind deterministisch: Frame `i` wird immer bei
`t = i * 1000 / fps` ms mit `synthetic_audio(t)` gerendert, nach 60
Warmup-Frames. Ein erneuter Lauf erzeugt byte-identische Dateien.

## Bekannte Eigenheiten

* `Frames` ist ohne angeschlossene LFX-/LedFx-Quelle schwarz — das ist der
  dokumentierte Default des Effekts, kein Renderfehler (kein `error`-Feld in
  der Summary).
* Statische Effekte (`TestPattern`, `Solid`, `TextStatic`) zeigen in allen
  sechs Kontaktblatt-Zellen dasselbe Bild; das ist beabsichtigt.
* Vier GIFs liegen knapp über 1 MB (`hypno_spirals`, `rotozoom`, `plasma`,
  `audio_tunnel`); siehe `summary.json` für exakte Bytes.

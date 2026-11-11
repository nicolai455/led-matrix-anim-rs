# Audiofähige Builtins – Inventar und Verifikationsstand

**Umfang:** Statisches Quellcodeinventar von 73 audiozugriffsrelevanten Builtins unter 82 registrierten Effekten. Die Liste beschreibt verwendete Features/Controls; sie ist keine vollständige Hör- oder Signalqualitätsabnahme.

| ID | Typische Merkmale / Controls | Evidenz und Grenze |
|---|---|---|
| `arcade_invaders` | Bass/Mid/Treble/Beat; `audio_reactive` | Render-/Parameter-Tests und separate Treffer-/Formationstests |
| `audio_mesh` | Bass/Mid/Treble/Beat/Bänder | generische Render-Tests |
| `audio_pulse` | Bass/Beat/Level | generische Render-Tests |
| `audio_tunnel` | Bass/Mid/Treble/Beat/Bänder/Level | gezielte Audio-, Parameter- und Lifecycle-Tests |
| `audio_waveform` | Waveform/Bänder/Level | generische Render-Tests |
| `black_hole` | Bass/Beat/Level; `audio_enabled`, `audio_amount` | generische Render-Tests |
| `bongo_cat` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `bouncing_speakers` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `chaser_cannons` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests |
| `cyber_rain` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests |
| `cyber_tunnel` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests |
| `dancing_banana` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `dancing_skeleton` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `derby_flower` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests + held-beat regression |
| `disco_ball` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `disco_floor` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests |
| `disco_pinwheel` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `dna_spiral` | Bass/Beat/Waveform; Schalter | generische Render-Tests |
| `doge_wow` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `domain_warp` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `equalizer_city` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests |
| `fire` | Bass/Mid/Beat; Schalter | generische Render-Tests |
| `fire_tornado` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `fireworks` | Bass/Mid/Beat; Schalter | generische Render-Tests |
| `freq_bars` | Bänder; `input_mode` Mic-only/Auto fallback/Demo wave | generische Render-Tests; keine gemeinsame Reaktionsmodi-API |
| `flappy_bird` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `galaxy_spiral` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `game_of_life` | Bass/Beat; Schalter | generische Render-Tests |
| `hamster_dance` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `hazard_pulsar` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `hypno_spirals` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `julia_orbit` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `kaleidoscope` | Bass/Mid/Treble über Band-Helper; `audio_enabled` | Render-/Lifecycle-Tests |
| `laser_harp` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests |
| `laser_scanner` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `laser_show` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `lava_lamp` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests + Timingregression |
| `liquid_oil` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `lissajous` | Bass/Mid/Waveform; Schalter | generische Render-Tests |
| `mario_stomp` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `matrix_rain` | Beat/Level; Schalter | generische Render-Tests |
| `metaballs` | Bass/Mid/Treble/Beat; Schalter | gezielte Audio-/Palette-/Lifecycle-Tests |
| `mirror_cavern` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests + Timingregression |
| `moving_heads` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `neon_heart` | Bass/Mid/Treble/Beat/Waveform; Schalter | generische Render-Tests |
| `nyan_cat` | Bass/Mid/Beat; Schalter | generische Render-Tests |
| `octopus_scanner` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests + held-beat regression |
| `outrun_drive` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `pacman_chase` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `party_dancers` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `party_duck` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `party_minion` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `plasma` | Beat/Level; Schalter | generische Render-Tests |
| `polar_lights` | Bass/Mid/Beat; Schalter | generische Render-Tests |
| `police_strobe` | Bass/Mid/Treble/Beat/Level; Schalter | generische Render-Tests |
| `popcat` | Bass/Mid/Treble/Beat/Level; Schalter | generische Render-Tests |
| `pulse_ball` | Bass/Beat/Level | generische Render-Tests |
| `radial_spectrum` | Bänder/Beat/Level | generische Render-Tests |
| `retro_breakout` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `ripple` | Bass/Beat/Level; Schalter | generische Render-Tests |
| `rotating_fish` | Bass/Beat | generische Render-Tests |
| `rotozoom` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `spectrogram` | Bänder/Beat/Level | generische Render-Tests |
| `spectrum_orb` | Bass/Mid/Treble/Bänder; Schalter | generische Render-Tests |
| `spinning_vinyl` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `starfield` | Beat/Level; Schalter | generische Render-Tests; kein Beleg für separates Starship-Label |
| `strobe_blinder` | Bass/Mid/Treble/Beat/Bänder; Schalter | generische Render-Tests |
| `stroboscope` | Beat/Level; Schalter | generische Render-Tests; manueller Modus getestet |
| `supernova` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `text_effects` | Bass/Beat; Schalter | generische Render-Tests |
| `ufo_abduction` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `water_caustics` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |
| `yin_yang` | Bass/Mid/Treble/Beat; Schalter | generische Render-Tests |

## Gemeinsame Evidenz

- 73 eindeutige Builtins mit Audiozugriffen/Controls von 82 registrierten; neun sind nicht im Audioinventar.
- `freq_bars` besitzt eigene Input-/Demo-Fallback-Wahl. `black_hole` bietet `audio_enabled`/`audio_amount`. `kaleidoscope` liest Bänder indirekt über Helper.
- Meiste Effekt-Tests rendern mit einem synthetischen aktiven Snapshot. Sie belegen Grenzen/Lifecycle, nicht Stille, Pegelantwort, Kick-Selektion, stale decay oder subjektive Qualität.
- Core-PCM-Analyzer-Tests decken synthetische Bass-Kicks bei 80/120/160 BPM, Tempoänderung, mehrere Sampleraten, Stille/BPM-Ablauf und non-finite PCM ab. Das ist keine Mikrofon-Hardwareevidenz.
- `ControlHub`, Firmware und Preview teilen die Core-Analyse/Telemetry. Audioquelle (`Off`/`Synthetic`/`Mic`) bleibt getrennt von den nicht implementierten per-Effekt-Reaktionsmodi.

## Offene Abdeckung

- Keine gemeinsame per-Effekt-API/Bedienung `Auto/Beat/Bass/Spectrum/Waveform/Off` mit Fähigkeiten-Metadaten.
- Keine umfassende Effekt-für-Effekt-Signalmatrix mit Quelle aus, Stille, leisem/lauten Signal, isoliertem Kick, konstantem Bass, wechselnden Bändern, Clipping und stale input.
- Keine echte Sicht-/Hörabnahme für 73 Effekte und keine ESP32-CPU-/RAM-Laufzeitwerte.

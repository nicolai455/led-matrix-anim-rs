# Audiofähige Builtins – Inventar und Verifikationsstand

**Umfang:** Quellcodeinventar der 73 registrierten Builtins (von 82), die Audiofelder lesen oder einen `audio_reactive`/`audio_enabled`-Schalter anbieten. Die Tabelle beschreibt gelesene Snapshot-Felder und explizite Controls; indirekte Helper wie `audio_bass()` greifen auf Frequenzbänder zu und sind nicht gleichbedeutend mit direktem `audio.bands`-Zugriff. Das ist kein Hör-/Signalqualitätsnachweis. Generische Effekt-Tests rendern mit synthetischem Aktivsnapshot und prüfen vor allem endliche Ausgabe, Parametergrenzen und Lifecycle.

| Effekt-ID | Features/Parameter laut Quelle | Teststand |
|---|---|---|
| `arcade_invaders` | Bass/Mid/Treble, Beat; `audio_reactive` | generische Renderingtests; Treffer-Regression separat |
| `audio_mesh` | Bass/Mid/Treble, Beat, Bands; Schalter | generische Renderingtests |
| `audio_pulse` | Bass/Beat/Level | generische Renderingtests |
| `audio_tunnel` | Bass/Mid/Treble, Beat, Bands/Level; Schalter | spezifische Audio-/Parameter-/Lifecycletests |
| `audio_waveform` | Waveform/Bands/Level | generische Renderingtests |
| `black_hole` | Bass/Beat/Level; Parameter `audio_enabled` und `audio_amount` | generische Renderingtests |
| `bongo_cat` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `bouncing_speakers` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `chaser_cannons` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Renderingtests |
| `cyber_rain` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Renderingtests |
| `cyber_tunnel` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Renderingtests |
| `dancing_banana` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `dancing_skeleton` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `derby_flower` | Bass/Mid/Treble/Beat; Schalter | generische Rendering- und gehaltene-Beat-Regression |
| `disco_ball` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `disco_floor` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Renderingtests |
| `disco_pinwheel` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `dna_spiral` | Bass/Beat/Waveform; Schalter | generische Renderingtests |
| `doge_wow` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `domain_warp` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `equalizer_city` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Renderingtests |
| `fire` | Bass/Mid/Beat; Schalter | generische Renderingtests |
| `fire_tornado` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `fireworks` | Bass/Mid/Beat; Schalter | generische Renderingtests |
| `freq_bars` | Frequenzbänder; Verfügbarkeits-/Input-Modus (`Mic only`, `Auto fallback`, `Demo wave`) | generische Renderingtests |
| `flappy_bird` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `galaxy_spiral` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `game_of_life` | Bass/Beat; Schalter | generische Renderingtests |
| `hamster_dance` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `hazard_pulsar` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `hypno_spirals` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `julia_orbit` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `kaleidoscope` | Bass/Mid/Treble über bandmittelnde Helper; `audio_enabled`; kein direkter Zugriff auf `bands` | Render-/Lifecycletests |
| `laser_harp` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Renderingtests |
| `laser_scanner` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `laser_show` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `lava_lamp` | Bass/Mid/Treble/Beat; Schalter | generische Rendering- und Timingregression |
| `liquid_oil` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `lissajous` | Bass/Mid/Waveform; Schalter | generische Renderingtests |
| `mario_stomp` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `matrix_rain` | Beat/Level; Schalter | generische Renderingtests |
| `metaballs` | Bass/Mid/Treble/Beat; Schalter | Audio-/Palette-/Lifecycletests |
| `mirror_cavern` | Bass/Mid/Treble/Beat; Schalter | generische Rendering- und Timingregression |
| `moving_heads` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `neon_heart` | Bass/Mid/Treble/Beat/Waveform; Schalter | generische Renderingtests |
| `nyan_cat` | Bass/Mid/Beat; Schalter | generische Renderingtests |
| `octopus_scanner` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Rendering- und gehaltene-Beat-Regression |
| `outrun_drive` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `pacman_chase` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `party_dancers` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `party_duck` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `party_minion` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `plasma` | Beat/Level; Schalter | generische Renderingtests |
| `polar_lights` | Bass/Mid/Beat; Schalter | generische Renderingtests |
| `police_strobe` | Bass/Mid/Treble/Beat/Level; Schalter | generische Renderingtests |
| `popcat` | Bass/Mid/Treble/Beat/Level; Schalter | generische Renderingtests |
| `pulse_ball` | Bass/Beat/Level | generische Renderingtests |
| `radial_spectrum` | Bands/Beat/Level | generische Renderingtests |
| `retro_breakout` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `ripple` | Bass/Beat/Level; Schalter | generische Renderingtests |
| `rotating_fish` | Bass/Beat | generische Renderingtests |
| `rotozoom` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `spectrogram` | Bands/Beat/Level | generische Renderingtests |
| `spectrum_orb` | Bass/Mid/Treble/Bands; Schalter | generische Renderingtests |
| `spinning_vinyl` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `starfield` | Beat/Level; Schalter | generische Renderingtests |
| `strobe_blinder` | Bass/Mid/Treble/Beat/Bands; Schalter | generische Renderingtests |
| `stroboscope` | Beat/Level; Schalter | generische Renderingtests; manueller Modus getestet |
| `supernova` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `text_effects` | Bass/Beat; Schalter | generische Renderingtests |
| `ufo_abduction` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `water_caustics` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |
| `yin_yang` | Bass/Mid/Treble/Beat; Schalter | generische Renderingtests |

## Abdeckungs- und Inventarhinweise

- Die Tabelle enthält 73 von 82 registrierten Builtins, die nach Prüfung der Quellcodezugriffe Audiofelder verwenden oder Audio-Controls anbieten. `freq_bars` nutzt Frequenzbänder und bietet eigene Eingangsmodi (`Mic only`, `Auto fallback`, `Demo wave`); es ist kein einfacher `audio_enabled`-Toggle.
- `black_hole` stellt die Parameter `audio_enabled` und `audio_amount` bereit.
- `kaleidoscope` berechnet Bass/Mid/Treble über bandmittelnde Helper; es liest `context.audio.bands` nicht direkt. Die Tabellenangabe beschreibt darum Helper-Abhängigkeit, keinen direkten Bands-Zugriff.
- Die übrigen neun registrierten Effekte ohne passenden Snapshot-/Helper-Zugriff gehören nicht in dieses Audio-Inventar.
- Keiner der Builtins liest derzeit direkt die neuen `AudioSnapshot`-Felder `bass`, `onset` oder `bpm`; sie werden bislang für gemeinsame Control-/Preview-Telemetrie bereitgestellt.

## Regressions- und Abdeckungsgrenzen

- Core-PCM-Tests erzeugen reproduzierbare Kicks bei 80/120/160 BPM; sie prüfen mehrere Sample-Raten, Tempoänderung, Stille/BPM-Ablauf und nicht-finite Samples. Firmware-Mikrofonpfad ruft denselben Core-Prozessor mit der tatsächlichen Abtastrate auf.
- `ControlHub` veröffentlicht `bass`, `onset`, `bpm`, `mid` und `treble` aus demselben `AudioSnapshot`; Firmware-Web-API und WASM-Preview übernehmen Snapshot-Werte statt eine zweite Analyse zu berechnen.
- Getestet ist nicht: pro-Effekt Audio Off, Mic-Ausfall, Stille, variable Pegel, Kick gegen konstanten Bass, Bandwechsel, Clipping, Stale-Fade, Hardware-Signalqualität und CPU/RAM auf ESP32. Der statische Quellcodehinweis ist keine Garantie, dass alle Effekte auf diese Modi angemessen reagieren. Keine Builtin-Zeile ist damit „audioqualitäts-freigegeben“.
- Eine einheitliche, benutzerauswählbare Reaktionsmodi-API (Auto/Beat/Bass/Spektrum/Waveform/Aus) mit vollständigen Metadaten pro Effekt ist nicht umgesetzt.
- Hardware-Sichtprüfung und ESP32-CPU-/RAM-Messung fehlen.

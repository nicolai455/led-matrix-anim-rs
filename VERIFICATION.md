# Verifikationsnotizen: Netzwerk, Audio und Playlist

## Offline-Prüfungen dieser Runde

- `cargo test --workspace`: bestanden (22 Testgruppen, 117 Tests).
- `cargo test --manifest-path firmware/Cargo.toml --offline`: bestanden: 37 Firmware-Unit-Tests und sämtliche Firmware-Host-Integrationstests (insgesamt 84 Testfälle über alle Firmware-Testtargets). Die Compile-Ausgabe enthält vorhandene `Atomic::fetch_update`-Deprecation-Warnungen.
- `cargo test --manifest-path tests/Cargo.toml --offline`: bestanden: 8 Tests.
- `python3 -m unittest discover -s scripts -p 'test_*.py'`: 12 bestanden.
- `node --test scripts/test-config-ui.cjs scripts/test-preview-ui.cjs`: 30 bestanden.
- `./firmware/build_firmware.sh esp32s3 release`: bestanden auf dem aktuellen Quellstand; App-Image 2.747.440 Byte, OTA-Slot 4.194.304 Byte, Reserve 1.446.864 Byte (65,5 % belegt). Ein vorheriger Wiederholungsversuch scheiterte an der Laufzeit-Suche nach `libpython3.14.so.1.0`; erneut ausgeführt mit `LD_LIBRARY_PATH=/usr/lib` erfolgreich. Bestehende Warnung: ungenutzte Methode `ConnectingTimer::elapsed_secs`.
- `cargo clippy --workspace --all-targets -- -D warnings`: bestanden nach gezielten Lintkorrekturen im betroffenen Effektcode/Testfixtures und lokalem Typalias.
- `cargo clippy --manifest-path tests/Cargo.toml --offline --all-targets -- -D warnings`: bestanden.
- Firmware-Clippy (`cargo clippy --manifest-path firmware/Cargo.toml --offline --all-targets -- -D warnings`) fails under both current stable Rust 1.99 and pinned ESP nightly Rust 1.97.0. Stable reports `Atomic::fetch_update` deprecations in `firmware/src/output.rs:176,213` plus fixture `field_reassign_with_default` in `firmware/src/mic.rs` and `firmware/src/net/wifi.rs`; the pinned toolchain does not report those deprecations but still finds fixture-style lints in those files and `tests/recovery.rs:41,53,65,79`. This is test/code-quality debt; firmware host tests pass.
- Firmware-Hosttests nach CSRF-Härtung: 37 Unit-Tests (darin zwei Request-Guard-Tests) und alle Firmware-Host-Integrationstests bestanden.
- UI nach Aufnahme des CSRF-Kontrollheaders: Node-Konfigurations-/Preview-Suite 30/30 bestanden; `scripts/test-preview-browser.cjs` sendet ebenfalls den Header.
- `cargo fmt --all -- --check` und Firmware-Rustfmt sind nicht sauber: Rustfmt-Output umfasst 89 einzigartige Workspace-Dateien und 16 Firmware-Dateien. Kein globaler Formatierungslauf ausgeführt. Das ausgeschlossene Integrationstestpaket (`cargo fmt --manifest-path tests/Cargo.toml -- --check`) ist sauber.
- `tests/system_test.py` gegen einen isolierten Loopback-Mosquitto-Broker bestand nach dem gezielten MQTT-Fix zweimal hintereinander. Die Läufe deckten 82 Effekte, 365 retained Discovery-Entitäten, HTTP 413 bei zu großem Body, HTTP-Reaktionsfähigkeit während Broker-Ausfall (max. rund 33 ms), MQTT-Reconnect und Home-Assistant-Onlinezustand ab.

### MQTT-Ursache und Korrektur

Root Cause: Der MQTT-Poller leitete alle `rumqttc::Event`s einschließlich Outgoing-, SubAck- und PubAck-Events in denselben begrenzten Kanal wie eingehende Befehle. Die retained Discovery-Ausgabe füllte den Kanal, sodass der Poller beim Weiterleiten blockierte und nachfolgende MQTT-Publishes nicht mehr verarbeitete. `crates/matrix-control/src/mqtt.rs` leitet nun nur eingehende ConnAck-, Publish- und Disconnect-Events weiter. `tests/system_test.py` wartet zusätzlich auf die retained `online`-Nachricht als Subscription-Startbarriere. Nachweis ist Simulator/Broker, kein ESP.

## Offline-Befunde und offene Arbeit

- Firmware-CSRF-Härtung: Der ESP-HTTP-Handler prüft jetzt `X-Matrix-Control: 1` und bei Browsern `Origin == http://Host` für generische `POST /api/*`-Routen sowie `POST /update`. Die Browser-UI und der Preview-Smoke senden den Marker. Das ist CSRF-Schutz, keine Authentifizierung: Ein anderer Client im LAN kann den Marker selbst setzen; OTA bleibt daher gegen LAN-Angreifer unauthentifiziert und ist vor netzwerkweiter Exposition abzusichern.

- `firmware/src/net/wifi.rs` erhält Runtime-Profile in der stabil nach Priorität sortierten Reihenfolge aus `NetworkConfig`; weder dieser Adapter noch der WifiManager sortieren nach RSSI. `client_configuration` setzt jeweils ausdrücklich die SSID. ESP-IDF `esp-idf-svc 0.51` bildet das in `wifi_sta_config_t.ssid` ab und nutzt bei der normalen Verbindung eine gezielte SSID-Suche. Hidden-Metadatum bleibt erhalten und ist kein Scanfilter. Hosttests prüfen sortierte Reihenfolge und Hidden-Metadaten. Das ist Code-/Hostevidenz, kein Funk-, Assoziations- oder DHCP-Test.
- `AUDIO_EFFECT_REVIEW.md` inventarisiert 73 eindeutige audiozugriffsrelevante Builtins unter 82. Es ist keine 73-fache Signalqualitätsfreigabe. Gemeinsame auswählbare per-Effekt-Modi `Auto/Beat/Bass/Spectrum/Waveform/Off`, Fähigkeiten-Metadaten und die vollständige Effekt-Signalmatrix sind nicht implementiert.
- `Starship` ist im Issue-Verlauf als Space-Invaders-/`arcade_invaders`-Story identifiziert. Es wurde kein eigenständiger Starship-ID-/Label-Defekt reproduziert; die Behauptung einer eigenen Starship-Reparatur wäre nicht belegt.
- Sichere Erstbereitstellung bleibt offen: ohne out-of-band provisioniertes, gerätespezifisches WPA2-AP-Passwort gibt es auf einem fabrikneuen Gerät keinen Setup-AP. Kein universelles Passwort und keine ungeschützte Bootstrap-Route ergänzt.
- Der ESP32 ist laut Nutzer aktuell unerreichbar. Kein Flash/OTA, keine physische Panel-/Lane-Zuordnung, kein WLAN-/DHCP-/Mikrofon-/Hör- oder Geräte-Neustartpersistenztest. Der S3-Crossbuild ist kein Gerätenachweis; `disabled_panels` bleibt standardmäßig aus.

Issue #12 bleibt offen. Weitere geräteabhängige Akzeptanz ist blockiert, bis Gerät und Versorgung sicher zugänglich sind; die Audio-Modusanforderung und sichere Erstbereitstellung erfordern separate Implementierung/Entscheidung.

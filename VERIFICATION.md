# Verifikationsnotizen: Netzwerk, Audio und Playlist

## Offline-Prüfungen dieser Runde

- `cargo test --workspace --offline`: bestanden nach Fixes; Workspace-Suite umfasst Core-, Control-, Effects-, Preview-Tests.
- `node --test scripts/test-config-ui.cjs`: 21/21 bestanden (inkl. Formularsemantik der Passwortfelder).
- `RUSTUP_TOOLCHAIN=esp cargo test --manifest-path firmware/Cargo.toml --offline --all-targets`: bestanden; 41 Firmware-Unit-Tests sowie alle Host-Integrationstesttargets.
- `cargo test --manifest-path tests/Cargo.toml --offline`: bestanden.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: bestanden.
- `RUSTUP_TOOLCHAIN=esp cargo clippy --manifest-path firmware/Cargo.toml --offline --all-targets -- -D warnings`: bestanden.
- `git diff --check`: bestanden nach Bereinigung der letzten Format-Whitespace-Abweichung.
- Die genannten Läufe sind Offline-/Hosttests. Partitionkapazität, Geräteeinrichtung, WLAN/DHCP, echte Audio-/LED-Ausgabe und OTA-Sicherheit sind dadurch nicht abgenommen.
- `cargo test --manifest-path firmware/Cargo.toml --offline`: bestanden: 37 Firmware-Unit-Tests und sämtliche Firmware-Host-Integrationstests (insgesamt 84 Testfälle über alle Firmware-Testtargets). Die Compile-Ausgabe enthält vorhandene `Atomic::fetch_update`-Deprecation-Warnungen.
- `cargo test --manifest-path tests/Cargo.toml --offline`: bestanden: 8 Tests.
- `python3 -m unittest discover -s scripts -p 'test_*.py'`: 12 bestanden.
- `node --test scripts/test-config-ui.cjs scripts/test-preview-ui.cjs`: 30 bestanden.
- `./firmware/build_firmware.sh esp32s3 release`: bestanden auf dem aktuellen Quellstand; App-Image 2.747.440 Byte, OTA-Slot 4.194.304 Byte, Reserve 1.446.864 Byte (65,5 % belegt). Ein vorheriger Wiederholungsversuch scheiterte an der Laufzeit-Suche nach `libpython3.14.so.1.0`; erneut ausgeführt mit `LD_LIBRARY_PATH=/usr/lib` erfolgreich. Bestehende Warnung: ungenutzte Methode `ConnectingTimer::elapsed_secs`.
- `cargo clippy --workspace --all-targets -- -D warnings`: bestanden nach gezielten Lintkorrekturen im betroffenen Effektcode/Testfixtures und lokalem Typalias.
- `cargo clippy --manifest-path tests/Cargo.toml --offline --all-targets -- -D warnings`: bestanden.
- `RUSTUP_TOOLCHAIN=esp cargo test --manifest-path firmware/Cargo.toml --offline --all-targets`: bestanden: 84 Tests über alle Firmware-Testtargets.
- `RUSTUP_TOOLCHAIN=esp cargo clippy --manifest-path firmware/Cargo.toml --offline --all-targets -- -D warnings`: bestanden nach gezielten Clippy-Fixturekorrekturen; Verhalten/Assertions unverändert.
- Ungepinntes aktuelles stabiles Rust/Clippy 1.99 meldet zusätzlich die zwei `Atomic::fetch_update`-Deprecations in `firmware/src/output.rs:176,213`; dies ist Toolchain-abhängig. Firmware-Hosttests mit der gepinnten Toolchain bestehen weiterhin.
- Firmware-Hosttests nach CSRF-Härtung: 37 Unit-Tests (darin zwei Request-Guard-Tests) und alle Firmware-Host-Integrationstests bestanden.
- UI nach Aufnahme des CSRF-Kontrollheaders: Node-Konfigurations-/Preview-Suite 30/30 bestanden; `scripts/test-preview-browser.cjs` sendet ebenfalls den Header.
- `cargo fmt --all -- --check` und Firmware-Rustfmt sind nicht sauber: Rustfmt-Output umfasst 89 einzigartige Workspace-Dateien und 16 Firmware-Dateien. Kein globaler Formatierungslauf ausgeführt. Das ausgeschlossene Integrationstestpaket (`cargo fmt --manifest-path tests/Cargo.toml -- --check`) ist sauber.
- `tests/system_test.py` gegen einen isolierten Loopback-Mosquitto-Broker bestand nach dem gezielten MQTT-Fix zweimal hintereinander. Die Läufe deckten 82 Effekte, 365 retained Discovery-Entitäten, HTTP 413 bei zu großem Body, HTTP-Reaktionsfähigkeit während Broker-Ausfall (max. rund 33 ms), MQTT-Reconnect und Home-Assistant-Onlinezustand ab.

### MQTT-Ursache und Korrektur

Root Cause: Der MQTT-Poller leitete alle `rumqttc::Event`s einschließlich Outgoing-, SubAck- und PubAck-Events in denselben begrenzten Kanal wie eingehende Befehle. Die retained Discovery-Ausgabe füllte den Kanal, sodass der Poller beim Weiterleiten blockierte und nachfolgende MQTT-Publishes nicht mehr verarbeitete. `crates/matrix-control/src/mqtt.rs` leitet nun nur eingehende ConnAck-, Publish- und Disconnect-Events weiter. `tests/system_test.py` wartet zusätzlich auf die retained `online`-Nachricht als Subscription-Startbarriere. Nachweis ist Simulator/Broker, kein ESP.

## Browser-Konsole – echte Gerätesitzung (2026-11-10)
- Ausgewertet: `/home/joe/.hermes/cache/scratch/esp-browser-console.log` plus gespeicherte S3 Device-Info-/State-Snapshots. Kein neues Gerät angefragt.
- Firmware-Logquelle/Serial-Bootlogs liegen im zugänglichen Arbeitsbereich nicht vor; deshalb keine Aussage über sämtliche Boot-/ESP-IDF-Laufzeitlogs. Gespeicherte Firmware-Metadaten zeigen einen gültigen App-Slot, die State-Snapshots zeigen LEDs/Audio aus. Das sind Momentaufnahmen, keine vollständige Fehlerfreiheit.
- Eine anwendungsbezogene Browserwarnung wurde gefunden und offline behoben: beide WLAN-Passwortfelder waren ursprünglich nicht in einem eigenen Formular. Chromium meldete zweimal „Password field is not contained in a form“. Beide Felder haben jetzt jeweils ein echtes Formular plus sichtbares assoziiertes Label/`aria-label`; ein Regressionstest sichert das. `node --test scripts/test-config-ui.cjs` besteht 21/21. Es gab keinen Live-Browser-Neulauf auf dem ESP.
- Übrige Einträge im Console-Dump beziehen sich auf Chromium-Sandbox/GPU, fehlende Browser-OAuth-Konfiguration, Erweiterungsmetadaten, Desktop-Portal und Browserprofil-SQLite-Flush beim Shutdown; keine Firmware-/Web-UI-JavaScript-Ausnahme erkennbar. Nicht als ESP32-Fehler bewertet.
- Vorhandene Firmware-Testlogs der ungepinnten Rust-1.99-Toolchain enthalten zwei `Atomic::fetch_update`-Deprecations; sie wurden nicht automatisch ersetzt, da der Firmware-Build pinned ESP nightly 1.97 nutzt und keine höhere MSRV unterstellt werden soll. Der Crossbuild warnte zusätzlich vor ungenutztem `ConnectingTimer::elapsed_secs`; die Methode wurde entfernt, da sie keine Aufrufer hatte. Gepinnter ESP-Clippy bleibt sauber.
- Offen: vollständige serielle ESP-IDF-Boot-/Runtime-Logs prüfen, sobald Zugriff ausdrücklich freigegeben und die Logs verfügbar sind. Keine Hardwareaktion oder ESP-GET im Rahmen dieser Prüfung ausgeführt.


- Firmware-Control-Gate: Mutierende HTTP-Routen und `POST /update` erfordern jetzt zusätzlich zur bestehenden CSRF-Prüfung eine TCP-Quell-IPv4 im konfigurierten Fallback-AP-Bereich `192.168.4.2–192.168.4.254`. Die ESP-IDF-HTTP-API stellt hier die Peer-Adresse, aber keinen geprüften ingress-Netif-Nachweis bereit; gleiche/überlappende STA-Subnetze, alternative Routingpfade und Geräteverhalten wurden nicht am ESP getestet. Das ist deshalb nur eine zusätzliche Netzfilterung, keine Authentifizierung oder belegte AP-only-Grenze. Sicherheitsblocker und Merge-Sperre bleiben bis unabhängiger Bewertung und belastbarem Abnahmetest bestehen.
- `firmware/src/net/wifi.rs` erhält Runtime-Profile in der stabil nach Priorität sortierten Reihenfolge aus `NetworkConfig`; weder dieser Adapter noch der WifiManager sortieren nach RSSI. `client_configuration` setzt jeweils ausdrücklich die SSID. ESP-IDF `esp-idf-svc 0.51` bildet das in `wifi_sta_config_t.ssid` ab und nutzt bei der normalen Verbindung eine gezielte SSID-Suche. Hidden-Metadatum bleibt erhalten und ist kein Scanfilter. Hosttests prüfen sortierte Reihenfolge und Hidden-Metadaten. Das ist Code-/Hostevidenz, kein Funk-, Assoziations- oder DHCP-Test.
- `AUDIO_EFFECT_REVIEW.md` inventarisiert 73 eindeutige audiozugriffsrelevante Builtins unter 82. Der Sound-Default enthält jetzt alle 73 inventarisierten IDs mit jeweils 10 Sekunden und Loop; das ist keine 73-fache Signalqualitätsfreigabe. Gemeinsame auswählbare per-Effekt-Modi `Auto/Beat/Bass/Spectrum/Waveform/Off`, Fähigkeiten-Metadaten und die vollständige Effekt-Signalmatrix sind nicht implementiert.
- `Starship` ist im Issue-Verlauf als Space-Invaders-/`arcade_invaders`-Story identifiziert. Es wurde kein eigenständiger Starship-ID-/Label-Defekt reproduziert; die Behauptung einer eigenen Starship-Reparatur wäre nicht belegt.
- Sichere Erstbereitstellung bleibt offen: ohne out-of-band provisioniertes, gerätespezifisches WPA2-AP-Passwort gibt es auf einem fabrikneuen Gerät keinen Setup-AP. Kein universelles Passwort und keine ungeschützte Bootstrap-Route ergänzt.
- Der ESP32 ist laut Nutzer aktuell unerreichbar. Kein Flash/OTA, keine physische Panel-/Lane-Zuordnung, kein WLAN-/DHCP-/Mikrofon-/Hör- oder Geräte-Neustartpersistenztest. Der S3-Crossbuild ist kein Gerätenachweis; `disabled_panels` bleibt standardmäßig aus.

Issue #12 bleibt offen. Weitere geräteabhängige Akzeptanz ist blockiert, bis Gerät und Versorgung sicher zugänglich sind; die Audio-Modusanforderung und sichere Erstbereitstellung erfordern separate Implementierung/Entscheidung.

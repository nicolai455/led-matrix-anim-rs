# Firmware network configuration

Network configuration is stored in NVS and uses canonical `wifiProfiles` JSON (up to five prioritized profiles); legacy `wifiSsid`/`wifiPassword` entries migrate to one profile. MQTT fields are retained. Wi-Fi credentials are accepted only from private per-device provisioning and are never returned by `/api/network` GET, which exposes configured flags instead.

The existing port-80 firmware HTTP server serves `/api/network` GET/POST. POST uses the existing same-origin control guard, caps the body at 4096 bytes, validates the candidate before atomically storing it, returns only redacted data, and schedules the existing controlled restart only after its response was sent. A new network configuration uses the requested AP recovery address `192.168.4.1`.

A fallback WPA2 AP starts only when an explicit per-device AP passphrase is configured. There is no universal password. Consequently a factory-fresh device without private provisioning has no setup AP; no unauthenticated bootstrap route is provided. This tradeoff is intentional and must be reflected in manufacturing/provisioning procedures. The web UI has no separate login; the AP passphrase is its chosen local access boundary.

Host firmware tests verify validation, NVS round-trips, redaction, profile order and fallback policy. They do not prove ESP-IDF cross-build success, actual station scanning/reconnect, HTTP behavior on the device, DHCP, or radio behavior. Do not treat host results as hardware verification.

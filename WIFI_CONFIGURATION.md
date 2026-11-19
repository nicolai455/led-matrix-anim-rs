# Network and Wi-Fi provisioning

Firmware configuration is stored in NVS and uses canonical `wifiProfiles` JSON (up to five prioritized profiles); legacy `wifiSsid`/`wifiPassword` migrate to one profile. MQTT fields remain in the same config. `/api/network` GET returns SSIDs/metadata and credential-configured flags only, never Wi-Fi, AP or MQTT passphrases. POST validates before persisting atomically and triggers a controlled restart only after returning a redacted response.

## Reaching the configuration from the home network

The previous behaviour accepted management writes **only** from the fallback AP
subnet `192.168.4.2–254`. Whoever joined the home network through a saved
profile could open the UI, but every write was rejected with `403`, which is why
only `JDJLIGHT` appeared to work.

The ingress is now classified in `firmware/src/request_guard.rs`:

| Scope | Peer | Write allowed |
|---|---|---|
| `FallbackAp` | inside `192.168.4.0/24`, `.2–.254` | yes, no token (bootstrap path) |
| `LocalSubnet` | inside the station subnet the device is currently attached to | yes, **with** the per-device setup token |
| `Untrusted` | anything else | no |

`LocalSubnet` requires the setup token (`X-Matrix-Setup-Token` header). It is
generated per device at first boot, stored in NVS, never written to `state.json`
and never logged. `/api/firmware` discloses it **only** to callers inside the
station subnet, so the UI can show it in the Wi-Fi card and keep it in that tab.

This is not authentication: anyone on the same subnet who reads the token once
can reuse it. It stops a random web page from reconfiguring the device, and the
CSRF guard (`X-Matrix-Control: 1` plus same-origin `Origin`/`Host`) still applies
to every scope. The AP passphrase remains the only real credential on the
bootstrap path, and there is no HTTP login.

## Validation rules (firmware and host adapter agree)

- 1–5 profiles, 1–32 byte SSID, no duplicate SSIDs.
- Passphrase 8–64 bytes, or **empty for an open network** (the UI offers an
  explicit "Open network" checkbox; an empty password field alone means "keep
  the stored secret").
- Per-profile priority 0–255; the list is kept sorted descending.
- `fallbackApPassword` 8–64 bytes. Without one the fallback AP does not start —
  that is the honest trade: no shared default credential, but also no recovery
  access point until it is provisioned. The UI warns before the AP is disabled.

The fallback AP requests gateway `192.168.4.1/24` with DHCP and is WPA2. There is
no universal password and no zero-touch factory bootstrap: an unprovisioned
device has no setup AP until private provisioning supplies a per-device
passphrase. Manufacturing/owner procedures must provision it out of band. The web
UI has no separate login; do not mistake AP WPA2 for HTTP authentication of
requests received through other interfaces.

## Reconnect policy

`firmware/src/net/wifi.rs` walks the profiles in priority order with an 18 s
timeout each and falls back to `Mixed` (AP + STA) so setup access survives. The
decision logic lives in `firmware/src/net/connect_policy.rs`:

- After `set_configuration` the driver gets 500 ms before `connect()`; without
  that pause the next profile was started before the channel switch finished.
- A failing profile backs off exponentially (18 s, 36 s, 72 s, then 5 min) and
  is marked `failed` after three attempts, visible per profile in `/api/state`.
- A profile with higher priority is re-acquired when the scan cache shows it in
  range again instead of staying on a lower-priority link forever.
- If every profile fails, `Mixed` stays up rather than dropping the AP.

## Scanning

`GET /api/network/scan` never blocks the HTTP thread: the Wi-Fi worker performs
the scan and the handler reads a timestamped cache, returning `stale: true` with
the previous result while a scan is in flight. At most 20 networks, SSIDs
truncated to 32 bytes, no BSSIDs. A scan briefly interrupts the station link, so
the response carries `connectionMayDrop`.

## Host adapter (no radio)

The desktop simulator serves `/api/network` from an in-memory store
(`crates/matrix-control/src/network_host.rs`) with the same response shape and
validation rules, so the whole card is testable without hardware. Every response
sets `networkAvailable: false` and carries a notice; `/api/network/scan` answers
an empty list instead of pretending to have a radio.

The UI keeps this configuration in its own variable rather than in the snapshot
state, because that state is replaced on every `/api/state` poll — storing it
there lost the profile list about a second after loading, which is why saving a
Wi-Fi profile appeared to do nothing.

## What is verified and what is not

Host-side: parsing, validation, ordering, redaction, NVS round-trip, the ingress
scope matrix, the setup-token disclosure rule, the backoff/profile-selection
policy, the host adapter, and the full UI flow through the browser E2E suite.

**Not verified: anything that needs a radio.** No ESP was connected while this
was written. ESP-IDF compilation is checked, but actual scanning, reconnect
timing, DHCP behaviour and AP/STA coexistence are unproven. Treat the reconnect
policy as reviewed code, not as measured behaviour.

Keep the device off untrusted networks.

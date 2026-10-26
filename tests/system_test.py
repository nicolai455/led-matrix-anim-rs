#!/usr/bin/env python3
"""Exercise the Rust simulator against a real local Mosquitto broker.

Run after `cargo build -p matrix-control --bin matrix-simulator`:
  python3 tests/system_test.py --mosquitto-dir /path/to/mosquitto/bin
No user credentials, existing configuration, or attached hardware are touched.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def eventually(fn, timeout=15):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = fn()
            if value:
                return value
        except (OSError, ValueError, AssertionError, urllib.error.URLError) as exc:
            last = exc
        time.sleep(0.05)
    raise AssertionError(f"condition not met within {timeout}s: {last}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/matrix-simulator")
    parser.add_argument("--mosquitto-dir", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    tools = {}
    for name in ("mosquitto", "mosquitto_pub", "mosquitto_sub"):
        tools[name] = str(args.mosquitto_dir / name) if args.mosquitto_dir else shutil.which(name)
        if not tools[name] or not Path(tools[name]).exists():
            parser.error(f"{name} not found; provide --mosquitto-dir")
    if not binary.exists():
        parser.error(f"simulator binary missing: {binary}")
    broker_port, http_port = free_port(), free_port()
    device = "rust-system-test"
    base_topic = f"ledmatrix/{device}"
    http = f"http://127.0.0.1:{http_port}"
    processes = []
    report = {}
    with tempfile.TemporaryDirectory(prefix="matrix-system-", dir=os.environ.get("TMPDIR")) as directory:
        directory = Path(directory)
        config = directory / "mosquitto.conf"
        config.write_text(f"listener {broker_port} 127.0.0.1\nallow_anonymous true\npersistence false\n")
        network = directory / "network.json"
        network.write_text(json.dumps({"mqttHost": "127.0.0.1", "mqttPort": broker_port}))
        logs = {}
        def launch(name, command):
            log = open(directory / f"{name}.log", "w+")
            logs[name] = log
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
            processes.append(process)
            return process
        def broker():
            return launch("broker", [tools["mosquitto"], "-c", str(config)])
        def mqtt(name, *flags):
            result = subprocess.run([tools[name], "-h", "127.0.0.1", "-p", str(broker_port), *flags], text=True, capture_output=True, timeout=15)
            # mosquitto_sub exits 27 when its -W receive window expires, even
            # after it has successfully returned all retained discovery records.
            if result.returncode != 0 and not (result.returncode == 27 and result.stdout):
                raise OSError(f"{name} failed ({result.returncode}): {result.stderr}")
            return result.stdout
        def read(path):
            with urllib.request.urlopen(http + path, timeout=2) as response:
                return json.load(response)
        def command(kind, payload):
            request = urllib.request.Request(http + "/api/command", json.dumps({"kind": kind, "payload": payload}).encode(), {"Content-Type": "application/json"}, method="POST")
            with urllib.request.urlopen(request, timeout=2) as response:
                assert 200 <= response.status < 300
                return json.load(response)
        try:
            broker_process = broker()
            eventually(lambda: socket.create_connection(("127.0.0.1", broker_port), timeout=0.2).close() is None)
            simulator = launch("simulator", [str(binary), "--listen", f"127.0.0.1:{http_port}", "--device-id", device, "--network", str(network)])
            state = eventually(lambda: read("/api/state"))
            assert state["state"] == "OFF", state
            oversized = urllib.request.Request(http + "/api/command", b"x" * 4097, {"Content-Type":"application/json"}, method="POST")
            try:
                urllib.request.urlopen(oversized,timeout=2)
                raise AssertionError("oversized HTTP body accepted")
            except urllib.error.HTTPError as error:
                assert error.code == 413, error.code
            assert read("/api/state")["state"] == "OFF"
            report["oversized_http_body"] = "rejected with 413; server remains responsive"
            catalog = read("/api/effects")
            if isinstance(catalog, dict):
                catalog = catalog["effects"]
            expected = json.loads((ROOT / "tests/legacy_catalog.json").read_text())
            actual = {effect["id"]: effect for effect in catalog}
            assert len(actual) == len(catalog), "duplicate effect IDs"
            for effect in expected:
                assert effect["id"] in actual, f"missing {effect['id']}"
                entry = actual[effect["id"]]
                assert entry["name"] == effect["name"], (entry, effect)
                params = {param["id"]: param for param in entry.get("parameters", entry.get("params", []))}
                assert set(params) == {param["id"] for param in effect["parameters"]}, (effect["id"], params)
                for param in effect["parameters"]:
                    for field in ("kind", "min", "max", "step", "default"):
                        assert params[param["id"]][field] == param[field], (effect["id"], param["id"], field, params[param["id"]][field], param[field])
            report["catalog_effects"] = len(catalog)
            report["legacy_parameters"] = sum(len(effect["parameters"]) for effect in expected)
            command("light", {"state": "ON", "brightness": 97, "effect": "Rainbow"})
            eventually(lambda: (s := read("/api/state"))["state"] == "ON" and s["brightness"] == 97)
            mqtt("mosquitto_pub", "-t", base_topic + "/light/set", "-m", '{"brightness":123,"effect":"Fire"}')
            eventually(lambda: (s := read("/api/state"))["brightness"] == 123 and s["effect"] == "Fire")
            mqtt("mosquitto_pub", "-t", base_topic + "/effect/fire/cooling/set", "-m", "77")
            eventually(lambda: read("/api/state")["effects"]["fire"]["cooling"] == 77)
            discovery_text = mqtt("mosquitto_sub", "-t", f"homeassistant/+/{device}/+/config", "-W", "3", "--retained-only", "-v")
            records = [line.split(" ", 1) for line in discovery_text.splitlines() if line]
            discovery = {topic: json.loads(payload) for topic, payload in records}
            expected_minimum = report["legacy_parameters"] + 7
            assert len(discovery) >= expected_minimum, (len(discovery), expected_minimum)
            assert all(value["availability_topic"] == base_topic + "/availability" for value in discovery.values())
            assert len({value["unique_id"] for value in discovery.values()}) == len(discovery)
            report["retained_discovery_entities"] = len(discovery)
            # Offline broker must not stall rendering or HTTP commands.
            broker_process.terminate()
            broker_process.wait(timeout=5)
            latencies = []
            for brightness in range(30, 50):
                before = time.monotonic()
                command("light", {"brightness": brightness})
                read("/api/state")
                latencies.append(time.monotonic() - before)
            assert max(latencies) < 1, latencies
            eventually(lambda: read("/api/state")["brightness"] == 49)
            report["broker_offline_max_http_roundtrip_ms"] = round(max(latencies) * 1000, 2)
            broker_process = broker()
            eventually(lambda: socket.create_connection(("127.0.0.1", broker_port), timeout=0.2).close() is None)
            retained = eventually(lambda: mqtt("mosquitto_sub", "-t", base_topic + "/state", "-C", "1", "-W", "3"), timeout=20)
            assert json.loads(retained)["brightness"] == 49
            mqtt("mosquitto_pub", "-t", "homeassistant/status", "-m", "online")
            online = mqtt("mosquitto_sub", "-t", base_topic + "/availability", "-C", "1", "-W", "5")
            assert online.strip() == "online", online
            with urllib.request.urlopen(http + "/", timeout=2) as response:
                html = response.read().decode()
                assert response.status == 200 and "<html" in html.lower() and "/api/" in html
            report["mqtt_reconnect_and_ha_online"] = "passed"
            print(json.dumps(report, indent=2))
        except BaseException:
            for name, log in logs.items():
                log.flush()
                log.seek(0)
                print(f"--- {name} ---\n{log.read()[-10000:]}")
            raise
        finally:
            for process in reversed(processes):
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
            for log in logs.values():
                log.close()


if __name__ == "__main__":
    main()

# LED Matrix Animation - Development Shell
#
# Usage: nix-shell shell.nix
# Provides: PlatformIO CLI, C++ toolchain for native tests, Python 3
#
{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "led-matrix-dev";

  buildInputs = with pkgs; [
    # PlatformIO for ESP32 build/flash/monitor
    platformio-core

    # C++ toolchain for native host tests (ASan/UBSan)
    gcc

    # Python for test runners
    python3

    # Serial tools
    minicom
    picocom

    # Useful for LittleFS image building
    nodejs  # for optional minification tooling
  ];

  shellHook = ''
    echo "=== LED Matrix Dev Shell ==="
    echo "  pio run              - Build firmware"
    echo "  pio run -t upload    - Flash to ESP32"
    echo "  pio device monitor   - Serial monitor"
    echo "  pio run -t uploadfs  - Upload LittleFS data/"
    echo ""
    echo "  python3 test/*/run.py - Run native host tests"
    echo ""
    echo "Environments:"
    echo "  esp32s3  - ESP32-S3 (default, 4D Systems board)"
    echo "  esp32    - ESP32 (generic devkit)"
    echo "==========================="

    # PlatformIO needs a writable home for its packages
    export PLATFORMIO_CORE_DIR="''${PLATFORMIO_CORE_DIR:-$HOME/.platformio}"
  '';
}

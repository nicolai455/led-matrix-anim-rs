#!/usr/bin/env bash
# ==============================================================================
# Matrix Firmware Reproducible Cross-Build Script
# Targets:
#   - esp32:        ESP32-WROOM (xtensa-esp32-espidf)
#   - esp32-bare:   ESP32-WROOM bare profile without mic (xtensa-esp32-espidf, --features bare)
#   - esp32s3:      ESP32-S3 N16R8 (xtensa-esp32s3-espidf, --features esp32s3)
#   - all:          Builds esp32, esp32-bare, and esp32s3
# Profiles:
#   - release (default)
#   - debug
# Framework: ESP-IDF v5.2.1 with Rust esp-idf-svc/sys/hal
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Helper to find first existing path matching patterns/globs
find_first() {
    for pat in "$@"; do
        # Use compgen or nullglob expansion
        for match in $pat; do
            if [ -e "$match" ]; then
                echo "$match"
                return 0
            fi
        done
    done
    return 1
}

# 1. Environment and Toolchain Paths Setup
# Allow standard espup installs by default, auto-discovering existing toolchains or cached paths.
DEFAULT_RUSTUP="$HOME/.rustup"
DEFAULT_CARGO="$HOME/.cargo"
if [ ! -d "$DEFAULT_RUSTUP/toolchains/esp" ] && [ -d "$HOME/.hermes/cache/scratch/rustup/toolchains/esp" ]; then
    DEFAULT_RUSTUP="$HOME/.hermes/cache/scratch/rustup"
fi
if [ ! -d "$DEFAULT_CARGO" ] && [ -d "$HOME/.hermes/cache/scratch/cargo_home" ]; then
    DEFAULT_CARGO="$HOME/.hermes/cache/scratch/cargo_home"
fi

export RUSTUP_HOME="${RUSTUP_HOME:-$DEFAULT_RUSTUP}"
export CARGO_HOME="${CARGO_HOME:-$DEFAULT_CARGO}"

# Auto-discover IDF_PATH
if [ -z "${IDF_PATH:-}" ]; then
    IDF_PATH=$(find_first \
        "$SCRIPT_DIR/.embuild/espressif/esp-idf/v5.2.1" \
        "$SCRIPT_DIR/.embuild/espressif/esp-idf/v5.2.*" \
        "$HOME/.espressif/esp-idf/v5.2.*" \
        "$HOME/.espressif/esp-idf/v5.*" || true)
    export IDF_PATH
fi

# Auto-discover IDF_TOOLS_PATH
if [ -z "${IDF_TOOLS_PATH:-}" ]; then
    IDF_TOOLS_PATH=$(find_first \
        "$SCRIPT_DIR/.embuild/espressif" \
        "$HOME/.espressif" || true)
    export IDF_TOOLS_PATH
fi

export PYTHON_DEPS_CHECKED=1
export IDF_COMPONENT_MANAGER=0
export PYTHONPATH=""
if [ -d "$IDF_TOOLS_PATH/python_env/idf5.2_py3.13_env" ]; then
    export IDF_PYTHON_ENV_PATH="$IDF_TOOLS_PATH/python_env/idf5.2_py3.13_env"
fi

# Auto-discover Clang library
if [ -z "${LIBCLANG_PATH:-}" ]; then
    LIBCLANG_PATH=$(find_first \
        "$IDF_TOOLS_PATH/tools/esp-clang"/*/esp-clang/lib \
        "$RUSTUP_HOME/toolchains/esp/xtensa-esp32-elf-clang"/*/esp-clang/lib \
        "$HOME/.rustup/toolchains/esp/xtensa-esp32-elf-clang"/*/esp-clang/lib || true)
    export LIBCLANG_PATH
fi

# Auto-discover Xtensa GCC toolchain
XTENSA_GCC_BIN=$(find_first \
    "$RUSTUP_HOME/toolchains/esp/xtensa-esp-elf"/*/xtensa-esp-elf/bin \
    "$IDF_TOOLS_PATH/tools/xtensa-esp-elf"/*/xtensa-esp-elf/bin \
    "$HOME/.rustup/toolchains/esp/xtensa-esp-elf"/*/xtensa-esp-elf/bin || true)

RUST_ESP_BIN="$RUSTUP_HOME/toolchains/esp/bin"

# Auto-discover build helpers (ldproxy, cmake, ninja, esptool.py)
# Check existing PATH first, then standard install paths, then Nix cache paths dynamically without hardcoded hashes
LDPROXY_BIN_PATH=$(find_first \
    "$(type -P ldproxy 2>/dev/null || true)" \
    "$CARGO_HOME/bin/ldproxy" \
    "$HOME/.cargo/bin/ldproxy" \
    /nix/store/*ldproxy*/bin/ldproxy || true)

CMAKE_BIN_PATH=$(find_first \
    "$(type -P cmake 2>/dev/null || true)" \
    "$IDF_TOOLS_PATH/tools/cmake"/*/bin/cmake \
    /nix/store/*cmake*/bin/cmake || true)

NINJA_BIN_PATH=$(find_first \
    "$(type -P ninja 2>/dev/null || true)" \
    "$IDF_TOOLS_PATH/tools/ninja"/*/ninja \
    /nix/store/*ninja*/bin/ninja || true)

ESPTOOL_PATH=$(find_first \
    "$(type -P esptool.py 2>/dev/null || true)" \
    "$IDF_TOOLS_PATH/python_env"/*/bin/esptool.py \
    "$HOME/.platformio/packages/tool-esptoolpy/esptool.py" \
    "$IDF_PATH/components/esptool_py/esptool/esptool.py" || true)

# Prepend discovered tool dirs to PATH
ADDITIONAL_PATHS=""
[ -n "$LDPROXY_BIN_PATH" ] && ADDITIONAL_PATHS="$ADDITIONAL_PATHS:$(dirname "$LDPROXY_BIN_PATH")"
[ -n "$CMAKE_BIN_PATH" ] && ADDITIONAL_PATHS="$ADDITIONAL_PATHS:$(dirname "$CMAKE_BIN_PATH")"
[ -n "$NINJA_BIN_PATH" ] && ADDITIONAL_PATHS="$ADDITIONAL_PATHS:$(dirname "$NINJA_BIN_PATH")"
[ -n "$RUST_ESP_BIN" ] && [ -d "$RUST_ESP_BIN" ] && ADDITIONAL_PATHS="$ADDITIONAL_PATHS:$RUST_ESP_BIN"
[ -n "$XTENSA_GCC_BIN" ] && [ -d "$XTENSA_GCC_BIN" ] && ADDITIONAL_PATHS="$ADDITIONAL_PATHS:$XTENSA_GCC_BIN"

export PATH="${ADDITIONAL_PATHS#:}:$PATH"

# Validate essentials
if ! command -v cargo &>/dev/null; then
    echo "ERROR: Cargo (esp toolchain) not found. Please install via espup or check RUSTUP_HOME/PATH." >&2
    exit 1
fi
if ! command -v ldproxy &>/dev/null; then
    echo "ERROR: ldproxy not found in PATH or standard cargo locations." >&2
    exit 1
fi

TARGET="${1:-all}"
PROFILE="${2:-release}"

# Validate profile
case "$PROFILE" in
    release|debug)
        ;;
    *)
        echo "ERROR: Invalid profile '$PROFILE'. Must be 'release' or 'debug'." >&2
        exit 1
        ;;
esac

# Function to verify partition table integrity & absence of overlaps
verify_partitions() {
    local table_file="$1"
    local gen_tool="$IDF_PATH/components/partition_table/gen_esp32part.py"
    if [ -f "$gen_tool" ]; then
        python3 "$gen_tool" -q "$table_file" /dev/null 2>/dev/null || {
            echo "ERROR: Partition table $table_file validation failed!" >&2
            python3 "$gen_tool" "$table_file" /dev/null
            return 1
        }
    fi
}

# Function to check binary size against partition slot
check_partition_fit() {
    local bin_file="$1"
    local table_file="$2"
    local mcu="$3"

    python3 - "$bin_file" "$table_file" "$mcu" << 'EOF'
import sys, re

bin_file = sys.argv[1]
table_file = sys.argv[2]
mcu = sys.argv[3]

with open(bin_file, 'rb') as f:
    f.seek(0, 2)
    bin_size = f.tell()

def parse_size(s):
    s = s.strip()
    if s.endswith('K') or s.endswith('k'):
        return int(s[:-1], 0) * 1024
    if s.endswith('M') or s.endswith('m'):
        return int(s[:-1], 0) * 1024 * 1024
    return int(s, 0)

app0_size = None
with open(table_file, 'r') as f:
    for line in f:
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        parts = [p.strip() for p in line.split(',')]
        if len(parts) >= 5 and parts[0] == 'app0':
            app0_size = parse_size(parts[4])
            break

if app0_size is None:
    print(f"WARN: Could not find 'app0' partition size in {table_file}")
    sys.exit(1)

diff = app0_size - bin_size
pct = (bin_size / app0_size) * 100

print(f"Partition Fit Check [{mcu}]:")
print(f"  App Image:      {bin_size:,} bytes ({bin_size/1024:.1f} KB, {bin_size/(1024*1024):.2f} MB)")
print(f"  Partition Slot: {app0_size:,} bytes ({app0_size/1024:.1f} KB, {app0_size/(1024*1024):.2f} MB)")

if diff >= 0:
    print(f"  Result:         PASS - Image fits with {diff:,} bytes ({diff/1024:.1f} KB) headroom ({pct:.1f}% used).")
else:
    print(f"  Result:         FAIL - Image EXCEEDS partition slot by {-diff:,} bytes ({-diff/1024:.1f} KB)!")
    sys.exit(1)
EOF
}

build_target() {
    local target_arch="$1"
    local mcu="$2"
    local defaults_file="$3"
    local table_file="$4"
    local extra_features="$5"

    local cargo_flags=("--target" "$target_arch")

    if [ "$PROFILE" = "release" ]; then
        cargo_flags+=("--release")
    fi

    if [ -n "$extra_features" ]; then
        cargo_flags+=("--features" "$extra_features")
    fi

    echo "===================================================================="
    echo " Building matrix-firmware for $mcu ($target_arch)"
    echo " Profile:  $PROFILE"
    [ -n "$extra_features" ] && echo " Features: $extra_features"
    echo "===================================================================="

    verify_partitions "$table_file"

    export MCU="$mcu"
    local resolved_defaults="$SCRIPT_DIR/target/matrix-sdkconfig-$mcu.defaults"
    mkdir -p "$SCRIPT_DIR/target"
    cp "$SCRIPT_DIR/sdkconfig.defaults" "$resolved_defaults"
    printf '\nCONFIG_PARTITION_TABLE_CUSTOM_FILENAME="%s/%s"\nCONFIG_PARTITION_TABLE_FILENAME="%s/%s"\n' \
        "$SCRIPT_DIR" "$table_file" "$SCRIPT_DIR" "$table_file" >> "$resolved_defaults"
    export ESP_IDF_SDKCONFIG_DEFAULTS="$SCRIPT_DIR/sdkconfig.defaults;$SCRIPT_DIR/$defaults_file;$resolved_defaults"

    mkdir -p "$SCRIPT_DIR/target/$target_arch/$PROFILE"
    local map_suffix="${extra_features:+-$extra_features}"
    export MATRIX_LINK_MAP="$SCRIPT_DIR/target/$target_arch/$PROFILE/matrix-firmware${map_suffix}.map"
    cargo build "${cargo_flags[@]}"

    local elf_path="$SCRIPT_DIR/target/$target_arch/$PROFILE/matrix-firmware"
    if [ ! -f "$elf_path" ]; then
        echo "ERROR: Artifact $elf_path was not generated." >&2
        return 1
    fi

    echo "SUCCESS: Built ELF $elf_path"

    # Read actual native sdkconfig to confirm custom partition configuration was applied and not using a stale cache
    local out_sdkconfig
    out_sdkconfig=$(find_first "$SCRIPT_DIR/target/$target_arch/$PROFILE/build"/esp-idf-sys-*/out/sdkconfig || true)
    if [ -n "$out_sdkconfig" ] && [ -f "$out_sdkconfig" ]; then
        if grep -q "CONFIG_PARTITION_TABLE_CUSTOM=y" "$out_sdkconfig" && \
           (grep -q "$table_file" "$out_sdkconfig" || grep -q "CONFIG_PARTITION_TABLE_CUSTOM_FILENAME=\".*$table_file\"" "$out_sdkconfig"); then
            echo "SDKConfig Verification: Verified active custom partition table '$table_file' in native sdkconfig."
        else
            echo "ERROR: Native sdkconfig ($out_sdkconfig) does not reflect custom partition table '$table_file'! Stale cache detected." >&2
            return 1
        fi
    fi

    if command -v "xtensa-$mcu-elf-size" &>/dev/null; then
        "xtensa-$mcu-elf-size" "$elf_path"
    elif [ -n "$XTENSA_GCC_BIN" ] && [ -x "$XTENSA_GCC_BIN/xtensa-$mcu-elf-size" ]; then
        "$XTENSA_GCC_BIN/xtensa-$mcu-elf-size" "$elf_path"
    elif command -v size &>/dev/null; then
        size "$elf_path"
    fi

    # Generate real flashable binary image via esptool.py
    local bin_suffix=""
    [ -n "$extra_features" ] && bin_suffix="-$extra_features"
    local bin_path="$SCRIPT_DIR/target/$target_arch/$PROFILE/matrix-firmware${bin_suffix}.bin"

    if [ -n "$ESPTOOL_PATH" ]; then
        echo "Generating flashable binary image using $ESPTOOL_PATH..."
        local esptool_cmd=("python3" "$ESPTOOL_PATH" "--chip" "$mcu" "elf2image" "$elf_path" "--output" "$bin_path")
        # If ESPTOOL_PATH is a python script in a virtualenv, prefer that virtualenv's python
        local esptool_dir
        esptool_dir="$(dirname "$ESPTOOL_PATH")"
        if [ -x "$esptool_dir/python" ]; then
            esptool_cmd=("$esptool_dir/python" "$ESPTOOL_PATH" "--chip" "$mcu" "elf2image" "$elf_path" "--output" "$bin_path")
        fi

        "${esptool_cmd[@]}"
        echo "Generated binary: $bin_path"
        check_partition_fit "$bin_path" "$table_file" "$mcu"
    else
        echo "ERROR: esptool.py required for image generation and OTA fit verification." >&2
        return 1
    fi
    echo ""
}

case "$TARGET" in
    esp32)
        build_target "xtensa-esp32-espidf" "esp32" "sdkconfig.defaults.esp32" "partitions.csv" ""
        ;;
    esp32-bare|esp32bare)
        build_target "xtensa-esp32-espidf" "esp32" "sdkconfig.defaults.esp32" "partitions.csv" "bare"
        ;;
    esp32s3)
        build_target "xtensa-esp32s3-espidf" "esp32s3" "sdkconfig.defaults.esp32s3" "partitions_16MB.csv" "esp32s3"
        ;;
    all)
        build_target "xtensa-esp32-espidf" "esp32" "sdkconfig.defaults.esp32" "partitions.csv" ""
        build_target "xtensa-esp32-espidf" "esp32" "sdkconfig.defaults.esp32" "partitions.csv" "bare"
        build_target "xtensa-esp32s3-espidf" "esp32s3" "sdkconfig.defaults.esp32s3" "partitions_16MB.csv" "esp32s3"
        ;;
    *)
        echo "ERROR: Invalid target '$TARGET'." >&2
        echo "Usage: $0 [esp32|esp32-bare|esp32s3|all] [release|debug]" >&2
        exit 1
        ;;
esac

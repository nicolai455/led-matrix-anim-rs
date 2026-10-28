#!/usr/bin/env bash
# Populate per-invocation Cargo flags; never export profile overrides across targets.
matrix_optimization_flags() {
    local mcu="$1" profile="$2"
    optimization_flags=()
    if [[ "$profile" == release && "$mcu" == esp32s3 ]]; then
        optimization_flags+=(--config 'profile.release.opt-level=3')
        local package
        for package in matrix-firmware matrix-control matrix-core matrix-effects; do
            optimization_flags+=(--config "profile.release.package.$package.opt-level=3")
        done
        optimization_flags+=(-Z build-std-features=backtrace-trace-only)
    fi
}

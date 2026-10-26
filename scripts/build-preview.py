#!/usr/bin/env python3
"""Rebuild the embedded preview after changing matrix-core or matrix-effects.

Requires wasm32-unknown-unknown std. CARGO and RUSTC may select a toolchain.
"""
import gzip
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
env = os.environ.copy()
env['CARGO_PROFILE_RELEASE_OPT_LEVEL'] = 'z'
env['CARGO_PROFILE_RELEASE_LTO'] = 'true'
env['CARGO_PROFILE_RELEASE_PANIC'] = 'abort'
env['CARGO_PROFILE_RELEASE_STRIP'] = 'symbols'
subprocess.run([env.get('CARGO', 'cargo'), 'build', '-p', 'matrix-preview',
                '--target', 'wasm32-unknown-unknown', '--release'], cwd=root, env=env, check=True)
target = Path(env.get('CARGO_TARGET_DIR', root / 'target'))
wasm = (target / 'wasm32-unknown-unknown/release/matrix_preview.wasm').read_bytes()
asset = root / 'crates/matrix-control/assets/preview.wasm.gz'
asset.parent.mkdir(parents=True, exist_ok=True)
asset.write_bytes(gzip.compress(wasm, compresslevel=9, mtime=0))
print(f'{asset}: {asset.stat().st_size} bytes compressed, {len(wasm)} bytes WASM')

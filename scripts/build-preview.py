#!/usr/bin/env python3
"""Rebuild the embedded preview after changing matrix-core or matrix-effects.

Requires wasm32-unknown-unknown std and Binaryen wasm-opt (tested with 129).
CARGO and RUSTC may select a toolchain; WASM_OPT selects the optimizer.
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER may override the default rust-lld.
"""
import gzip
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
LINKER = 'CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER'


def main():
    env = os.environ.copy()
    optimizer = env.get('WASM_OPT', 'wasm-opt')
    if not shutil.which(optimizer, path=env.get('PATH')):
        raise SystemExit(f'wasm-opt not found: {optimizer}; install Binaryen or set WASM_OPT')

    compressor = env.get('ZOPFLI', 'zopfli')
    if not shutil.which(compressor, path=env.get('PATH')):
        raise SystemExit(f'zopfli not found: {compressor}; install Zopfli or set ZOPFLI')

    if not env.get(LINKER):
        rustc = env.get('RUSTC', 'rustc')
        sysroot = subprocess.check_output([rustc, '--print', 'sysroot'], cwd=ROOT, env=env, text=True).strip()
        version = subprocess.check_output([rustc, '-vV'], cwd=ROOT, env=env, text=True)
        host = next((line[6:] for line in version.splitlines() if line.startswith('host: ')), None)
        if not host:
            raise SystemExit('could not determine rustc host from rustc -vV')
        linker = Path(sysroot) / 'lib' / 'rustlib' / host / 'bin' / ('rust-lld.exe' if os.name == 'nt' else 'rust-lld')
        if not linker.is_file():
            raise SystemExit(f'rust-lld not found: {linker}; set {LINKER} to a linker')
        env[LINKER] = str(linker)

    env['CARGO_PROFILE_RELEASE_OPT_LEVEL'] = 'z'
    env['CARGO_PROFILE_RELEASE_LTO'] = 'true'
    env['CARGO_PROFILE_RELEASE_PANIC'] = 'abort'
    env['CARGO_PROFILE_RELEASE_STRIP'] = 'symbols'
    subprocess.run([env.get('CARGO', 'cargo'), 'build', '-p', 'matrix-preview',
                    '--target', 'wasm32-unknown-unknown', '--release'], cwd=ROOT, env=env, check=True)
    target = Path(env.get('CARGO_TARGET_DIR', ROOT / 'target'))
    if not target.is_absolute():
        target = ROOT / target
    source = target / 'wasm32-unknown-unknown/release/matrix_preview.wasm'
    with tempfile.TemporaryDirectory() as tmp:
        optimized = Path(tmp) / 'preview.wasm'
        subprocess.run([optimizer, str(source), '-Oz', '--converge', '--enable-bulk-memory-opt',
                        '--enable-nontrapping-float-to-int', '--enable-sign-ext',
                        '-o', str(optimized)], cwd=ROOT, env=env, check=True)
        wasm = optimized.read_bytes()
        compressed = subprocess.check_output([compressor, "--i50", "--gzip", "-c", str(optimized)], cwd=ROOT, env=env)
    asset = ROOT / 'crates/matrix-control/assets/preview.wasm.gz'
    asset.parent.mkdir(parents=True, exist_ok=True)
    asset.write_bytes(compressed)
    print(f'{asset}: {asset.stat().st_size} bytes compressed, {len(wasm)} bytes WASM')


if __name__ == '__main__':
    main()

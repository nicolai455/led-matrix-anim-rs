import gzip
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location('build_preview', Path(__file__).with_name('build-preview.py'))
assert spec is not None and spec.loader is not None
preview = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preview)


class BuildPreviewTests(unittest.TestCase):
    def test_missing_optimizer_fails_before_build(self):
        with patch.dict(os.environ, {'PATH': ''}, clear=True), patch('subprocess.run') as run:
            with self.assertRaisesRegex(SystemExit, 'wasm-opt'):
                preview.main()
            run.assert_not_called()

    def test_build_optimizes_before_writing_deterministic_asset(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            linker = root / 'sysroot/lib/rustlib/test-host/bin/rust-lld'
            linker.parent.mkdir(parents=True)
            linker.touch()
            calls = []
            def run(cmd, **kwargs):
                calls.append((cmd, kwargs))
                if cmd[0] == 'optimizer':
                    Path(cmd[-1]).write_bytes(b'optimized wasm')
            with patch.object(preview, 'ROOT', root), patch.dict(os.environ, {
                'PATH': '/bin', 'WASM_OPT': 'optimizer', 'RUSTC': 'custom-rustc',
                'CARGO': 'custom-cargo', 'CARGO_TARGET_DIR': 'custom-target', 'RUSTFLAGS': '-C debuginfo=0',
            }, clear=True), patch('shutil.which', return_value='/bin/optimizer'), \
                 patch('subprocess.check_output', side_effect=[str(root / 'sysroot'), 'host: test-host\n', gzip.compress(b'optimized wasm', compresslevel=9, mtime=0)]) as query, \
                 patch('subprocess.run', side_effect=run):
                preview.main()
            self.assertEqual(query.call_args_list[0].args[0], ['custom-rustc', '--print', 'sysroot'])
            self.assertEqual(calls[0][0][0], 'custom-cargo')
            self.assertEqual(calls[0][1]['env'][preview.LINKER], str(linker))
            self.assertEqual(calls[0][1]['env']['RUSTFLAGS'], '-C debuginfo=0')
            command = calls[1][0]
            self.assertEqual(command[:7], ['optimizer', str(root / 'custom-target/wasm32-unknown-unknown/release/matrix_preview.wasm'), '-Oz', '--converge', '--enable-bulk-memory-opt', '--enable-nontrapping-float-to-int', '--enable-sign-ext'])
            asset = root / 'crates/matrix-control/assets/preview.wasm.gz'
            self.assertEqual(asset.read_bytes(), gzip.compress(b'optimized wasm', compresslevel=9, mtime=0))

    def test_linker_override_and_optimizer_failure_preserves_asset(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            asset = root / 'crates/matrix-control/assets/preview.wasm.gz'
            asset.parent.mkdir(parents=True)
            asset.write_bytes(b'original')
            def run(cmd, **kwargs):
                if cmd[0] == 'optimizer':
                    raise subprocess.CalledProcessError(1, cmd)
                self.assertEqual(kwargs['env'][preview.LINKER], 'my-linker')
            with patch.object(preview, 'ROOT', root), patch.dict(os.environ, {
                'PATH': '/bin', 'WASM_OPT': 'optimizer', preview.LINKER: 'my-linker',
            }, clear=True), patch('shutil.which', return_value='/bin/optimizer'), \
                 patch('subprocess.check_output') as query, patch('subprocess.run', side_effect=run):
                with self.assertRaises(subprocess.CalledProcessError):
                    preview.main()
                query.assert_not_called()
            self.assertEqual(asset.read_bytes(), b'original')


if __name__ == '__main__':
    unittest.main()

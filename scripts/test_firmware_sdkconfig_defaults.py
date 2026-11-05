"""Exercise build-script defaults selection without compiling firmware."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
FIRMWARE = ROOT / 'firmware'


class FirmwareDefaultsTests(unittest.TestCase):
    def test_target_defaults_override_common_before_partition_paths(self):
        for target, expected_stack, table in [
            ('esp32', '24576', 'partitions.csv'),
            ('esp32-bare', '24576', 'partitions.csv'),
            ('esp32s3', '20480', 'partitions_16MB.csv'),
        ]:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                firmware = root / 'firmware'
                firmware.mkdir()
                for name in ('build_firmware.sh', 'build_optimization.sh',
                             'sdkconfig.defaults', 'sdkconfig.defaults.esp32',
                             'sdkconfig.defaults.esp32s3', 'partitions.csv',
                             'partitions_16MB.csv'):
                    shutil.copy2(FIRMWARE / name, firmware / name)
                bin_dir = root / 'bin'
                bin_dir.mkdir()
                cargo = bin_dir / 'cargo'
                cargo.write_text('''#!/usr/bin/env python3
import json, os
from pathlib import Path
files = os.environ['ESP_IDF_SDKCONFIG_DEFAULTS'].split(';')
Path(os.environ['CAPTURE']).write_text(json.dumps([[f, Path(f).read_text()] for f in files]))
raise SystemExit(42)
''')
                cargo.chmod(0o755)
                ldproxy = bin_dir / 'ldproxy'
                ldproxy.write_text('#!/bin/sh\nexit 0\n')
                ldproxy.chmod(0o755)
                capture = root / 'capture.json'
                env = os.environ.copy()
                env.update(HOME=str(root), RUSTUP_HOME=str(root / 'rustup'),
                           CARGO_HOME=str(root / 'cargo-home'), IDF_PATH='',
                           IDF_TOOLS_PATH='', PATH=f'{bin_dir}:{os.environ["PATH"]}',
                           CAPTURE=str(capture))
                result = subprocess.run(['bash', str(firmware / 'build_firmware.sh'), target],
                                        env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 42, result.stdout + result.stderr)
                files = json.loads(capture.read_text())
                merged = {}
                for _, text in files:
                    for line in text.splitlines():
                        if line.startswith('CONFIG_') and '=' in line:
                            key, value = line.split('=', 1)
                            merged[key] = value
                self.assertEqual(merged['CONFIG_ESP_MAIN_TASK_STACK_SIZE'], expected_stack)
                for key in ('CONFIG_PARTITION_TABLE_CUSTOM_FILENAME', 'CONFIG_PARTITION_TABLE_FILENAME'):
                    self.assertEqual(merged[key], f'"{firmware / table}"')
                # The final generated layer must only set partition paths, so it
                # cannot accidentally reset any target-specific defaults.
                final_keys = {line.split('=', 1)[0] for line in files[-1][1].splitlines()
                              if line.startswith('CONFIG_') and '=' in line}
                self.assertEqual(final_keys, {'CONFIG_PARTITION_TABLE_CUSTOM_FILENAME',
                                              'CONFIG_PARTITION_TABLE_FILENAME'})


if __name__ == '__main__':
    unittest.main()

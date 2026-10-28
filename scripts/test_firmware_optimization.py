from pathlib import Path
import subprocess
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class TargetOptimizationTests(unittest.TestCase):
    def flags(self, *targets):
        script = 'source "$1"; shift; for target in "$@"; do matrix_optimization_flags "$target" release; printf "%s\\0" "${optimization_flags[@]}"; printf "\\n"; done'
        result = subprocess.check_output(['bash', '-c', script, 'test', str(ROOT / 'firmware/build_optimization.sh'), *targets])
        return [[flag.decode() for flag in line.split(b'\0') if flag] for line in result.splitlines()]

    def test_s3_overrides_all_size_settings(self):
        flags = self.flags('esp32s3')[0]
        self.assertIn('profile.release.opt-level=3', flags)
        for name in ['matrix-firmware', 'matrix-control', 'matrix-core', 'matrix-effects']:
            self.assertIn(f'profile.release.package.{name}.opt-level=3', flags)
        self.assertIn('build-std-features=backtrace-trace-only', flags)
        self.assertNotIn('optimize_for_size', ' '.join(flags))

    def test_all_target_build_cannot_leak_s3_flags_into_esp32(self):
        flags = self.flags('esp32', 'esp32s3', 'esp32')
        self.assertEqual(flags[0], [])
        self.assertTrue(flags[1])
        self.assertEqual(flags[2], [])

    def test_default_profile_retains_esp32_size_optimization(self):
        manifest = tomllib.loads((ROOT / 'firmware/Cargo.toml').read_text())
        self.assertEqual(manifest['profile']['release']['opt-level'], 'z')
        for settings in manifest['profile']['release']['package'].values():
            self.assertEqual(settings['opt-level'], 'z')
        config = tomllib.loads((ROOT / 'firmware/.cargo/config.toml').read_text())
        self.assertIn('optimize_for_size', config['unstable']['build-std-features'])

    def test_debug_has_no_release_overrides(self):
        result = subprocess.check_output([
            'bash', '-c',
            'source "$1"; matrix_optimization_flags esp32s3 debug; printf "%s" "${optimization_flags[@]}"',
            'test', str(ROOT / 'firmware/build_optimization.sh'),
        ])
        self.assertEqual(result, b'')

    def test_build_script_passes_per_target_flags_to_cargo(self):
        script = (ROOT / 'firmware/build_firmware.sh').read_text()
        self.assertIn('source "$SCRIPT_DIR/build_optimization.sh"', script)
        self.assertIn('matrix_optimization_flags "$mcu" "$PROFILE"', script)
        self.assertIn('cargo build "${cargo_flags[@]}" "${optimization_flags[@]}"', script)

    def test_idf_optimization_is_target_specific(self):
        for target, mode in [('esp32', 'SIZE'), ('esp32s3', 'PERF')]:
            text = (ROOT / f'firmware/sdkconfig.defaults.{target}').read_text()
            self.assertIn(f'CONFIG_COMPILER_OPTIMIZATION_{mode}=y', text)


if __name__ == '__main__':
    unittest.main()

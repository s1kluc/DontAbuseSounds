import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from scripts.build import sign_windows


class SigningTests(unittest.TestCase):
    def test_signs_unsigned_payload_and_preserves_vendor_signature(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            (root / '_internal').mkdir()
            for name in ('app.exe', '_internal/vendor.dll', '_internal/audio.pyd', 'README.md'):
                (root / name).touch()
            with patch.dict(os.environ, {'WINDOWS_CERTIFICATE_THUMBPRINT': 'test'}), \
                    patch('scripts.build.shutil.which', return_value='signtool'), \
                    patch('scripts.build.subprocess.run') as run:
                def execute(command, **kwargs):
                    return subprocess.CompletedProcess(command, 0 if kwargs.get('check') or command[-1].endswith('vendor.dll') else 1)
                run.side_effect = execute
                sign_windows(root)
                signed = [Path(call.args[0][-1]).name for call in run.call_args_list
                          if call.args[0][1] == 'sign']
                self.assertCountEqual(signed, ['app.exe', 'audio.pyd'])
                verified = [Path(call.args[0][-1]).name for call in run.call_args_list
                            if call.args[0][1] == 'verify' and call.kwargs.get('check')]
                self.assertCountEqual(verified, ['app.exe', 'audio.pyd', 'vendor.dll'])

    def test_signing_failure_stops_build(self):
        with tempfile.TemporaryDirectory() as root:
            (Path(root) / 'app.exe').touch()
            with patch.dict(os.environ, {'WINDOWS_CERTIFICATE_THUMBPRINT': 'test'}), \
                    patch('scripts.build.shutil.which', return_value='signtool'), \
                    patch('scripts.build.subprocess.run') as run:
                run.side_effect = [subprocess.CompletedProcess([], 1),
                                   subprocess.CalledProcessError(1, 'signtool')]
                with self.assertRaises(subprocess.CalledProcessError):
                    sign_windows(Path(root))

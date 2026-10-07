import hashlib
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

from build_portable import download, extract, require_windows_executable


class PackageTests(unittest.TestCase):
    def test_corrupted_cached_download_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            (cache / "python.zip").write_bytes(b"alterado")
            metadata = {"filename": "python.zip", "sha256": hashlib.sha256(b"original").hexdigest()}
            with self.assertRaisesRegex(RuntimeError, "hash"):
                download(metadata, cache)

    def test_archive_cannot_write_outside_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ["../outside", "..\\outside", "/outside", "C:/outside"]:
                archive = root / "malformed.zip"
                with zipfile.ZipFile(archive, "w") as output:
                    output.writestr("legitimate.txt", "ok")
                    output.writestr(name, "invalid")
                with self.assertRaises(RuntimeError):
                    extract(archive, root / "runtime")
                self.assertFalse((root / "runtime" / "legitimate.txt").exists())
                self.assertFalse((root / "outside").exists())

    def test_only_windows_x64_application_can_be_packaged(self):
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "application.exe"
            executable.write_bytes(b"\x7fELF" + b"\0" * 100)
            with self.assertRaises(RuntimeError):
                require_windows_executable(executable)
            header = bytearray(64)
            header[:2] = b"MZ"
            struct.pack_into("<I", header, 60, 64)
            executable.write_bytes(header + b"PE\0\0\x4c\x01")
            with self.assertRaises(RuntimeError):
                require_windows_executable(executable)
            executable.write_bytes(header + b"PE\0\0\x64\x86")
            require_windows_executable(executable)


if __name__ == "__main__":
    unittest.main()

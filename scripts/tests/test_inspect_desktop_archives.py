#!/usr/bin/env python3
"""Tests for scripts/release/inspect_desktop_archives.py."""

from __future__ import annotations

import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "release"))

import inspect_desktop_archives as ida  # noqa: E402

VERSION = "0.3.1"


def _write_zip(path: Path, members: dict[str, bytes]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as zf:
        for name, data in members.items():
            zf.writestr(name, data)


class InspectDesktopArchivesTests(unittest.TestCase):
    def test_clean_portable_zip_ok(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            _write_zip(
                dist / f"Mnemosyne-{VERSION}-windows-x64-portable.zip",
                {
                    "Mnemosyne.exe": b"exe",
                    "README-PORTABLE.txt": b"readme",
                },
            )
            result = ida.inspect_desktop_archives(dist)
            self.assertTrue(result.ok, ida.format_report(result))
            self.assertEqual(len(result.archives), 1)

    def test_rejects_secret_member(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            _write_zip(
                dist / f"Mnemosyne-{VERSION}-windows-x64-portable.zip",
                {".env": b"SECRET=1", "Mnemosyne.exe": b"exe"},
            )
            result = ida.inspect_desktop_archives(dist)
            self.assertFalse(result.ok)
            self.assertTrue(any(f.kind == "secret_path" for f in result.findings))

    def test_rejects_ordinary_pem_filename(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            _write_zip(
                dist / f"Mnemosyne-{VERSION}-windows-x64-portable.zip",
                {"certs/private-key.pem": b"-----BEGIN", "Mnemosyne.exe": b"exe"},
            )
            result = ida.inspect_desktop_archives(dist)
            self.assertFalse(result.ok)
            self.assertTrue(any(f.kind == "secret_path" for f in result.findings))
            self.assertTrue(any("private-key.pem" in f.member for f in result.findings))

    def test_rejects_absolute_build_path_member(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            _write_zip(
                dist / f"Mnemosyne-{VERSION}-macos-aarch64-app.zip",
                {
                    "Users/runner/work/secret.key": b"x",
                    "Mnemosyne.app/Contents/Info.plist": b"<plist/>",
                },
            )
            result = ida.inspect_desktop_archives(dist)
            self.assertFalse(result.ok)
            kinds = {f.kind for f in result.findings}
            self.assertTrue(kinds & {"secret_path", "build_path"})


if __name__ == "__main__":
    unittest.main()

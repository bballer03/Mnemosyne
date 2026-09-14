#!/usr/bin/env python3
"""Tests for scripts/release/probe_desktop_bundles.py (synthetic artifacts only)."""

from __future__ import annotations

import plistlib
import struct
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "release"))

import probe_desktop_bundles as pdb  # noqa: E402

VERSION = "0.3.1"


def _elf(machine: int) -> bytes:
    # Minimal ELF64 header with e_machine at offset 18.
    header = bytearray(64)
    header[0:4] = b"\x7fELF"
    header[4] = 2  # ELFCLASS64
    header[5] = 1  # ELFDATA2LSB
    struct.pack_into("<H", header, 18, machine)
    return bytes(header)


def _macos_zip(path: Path, *, version: str = VERSION, exe: str = "Mnemosyne") -> None:
    plist = {
        "CFBundleIdentifier": "app.mnemosyne.desktop",
        "CFBundleExecutable": exe,
        "CFBundleShortVersionString": version,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as zf:
        zf.writestr(
            "Mnemosyne.app/Contents/Info.plist",
            plistlib.dumps(plist),
        )
        zf.writestr(f"Mnemosyne.app/Contents/MacOS/{exe}", b"\xcf\xfafake")


class ProbeDesktopBundlesTests(unittest.TestCase):
    def test_macos_zip_ok(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            _macos_zip(dist / f"Mnemosyne-{VERSION}-macos-aarch64-app.zip")
            result = pdb.probe_desktop_bundles(dist, VERSION)
            self.assertTrue(result.ok, pdb.format_report(result))
            self.assertEqual(len(result.probed), 1)

    def test_macos_missing_plist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            path = dist / f"Mnemosyne-{VERSION}-macos-x64-app.zip"
            path.parent.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(path, "w") as zf:
                zf.writestr("Mnemosyne.app/Contents/MacOS/Mnemosyne", b"x")
            result = pdb.probe_desktop_bundles(dist, VERSION)
            self.assertFalse(result.ok)
            self.assertTrue(any(f.kind == "info_plist" for f in result.findings))

    def test_appimage_arch_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            dist.mkdir()
            # Label says x86_64 but ELF is aarch64.
            path = dist / f"Mnemosyne-{VERSION}-linux-x86_64.AppImage"
            path.write_bytes(_elf(pdb.EM_AARCH64))
            path.chmod(0o755)
            result = pdb.probe_desktop_bundles(dist, VERSION)
            self.assertFalse(result.ok)
            self.assertTrue(any(f.kind == "appimage_arch" for f in result.findings))

    def test_appimage_arch_ok(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            dist.mkdir()
            path = dist / f"Mnemosyne-{VERSION}-linux-aarch64.AppImage"
            path.write_bytes(_elf(pdb.EM_AARCH64))
            path.chmod(0o755)
            result = pdb.probe_desktop_bundles(dist, VERSION)
            # mode check may warn on some filesystems; filter to arch only for this assert
            arch_findings = [f for f in result.findings if f.kind.startswith("appimage_arch")]
            self.assertEqual(arch_findings, [])


if __name__ == "__main__":
    unittest.main()

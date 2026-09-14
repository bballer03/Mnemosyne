#!/usr/bin/env python3
"""Tests for scripts/release/normalize_desktop_assets.py (synthetic trees only)."""

from __future__ import annotations

import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "release"))

import normalize_desktop_assets as nda  # noqa: E402

VERSION = "0.3.1"


def _write(path: Path, content: bytes = b"payload") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def _fake_app(root: Path) -> Path:
    app = root / "bundle" / "macos" / "Mnemosyne.app"
    _write(app / "Contents" / "Info.plist", b"<plist/>")
    _write(app / "Contents" / "MacOS" / "Mnemosyne", b"\x7fELF-fake")
    return app


class NormalizeDesktopAssetsTests(unittest.TestCase):
    def test_rename_appimage_in_place(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            src = dist / f"Mnemosyne_{VERSION}_amd64.AppImage"
            _write(src, b"appimage-x64")
            result = nda.normalize_desktop_assets(
                VERSION, dist=dist, in_place=True
            )
            self.assertTrue(result.ok, nda.format_report(result))
            dest = dist / f"Mnemosyne-{VERSION}-linux-x86_64.AppImage"
            self.assertTrue(dest.is_file())
            self.assertFalse(src.exists())
            self.assertEqual(dest.read_bytes(), b"appimage-x64")

    def test_rename_aarch64_and_arm64_appimage(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            _write(dist / f"Mnemosyne_{VERSION}_aarch64.AppImage", b"a")
            result = nda.normalize_desktop_assets(
                VERSION, dist=dist, in_place=True
            )
            self.assertTrue(result.ok, nda.format_report(result))
            self.assertTrue(
                (dist / f"Mnemosyne-{VERSION}-linux-aarch64.AppImage").is_file()
            )

            dist2 = Path(tmp) / "dist2"
            _write(dist2 / f"Mnemosyne_{VERSION}_arm64.AppImage", b"b")
            result2 = nda.normalize_desktop_assets(
                VERSION, dist=dist2, in_place=True
            )
            self.assertTrue(result2.ok, nda.format_report(result2))
            self.assertTrue(
                (dist2 / f"Mnemosyne-{VERSION}-linux-aarch64.AppImage").is_file()
            )

    def test_rename_dmg_to_frozen_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            _write(dist / f"Mnemosyne_{VERSION}_x64.dmg", b"dmg")
            result = nda.normalize_desktop_assets(
                VERSION, dist=dist, in_place=True
            )
            self.assertTrue(result.ok, nda.format_report(result))
            self.assertTrue((dist / f"Mnemosyne-{VERSION}-macos-x64.dmg").is_file())

    def test_copy_windows_msi_and_nsis_identity(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            search = root / "tauri" / "target" / "release" / "bundle"
            _write(search / "msi" / f"Mnemosyne_{VERSION}_x64_en-US.msi", b"msi")
            _write(search / "nsis" / f"Mnemosyne_{VERSION}_x64-setup.exe", b"nsis")
            out = root / "dist-normalized"
            result = nda.normalize_desktop_assets(
                VERSION, search_root=root / "tauri" / "target", out_dir=out
            )
            self.assertTrue(result.ok, nda.format_report(result))
            self.assertTrue((out / f"Mnemosyne_{VERSION}_x64_en-US.msi").is_file())
            self.assertTrue((out / f"Mnemosyne_{VERSION}_x64-setup.exe").is_file())

    def test_zip_macos_app_refuses_python_without_flag(self) -> None:
        """Without ditto, production path must refuse Python zipfile."""
        if nda.shutil.which("ditto") is not None:
            self.skipTest("ditto present; refuse-path not exercised")
        with tempfile.TemporaryDirectory() as tmp:
            search = Path(tmp) / "target"
            _fake_app(search)
            out = Path(tmp) / "out"
            result = nda.normalize_desktop_assets(
                VERSION,
                search_root=search,
                out_dir=out,
                macos_arch="aarch64",
            )
            self.assertFalse(result.ok)
            self.assertTrue(any("ditto" in e.lower() or "python zipfile" in e.lower() for e in result.errors))

    def test_zip_macos_app_to_out_dir_unsafe_python_for_tests(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            search = Path(tmp) / "target"
            app = _fake_app(search)
            out = Path(tmp) / "out"
            result = nda.normalize_desktop_assets(
                VERSION,
                search_root=search,
                out_dir=out,
                macos_arch="aarch64",
                allow_unsafe_python_zip=True,
            )
            self.assertTrue(result.ok, nda.format_report(result))
            dest = out / f"Mnemosyne-{VERSION}-macos-aarch64-app.zip"
            self.assertTrue(dest.is_file())
            with zipfile.ZipFile(dest) as zf:
                names = zf.namelist()
            self.assertTrue(
                any(n.startswith("Mnemosyne.app/Contents/") for n in names),
                names,
            )
            self.assertTrue(app.is_dir())  # source bundle left intact when copying
            self.assertTrue(
                any("backend=" in a.detail for a in result.actions if a.kind == "zip_app")
            )

    def test_copy_mode_leaves_source(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            search = Path(tmp) / "target"
            src = search / "bundle" / "appimage" / f"Mnemosyne_{VERSION}_amd64.AppImage"
            _write(src, b"img")
            out = Path(tmp) / "out"
            result = nda.normalize_desktop_assets(
                VERSION, search_root=search, out_dir=out
            )
            self.assertTrue(result.ok, nda.format_report(result))
            self.assertTrue(src.is_file())
            self.assertTrue(
                (out / f"Mnemosyne-{VERSION}-linux-x86_64.AppImage").is_file()
            )

    def test_cli_json_lists_frozen_names(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            dist.mkdir()
            code = nda.main(
                [
                    "--version",
                    VERSION,
                    "--dist",
                    str(dist),
                    "--in-place",
                    "--json",
                ]
            )
            self.assertEqual(code, 0)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Tests for scripts/release/verify_desktop_assets.py against a synthetic dist/."""

from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "release"))

import verify_desktop_assets as vda  # noqa: E402


VERSION = "0.3.1"


def _write(path: Path, content: bytes = b"payload") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _complete_dist(root: Path, *, with_checksums: bool = True) -> Path:
    dist = root / "dist"
    payloads = {
        f"Mnemosyne-{VERSION}-windows-x64-portable.zip": b"win-portable",
        f"Mnemosyne-{VERSION}-macos-aarch64-app.zip": b"mac-arm-app",
        f"Mnemosyne-{VERSION}-macos-x64-app.zip": b"mac-x64-app",
        f"Mnemosyne-{VERSION}-linux-x86_64.AppImage": b"linux-x64",
        f"Mnemosyne-{VERSION}-linux-aarch64.AppImage": b"linux-arm",
    }
    for name, data in payloads.items():
        _write(dist / name, data)

    if with_checksums:
        lines = [f"{_sha256(data)}  {name}" for name, data in payloads.items()]
        (dist / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return dist


class VerifyDesktopAssetsTests(unittest.TestCase):
    def test_complete_set_ok(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp))
            result = vda.verify_desktop_assets(dist, VERSION, require_checksums=True)
            self.assertTrue(result.ok, vda.format_report(result))
            self.assertEqual(len(result.present_primary), 5)

    def test_missing_primary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp), with_checksums=False)
            (dist / f"Mnemosyne-{VERSION}-windows-x64-portable.zip").unlink()
            result = vda.verify_desktop_assets(dist, VERSION)
            kinds = {f.kind for f in result.findings}
            self.assertIn("missing", kinds)
            missing = [f for f in result.findings if f.kind == "missing"]
            self.assertTrue(
                any("windows-x64-portable" in f.path for f in missing),
                result.findings,
            )

    def test_duplicate_basename(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp), with_checksums=False)
            dup = dist / "extra" / f"Mnemosyne-{VERSION}-windows-x64-portable.zip"
            _write(dup, b"duplicate-copy")
            result = vda.verify_desktop_assets(dist, VERSION)
            self.assertFalse(result.ok)
            self.assertTrue(
                any(f.kind == "duplicate" for f in result.findings),
                result.findings,
            )

    def test_misnamed_asset(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp), with_checksums=False)
            _write(dist / f"Mnemosyne-{VERSION}-win64-portable.zip", b"bad-name")
            result = vda.verify_desktop_assets(dist, VERSION)
            self.assertFalse(result.ok)
            misnamed = [f for f in result.findings if f.kind == "misnamed"]
            self.assertTrue(
                any("win64-portable" in f.path for f in misnamed),
                result.findings,
            )

    def test_checksum_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp), with_checksums=True)
            target = dist / f"Mnemosyne-{VERSION}-linux-x86_64.AppImage"
            target.write_bytes(b"tampered")
            result = vda.verify_desktop_assets(dist, VERSION, require_checksums=True)
            self.assertFalse(result.ok)
            self.assertTrue(
                any(f.kind == "checksum_mismatch" for f in result.findings),
                result.findings,
            )

    def test_require_checksums_when_absent(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp), with_checksums=False)
            result = vda.verify_desktop_assets(dist, VERSION, require_checksums=True)
            self.assertFalse(result.ok)
            self.assertTrue(
                any(f.kind == "checksum_missing" for f in result.findings),
                result.findings,
            )

    def test_cli_exit_codes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp))
            self.assertEqual(
                vda.main(["--dist", str(dist), "--version", VERSION, "--require-checksums"]),
                0,
            )
            (dist / f"Mnemosyne-{VERSION}-macos-x64-app.zip").unlink()
            self.assertEqual(vda.main(["--dist", str(dist), "--version", VERSION]), 1)

    def test_allow_warnings_exits_zero_with_findings(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = _complete_dist(Path(tmp), with_checksums=False)
            (dist / f"Mnemosyne-{VERSION}-macos-x64-app.zip").unlink()
            self.assertEqual(
                vda.main(
                    [
                        "--dist",
                        str(dist),
                        "--version",
                        VERSION,
                        "--allow-warnings",
                    ]
                ),
                0,
            )


if __name__ == "__main__":
    unittest.main()

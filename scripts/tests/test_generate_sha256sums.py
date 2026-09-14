#!/usr/bin/env python3
"""Tests for scripts/release/generate_sha256sums.py against a synthetic dist/."""

from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "release"))

import generate_sha256sums as gen  # noqa: E402


VERSION = "0.3.1"


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class GenerateSha256SumsTests(unittest.TestCase):
    def test_desktop_only_hashes_and_skips_cli(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            dist.mkdir()
            portable = f"Mnemosyne-{VERSION}-windows-x64-portable.zip"
            (dist / portable).write_bytes(b"win-portable")
            (dist / "mnemosyne-cli-x86_64-unknown-linux-gnu.tar.gz").write_bytes(b"cli")

            text, mapping = gen.generate_sha256sums(dist, desktop_only=True)
            self.assertEqual(set(mapping), {portable})
            self.assertIn(_sha256(b"win-portable"), text)
            self.assertNotIn("mnemosyne-cli", text)

    def test_refuse_duplicate_basenames(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            name = f"Mnemosyne-{VERSION}-windows-x64-portable.zip"
            (dist / name).parent.mkdir(parents=True, exist_ok=True)
            (dist / name).write_bytes(b"a")
            nested = dist / "nested"
            nested.mkdir()
            (nested / name).write_bytes(b"b")
            with self.assertRaises(ValueError):
                gen.generate_sha256sums(dist, desktop_only=True)

    def test_cli_writes_sums_and_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            dist = Path(tmp) / "dist"
            dist.mkdir()
            name = f"Mnemosyne-{VERSION}-linux-x86_64.AppImage"
            (dist / name).write_bytes(b"appimage")
            code = gen.main(
                [
                    "--dist",
                    str(dist),
                    "--desktop-only",
                    "--version",
                    VERSION,
                    "--manifest",
                    "auto",
                ]
            )
            self.assertEqual(code, 0)
            sums = (dist / "SHA256SUMS").read_text(encoding="utf-8")
            self.assertIn(name, sums)
            manifest = json.loads((dist / "asset-manifest.json").read_text(encoding="utf-8"))
            self.assertFalse(manifest["launch_tested"])
            self.assertEqual(manifest["version"], VERSION)
            self.assertEqual(manifest["files"][0]["name"], name)


if __name__ == "__main__":
    unittest.main()

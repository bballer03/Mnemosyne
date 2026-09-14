#!/usr/bin/env python3
"""Verify Mnemosyne desktop release asset names and optional checksums (M21.A).

WSL-safe: inspects a dist/ tree only. Does not launch installers or claim GUI smoke.

Primary click-to-run assets (required by default):
  Mnemosyne-<version>-windows-x64-portable.zip
  Mnemosyne-<version>-macos-aarch64-app.zip
  Mnemosyne-<version>-macos-x64-app.zip
  Mnemosyne-<version>-linux-x86_64.AppImage
  Mnemosyne-<version>-linux-aarch64.AppImage

Checksums: when SHA256SUMS (or --checksums) is present, every listed file under
dist/ must match. --require-checksums fails if no checksum file exists.

Exit codes:
  0 — no findings (or warnings only when --allow-warnings)
  1 — missing / duplicate / misnamed / checksum failure
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path

CHECKSUM_CANDIDATES = ("SHA256SUMS", "SHA256SUMS.txt", "SHA256SUMS.desktop", "checksums.sha256")

# Files that are expected alongside assets and never treated as misnamed.
IGNORE_BASENAMES = frozenset(
    {
        "SHA256SUMS",
        "SHA256SUMS.txt",
        "SHA256SUMS.desktop",
        "checksums.sha256",
        "asset-manifest.json",
        ".gitkeep",
    }
)


@dataclass(frozen=True)
class AssetSpec:
    role: str
    filename: str
    required: bool = True


@dataclass
class Finding:
    kind: str  # missing | duplicate | misnamed | checksum_missing | checksum_mismatch | extra_checksum
    path: str
    detail: str


@dataclass
class VerifyResult:
    version: str
    dist: str
    expected_primary: list[str] = field(default_factory=list)
    present_primary: list[str] = field(default_factory=list)
    findings: list[Finding] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.findings


def primary_specs(version: str) -> list[AssetSpec]:
    return [
        AssetSpec("windows-x64-portable", f"Mnemosyne-{version}-windows-x64-portable.zip"),
        AssetSpec("macos-aarch64-app", f"Mnemosyne-{version}-macos-aarch64-app.zip"),
        AssetSpec("macos-x64-app", f"Mnemosyne-{version}-macos-x64-app.zip"),
        AssetSpec("linux-x86_64-appimage", f"Mnemosyne-{version}-linux-x86_64.AppImage"),
        AssetSpec("linux-aarch64-appimage", f"Mnemosyne-{version}-linux-aarch64.AppImage"),
    ]


def fallback_filenames(version: str) -> set[str]:
    """Documented installer alternatives — allowed, not required for the portable gate."""
    return {
        f"Mnemosyne_{version}_x64_en-US.msi",
        f"Mnemosyne_{version}_x64-setup.exe",
        # Frozen fallback DMG names (normalize_desktop_assets.py).
        f"Mnemosyne-{version}-macos-aarch64.dmg",
        f"Mnemosyne-{version}-macos-x64.dmg",
        # Transitional Tauri DMG names (pre-normalize / accidental upload).
        f"Mnemosyne_{version}_aarch64.dmg",
        f"Mnemosyne_{version}_x64.dmg",
        f"Mnemosyne_{version}_amd64.deb",
        f"Mnemosyne_{version}_arm64.deb",
        f"Mnemosyne-{version}-1.x86_64.rpm",
        f"Mnemosyne-{version}-1.aarch64.rpm",
        # Transitional Tauri AppImage names (allowed if normalize missed a file).
        f"Mnemosyne_{version}_amd64.AppImage",
        f"Mnemosyne_{version}_aarch64.AppImage",
        f"Mnemosyne_{version}_arm64.AppImage",
    }


def _looks_like_desktop_asset(name: str) -> bool:
    lower = name.lower()
    if name in IGNORE_BASENAMES:
        return False
    if not name.startswith("Mnemosyne") and not name.startswith("mnemosyne"):
        return False
    return any(
        token in lower
        for token in (
            "portable",
            "appimage",
            ".dmg",
            ".msi",
            "setup",
            "-app.zip",
            "_app.zip",
            ".deb",
            ".rpm",
            ".exe",
            ".app",
        )
    ) or lower.endswith(".zip")


def list_dist_files(dist: Path) -> list[Path]:
    if not dist.is_dir():
        raise FileNotFoundError(f"dist directory not found: {dist}")
    files: list[Path] = []
    for path in sorted(dist.rglob("*")):
        if path.is_file():
            files.append(path)
    return files


def resolve_checksum_file(dist: Path, explicit: Path | None) -> Path | None:
    if explicit is not None:
        return explicit
    for name in CHECKSUM_CANDIDATES:
        candidate = dist / name
        if candidate.is_file():
            return candidate
    return None


def parse_sha256sums(path: Path) -> dict[str, str]:
    """Parse GNU sha256sum lines: `<hex>  <name>` or `<hex> *<name>`."""
    mapping: dict[str, str] = {}
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        match = re.match(r"^([0-9a-fA-F]{64})\s+\*?(.+)$", line)
        if not match:
            raise ValueError(f"{path}:{lineno}: invalid sha256sum line: {raw!r}")
        digest, name = match.group(1).lower(), match.group(2).strip()
        # Allow "dist/foo" style; compare on basename.
        mapping[Path(name).name] = digest
    return mapping


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def verify_desktop_assets(
    dist: Path,
    version: str,
    *,
    require_checksums: bool = False,
    checksums: Path | None = None,
) -> VerifyResult:
    result = VerifyResult(version=version, dist=str(dist))
    specs = primary_specs(version)
    result.expected_primary = [spec.filename for spec in specs]
    allowed = {spec.filename for spec in specs} | fallback_filenames(version) | set(IGNORE_BASENAMES)

    try:
        files = list_dist_files(dist)
    except FileNotFoundError as exc:
        result.findings.append(Finding("missing", str(dist), str(exc)))
        return result

    by_basename: dict[str, list[Path]] = {}
    for path in files:
        by_basename.setdefault(path.name, []).append(path)

    for name, paths in sorted(by_basename.items()):
        if len(paths) > 1:
            joined = ", ".join(str(p.relative_to(dist)) for p in paths)
            result.findings.append(
                Finding(
                    "duplicate",
                    name,
                    f"basename appears {len(paths)} times under dist/: {joined}",
                )
            )

    present = set(by_basename)
    for spec in specs:
        if spec.filename in present:
            result.present_primary.append(spec.filename)
        elif spec.required:
            result.findings.append(
                Finding("missing", spec.filename, f"required primary asset missing ({spec.role})")
            )

    for name in sorted(present):
        if name in IGNORE_BASENAMES:
            continue
        if name in allowed:
            continue
        if _looks_like_desktop_asset(name):
            result.findings.append(
                Finding(
                    "misnamed",
                    name,
                    "matches desktop-asset heuristics but is not a frozen primary/fallback name",
                )
            )

    checksum_path = resolve_checksum_file(dist, checksums)
    if require_checksums and checksum_path is None:
        result.findings.append(
            Finding(
                "checksum_missing",
                "SHA256SUMS",
                "checksum file required but none of SHA256SUMS / SHA256SUMS.txt / checksums.sha256 found",
            )
        )
    elif checksum_path is not None:
        try:
            expected_hashes = parse_sha256sums(checksum_path)
        except ValueError as exc:
            result.findings.append(Finding("checksum_mismatch", str(checksum_path), str(exc)))
            return result

        for name, expected in sorted(expected_hashes.items()):
            paths = by_basename.get(name)
            if not paths:
                result.findings.append(
                    Finding(
                        "checksum_missing",
                        name,
                        f"listed in {checksum_path.name} but absent from dist/",
                    )
                )
                continue
            actual = sha256_file(paths[0])
            if actual != expected:
                result.findings.append(
                    Finding(
                        "checksum_mismatch",
                        name,
                        f"expected {expected}, got {actual}",
                    )
                )

        # Primary assets present on disk should be covered when a checksum file exists.
        for name in result.present_primary:
            if name not in expected_hashes:
                result.findings.append(
                    Finding(
                        "checksum_missing",
                        name,
                        f"present in dist/ but not listed in {checksum_path.name}",
                    )
                )

    return result


def format_report(result: VerifyResult) -> str:
    lines = [
        f"M21.A desktop asset verify — version={result.version} dist={result.dist}",
        f"primary expected: {len(result.expected_primary)} present: {len(result.present_primary)}",
    ]
    if not result.findings:
        lines.append("OK — no missing/duplicate/misnamed/checksum findings.")
        return "\n".join(lines)

    lines.append(f"FINDINGS ({len(result.findings)}):")
    for finding in result.findings:
        lines.append(f"  [{finding.kind}] {finding.path}: {finding.detail}")
    lines.append(
        "Note: verifier success is not launch-tested proof (WSL/native GUI smoke is out of scope)."
    )
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", type=Path, required=True, help="Directory of release assets")
    parser.add_argument("--version", required=True, help="Release version without leading v")
    parser.add_argument(
        "--checksums",
        type=Path,
        default=None,
        help="Optional sha256sums file (default: look for SHA256SUMS under --dist)",
    )
    parser.add_argument(
        "--require-checksums",
        action="store_true",
        help="Fail when no checksum file is present",
    )
    parser.add_argument(
        "--allow-warnings",
        action="store_true",
        help=(
            "Print findings but exit 0 (warn mode for CI until frozen names + "
            "strict gate land). Never claims launch success."
        ),
    )
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON")
    args = parser.parse_args(argv)

    result = verify_desktop_assets(
        args.dist,
        args.version,
        require_checksums=args.require_checksums,
        checksums=args.checksums,
    )

    if args.json:
        payload = {
            "ok": result.ok,
            "version": result.version,
            "dist": result.dist,
            "expected_primary": result.expected_primary,
            "present_primary": result.present_primary,
            "findings": [asdict(f) for f in result.findings],
            "allow_warnings": args.allow_warnings,
            "launch_tested": False,
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(format_report(result))
        if args.allow_warnings and not result.ok:
            print(
                "WARN mode (--allow-warnings): findings above do not fail the process; "
                "not launch-tested.",
                file=sys.stderr,
            )

    if result.ok:
        return 0
    return 0 if args.allow_warnings else 1


if __name__ == "__main__":
    sys.exit(main())

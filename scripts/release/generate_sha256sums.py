#!/usr/bin/env python3
"""Generate desktop-scoped SHA256SUMS for release assets under dist/ (M21.B/C/D).

WSL-safe: hashes files on disk only. Does not launch installers or claim GUI smoke.
Writes GNU sha256sum lines (`<hex>  <basename>`) suitable for verify_desktop_assets.py.

Default output basename is SHA256SUMS (verifier compatibility). File header comments
and asset-manifest.json declare scope=desktop-heuristic and unsigned provenance.
This is not an attestable signed SBOM — CI identity fields are informational only.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from pathlib import Path

# Meta files we never list as hashed payloads.
SKIP_BASENAMES = frozenset(
    {
        "SHA256SUMS",
        "SHA256SUMS.txt",
        "SHA256SUMS.desktop",
        "checksums.sha256",
        "asset-manifest.json",
        ".gitkeep",
    }
)

# Heuristic for "desktop" when --desktop-only is set (M21 primary + fallbacks).
_DESKTOP_TOKENS = (
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


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def _looks_desktop(name: str) -> bool:
    lower = name.lower()
    if not (name.startswith("Mnemosyne") or name.startswith("mnemosyne")):
        return False
    return any(token in lower for token in _DESKTOP_TOKENS) or lower.endswith(".zip")


def collect_files(dist: Path, *, desktop_only: bool) -> list[Path]:
    if not dist.is_dir():
        raise FileNotFoundError(f"dist directory not found: {dist}")
    files: list[Path] = []
    for path in sorted(dist.rglob("*")):
        if not path.is_file():
            continue
        if path.name in SKIP_BASENAMES:
            continue
        if desktop_only and not _looks_desktop(path.name):
            continue
        files.append(path)
    return files


def provenance_from_env() -> dict[str, str | None]:
    """Informational CI/git identity — not a signed attestation."""
    return {
        "git_sha": os.environ.get("GITHUB_SHA") or os.environ.get("GIT_SHA"),
        "github_run_id": os.environ.get("GITHUB_RUN_ID"),
        "github_run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
        "github_repository": os.environ.get("GITHUB_REPOSITORY"),
        "github_workflow": os.environ.get("GITHUB_WORKFLOW"),
        "github_ref": os.environ.get("GITHUB_REF"),
    }


def generate_sha256sums(
    dist: Path,
    *,
    desktop_only: bool = False,
) -> tuple[str, dict[str, str]]:
    """Return (SHA256SUMS text with header, basename→digest map). Uses basenames."""
    files = collect_files(dist, desktop_only=desktop_only)
    if not files:
        raise ValueError(f"no files to hash under {dist} (desktop_only={desktop_only})")

    by_basename: dict[str, list[Path]] = {}
    for path in files:
        by_basename.setdefault(path.name, []).append(path)

    dupes = {name: paths for name, paths in by_basename.items() if len(paths) > 1}
    if dupes:
        detail = "; ".join(
            f"{name} ({len(paths)} copies)" for name, paths in sorted(dupes.items())
        )
        raise ValueError(f"duplicate basenames under dist/; refuse ambiguous SHA256SUMS: {detail}")

    mapping: dict[str, str] = {}
    lines: list[str] = []
    for name in sorted(by_basename):
        path = by_basename[name][0]
        digest = sha256_file(path)
        mapping[name] = digest
        lines.append(f"{digest}  {name}")

    scope = "desktop-heuristic" if desktop_only else "all-files-under-dist"
    header = [
        f"# Mnemosyne {scope} SHA256SUMS (unsigned; not launch proof)",
        f"# schema=mnemosyne.desktop-sha256sums.v1 scope={scope}",
        "# Completeness: only files matched by the generator filters at write time.",
        "# Not a signed attestation; verify against asset-manifest.json provenance fields.",
    ]
    return "\n".join(header + lines) + "\n", mapping


def write_manifest(
    path: Path,
    version: str | None,
    mapping: dict[str, str],
    *,
    desktop_only: bool,
) -> None:
    scope = "desktop-heuristic" if desktop_only else "all-files-under-dist"
    payload = {
        "schema": "mnemosyne.desktop-asset-manifest.v1",
        "version": version,
        "scope": scope,
        "launch_tested": False,
        "signed": False,
        "attestation": None,
        "checksum_algorithm": "sha256",
        "provenance": provenance_from_env(),
        "files": [{"name": name, "sha256": digest} for name, digest in sorted(mapping.items())],
        "notes": [
            "Checksums cover the filtered desktop (or all) files present when generated.",
            "Unsigned: no Sigstore/GPG signature is attached by this generator.",
            "launch_tested remains false until native-host evidence lands (M21.F).",
        ],
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", type=Path, required=True, help="Directory of release assets")
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="SHA256SUMS path (default: <dist>/SHA256SUMS)",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=None,
        help="Optional asset-manifest.json path (default: <dist>/asset-manifest.json when set to 'auto')",
    )
    parser.add_argument(
        "--version",
        default=None,
        help="Optional release version recorded in asset-manifest.json",
    )
    parser.add_argument(
        "--desktop-only",
        action="store_true",
        help="Hash only Mnemosyne desktop-like filenames (portable/AppImage/dmg/msi/…)",
    )
    args = parser.parse_args(argv)

    try:
        text, mapping = generate_sha256sums(args.dist, desktop_only=args.desktop_only)
    except (FileNotFoundError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    output = args.output if args.output is not None else args.dist / "SHA256SUMS"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(text, encoding="utf-8")
    print(f"Wrote {output} ({len(mapping)} files; unsigned desktop-scope checksums)")

    manifest = args.manifest
    if manifest is not None:
        if str(manifest) == "auto":
            manifest = args.dist / "asset-manifest.json"
        write_manifest(manifest, args.version, mapping, desktop_only=args.desktop_only)
        print(f"Wrote {manifest}")

    return 0


if __name__ == "__main__":
    sys.exit(main())

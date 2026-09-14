#!/usr/bin/env python3
"""Inspect desktop zip archives for credential-like paths (M21 honesty gate).

WSL-safe: reads zip central directories only. Does not extract or launch.
Refuses release uploads that embed obvious secrets or build-path debris.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import zipfile
from dataclasses import asdict, dataclass, field
from pathlib import Path

# Basename / path fragment denylist (case-insensitive).
# Match credential-like basenames OR extensions on ordinary filenames (e.g. private-key.pem).
SECRET_BASENAME_RE = re.compile(
    r"(^|/|\\)"
    r"("
    r"\.env(\.[^/\\]+)?|"
    r"id_rsa|"
    r"id_ed25519|"
    r"id_ecdsa|"
    r"credentials(\.json)?|"
    r"secrets?(\.json|\.yml|\.yaml|\.toml)?|"
    r"api[_-]?keys?|"
    r"\.aws/credentials|"
    r"keystore|"
    r"private[_-]?key([._-][^/\\]+)?|"
    r"[^/\\]+\.(pem|p12|pfx|key)$"
    r")"
    r"($|/|\\)",
    re.IGNORECASE,
)

# Absolute / home build paths that should never ship inside portable zips.
BUILD_PATH_RE = re.compile(
    r"(^|/|\\)(Users|home|home/runner|github/workspace)(/|\\)|"
    r"[A-Za-z]:\\\\Users\\\\|"
    r"/var/folders/",
    re.IGNORECASE,
)

@dataclass
class ArchiveFinding:
    archive: str
    member: str
    kind: str  # secret_path | build_path
    detail: str


@dataclass
class InspectResult:
    archives: list[str] = field(default_factory=list)
    findings: list[ArchiveFinding] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.findings


def _is_desktop_zip(name: str) -> bool:
    lower = name.lower()
    if not (name.startswith("Mnemosyne") or name.startswith("mnemosyne")):
        return False
    return lower.endswith(".zip") and any(
        token in lower for token in ("portable", "-app.zip", "_app.zip", "macos")
    )


def inspect_zip_members(archive: Path) -> list[ArchiveFinding]:
    findings: list[ArchiveFinding] = []
    with zipfile.ZipFile(archive) as zf:
        for info in zf.infolist():
            member = info.filename.replace("\\", "/")
            if SECRET_BASENAME_RE.search(member):
                findings.append(
                    ArchiveFinding(
                        str(archive),
                        member,
                        "secret_path",
                        "member path matches credential denylist",
                    )
                )
            if BUILD_PATH_RE.search(member):
                findings.append(
                    ArchiveFinding(
                        str(archive),
                        member,
                        "build_path",
                        "member path looks like absolute host/build path",
                    )
                )
    return findings


def inspect_desktop_archives(dist: Path) -> InspectResult:
    result = InspectResult()
    if not dist.is_dir():
        result.findings.append(
            ArchiveFinding(str(dist), "", "secret_path", f"dist not a directory: {dist}")
        )
        return result

    for path in sorted(dist.rglob("*.zip")):
        if not path.is_file() or not _is_desktop_zip(path.name):
            continue
        result.archives.append(str(path))
        try:
            result.findings.extend(inspect_zip_members(path))
        except zipfile.BadZipFile as exc:
            result.findings.append(
                ArchiveFinding(str(path), "", "secret_path", f"unreadable zip: {exc}")
            )
    return result


def format_report(result: InspectResult) -> str:
    lines = [
        f"M21 archive inspect — archives={len(result.archives)} findings={len(result.findings)}",
    ]
    for finding in result.findings:
        lines.append(
            f"  [{finding.kind}] {finding.archive} :: {finding.member} ({finding.detail})"
        )
    if result.ok:
        lines.append("OK — no credential/build-path members in scanned desktop zips.")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", type=Path, required=True, help="Directory of release assets")
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON")
    args = parser.parse_args(argv)

    result = inspect_desktop_archives(args.dist)
    if args.json:
        print(
            json.dumps(
                {
                    "ok": result.ok,
                    "archives": result.archives,
                    "findings": [asdict(f) for f in result.findings],
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(format_report(result))
    return 0 if result.ok else 1


if __name__ == "__main__":
    sys.exit(main())

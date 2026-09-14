#!/usr/bin/env python3
"""Docs consistency checks for M21 portable install wording (21.E).

Fails when required frozen asset-name examples or “No Java/JVM” honesty phrases
are missing from the listed docs. Does not claim launch proof.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REQUIRED_PHRASES = (
    "No Java",
    "WebView2",
    "AppImage",
)

REQUIRED_NAME_FRAGMENTS = (
    "windows-x64-portable.zip",
    "macos-aarch64-app.zip",
    "linux-x86_64.AppImage",
)

DEFAULT_DOCS = (
    "README.md",
    "docs/user-guide.md",
    "docs/troubleshooting.md",
    "SECURITY.md",
    "docs/design/milestone-21-eclipse-style-installability.md",
)


def check_docs(repo_root: Path, rel_paths: list[str]) -> list[str]:
    errors: list[str] = []
    # README must itself name the frozen primaries (not only via design doc).
    readme = repo_root / "README.md"
    if readme.is_file():
        text = readme.read_text(encoding="utf-8")
        for frag in REQUIRED_NAME_FRAGMENTS:
            if frag not in text:
                errors.append(f"README.md: missing frozen asset fragment {frag!r}")
        for phrase in REQUIRED_PHRASES:
            if phrase not in text:
                errors.append(f"README.md: missing phrase {phrase!r}")
    else:
        errors.append("missing doc: README.md")

    combined = ""
    for rel in rel_paths:
        path = repo_root / rel
        if not path.is_file():
            errors.append(f"missing doc: {rel}")
            continue
        text = path.read_text(encoding="utf-8")
        combined += "\n" + text
        if rel in ("docs/user-guide.md", "docs/troubleshooting.md"):
            for phrase in ("No Java", "WebView2"):
                if phrase not in text:
                    errors.append(f"{rel}: missing phrase {phrase!r}")
    for frag in REQUIRED_NAME_FRAGMENTS:
        if frag not in combined:
            errors.append(f"no doc mentions asset fragment {frag!r}")
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parents[2],
    )
    args = parser.parse_args(argv)
    errors = check_docs(args.repo_root, list(DEFAULT_DOCS))
    if errors:
        print("M21.E docs check FAILED:")
        for err in errors:
            print(f"  - {err}")
        return 1
    print("M21.E docs check OK — required phrases/asset fragments present (not launch proof).")
    return 0


if __name__ == "__main__":
    sys.exit(main())

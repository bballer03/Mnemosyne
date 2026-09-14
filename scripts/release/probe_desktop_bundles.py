#!/usr/bin/env python3
"""Probe macOS app zips and Linux AppImages without launching (M21.C/D).

WSL-safe: reads zip central directories / file headers only. Never claims GUI smoke.
When an asset is present, checks structural honesty gates; missing assets are skipped
(name presence remains verify_desktop_assets.py's job).
"""

from __future__ import annotations

import argparse
import json
import plistlib
import re
import struct
import sys
import zipfile
from dataclasses import asdict, dataclass, field
from pathlib import Path

CFBundle_ID_RE = re.compile(r"mnemosyne", re.IGNORECASE)

# ELF e_machine values
EM_X86_64 = 62
EM_AARCH64 = 183


@dataclass
class ProbeFinding:
    asset: str
    kind: str
    detail: str


@dataclass
class ProbeResult:
    probed: list[str] = field(default_factory=list)
    findings: list[ProbeFinding] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.findings


def _read_info_plist(zf: zipfile.ZipFile) -> tuple[str | None, dict | None]:
    """Return (member_path, parsed_plist) for Mnemosyne.app/Contents/Info.plist."""
    candidates = [
        n
        for n in zf.namelist()
        if n.replace("\\", "/").endswith("Mnemosyne.app/Contents/Info.plist")
        or n.replace("\\", "/") == "Contents/Info.plist"
    ]
    # Prefer nested Mnemosyne.app path (ditto --keepParent).
    preferred = sorted(
        candidates,
        key=lambda n: (0 if "Mnemosyne.app/Contents/Info.plist" in n.replace("\\", "/") else 1, n),
    )
    if not preferred:
        return None, None
    member = preferred[0]
    raw = zf.read(member)
    try:
        return member, plistlib.loads(raw)
    except Exception as exc:  # noqa: BLE001 — surface as finding
        raise ValueError(f"Info.plist parse failed: {exc}") from exc


def probe_macos_app_zip(path: Path, *, expected_version: str | None) -> list[ProbeFinding]:
    findings: list[ProbeFinding] = []
    try:
        with zipfile.ZipFile(path) as zf:
            names = {n.replace("\\", "/") for n in zf.namelist()}
            try:
                member, plist = _read_info_plist(zf)
            except ValueError as exc:
                findings.append(ProbeFinding(path.name, "info_plist", str(exc)))
                return findings
            if member is None or plist is None:
                findings.append(
                    ProbeFinding(
                        path.name,
                        "info_plist",
                        "Mnemosyne.app/Contents/Info.plist missing from archive",
                    )
                )
                return findings

            bundle_id = str(plist.get("CFBundleIdentifier", ""))
            if not CFBundle_ID_RE.search(bundle_id):
                findings.append(
                    ProbeFinding(
                        path.name,
                        "bundle_id",
                        f"CFBundleIdentifier={bundle_id!r} does not mention mnemosyne",
                    )
                )

            exe_name = str(plist.get("CFBundleExecutable", "")).strip()
            if not exe_name:
                findings.append(
                    ProbeFinding(path.name, "executable", "CFBundleExecutable missing/empty")
                )
            else:
                # ditto keeps Mnemosyne.app/ prefix
                prefix = "Mnemosyne.app/Contents/MacOS/"
                alt = "Contents/MacOS/"
                exe_paths = {prefix + exe_name, alt + exe_name}
                if not any(p in names for p in exe_paths):
                    findings.append(
                        ProbeFinding(
                            path.name,
                            "executable",
                            f"MacOS/{exe_name} not present in zip (looked for {sorted(exe_paths)})",
                        )
                    )

            if expected_version is not None:
                short = str(plist.get("CFBundleShortVersionString", ""))
                # Allow tag with or without leading v; exact match preferred.
                ok_versions = {expected_version, f"v{expected_version}"}
                if short and short not in ok_versions and expected_version not in short:
                    findings.append(
                        ProbeFinding(
                            path.name,
                            "version",
                            f"CFBundleShortVersionString={short!r} does not match release {expected_version!r}",
                        )
                    )
    except zipfile.BadZipFile as exc:
        findings.append(ProbeFinding(path.name, "info_plist", f"unreadable zip: {exc}"))
    return findings


def _elf_machine(path: Path) -> int | None:
    """Return ELF e_machine or None if not ELF."""
    with path.open("rb") as handle:
        header = handle.read(20)
    if len(header) < 20 or header[:4] != b"\x7fELF":
        return None
    return struct.unpack_from("<H", header, 18)[0]


def probe_appimage(path: Path, *, expected_arch: str) -> list[ProbeFinding]:
    findings: list[ProbeFinding] = []
    machine = _elf_machine(path)
    if machine is None:
        findings.append(
            ProbeFinding(path.name, "appimage_elf", "file does not start with ELF magic")
        )
        return findings

    expected_machine = EM_X86_64 if expected_arch == "x86_64" else EM_AARCH64
    if machine != expected_machine:
        findings.append(
            ProbeFinding(
                path.name,
                "appimage_arch",
                f"ELF e_machine={machine} expected {expected_machine} for {expected_arch}",
            )
        )

    # Executable bit is best-effort (Windows/WSL mounts may not preserve mode).
    try:
        mode = path.stat().st_mode
        if (mode & 0o111) == 0:
            findings.append(
                ProbeFinding(
                    path.name,
                    "appimage_mode",
                    f"file mode {oct(mode)} has no execute bits (may be host/fs limitation)",
                )
            )
    except OSError as exc:
        findings.append(ProbeFinding(path.name, "appimage_mode", str(exc)))
    return findings


def probe_desktop_bundles(dist: Path, version: str) -> ProbeResult:
    result = ProbeResult()
    if not dist.is_dir():
        result.findings.append(ProbeFinding(str(dist), "missing", "dist is not a directory"))
        return result

    checks: list[tuple[Path, str]] = []
    mapping = {
        f"Mnemosyne-{version}-macos-aarch64-app.zip": ("macos", None),
        f"Mnemosyne-{version}-macos-x64-app.zip": ("macos", None),
        f"Mnemosyne-{version}-linux-x86_64.AppImage": ("appimage", "x86_64"),
        f"Mnemosyne-{version}-linux-aarch64.AppImage": ("appimage", "aarch64"),
    }
    for path in sorted(dist.rglob("*")):
        if not path.is_file() or path.name not in mapping:
            continue
        kind, arch = mapping[path.name]
        result.probed.append(str(path))
        if kind == "macos":
            result.findings.extend(probe_macos_app_zip(path, expected_version=version))
        else:
            assert arch is not None
            result.findings.extend(probe_appimage(path, expected_arch=arch))
        checks.append((path, kind))

    return result


def format_report(result: ProbeResult) -> str:
    lines = [
        f"M21.C/D desktop bundle probe — probed={len(result.probed)} findings={len(result.findings)}",
        "Note: structural probe only; not launch-tested.",
    ]
    for finding in result.findings:
        lines.append(f"  [{finding.kind}] {finding.asset}: {finding.detail}")
    if result.ok:
        lines.append("OK — present macOS/AppImage assets passed structural probes.")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--version", required=True, help="Release version without leading v")
    parser.add_argument(
        "--allow-warnings",
        action="store_true",
        help="Print findings but exit 0 (CI soft gate until 21.F)",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    result = probe_desktop_bundles(args.dist, args.version)
    if args.json:
        print(
            json.dumps(
                {
                    "ok": result.ok,
                    "probed": result.probed,
                    "findings": [asdict(f) for f in result.findings],
                    "launch_tested": False,
                },
                indent=2,
                sort_keys=True,
            )
        )
    else:
        print(format_report(result))

    if result.ok or args.allow_warnings:
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())

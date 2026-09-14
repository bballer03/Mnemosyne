#!/usr/bin/env python3
"""Normalize Tauri desktop artifact names to M21 frozen names (21.C / 21.D).

WSL-safe: renames/copies/zips on disk only. Does not launch apps or claim GUI smoke.

Frozen primaries (required by verify_desktop_assets.py):
  Mnemosyne-<version>-macos-aarch64-app.zip
  Mnemosyne-<version>-macos-x64-app.zip
  Mnemosyne-<version>-linux-x86_64.AppImage
  Mnemosyne-<version>-linux-aarch64.AppImage

Frozen fallback DMGs (allowed, not required for the portable gate):
  Mnemosyne-<version>-macos-aarch64.dmg
  Mnemosyne-<version>-macos-x64.dmg

Typical Tauri sources (productName=Mnemosyne):
  Mnemosyne_<version>_amd64.AppImage / _aarch64.AppImage / _arm64.AppImage
  Mnemosyne_<version>_aarch64.dmg / _x64.dmg
  **/bundle/macos/Mnemosyne.app  → zipped to frozen app zip when --macos-arch is set
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import zipfile
from dataclasses import asdict, dataclass, field
from pathlib import Path


@dataclass
class NormalizeAction:
    kind: str  # rename | copy | zip_app | skip
    source: str
    dest: str
    detail: str = ""


@dataclass
class NormalizeResult:
    version: str
    actions: list[NormalizeAction] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not self.errors


def frozen_primary_names(version: str) -> dict[str, str]:
    return {
        "macos-aarch64-app": f"Mnemosyne-{version}-macos-aarch64-app.zip",
        "macos-x64-app": f"Mnemosyne-{version}-macos-x64-app.zip",
        "linux-x86_64-appimage": f"Mnemosyne-{version}-linux-x86_64.AppImage",
        "linux-aarch64-appimage": f"Mnemosyne-{version}-linux-aarch64.AppImage",
    }


def frozen_fallback_dmg_names(version: str) -> dict[str, str]:
    return {
        "macos-aarch64-dmg": f"Mnemosyne-{version}-macos-aarch64.dmg",
        "macos-x64-dmg": f"Mnemosyne-{version}-macos-x64.dmg",
    }


def tauri_rename_map(version: str) -> dict[str, str]:
    """Map known Tauri basenames → frozen primary or fallback basenames."""
    primaries = frozen_primary_names(version)
    dmgs = frozen_fallback_dmg_names(version)
    return {
        # Linux AppImage (Tauri / linuxdeploy amd64; aarch64 and transitional arm64)
        f"Mnemosyne_{version}_amd64.AppImage": primaries["linux-x86_64-appimage"],
        f"Mnemosyne_{version}_x86_64.AppImage": primaries["linux-x86_64-appimage"],
        f"Mnemosyne_{version}_aarch64.AppImage": primaries["linux-aarch64-appimage"],
        f"Mnemosyne_{version}_arm64.AppImage": primaries["linux-aarch64-appimage"],
        # macOS DMG (fallback installers; primary remains app zip)
        f"Mnemosyne_{version}_aarch64.dmg": dmgs["macos-aarch64-dmg"],
        f"Mnemosyne_{version}_arm64.dmg": dmgs["macos-aarch64-dmg"],
        f"Mnemosyne_{version}_x64.dmg": dmgs["macos-x64-dmg"],
        f"Mnemosyne_{version}_x86_64.dmg": dmgs["macos-x64-dmg"],
    }


def _unique_files(root: Path) -> list[Path]:
    if not root.is_dir():
        return []
    return sorted(p for p in root.rglob("*") if p.is_file())


def _find_app_bundles(root: Path) -> list[Path]:
    if not root.is_dir():
        return []
    apps = [
        p
        for p in root.rglob("Mnemosyne.app")
        if p.is_dir() and (p / "Contents").is_dir()
    ]
    return sorted(apps)


def _zip_app_bundle(app_dir: Path, dest_zip: Path) -> None:
    """Zip .app preserving relative paths under the bundle root (Contents/...)."""
    dest_zip.parent.mkdir(parents=True, exist_ok=True)
    if dest_zip.exists():
        dest_zip.unlink()
    # zipfile stores files; we prefix with Mnemosyne.app/ so unzip yields a bundle.
    prefix = app_dir.name
    with zipfile.ZipFile(dest_zip, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for path in sorted(app_dir.rglob("*")):
            if path.is_file():
                arcname = str(Path(prefix) / path.relative_to(app_dir))
                zf.write(path, arcname=arcname)


def _place(src: Path, dest: Path, *, in_place: bool, dry_run: bool) -> str:
    """Rename or copy src → dest. Returns action kind."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dry_run:
        return "rename" if in_place and src.parent == dest.parent else "copy"
    if dest.exists() and dest.resolve() == src.resolve():
        return "skip"
    if dest.exists():
        dest.unlink()
    if in_place and src.parent == dest.parent:
        src.rename(dest)
        return "rename"
    shutil.copy2(src, dest)
    if in_place and src.resolve() != dest.resolve():
        src.unlink()
        return "rename"
    return "copy"


def normalize_desktop_assets(
    version: str,
    *,
    dist: Path | None = None,
    search_root: Path | None = None,
    out_dir: Path | None = None,
    macos_arch: str | None = None,
    in_place: bool = False,
    dry_run: bool = False,
) -> NormalizeResult:
    result = NormalizeResult(version=version)
    rename_map = tauri_rename_map(version)
    primaries = frozen_primary_names(version)

    scan_roots: list[Path] = []
    if dist is not None:
        scan_roots.append(dist)
    if search_root is not None:
        scan_roots.append(search_root)
    if not scan_roots:
        result.errors.append("must provide --dist and/or --search-root")
        return result

    if in_place:
        if dist is None:
            result.errors.append("--in-place requires --dist")
            return result
        dest_root = dist
    else:
        if out_dir is None:
            result.errors.append("non-in-place mode requires --out-dir")
            return result
        dest_root = out_dir

    # 1) Rename/copy mapped Tauri basenames.
    seen_sources: set[Path] = set()
    for root in scan_roots:
        for path in _unique_files(root):
            target_name = rename_map.get(path.name)
            if target_name is None:
                continue
            if path in seen_sources:
                continue
            seen_sources.add(path)
            dest = dest_root / target_name
            if dry_run:
                kind = "rename" if in_place else "copy"
                result.actions.append(
                    NormalizeAction(kind, str(path), str(dest), "dry-run")
                )
                continue
            try:
                kind = _place(path, dest, in_place=in_place, dry_run=False)
                result.actions.append(
                    NormalizeAction(kind, str(path), str(dest), "tauri→frozen")
                )
            except OSError as exc:
                result.errors.append(f"{path} → {dest}: {exc}")

    # 2) Zip macOS .app → frozen app zip when arch is known.
    if macos_arch is not None:
        if macos_arch not in ("aarch64", "x64"):
            result.errors.append("--macos-arch must be aarch64 or x64")
            return result
        role = "macos-aarch64-app" if macos_arch == "aarch64" else "macos-x64-app"
        dest_zip = dest_root / primaries[role]
        apps: list[Path] = []
        for root in scan_roots:
            apps.extend(_find_app_bundles(root))
        # Prefer the first bundle; duplicates are unusual in a single target build.
        if not apps:
            result.actions.append(
                NormalizeAction(
                    "skip",
                    "(no Mnemosyne.app)",
                    str(dest_zip),
                    f"no .app found under {[str(r) for r in scan_roots]}",
                )
            )
        else:
            app = apps[0]
            if dry_run:
                result.actions.append(
                    NormalizeAction("zip_app", str(app), str(dest_zip), "dry-run")
                )
            else:
                try:
                    _zip_app_bundle(app, dest_zip)
                    result.actions.append(
                        NormalizeAction(
                            "zip_app",
                            str(app),
                            str(dest_zip),
                            f"macos-arch={macos_arch}",
                        )
                    )
                except OSError as exc:
                    result.errors.append(f"zip {app} → {dest_zip}: {exc}")

    # 3) Optional: copy already-frozen files from search_root into out_dir.
    if not in_place and out_dir is not None and search_root is not None:
        frozen = set(primaries.values()) | set(frozen_fallback_dmg_names(version).values())
        for path in _unique_files(search_root):
            if path.name not in frozen:
                continue
            dest = dest_root / path.name
            if dest.exists() and not dry_run:
                continue
            if dry_run:
                result.actions.append(
                    NormalizeAction("copy", str(path), str(dest), "already-frozen dry-run")
                )
                continue
            try:
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, dest)
                result.actions.append(
                    NormalizeAction("copy", str(path), str(dest), "already-frozen")
                )
            except OSError as exc:
                result.errors.append(f"copy frozen {path}: {exc}")

    return result


def format_report(result: NormalizeResult) -> str:
    lines = [
        f"M21.C/D normalize desktop assets — version={result.version}",
        f"actions: {len(result.actions)} errors: {len(result.errors)}",
    ]
    for action in result.actions:
        lines.append(f"  [{action.kind}] {action.source} → {action.dest} ({action.detail})")
    for err in result.errors:
        lines.append(f"  [error] {err}")
    lines.append("Note: normalize success is not launch-tested proof.")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="Release version without leading v")
    parser.add_argument(
        "--dist",
        type=Path,
        default=None,
        help="Directory of release assets (in-place target, or extra scan root)",
    )
    parser.add_argument(
        "--search-root",
        type=Path,
        default=None,
        help="Extra tree to scan (e.g. tauri/target after a local/CI build)",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="Write normalized assets here (required unless --in-place)",
    )
    parser.add_argument(
        "--macos-arch",
        choices=("aarch64", "x64"),
        default=None,
        help="When set, zip Mnemosyne.app to the frozen macOS app zip for this arch",
    )
    parser.add_argument(
        "--in-place",
        action="store_true",
        help="Rename under --dist instead of copying to --out-dir",
    )
    parser.add_argument("--dry-run", action="store_true", help="Print planned actions only")
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON")
    args = parser.parse_args(argv)

    result = normalize_desktop_assets(
        args.version,
        dist=args.dist,
        search_root=args.search_root,
        out_dir=args.out_dir,
        macos_arch=args.macos_arch,
        in_place=args.in_place,
        dry_run=args.dry_run,
    )

    if args.json:
        print(
            json.dumps(
                {
                    "ok": result.ok,
                    "version": result.version,
                    "actions": [asdict(a) for a in result.actions],
                    "errors": result.errors,
                    "launch_tested": False,
                    "frozen_primary": frozen_primary_names(args.version),
                    "frozen_fallback_dmg": frozen_fallback_dmg_names(args.version),
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

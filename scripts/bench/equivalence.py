#!/usr/bin/env python3

"""Compute top-K class-set overlap between Mnemosyne deep JSON and MAT output.

Supported MAT inputs:
- an extracted CSV file with class-name and retained-heap columns
- an extracted HTML file with a table that includes class-name and retained-heap columns
- the original MAT zip artifact (*.zip) containing one of those files

This helper is intentionally limited to Mnemosyne-vs-MAT comparisons. hprof-slurp
uses a different aggregation surface, so this script does not try to compare it.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import re
import sys
import zipfile
from dataclasses import dataclass
from html import unescape
from html.parser import HTMLParser
from pathlib import Path


class MatInputShapeError(RuntimeError):
    """Raised when a MAT artifact exists but does not contain the expected table."""


@dataclass(frozen=True)
class RankedClass:
    name: str
    retained_bytes: int


def normalize_class_name(value: str) -> str:
    text = " ".join(unescape(value).strip().split())
    text = re.sub(r"^class\s+", "", text, flags=re.IGNORECASE)
    text = re.sub(r"\s+\([^)]*\)$", "", text)
    return text.replace("/", ".")


def parse_size_to_bytes(value: str) -> int | None:
    cleaned = " ".join(unescape(value).strip().split())
    if not cleaned:
        return None

    match = re.search(r"([-+]?[0-9][0-9,]*(?:\.[0-9]+)?)\s*([A-Za-z]+)?", cleaned)
    if not match:
        return None

    number = float(match.group(1).replace(",", ""))
    unit = (match.group(2) or "B").upper()

    multipliers = {
        "B": 1,
        "BYTE": 1,
        "BYTES": 1,
        "KB": 1000,
        "KIB": 1024,
        "MB": 1000**2,
        "MIB": 1024**2,
        "GB": 1000**3,
        "GIB": 1024**3,
    }

    multiplier = multipliers.get(unit)
    if multiplier is None:
        return None

    return int(number * multiplier)


def unique_top_k(entries: list[RankedClass], top_k: int) -> list[str]:
    seen: set[str] = set()
    ordered: list[str] = []

    for entry in sorted(entries, key=lambda item: (-item.retained_bytes, item.name)):
        if entry.name in seen:
            continue
        seen.add(entry.name)
        ordered.append(entry.name)
        if len(ordered) == top_k:
            break

    return ordered


def parse_mnemo_classes(path: Path, top_k: int) -> tuple[str, list[str]]:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)

    if data.get("mode") == "overview":
        raise RuntimeError(
            "Mnemosyne overview output does not contain retained-size class rankings; use deep-mode analyze JSON instead."
        )

    histogram = data.get("histogram") or {}
    entries = histogram.get("entries") or []
    ranked: list[RankedClass] = []
    for entry in entries:
        class_name = entry.get("key")
        retained_size = entry.get("retained_size")
        if not class_name or retained_size is None:
            continue
        ranked.append(RankedClass(normalize_class_name(str(class_name)), int(retained_size)))

    if not ranked:
        raise RuntimeError(
            "Mnemosyne JSON does not contain histogram.entries[*].retained_size; rerun deep analyze JSON with class histogram output."
        )

    fixture = (
        data.get("summary", {}).get("heap_path")
        or data.get("overview", {}).get("heap_path")
        or path.name
    )
    return fixture, unique_top_k(ranked, top_k)


def normalized_header(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", " ", value.lower()).strip()


def find_columns(header_row: list[str]) -> tuple[int | None, int | None]:
    class_index = None
    retained_index = None

    for index, column in enumerate(header_row):
        normalized = normalized_header(column)
        if class_index is None and (
            "class" in normalized or normalized in {"component", "suspect", "leak suspect"}
        ):
            class_index = index
        if retained_index is None and "retained" in normalized:
            retained_index = index

    return class_index, retained_index


def ranked_from_rows(rows: list[list[str]]) -> list[RankedClass]:
    for header_index, header_row in enumerate(rows[:10]):
        class_index, retained_index = find_columns(header_row)
        if class_index is None or retained_index is None:
            continue

        ranked: list[RankedClass] = []
        for row in rows[header_index + 1 :]:
            if len(row) <= max(class_index, retained_index):
                continue
            class_name = normalize_class_name(row[class_index])
            retained_bytes = parse_size_to_bytes(row[retained_index])
            if not class_name or retained_bytes is None:
                continue
            ranked.append(RankedClass(class_name, retained_bytes))

        if ranked:
            return ranked

    raise MatInputShapeError(
        "expected a MAT table with both class-name and retained-heap columns; pass *_Suspects.html, an extracted CSV, or the original MAT zip artifact"
    )


def parse_csv_text(text: str) -> list[RankedClass]:
    sample = text[:4096] or "class,retained\n"
    try:
        dialect = csv.Sniffer().sniff(sample, delimiters=",;\t")
    except csv.Error:
        dialect = csv.excel

    reader = csv.reader(io.StringIO(text), dialect)
    rows = [row for row in reader if any(cell.strip() for cell in row)]
    if not rows:
        raise MatInputShapeError("CSV input is empty")
    return ranked_from_rows(rows)


class TableExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.rows: list[list[str]] = []
        self._in_row = False
        self._in_cell = False
        self._current_row: list[str] = []
        self._current_cell: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "tr":
            self._in_row = True
            self._current_row = []
        elif tag in {"td", "th"} and self._in_row:
            self._in_cell = True
            self._current_cell = []

    def handle_endtag(self, tag: str) -> None:
        if tag in {"td", "th"} and self._in_cell:
            self._in_cell = False
            self._current_row.append("".join(self._current_cell).strip())
        elif tag == "tr" and self._in_row:
            self._in_row = False
            if any(cell for cell in self._current_row):
                self.rows.append(self._current_row)

    def handle_data(self, data: str) -> None:
        if self._in_cell:
            self._current_cell.append(data)


def parse_html_text(text: str) -> list[RankedClass]:
    parser = TableExtractor()
    parser.feed(text)
    if not parser.rows:
        raise MatInputShapeError("HTML input does not contain any table rows")
    return ranked_from_rows(parser.rows)


def parse_zip_input(path: Path) -> list[RankedClass]:
    with zipfile.ZipFile(path) as archive:
        members = [
            name
            for name in archive.namelist()
            if not name.endswith("/") and name.lower().endswith((".csv", ".html", ".htm"))
        ]
        if not members:
            raise MatInputShapeError(
                f"{path} does not contain CSV or HTML members; expected a MAT *_Suspects.zip or *_System_Overview.zip artifact"
            )

        best_result: list[RankedClass] | None = None
        best_error: Exception | None = None
        for member in members:
            raw = archive.read(member)
            text = raw.decode("utf-8", errors="replace")
            try:
                if member.lower().endswith(".csv"):
                    ranked = parse_csv_text(text)
                else:
                    ranked = parse_html_text(text)
            except MatInputShapeError as exc:
                best_error = exc
                continue

            if best_result is None or len(ranked) > len(best_result):
                best_result = ranked

        if best_result is None:
            if best_error is not None:
                raise best_error
            raise MatInputShapeError(f"{path} did not contain a parseable MAT ranking table")
        return best_result


def parse_mat_classes(path: Path, top_k: int) -> list[str]:
    if not path.exists():
        raise MatInputShapeError(
            f"MAT input not found: {path}. Pass *_Suspects.html, an extracted CSV, or the original MAT zip artifact."
        )

    suffix = path.suffix.lower()
    if suffix == ".csv":
        ranked = parse_csv_text(path.read_text(encoding="utf-8", errors="replace"))
    elif suffix in {".html", ".htm"}:
        ranked = parse_html_text(path.read_text(encoding="utf-8", errors="replace"))
    elif suffix == ".zip":
        ranked = parse_zip_input(path)
    else:
        raise MatInputShapeError(
            f"unsupported MAT input type: {path.suffix or '<none>'}. Expected HTML, CSV, or ZIP."
        )

    return unique_top_k(ranked, top_k)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Compute top-K class-set Jaccard overlap between Mnemosyne deep JSON and MAT retained-heap output."
    )
    parser.add_argument("--mnemo", required=True, help="Path to Mnemosyne deep analyze JSON output")
    parser.add_argument(
        "--mat",
        required=True,
        help="Path to a MAT HTML/CSV artifact, or the original *_Suspects.zip / *_System_Overview.zip file",
    )
    parser.add_argument("--top-k", type=int, default=10, help="Top-K classes to compare (default: 10)")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.top_k <= 0:
        print("error: --top-k must be a positive integer", file=sys.stderr)
        return 1

    mnemo_path = Path(args.mnemo)
    mat_path = Path(args.mat)

    try:
        fixture, mnemo_classes = parse_mnemo_classes(mnemo_path, args.top_k)
        mat_classes = parse_mat_classes(mat_path, args.top_k)
    except MatInputShapeError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    except Exception as exc:  # pragma: no cover - surfaced to the user verbatim
        print(f"error: {exc}", file=sys.stderr)
        return 1

    mnemo_set = set(mnemo_classes)
    mat_set = set(mat_classes)
    intersection = sorted(mnemo_set & mat_set)
    union = mnemo_set | mat_set
    jaccard = len(intersection) / len(union) if union else 1.0

    payload = {
        "fixture": fixture,
        "top_k": args.top_k,
        "mnemo_classes": mnemo_classes,
        "mat_classes": mat_classes,
        "intersection": intersection,
        "jaccard": round(jaccard, 6),
    }
    json.dump(payload, sys.stdout, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
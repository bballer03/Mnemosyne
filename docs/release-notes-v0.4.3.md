# Mnemosyne v0.4.3 Release Notes

> Release date: 2026-09-14
> Tag: v0.4.3
> Previous release: [v0.4.2](release-notes-v0.4.2.md)

Minor release: make desktop **Open heap dump** finish on large dumps, plus host file logging and dependency/CI pins.

## Highlights

- **Lean first-open analysis** — Home Open no longer enables strings/collections/threads/duplicate-arrays/by-referrer by default (those force field-data retention and multi-GB RSS). First open keeps histogram, leak triage, classloaders, and top instances.
- **Honest button phases** — **Opening…** during the native file picker; **Analyzing…** while analysis runs (no longer one endless “Opening…” for the whole wait).
- **Desktop file log** — `desktop.log` under the platform data-local `mnemosyne/logs` directory (`MNEMOSYNE_LOG_DIR` / `RUST_LOG`). Validation Console shows the path via `get_desktop_log_path`.
- Actions / Docker / Cargo / UI dependency refreshes (Tailwind 4 / Vite 8 / React 19 types still deferred).

## Breaking Changes

None for CLI/MCP contracts. Desktop Open-heap **default report set is thinner** than v0.4.2 incident defaults; deep optional reports remain available when callers pass explicit enable flags.

## Known Limitations

Unsigned desktop default; native launch matrix / MAT golden / Sol closeouts still open. Homebrew SHAs are placeholders until archives publish. **v0.5.0** remains reserved for M24. Overview-mode desktop analysis is still not wired.

## Upgrade

Prefer `v0.4.3` desktop installers over `v0.4.2` when opening multi-GB heap dumps from Home.

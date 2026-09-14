# Mnemosyne v0.4.2 Release Notes

> Release date: 2026-09-14
> Tag: v0.4.2
> Previous release: [v0.4.1](release-notes-v0.4.1.md)

Hotfix for desktop **Open heap dump**: v0.4.1 injected the bridge correctly, but the native picker returned `source_id` / `display_name` while the UI expected `sourceId` / `displayName`, so analysis failed and the UI showed a generic “Failed to open heap dump” because Tauri rejects are often not `Error` instances.

## Highlights

- Serialize pick-file results with camelCase field names for the React bridge.
- Normalize snake_case pick payloads defensively in the UI host bridge.
- Surface the real host/Tauri error text (and log `[mnemosyne] …` to DevTools) instead of a generic fallback.
- Validation Console note: desktop DevTools (Inspect / F12) for full console logs.

## Breaking Changes

None.

## Known Limitations

Unchanged from v0.4.1 — unsigned desktop default; native launch matrix / MAT golden / Sol closeouts still open. Homebrew SHAs are placeholders until archives publish. **v0.5.0** remains reserved for M24.

## Upgrade

Prefer `v0.4.2` desktop installers over `v0.4.1` for Open heap dump.

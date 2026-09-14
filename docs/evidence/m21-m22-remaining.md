# M21 / M22 remaining work (honest)

**Branch:** `sync/m15g-m16bcd`  
**Date:** 2026-09-14  
**Tip at write:** see `git log -1`

## Shipped on this host (command-layer)

| Item | Evidence |
| --- | --- |
| Desktop asset verifier | `7ced971` — `scripts/release/verify_desktop_assets.py` + synthetic tests |
| SHA256SUMS + warn-mode verify in CI | `3ad3ba2` |
| Frozen macOS/AppImage name normalization | `b7bc663` |
| M22.D Unique Classes CLI + hierarchy gate | `175a6ce`, Terra cycle fix `f004efd` |
| OQL corpus honesty relabel | `f004efd` — handbook cases are `documentation-referenced` only |

## Blocked / needs native hosts (cannot close on WSL)

| Item | Why |
| --- | --- |
| Packaged GUI smoke | WebKitGTK/GTK absent on this WSL environment |
| Clean Windows unzip → double-click | Needs clean Windows image; WebView2 prerequisite |
| macOS Gatekeeper / Linux AppImage launch | Needs matching hosts |
| Live AI provider round-trip | Optional secrets + network; not proven here |
| Recorded MAT golden dump equivalency | Needs Eclipse MAT versioned results; corpus is docs-linked only |
| Final Terra/Sol milestone closeouts | Human review gates |

## Do not claim

- Full MAT equivalency
- Unsigned artifact = launch-proven
- Green unit tests = packaged desktop smoke

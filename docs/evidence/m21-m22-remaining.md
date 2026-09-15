# M21 / M22 remaining work (honest)

**Branch:** `docs/older-partials-closeout-spec`  
**Date:** 2026-09-15  
**Tip at write:** see `git log -1` on this branch after M22.E evidence commit

## Shipped on this host (command-layer)

| Item | Evidence |
| --- | --- |
| Desktop asset verifier | `7ced971` — `scripts/release/verify_desktop_assets.py` + synthetic tests |
| SHA256SUMS + verify in CI | `3ad3ba2` warn-mode → tip **fail-closed** `--require-checksums` |
| Frozen macOS/AppImage name normalization | `b7bc663` |
| ditto-only macOS zip + unsigned provenance + zip secret inspect | `9477e35` |
| Info.plist / AppImage ELF structural probe + README frozen names | `0a5292d` |
| Fail-closed name/checksum/structure release gate | tip — still ≠ native launch; see `m21-portable-installability.md` |
| M22.D Unique Classes CLI + hierarchy gate | `175a6ce`, Terra cycle fix `f004efd` |
| OQL corpus honesty relabel | `f004efd` — handbook cases are `documentation-referenced` only |
| M22.E corpus closeout evidence | 2026-09-15 — [m22-oql-compatibility.md](m22-oql-compatibility.md); **13 total, 12 shipped, 1 unsupported, 0 mat-referenced** |

## Blocked / needs native hosts (cannot close on WSL)

| Item | Why |
| --- | --- |
| Packaged GUI smoke | **PARTIAL** — Windows portable launch-tested 2026-09-15; Open-heap UI click NOT PROVEN; macOS/Linux still blocked |
| Clean Windows unzip → double-click | **PARTIAL** — portable extract + `Start-Process Mnemosyne.exe` proven; SmartScreen/double-click UX not separately recorded |
| macOS Gatekeeper / Linux AppImage launch | Needs matching hosts |
| Live AI provider round-trip | Optional secrets + network; not proven here |
| Recorded MAT golden dump equivalency | **NOT PROVEN** — needs Eclipse MAT versioned results; corpus is docs-linked only |
| Final Terra/Sol milestone closeouts for M20/M21/M23 | In progress under older-partials closeout program |

## Do not claim

- Full MAT equivalency
- Unsigned artifact = launch-proven
- Green unit tests = packaged desktop smoke
- M22 corpus green = MAT golden (0 `mat-referenced` cases)

# M30 product hardening — evidence summary

**Branch:** `feature/mat-maturity-m25-plus`  
**Date:** 2026-09-15  
**Plan:** [docs/superpowers/plans/2026-09-15-m30-product-hardening.md](../superpowers/plans/2026-09-15-m30-product-hardening.md)  
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)

This note records focused theme, test-memory, logging, and accessibility-contract evidence on WSL. It does **not** claim packaged desktop launch, native keyboard smoke, signing, or notarization.

## Implementation commits

| Slice | Implementation | Verified contract |
| --- | --- | --- |
| M30.A theme/responsive | `e71ea44` | Expanded `--mn-*` tokens, `theme-tokens` / `useCompactLayout`, workbench shells stack at 980px |
| M30.D UI test RSS | `0ed9370` | Per-batch `/proc` RSS ceilings, named timeout/SIGTERM failures, store/bridge reset helpers |
| M30.E logging | `fa33266` | Daily `desktop.*` rotation, path-free init log, `redact_log_message` unit tests |
| WSL stability follow-up | `6e7d96e` | Smaller histogram fixtures, isolated histogram batch, SIGTERM→SIGKILL escalation, reclaim script |

## M30.B — Keyboard / accessibility (partial)

### Keyboard-complete investigation flow (planned path)

1. **Open** — `Open heap dump` / artifact drop (button / file input with accessible name)
2. **Histogram** — `Search histogram`, `Select <classKey>` row buttons (`aria-pressed`)
3. **Instances** — `Open object <id>` buttons
4. **Inspector** — outgoing/incoming/dominator `<Link>` chips + `Request field data`
5. **GC path** — leak workspace GC-path controls / deep links from Assistant/workflows

### Automated checks (WSL)

Existing focused suites already assert accessible names for histogram selection, instance open, and inspector reference links. Additional `getByRole({ name: "…[]…" })` probes were **removed** after they hung Bun/jsdom on this host (Testing Library string→RegExp character-class trap). Prefer `getByLabelText` / exact function matchers if reintroduced.

### Manual native checklist — **NOT PROVEN** on WSL

| Step | Host | Result |
| --- | --- | --- |
| Tab order Open → histogram → instance → inspector → GC path | Windows / macOS / Linux packaged app | **NOT PROVEN** |
| Visible focus rings on primary actions | matching native hosts | **NOT PROVEN** |
| Screen-reader announcement spot-check | matching native hosts | **NOT PROVEN** |

## M30.C — Native release evidence — **NOT PROVEN** on WSL

| Claim | Status |
| --- | --- |
| Packaged Windows/macOS/Linux open → inspect → path scenarios | **NOT PROVEN** — no matching native host run in this closeout |
| Installer hashes / signing / notarization | **NOT PROVEN** |
| Folded M20–M23 Terra/Sol native launch matrix | Remains open under [m21-m22-remaining](m21-m22-remaining.md) / M31.E |

## M30.D / M30.E logging — verified on WSL

- RSS gate logs `[ui-test-rss] batch=… peak=… ceiling=…` and fails named batches on timeout/signal/ceiling.
- After timeout the runner sends **SIGTERM**, then **SIGKILL** after 5s so hung Bun cannot pin WSL RAM.
- Operators can reclaim leftovers with `ui/scripts/kill-stale-ui-tests.sh` (targets real `bun test` PIDs only).
- Desktop logging uses daily rotation (`desktop.YYYY-MM-DD`); init no longer logs filesystem paths; redaction helper is unit-tested in `tauri/src/log_redact.rs`.
- Full `cargo check` / packaged Tauri compile on WSL remains **environment-blocked** where GTK/WebKit libs are missing.

## Focused gates recorded this closeout

```bash
cd ui
bun test run-tests.test.ts --max-concurrency=1
bun test src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx --max-concurrency=1
```

Results: harness **10/10 pass**; histogram **7/7 pass**; free memory remained ~8.9 GiB available after the histogram suite on this 9.7 GiB WSL2 VM.

## Remaining gated work (M31+)

See [m31 gated follow-ons](../superpowers/plans/2026-09-15-m31-gated-follow-ons.md): OS integration, MAT golden/equivalence, perspectives, live JVM/plugins, signing/updater/Homebrew Cask. None of those gates are met on this WSL host.

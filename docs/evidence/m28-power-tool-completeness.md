# M28 power-tool completeness — evidence summary

**Branch:** `feature/mat-maturity-m25-plus`  
**Date:** 2026-09-15  
**Plan:** [docs/superpowers/plans/2026-09-15-m28-power-tool-completeness.md](../superpowers/plans/2026-09-15-m28-power-tool-completeness.md)  
**Ledger:** [docs/product/ui-capability-matrix.md](../product/ui-capability-matrix.md)

This note records focused client, component, bridge, session, and command-contract evidence for M28.A–C on WSL. It does not infer packaged desktop launch or download-dialog behavior from unit tests or a browser production build.

## Implementation commits

| Slice | Plan | Implementation | Verified contract |
| --- | --- | --- | --- |
| M28.A OQL workbench | `856b89c` | `15f6a98` | Structured parser locations, privacy-safe failures, bounded history/examples, object-ID navigation, stale-response rejection |
| M28.B on-demand analyzers | `2d1b4cc` | `7ca3e29` | One accumulated opt-in request, field-data cost disclosure, honest unavailable/provenance states, opaque policy baseline, compare-strategy disclosure |
| M28.C visualization/export | `1749698` | `0e8bdd0` | SVG/folded-stack/JSON flamegraphs, Text/Markdown/HTML/TOON/JSON reports, allowlisted downloads, mode/provenance labels, no returned-markup injection |

## Verified behavior

- The query workbench keeps history in memory, caps it at ten unique entries, exposes vetted examples and named syntax deferrals, maps UTF-8 parser byte offsets to line/column, and rejects stale workspace responses.
- Analyzer enrichment is opt-in and sends one accumulated request. Field-data analyzers disclose full-host reparse and memory/duration cost; missing sections remain unavailable rather than becoming empty successes.
- Policy growth checks select a baseline by opaque source ID without replacing the remembered current heap. Compare controls disclose heuristic identity trade-offs and the retained-field-data cost of full fingerprints.
- Flamegraph controls expose `svg`, `folded-stack`, and `json`. Only SVG receives an object-URL `<img>` preview; folded-stack and JSON content remain download-only.
- Report controls expose only `text`, `markdown`, `html`, `toon`, and `json`. The native command renders the last committed analysis, resolves the opaque active source, substitutes a display-safe basename, and rejects custom/unknown formats.
- Download preparation strips path components and unsafe controls, caps filenames, uses an explicit extension/MIME allowlist, serializes JSON objects, and revokes temporary object URLs.
- HTML/script/event-handler sentinels returned by the host never enter the workspace DOM. Mode plus every response-level `Partial`/`Fallback` marker and detail render as React text.
- Focused UI tests mount component/router adapters only; they do not mount the production route tree.

## Focused local gates

Verified:

```bash
cd ui
bun test \
  src/features/heap-explorer/components/QueryConsolePanel.test.tsx \
  src/features/heap-explorer/heap-explorer-query-client.test.ts \
  src/features/heap-explorer/HeapQueryConsolePage.test.tsx \
  src/host/tauri-bridge.test.ts \
  --max-concurrency=1
```

Result: **73 passed, 0 failed**.

```bash
cd ui
bun test \
  src/features/artifact-explorer/analyzer-enrichment-client.test.ts \
  src/features/artifact-explorer/components/AnalyzerEnrichmentPanel.test.tsx \
  src/features/artifact-explorer/ArtifactExplorerPage.test.tsx \
  --max-concurrency=1
bun test \
  src/features/artifact-loader/ArtifactLoaderPage.test.tsx \
  src/features/policy/PolicyCheckPage.test.tsx \
  src/features/comparison/ComparisonPicker.test.tsx \
  --max-concurrency=1
```

Results: **19 passed, 0 failed** and **28 passed, 0 failed**. The closeout intentionally kept these as focused batches instead of mounting the full route tree or running the RSS-sensitive all-UI suite.

```bash
cd ui
bun test \
  src/features/flamegraph/export-download.test.ts \
  src/features/flamegraph/FlamegraphPage.test.tsx \
  src/host/tauri-bridge.test.ts \
  --max-concurrency=1
bun run lint
bun run build
```

Results: **31 passed, 0 failed**; `tsc --noEmit` passed; the Vite production build completed. Vite emitted its existing native-config-loader advisory and chunk-size warning; neither is a compile failure or packaged-GUI evidence.

```bash
cargo test --manifest-path tauri/session-ops/Cargo.toml
cargo test -p mnemosyne-core --features test-fixtures --lib html_escaping_prevents_xss
```

Results: **11 passed, 0 failed** session-operation tests and **1 passed, 0 failed** focused core HTML-escaping test.

Attempted:

```bash
cargo check --manifest-path tauri/Cargo.toml
```

Result: **environment-blocked** before checking the desktop command crate because this WSL host lacks the `javascriptcoregtk-4.1` and `webkit2gtk-4.1` pkg-config libraries. Rust command registration and helper assertions are present, but this result is not claimed as a native Tauri compile, launch, or package proof.

## NOT PROVEN on this WSL host

| Claim | Status / reason |
| --- | --- |
| Packaged Windows/macOS/Linux GUI: generate, preview, and download exports | **NOT PROVEN** — no packaged desktop was launched on this WSL host |
| Native save/download dialog and OS file-open behavior | **NOT PROVEN** — browser component tests prove Blob/anchor contracts only |
| Native Tauri compile/link or installer packaging | **NOT PROVEN** — `cargo check` is blocked by missing JavaScriptCoreGTK/WebKitGTK 4.1 system libraries |
| Native visual/a11y interaction and screenshots | **NOT PROVEN** — focused component semantics passed; no packaged-host screenshot was captured |
| Full `bun run test` or full workspace `cargo test` | **NOT RUN** — focused M28 gates were used to avoid known WSL/Bun RSS pressure |

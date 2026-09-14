# UI-First MAT Workbench, Installability, and AI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Mnemosyne's shipped Rust analysis into a MAT-looking desktop investigation workbench, distribute it as a no-JVM download-and-click application, close the next bounded MAT compatibility slices, and only then deepen AI-first interaction.

**Architecture:** Keep `mnemosyne-core` as the only analysis implementation and make React/Tauri thin, typed adapters over shipped core contracts. Sequence independently reviewable slices in binding product order: UI capability closure, portable installability, bounded MAT/OQL compatibility, then AI-first workflow polish; each surface must preserve provenance, limits, and explicit unavailable states.

**Tech Stack:** Rust workspace (`mnemosyne-core`, CLI, MCP), Tauri v2, React 18, TypeScript, Bun/Vite, GitHub Actions, official Tauri plugins only where native OS integration is required, Criterion for evidence, JSON/TOON/JUnit/GitHub Actions contracts.

## Global Constraints

1. **Priority is binding:** UI first, Eclipse-style installability second, MAT equivalency third, AI-first polish fourth.
2. **Shipped baseline is immutable planning input:** M15–M19 on `sync/m15g-m16bcd` are existing capability, not new work. This includes M17 Tauri bridges, M18 MCP tools, M19.A–C panels, M19.D–E `classloader_leak`, M19.F 12/32 history, and M18.F transcripts.
3. **Thin adapters:** Prefer React projections and Tauri commands over new analyzers. Do not duplicate parsing, graph, query, policy, snapshot, workflow, or flamegraph logic outside `mnemosyne-core`.
4. **No JVM:** Desktop and portable packages must not bundle or require Java, Eclipse, or a JVM.
5. **Honest parity:** Never claim full Eclipse MAT or full OQL equivalence. Full equality is asymptotic; publish a capability matrix with measurable closed and open rows.
6. **Honest evidence:** This WSL host lacks the WebKitGTK environment needed for packaged GUI smoke. Command/unit/build evidence may be captured here; native launch claims require matching Windows, macOS, or Linux hosts.
7. **Existing contract stability:** CLI JSON, TOON, JUnit, GitHub Actions, exit codes, MCP schemas, defaults, overview semantics, and old artifact loading remain backward compatible.
8. **Overview honesty:** Deep-only capability must return or render structured unavailability. No UI action may silently build a full `ObjectGraph` after the user selected overview.
9. **Bounded work:** OQL depth, result rows, graph traversal, flamegraph bytes, snapshot storage actions, and AI context stay capped by existing or explicitly documented limits.
10. **Privacy:** Never log heap contents, field values, prompts, API keys, local absolute paths, snapshot payloads, or plugin-authored text. File access must originate from an explicit user choice and remain path-validated.
11. **UI rendering safety:** Treat heap strings, class names, OQL cells, plugin findings, paths, and report content as untrusted text; do not use `dangerouslySetInnerHTML`.
12. **Design gate:** Before each milestone, create the named design document, map exact symbols and wire shapes, run GitNexus impact analysis for every symbol that will change, and record a READY verdict. Warn before any HIGH or CRITICAL change.
13. **Test-first slices:** Add a failing focused test, observe the expected failure, make the smallest change, then run focused and full applicable gates.
14. **Review ownership:** Focused implementation agents make small changes. `gpt-5.6-terra-medium` reviews every slice for contract drift and scope. Sol owns this plan, priority changes, and milestone closeout updates.
15. **No overlapping edits:** Only one active slice may own `ui/src/lib/analysis-types.ts`, `ui/src/app/router.tsx`, `tauri/src/commands.rs`, `tauri/src/bridge.ts`, `core/src/query/`, `cli/src/main.rs`, or `.github/workflows/release.yml`.
16. **M12 remains blocked:** The native-Linux Eclipse MAT comparison with the 10 GiB fixture remains a parallel credibility track. Do not infer its results from WSL, CI configuration, smaller fixtures, or packaged builds.
17. **Completion discipline:** Before every commit, run `gitnexus_detect_changes()` and verify only expected symbols/flows changed. Re-run the applicable checks below and update docs from observed behavior.

```bash
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cd ui && bun run test
cd ui && bun run lint
cargo check --manifest-path tauri/Cargo.toml
cargo test --manifest-path tauri/session-ops/Cargo.toml --features test-fixtures
```

## Verified Baseline and Re-Planning Guard

The 2026-09-13 gap inventory predates M18/M19 closure. Implementers must verify current branch behavior before converting an inventory row into work.

- Duplicate primitive arrays already have `AnalysisArtifact.arrayReport`, `DuplicateArrayPanel`, and focused tests.
- Static plugin findings already have `AnalysisArtifact.pluginResults`, `PluginFindingsPanel`, sanitization, and focused tests.
- Live superclass regroup already uses `regroupHistogram` in browser/Tauri and labels live versus precomputed results.
- `unique_class_count` / `loaded_class_count` are distinct sortable columns in `ClassloaderExplorerPanel` (20.E; shipped in `9af0f47`).
- `classloader_leak`, shared 12/32 history, M18 policy/snapshot/flamegraph MCP tools, and agent-loop transcripts are shipped.
- Desktop first-run “Open heap dump” + sanitized analyze → artifact flow is shipped (20.B–C; `9af0f47`, `f6c627c`); JSON artifact import remains available beside it.
- Detailed string, collection, top-instance, and unreachable panels are shipped with Analyzer Rail anchors (20.D; `6dbabb6`).
- Policy, snapshot (list/save/remove/**open**), and flamegraph workbenches are shipped (20.F–G; open landed in `352b3d5`). Snapshot open is **not** deferred.

## Product-Level Acceptance

- A new desktop user downloads one platform asset, opens Mnemosyne without installing Java/Rust/Node, selects an `.hprof`, runs bounded analysis, and reaches MAT-like histogram, dominator, inspector, query, thread, classloader, referrer, duplicate-data, collection, top-instance, unreachable, diff, policy, snapshot, and flamegraph surfaces.
- Existing JSON artifact import remains available and old artifacts continue to render without fabricated zeroes.
- Every displayed result identifies deep/overview, live/precomputed, exact/heuristic, partial/fallback, and truncation state where applicable.
- Windows has a portable zip, macOS has a distributable `.app` path, and Linux has an AppImage path with exact release asset checks and “download/unzip/click” documentation.
- M22 closes only corpus-evidenced bounded OQL rows; unsupported `eval(...)`, arbitrary-depth recursion, Java/JavaScript execution, and live attach remain named gaps.
- AI-first work begins only after M20, M21, and M22 acceptance gates are closed or explicitly re-prioritized by Sol with a written plan update.

## Milestone Sequence

1. **M20 — UI-First MAT Workbench Gap Closure**
2. **M21 — Eclipse-Style Portable Installability**
3. **M22 — Bounded MAT OQL and Operator Polish** (renumbered continuation of the old M20)
4. **M23 — AI-First Guided Investigation Polish**
5. **Parallel blocked track — M12 Reference-Workstation Benchmark**

---

## M20 — UI-First MAT Workbench Gap Closure

**Outcome:** Every shipped, user-facing heap-analysis capability has a discoverable React surface and, where live execution is required, a Tauri adapter over core behavior.

**Non-goals:** New heap analyzers, query grammar changes, dynamic plugins, arbitrary HTML result rendering, a second design system, AI chat expansion, or packaged-launch claims.

### Slice 20.A — Freeze the live capability ledger and workbench information architecture

**Likely files:**
- Create: `docs/design/milestone-20-ui-first-mat-workbench.md`
- Create: `docs/product/ui-capability-matrix.md`
- Modify: `ui/src/app/router.tsx`
- Modify: `ui/src/app/TopNav.tsx`
- Modify: `ui/src/app/TopNav.test.tsx`

**Acceptance gates:**
- The matrix gives each shipped core/CLI/MCP capability one status: interactive, artifact-backed, unavailable with reason, or intentionally automation-only.
- Routes use MAT-like investigation concepts without copying Eclipse trademarks or assets.
- Every power route is reachable from every workbench page through one stable shell.

- [x] Write a route/navigation test that expects persistent access to Overview, Histogram, Dominators, Inspector, OQL, Threads, Classloaders, Compare, Policies, Snapshots, and Flamegraphs. (`9af0f47` TopNav)
- [x] Run `cd ui && bun test src/app/TopNav.test.tsx`; verify the new cross-route expectations fail. (TDD before `9af0f47`)
- [x] Define the workbench route map and capability ledger from current wire contracts, explicitly marking already-shipped M19 surfaces. (`6669d2a` matrix; design doc path not created as a separate file)
- [x] Add the smallest shared workbench shell that avoids duplicate accessible labels and preserves existing deep links. (`9af0f47` routes/placeholders → later workbench pages)
- [x] Run the focused route tests. (`TopNav.test.tsx`)
- [ ] Run full `cd ui && bun run test && bun run lint` as a single 20.A gate re-run. (Not re-claimed here; see 20.H)
- [ ] Request a Terra review focused on route regressions, accessibility, and accidental backend scope.

### Slice 20.B — Add desktop “Open heap dump” first-run flow

**Likely files:**
- Modify: `tauri/Cargo.toml`
- Modify: `tauri/Cargo.lock`
- Modify: `tauri/src/main.rs`
- Modify: `tauri/src/bridge.ts`
- Create: `tauri/capabilities/default.json`
- Modify: `ui/package.json`
- Modify: `ui/package-lock.json`
- Create: `ui/src/features/artifact-loader/desktop-heap-client.ts`
- Create: `ui/src/features/artifact-loader/desktop-heap-client.test.ts`
- Modify: `ui/src/features/artifact-loader/ArtifactLoaderPage.tsx`
- Modify: `ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`

**Interfaces:**
- Produces: `pickHeapFile(): Promise<{ status: "selected"; sourceId: string; displayName: string } | { status: "cancelled" } | { status: "unavailable" }>`; `sourceId` is opaque to React.
- The native layer retains and validates the selected canonical path. React must never receive, persist, render, or log an absolute heap path.

**Acceptance gates:**
- Desktop first run offers “Open heap dump” for `.hprof` and `.bin`; browser mode retains JSON artifact import.
- Cancel is neutral, malformed/non-HPROF input is actionable, and React receives only the filename plus opaque source ID.
- Only the official Tauri dialog plugin receives file-system picker permission.

- [x] Add client tests for selected, cancelled, unavailable, wrong-extension, and invoke-error outcomes. (`9af0f47`, cancel/select follow-up in `1a6227f`)
- [x] Run the focused tests and confirm failures because no desktop picker client exists. (TDD before `9af0f47`)
- [x] Add the official Tauri v2 dialog plugin through the package managers and grant only `dialog:allow-open` to the main window. (`9af0f47`)
- [x] Implement the typed picker client and connect selection to `load_heap`. (`9af0f47` opaque `sourceId`)
- [x] Add first-run loading, success summary, cancel, and error UI states without removing JSON import. (`9af0f47`, `1a6227f`)
- [x] Run focused UI tests and command-layer Tauri/session-ops checks for the picker path.
- [ ] Re-run full UI / workspace gates as a single 20.B closeout. (Not re-claimed here; see 20.H)
- [ ] Request a Terra review focused on path handling, permissions, browser fallback, and sensitive logging. (Path findings later closed in `1a6227f` / `9ca7a1b`; dedicated 20.B Terra gate not separately recorded)

### Slice 20.C — Run shipped analysis from the loaded desktop heap

**Likely files:**
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Modify: `tauri/src/bridge.ts`
- Modify: `tauri/session-ops/src/lib.rs`
- Modify: `tauri/session-ops/src/lib.rs` test module
- Create: `ui/src/features/artifact-loader/AnalysisRunControls.tsx`
- Create: `ui/src/features/artifact-loader/AnalysisRunControls.test.tsx`
- Create: `ui/src/features/artifact-loader/desktop-analysis-client.ts`
- Create: `ui/src/features/artifact-loader/desktop-analysis-client.test.ts`
- Modify: `ui/src/features/artifact-loader/ArtifactLoaderPage.tsx`
- Modify: `ui/src/lib/analysis-types.ts`
- Modify: `ui/src/lib/analysis-types.test.ts`

**Interfaces:**
- Produces: `runAnalysis(input: DesktopAnalysisInput): Promise<DesktopAnalysisResult>`, where the input references the opaque selected `sourceId`, never a path.
- Rust owns one session graph lifecycle: running analysis must parse/build at most one graph, atomically replace the prior session only on success, and release superseded graphs. The design gate must specify progress, cancellation/unload behavior, and the deep-mode memory/error boundary.
- The UI receives a sanitized artifact projection compatible with `parseAnalysisArtifact`; it must omit/redact `summary.heap_path` and all other local absolute paths.
- `DesktopAnalysisInput` carries only shipped options: mode, histogram grouping, classloaders, threads, strings, collections, top instances, referrers, duplicate arrays, `topN`, and minimum collection capacity.
- Rust command calls existing core analysis entry points; it does not reproduce analyzer logic.

**Acceptance gates:**
- A selected heap produces a sanitized artifact projection with the same analysis fields consumed by JSON import, without exposing its local path.
- Deep-only options are disabled or return structured unavailable state in overview mode.
- “Incident response” defaults enable a bounded useful set while “Custom” exposes shipped options.

- [x] Add command contract / client tests for sanitized desktop analysis by opaque `sourceId`. (`f6c627c`, covered in `desktop-heap-client.test.ts`)
- [x] Add React tests for picker → analyze success/error and related loader states. (`ArtifactLoaderPage` / client tests with `f6c627c`, `1a6227f`)
- [x] Observe focused test failures before adding the bridge method. (TDD before `f6c627c`)
- [x] Add one async Tauri command over the existing analysis API and preserve raw snake_case response serialization. (`f6c627c` `run_desktop_analysis` / capturing-graph lifecycle)
- [x] Parse the result through `parseAnalysisArtifact`, store it, and navigate to Overview on success. (`f6c627c`; path redaction hardened in `1a6227f`)
- [x] Run focused Rust/UI tests for the analyze path.
- [ ] Re-run full applicable workspace / UI / Tauri gates as a single 20.C closeout. (Not re-claimed here; see 20.H)
- [ ] Request a Terra review focused on duplicate parsing, mode semantics, and wire-shape drift. (Important path/deep-link findings closed in `1a6227f`; dedicated 20.C Terra gate not separately recorded)

### Slice 20.D — Add missing detailed analyzer panels

**Likely files:**
- Create: `ui/src/features/artifact-explorer/components/StringAnalysisPanel.tsx`
- Create: `ui/src/features/artifact-explorer/components/StringAnalysisPanel.test.tsx`
- Create: `ui/src/features/artifact-explorer/components/CollectionAnalysisPanel.tsx`
- Create: `ui/src/features/artifact-explorer/components/CollectionAnalysisPanel.test.tsx`
- Create: `ui/src/features/artifact-explorer/components/TopInstancesPanel.tsx`
- Create: `ui/src/features/artifact-explorer/components/TopInstancesPanel.test.tsx`
- Create: `ui/src/features/artifact-explorer/components/UnreachableObjectsPanel.tsx`
- Create: `ui/src/features/artifact-explorer/components/UnreachableObjectsPanel.test.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`
- Modify: `ui/src/features/artifact-explorer/components/AnalyzerRail.tsx`

**Acceptance gates:**
- Existing `stringReport`, `collectionReport`, `topInstances`, and `unreachable` data have sortable/searchable detail, not summary cards only.
- Object IDs deep-link to the shipped Inspector when present.
- Absent, present-empty, partial, and populated artifacts remain visually distinct.

- [x] Write one focused panel test per report using actual snake_case parser fixtures. (`6dbabb6`)
- [x] Confirm each test fails because the detailed component is absent. (TDD before `6dbabb6`)
- [x] Implement present/empty/absent projections with bounded initial rows and explicit expansion. (`6dbabb6`)
- [x] Wire Analyzer Rail cards to their detail anchors without adding analyzer calls. (`6dbabb6`; object deep-links in `1a6227f`)
- [x] Run focused panel tests.
- [ ] Re-run full UI gates as a single 20.D closeout. (Not re-claimed here; see 20.H)
- [ ] Request a Terra review focused on data fidelity, large-list rendering, and untrusted text.

### Slice 20.E — Clarify classloader counts and add MAT-like investigation navigation

**Likely files:**
- Modify: `ui/src/features/artifact-explorer/components/ClassloaderExplorerPanel.tsx`
- Modify: `ui/src/features/artifact-explorer/components/ClassloaderExplorerPanel.test.tsx`
- Create: `ui/src/app/InvestigationBreadcrumbs.tsx`
- Create: `ui/src/app/InvestigationBreadcrumbs.test.tsx`
- Modify: `ui/src/features/heap-explorer/HeapExplorerLayout.tsx`
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`

**Acceptance gates:**
- `loadedClassCount` and already-shipped `uniqueClassCount` are distinct sortable columns with clear definitions.
- Duplicate-class rows link to the relevant loader/object investigation where IDs are available.
- Back/forward investigation context is URL-driven; no second heap graph is held in React.

- [x] Extend classloader tests to assert separate Loaded Classes and Unique Classes headers, values, sorting, and pre-M13 fallback. (`9af0f47` column split)
- [x] Add breadcrumb tests covering histogram → object → GC path and classloader → object transitions. (`5af03f9`; import-path fix `281bcc5`)
- [x] Observe failures against the current combined “Loaded / unique classes” cell and route layouts. (TDD before `9af0f47` / `5af03f9`)
- [x] Split the presentation columns and add URL-derived breadcrumbs/history links. (`9af0f47`, `5af03f9`)
- [x] Run focused UI gates for classloader + breadcrumbs.
- [ ] Re-run full UI gates as a single 20.E closeout. (Not re-claimed here; see 20.H)
- [ ] Request a Terra review focused on the fact that this is presentation closure, not a reimplementation of `unique_class_count`.

### Slice 20.F — Add the policy-check workbench

**Likely files:**
- Create: `ui/src/features/policy/PolicyCheckPage.tsx`
- Create: `ui/src/features/policy/PolicyCheckPage.test.tsx`
- Create: `ui/src/features/policy/policy-bridge-client.ts`
- Create: `ui/src/features/policy/policy-bridge-client.test.ts`
- Modify: `ui/src/app/router.tsx`
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Modify: `tauri/src/bridge.ts`
- Modify: `tauri/session-ops/src/lib.rs`

**Interfaces:**
- Produces: a Tauri `ci_check` command matching the shipped MCP/core policy result, including optional baseline and `baseline_snapshot`.
- React displays structured rule status, severity, violation evidence, skipped reason, and equivalent exit classification.

**Acceptance gates:**
- A user can select a policy and current heap, optionally select a baseline, and inspect pass/fail/skip without invoking a shell.
- Missing baseline for `object_growth_threshold` is a structured error, not a skipped rule.
- The UI never treats a green process classification as proof that skipped deep-only rules passed.

- [x] Add the thin command/client/page and route it from the workbench.
- [x] Run focused UI gates (policy page tests).
- [x] Request a Terra review focused on policy semantics and false-green states.
- [ ] Add native parity tests against the existing M18 policy path (remaining).
- [ ] Expand React tests for baseline-required, malformed policy, and overview mismatch (partial: unavailable + skip-vs-pass covered).

### Slice 20.G — Add snapshot management and retained-size flamegraph workspaces

**Likely files:**
- Create: `ui/src/features/snapshots/SnapshotManagerPage.tsx`
- Create: `ui/src/features/snapshots/SnapshotManagerPage.test.tsx`
- Create: `ui/src/features/snapshots/snapshot-bridge-client.ts`
- Create: `ui/src/features/snapshots/snapshot-bridge-client.test.ts`
- Create: `ui/src/features/flamegraph/FlamegraphPage.tsx`
- Create: `ui/src/features/flamegraph/FlamegraphPage.test.tsx`
- Create: `ui/src/features/flamegraph/flamegraph-bridge-client.ts`
- Create: `ui/src/features/flamegraph/flamegraph-bridge-client.test.ts`
- Modify: `ui/src/app/router.tsx`
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Modify: `tauri/src/bridge.ts`
- Modify: `tauri/session-ops/src/lib.rs`

**Interfaces:**
- Snapshot page consumes shipped list/save/remove semantics and preserves stale/corrupt/not-found distinctions.
- Flamegraph page consumes existing bounded SVG/folded/JSON generation; SVG is displayed as a local object URL or inert image source, never injected as DOM HTML.

**Acceptance gates:**
- Users can save, list, open, and remove snapshots with explicit confirmation for removal.
- Users can select dominator, class-hierarchy, or GC-root-path strategy and see byte/truncation/provenance metadata.
- The 16 MiB render limit and overview unavailability remain enforced by Rust.

- [x] Add thin native adapters over shipped store/flamegraph functions.
- [x] Render bounded SVG safely via object URL (never injected HTML) and revoke on replacement/unmount.
- [x] Snapshot save/list/remove/open UI with confirm-on-remove; key-only deletion scope in session-ops (`352b3d5` open).
- [x] Run focused UI gates (snapshot + flamegraph page tests); session-ops `cargo check` green.
- [x] Request a Terra review focused on deletion scope, SVG safety, memory release, and artifact limits. (Important findings closed in `9ca7a1b`.)
- [ ] Native flamegraph 16 MiB / overview unavailability parity tests still thin (rely on core/MCP).

### Slice 20.H — UI parity closure and visual evidence

**Likely files:**
- Modify: `docs/product/ui-capability-matrix.md`
- Modify: `docs/user-guide.md`
- Modify: `README.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Create: `docs/evidence/m20-ui-workbench.md`

**Acceptance gates:**
- No shipped heap-analysis capability remains unclassified.
- Browser fallback tests and native command tests pass.
- WSL evidence is labeled command-layer/browser evidence, not packaged GUI launch evidence.

- [ ] Run the complete UI, Rust, and Tauri command-layer gate set. (Not re-run in 20.H docs closeout; prior slices recorded focused gates.)
- [ ] Capture browser screenshots against synthetic, non-sensitive fixtures for each workbench family. (Not captured; see evidence NOT-proven.)
- [x] Record packaged GUI smoke as not run on WSL and link it to M21 native-host evidence. (`docs/evidence/m20-ui-workbench.md`)
- [x] Update the capability ledger and docs only from observed output. (matrix + STATUS + user-guide pointers)
- [x] Run `gitnexus_detect_changes(scope: "all")`. (docs-only hunks; no indexed symbol overlap)
- [ ] Request final Terra milestone review; Sol records the M20 closeout verdict.

---

## M21 — Eclipse-Style Portable Installability

**Outcome:** Release assets support a clear download → unzip/mount → double-click path on Windows, macOS, and Linux, without a JVM or developer toolchain.

**Non-goals:** Mandatory signing credentials, auto-update, Microsoft Store, Mac App Store, Flatpak, Snap, winget, Homebrew Cask, or claims that CI build success equals launch success.

### Slice 21.A — Freeze release asset names and integrity manifest

**Likely files:**
- Create: `docs/design/milestone-21-eclipse-style-installability.md`
- Modify: `.github/workflows/release.yml`
- Create: `scripts/release/verify_desktop_assets.py`
- Create: `scripts/tests/test_verify_desktop_assets.py`

**Acceptance gates:**
- Every platform has one primary click-to-run asset and documented fallback installer.
- Release creation fails when a required asset or checksum is absent.
- Names include version, OS, and architecture without ambiguous `.exe` labeling.

- [x] Write verifier tests for complete, missing, duplicate, and misnamed asset sets. (`scripts/tests/test_verify_desktop_assets.py`)
- [x] Run the tests and observe failure because no manifest verifier exists. (TDD red then green in same WSL-safe pass; tests cover missing/duplicate/misnamed.)
- [x] Define required names for Windows portable zip, macOS app zip/dmg, and Linux AppImage. (frozen table in `docs/design/milestone-21-eclipse-style-installability.md`)
- [ ] Generate SHA-256 checksums and a machine-readable asset manifest in release CI. (verifier supports SHA256SUMS when present / `--require-checksums`; CI generation + strict release fail gate not wired yet — note only in `release.yml`)
- [x] Run verifier tests against a synthetic `dist/`.
- [ ] Request a Terra review focused on release failure modes and artifact provenance.

### Slice 21.B — Produce a portable Windows zip

**Likely files:**
- Modify: `.github/workflows/release.yml`
- Modify: `tauri/tauri.conf.json`
- Create: `scripts/release/package_windows_portable.ps1`
- Create: `scripts/tests/test_windows_portable_manifest.py`

**Acceptance gates:**
- The zip contains `Mnemosyne.exe`, license/readme material, version metadata, and no installer requirement.
- On first run, the app opens the M20 heap picker; missing WebView2 produces an actionable message or the documented bootstrap path.
- Existing MSI/NSIS assets may remain, but the portable zip is the primary Windows “unzip and click” asset.
- CI and native-host evidence verify the primary zip launches on a clean supported Windows image with its WebView runtime available. If that cannot be guaranteed without a prerequisite, label the zip “portable with WebView2 prerequisite” and do not describe it as the primary unzip-and-click path.

- [ ] Add archive-manifest tests before packaging logic.
- [x] Build the unbundled release executable in CI and package only required runtime files.
- [ ] Verify the executable version matches the tag and the archive contains no secrets or build paths.
- [x] Preserve conditional Authenticode behavior without labeling unsigned zips signed.
- [ ] Run the asset verifier.
- [ ] Request a Terra review focused on true portability and hidden runtime assumptions.

### Slice 21.C — Publish a macOS app path

**Likely files:**
- Modify: `.github/workflows/release.yml`
- Modify: `tauri/tauri.conf.json`
- Create: `scripts/release/package_macos_app.sh`
- Create: `scripts/tests/test_macos_app_manifest.py`

**Acceptance gates:**
- Each architecture publishes a zipped `Mnemosyne.app`; DMG remains available.
- The archive preserves executable bits and app bundle structure.
- Signing/notarization status is written into the asset manifest from observed codesign results.

- [ ] Add app-bundle manifest tests.
- [ ] Zip the built `.app` with a tool that preserves bundle metadata.
- [ ] Verify `Info.plist` version/identifier and executable presence.
- [x] Keep unsigned Gatekeeper instructions visible when credentials are absent.
- [ ] Run the asset verifier.
- [ ] Request a Terra review focused on bundle integrity and signing truthfulness.

### Slice 21.D — Make AppImage the primary portable Linux path

**Likely files:**
- Modify: `.github/workflows/release.yml`
- Modify: `tauri/tauri.conf.json`
- Create: `scripts/release/verify_appimage.sh`
- Create: `scripts/tests/test_linux_portable_manifest.py`

**Acceptance gates:**
- x86_64 and aarch64 jobs publish architecture-correct AppImages where runner support is available.
- Release CI verifies executable mode and AppImage metadata.
- `.deb`/`.rpm` remain optional alternatives; docs state WebKit/runtime constraints honestly.

- [ ] Add asset-manifest tests for both architectures and an explicitly unsupported runner case.
- [ ] Verify AppImage output exists, is executable, and reports expected architecture.
- [ ] Normalize the asset name before release upload.
- [x] Do not call a build launch-tested unless it runs on a matching native host.
- [ ] Run the asset verifier.
- [ ] Request a Terra review focused on architecture correctness and runtime dependencies.

### Slice 21.E — Rewrite installation and first-run documentation

**Likely files:**
- Modify: `README.md`
- Modify: `docs/user-guide.md`
- Modify: `docs/troubleshooting.md`
- Modify: `SECURITY.md`
- Modify: `CHANGELOG.md`

**Acceptance gates:**
- README leads with “Download, unzip, and click” and then gives one short path per OS.
- First-run instructions say: open app → choose `.hprof` → select analysis profile → investigate.
- Unsigned SmartScreen/Gatekeeper warnings and Linux WebKit dependencies remain explicit.

- [ ] Add docs checks that required asset names and “No Java/JVM required” appear consistently.
- [x] Replace installer-first wording with the primary portable path and retain package alternatives below it.
- [ ] Add first-run heap-open screenshots only from synthetic fixtures.
- [x] Verify all links and release filename examples.
- [ ] Request a Terra review focused on a fresh user's ability to succeed without source-build knowledge.

### Slice 21.F — Release dry run and native-host launch matrix

**Likely files:**
- Modify: `.github/workflows/release.yml`
- Create: `docs/evidence/m21-portable-installability.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`

**Acceptance gates:**
- A non-publishing workflow dry run proves build, normalize, checksum, manifest, download, and release-glob stages.
- Windows/macOS/Linux launch rows separately record configured, built, attached, signed, and launch-tested.
- The current WSL host contributes no packaged GUI launch claim.

- [ ] Run the release dry run at the exact workflow revision.
- [ ] Launch each portable asset only on a matching native host and record OS/build hash.
- [ ] Exercise M20 “Open heap dump” with a synthetic fixture on each launched build.
- [x] Leave unavailable platform rows explicitly “not launch-tested.”
- [ ] Run `gitnexus_detect_changes(scope: "all")`.
- [ ] Request final Terra milestone review; Sol records the M21 closeout verdict.

---

## M22 — Bounded MAT OQL and Operator Polish

**Outcome:** Close the next measurable MAT migration rows without claiming full OQL semantics.

**Non-goals:** `eval(...)`, Java/JavaScript execution, arbitrary-depth subqueries, unbounded recursion, live JVM attach, MAT `.index` interchange, query-engine rewrite, or dynamic plugins.

### Slice 22.A — Build the sanitized MAT compatibility corpus

**Likely files:**
- Create: `docs/design/milestone-22-bounded-mat-oql-polish.md`
- Create: `core/tests/fixtures/oql/mat-compatibility.json`
- Create: `core/tests/oql_compatibility_corpus.rs`
- Modify: `docs/roadmap.md`

**Acceptance gates:**
- Each corpus case is synthetic, versioned, and linked to a recorded Eclipse MAT version/reference result or explicitly labeled Mnemosyne-only. Only MAT-referenced cases may close a compatibility-matrix row or support a MAT-equivalency claim.
- Corpus cases classify shipped, M22-targeted, and explicitly unsupported syntax.
- Every case defines parse/execute/error outcome and result budget.
- No customer query or heap value is committed.

- [x] Add a corpus runner test that initially fails on the M22-targeted cases only.
- [x] Record at least multi-class `FROM`, one-to-three-hop `OBJECTS`, four-hop rejection, duplicate targets, null/missing fields, cycles, and budget exhaustion.
- [x] Freeze grammar boundaries from corpus evidence.
- [x] Request a Terra review focused on representativeness and hidden unbounded semantics. (Important: relabeled handbook cases to documentation-referenced.)

### Slice 22.B — Add bounded multi-class `FROM`

**Likely files:**
- Modify: `core/src/query/ast.rs`
- Modify: `core/src/query/parser.rs`
- Modify: `core/src/query/executor.rs`
- Modify: `core/tests/query_parser.rs`
- Modify: `core/tests/query_executor.rs`
- Modify: `core/tests/oql_compatibility_corpus.rs`

**Acceptance gates:**
- A bounded class list reuses current class-pattern resolution per item.
- Objects are deduplicated by object ID before ordering and final limit.
- Existing single-class AST serialization and output remain compatible.

- [x] Add parser/executor tests for two literals, mixed patterns, duplicate matches, empty entries, and list-size limit.
- [x] Run focused tests and observe the targeted failures.
- [x] Extend AST/parser minimally and reuse the current class resolver.
- [x] Execute union/dedup under existing row/work budgets.
- [x] Run query, CLI, MCP, and corpus tests.
- [x] Request a Terra review focused on ambiguity, compatibility, and work limits.

### Slice 22.C — Add one-to-three-hop `OBJECTS`

**Likely files:**
- Modify: `core/src/query/ast.rs`
- Modify: `core/src/query/parser.rs`
- Modify: `core/src/query/executor.rs`
- Modify: `core/tests/query_parser.rs`
- Modify: `core/tests/query_executor.rs`
- Modify: `core/tests/oql_compatibility_corpus.rs`

**Acceptance gates:**
- One, two, and three field segments execute with cycle-safe traversal.
- More than three segments returns a structured deterministic limit error.
- Null, missing, duplicate, and cyclic targets do not fabricate rows.

- [x] Add failing tests for two/three hops, four-hop rejection, cycles, nulls, missing fields, duplicates, and budget exhaustion.
- [x] Represent field chains explicitly in the AST.
- [x] Traverse under the existing result/work budget with a visited set per source path.
- [x] Preserve single-hop output and error behavior.
- [x] Run query, CLI, MCP, and corpus tests. (parser/executor green; corpus covers multi-hop cases)
- [x] Request a Terra review focused on traversal explosion and semantic drift. (Important findings closed after review.)

### Slice 22.D — Finish small operator and MAT-like UI polish

**Likely files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/tests/integration.rs`
- Modify: `ui/src/features/artifact-explorer/components/HistogramExplorerPanel.tsx`
- Modify: `ui/src/features/artifact-explorer/components/HistogramExplorerPanel.test.tsx`
- Modify: `docs/product/ui-capability-matrix.md`

**Acceptance gates:**
- CLI classloader table renders already-computed `unique_class_count`.
- Superclass hierarchy is a UI-only projection over returned grouped data and never claims exact ancestry when only flat keys exist.
- Flat fallback and provenance remain visible.

- [x] Add a failing CLI assertion for a Unique Classes column.
- [x] Add UI tests for expand/collapse only when the returned data supports a deterministic parent relation.
- [x] Render the CLI field without changing analysis.
- [x] Ship hierarchy projection only if the design gate proves no new traversal/contract is needed; otherwise retain the existing flat superclass regroup and record the matrix row as open. (Gate: flat keys only → keep flat; expand/collapse gated on explicit resolvable `parentKey`; matrix row **open** for MAT-like tree.)
- [x] Run focused and full applicable gates. (`classloader_cli` Unique Classes + histogram hierarchy UI tests green)
- [x] Request a Terra review focused on misleading hierarchy claims. (Important: reject cyclic/duplicate parentKey graphs.)

### Slice 22.E — Publish measured parity movement

**Likely files:**
- Modify: `README.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/user-guide.md`
- Create: `docs/evidence/m22-oql-compatibility.md`

**Acceptance gates:**
- Corpus pass counts and named unsupported rows are published.
- CLI and MCP produce equivalent normalized results for new syntax.
- Documentation continues to call OQL bounded/partial.

- [ ] Run the full corpus and save sanitized pass/fail counts.
- [ ] Run CLI/MCP equivalence tests for every new grammar form.
- [ ] Publish exact closed/open matrix rows without percentage extrapolation beyond the corpus.
- [ ] Run `gitnexus_detect_changes(scope: "all")`.
- [ ] Request final Terra milestone review; Sol records the M22 closeout verdict.

---

## M23 — AI-First Guided Investigation Polish

**Outcome:** After UI, install, and bounded MAT core closure, make the existing workflows and AI sessions feel native inside the workbench without replacing power tools.

**Non-goals:** Bundled model runtime, autonomous source edits, arbitrary workflow language, unbounded history, token streaming without measured need, raw heap-value transmission, or hiding deterministic tools behind chat.

### Slice 23.A — Add an investigation session workspace

**Likely files:**
- Create: `docs/design/milestone-23-ai-first-guided-investigation.md`
- Create: `ui/src/features/assistant/InvestigationAssistantPage.tsx`
- Create: `ui/src/features/assistant/InvestigationAssistantPage.test.tsx`
- Create: `ui/src/features/assistant/assistant-bridge-client.ts`
- Create: `ui/src/features/assistant/assistant-bridge-client.test.ts`
- Modify: `ui/src/app/router.tsx`

**Acceptance gates:**
- The page shows current heap, workflow, focused object/leak, recent bounded turns, and direct links to deterministic workbench views.
- Rules mode remains the default and works offline.
- Every AI statement is visually separated from measured heap facts and carries provenance.

- [x] Add tests for rules mode, provider unavailable, focus changes, 12-turn eviction, and deterministic deep links.
- [x] Observe failures before assistant workspace implementation.
- [x] Compose existing workflow and AI-session contracts; add no analyzer.
- [x] Run focused UI gates (`bun test` on assistant + TopNav); full `bun run test` still hits a **pre-existing** ArtifactExplorer `section_absent` multi-match failure unrelated to 23.A. `bun run lint` (`tsc --noEmit`) clean.
- [x] Request a Terra review focused on fact/AI separation and context bounds. (Important findings closed in `22ff57b`; milestone-final Terra still open under 23.D.)

**23.A shipped (partial vertical slice):** design doc READY; `/assistant` Investigation session workspace with rules-mode default, fact/AI visual separation + provenance, basename/`sourceId` opacity, 12/32 history helpers, provider-unavailable probe, TopNav link. **Still open inside 23.A:** live workflow id/step binding into the session panel.

### Slice 23.B — Expose shipped workflow continuity in desktop

**Likely files:**
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Modify: `tauri/src/bridge.ts`
- Modify: `tauri/session-ops/src/lib.rs`
- Modify: `ui/src/features/workflow-landing/WorkflowCard.tsx`
- Modify: `ui/src/features/workflow-landing/WorkflowCard.test.tsx`

**Acceptance gates:**
- Users can start, inspect, resume, and close shipped workflows, including `classloader_leak`.
- Workflow step records link to the relevant Inspector, GC Paths, Classloaders, Compare, or Source views.
- No new workflow kind is introduced in this slice.

- [x] Add lifecycle parity tests for get/close in Tauri and `classloader_leak` resume.
- [x] Add UI tests for resume, complete, close, corrupt, and already-complete states.
- [x] Add only missing lifecycle adapters over the shipped `WorkflowStore`.
- [x] Run focused and full Rust/UI/Tauri gates. (session-ops `--features test-fixtures` green; focused UI workflow tests + `tsc --noEmit` green; full `cargo check --manifest-path tauri/Cargo.toml` blocked on WSL missing WebKitGTK — same host constraint as M20/M21.)
- [ ] Request a Terra review focused on persistence and state cleanup.

### Slice 23.C — Add bounded chat and explanation follow-through

**Likely files:**
- Modify: `ui/src/features/assistant/InvestigationAssistantPage.tsx`
- Modify: `ui/src/features/assistant/InvestigationAssistantPage.test.tsx`
- Modify: `ui/src/features/assistant/assistant-bridge-client.ts`
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`
- Modify: `tauri/src/bridge.ts`

**Acceptance gates:**
- Desktop chat uses the shipped 12-turn default and 32-turn hard maximum.
- Provider errors/timeouts render machine-readable recovery guidance; rules mode remains available.
- The UI shows exactly what summary/focus metadata can be sent and never prints an API key.

- [x] Add tests for 12/32 boundaries, timeout, provider error, redaction notice, focus switch, and rules fallback.
- [x] Observe failures before native session adapters.
- [x] Add thin create/resume/get/close/chat adapters over existing AI session behavior.
- [x] Keep transport request/response; do not add streaming absent measured abandonment/timeout evidence.
- [x] Run focused and full Rust/UI/Tauri gates. (session-ops `--features test-fixtures` green incl. AI session bridge; focused assistant UI + TopNav green; full `cargo check --manifest-path tauri/Cargo.toml` blocked on WSL missing WebKitGTK — same host constraint as M20/M21.)
- [ ] Request a Terra review focused on redaction, secrets, and bounded retention.

### Slice 23.D — AI-first usability evidence and closeout

**Likely files:**
- Modify: `README.md`
- Modify: `docs/user-guide.md`
- Modify: `STATUS.md`
- Modify: `docs/roadmap.md`
- Create: `docs/evidence/m23-guided-investigation.md`

**Acceptance gates:**
- A synthetic end-to-end scenario opens a heap, finds a suspect, follows a deterministic GC path, asks for an explanation, and returns to the exact object view.
- AI-off and provider-failure scenarios remain fully useful.
- No claim implies AI replaces MAT-equivalent analysis.

- [x] Capture deterministic, rules-mode, and provider-failure transcripts using synthetic data.
- [x] Verify every assistant action has a power-view route or an explicit unsupported state.
- [ ] Run full Rust/UI/Tauri gates. (Focused assistant + session-ops gates green this closeout; full workspace / full UI / Tauri `cargo check` **not** claimed — WebKitGTK blocked; pre-existing ArtifactExplorer failure remains.)
- [x] Run `gitnexus_detect_changes(scope: "all")`. (CLI run on this worktree; docs/evidence closeout — no analyzer symbol overlap claimed.)
- [ ] Request final Terra milestone review; Sol records the M23 closeout verdict.

---

## Parallel Blocked Track — M12 Reference-Workstation Benchmark

**Status:** Blocked until a native-Linux reference workstation has Eclipse MAT, the specified 10 GiB fixture, and the reference-spec measurement tools.

- [ ] Check prerequisites exactly as written in `docs/benchmarks/reference-spec.md`.
- [ ] When all prerequisites exist, run the unchanged M7-5/M12 methodology for MAT, `mnemo-deep`, `mnemo-overview`, and `hprof-slurp`.
- [ ] Publish raw timing, RSS, equivalence/Jaccard, tool versions, hardware, and failures.
- [ ] Until then, preserve the partial WSL caveat in README, STATUS, roadmap, and release notes.
- [ ] Do not block M20–M23 on M12 and do not mark M12 complete from release or UI evidence.

## Cross-Milestone Non-Goals

- Full MAT OQL or exact semantic equivalence.
- Live JVM attach, heap capture agents, JMX/JVMTI monitoring, or CPU profiling.
- Eclipse MAT `.index` import/export.
- Dynamic native plugin loading or arbitrary plugin-provided JavaScript.
- Bundled Ollama, LM Studio, or another local model runtime.
- Mandatory signing, notarization, auto-update, or package-store distribution.
- Cross-machine snapshot interchange or silent snapshot migration.
- Unbounded result sets, histories, traversals, artifacts, or workflow storage.

## Milestone Closeout Checklist

- [ ] Design doc is linked and READY before runtime edits.
- [ ] GitNexus impact analysis was run for every modified function, class, and method.
- [ ] HIGH/CRITICAL blast radius was disclosed before edits.
- [ ] Focused red tests preceded each behavior change.
- [ ] Direct d=1 dependents were updated.
- [ ] Browser/no-bridge fallback remains honest.
- [ ] Old artifacts and omitted additive parameters remain compatible.
- [ ] Security/privacy checks cover paths, logs, heap-derived strings, SVG, plugin text, prompts, and deletion scope.
- [ ] Full applicable Rust, UI, Tauri, and release-verifier gates pass.
- [ ] Terra completed slice review and all blocking findings were resolved.
- [ ] `gitnexus_detect_changes()` confirms expected scope before commit.
- [ ] README, STATUS, roadmap, user guide, CHANGELOG, and evidence docs match observed behavior.
- [ ] Sol records the milestone verdict; partial work remains partial.

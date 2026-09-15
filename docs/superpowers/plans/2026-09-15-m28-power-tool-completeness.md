# M28 Power-Tool Completeness Stub Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:writing-plans to expand the selected slice, then superpowers:subagent-driven-development or superpowers:executing-plans to implement it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mature the existing query, analyzer, policy, visualization, and export surfaces without widening the bounded backend scope.

**Architecture:** Treat each power tool as an on-demand projection of the active investigation. Reuse shipped query/analyzer/flamegraph/policy APIs and M26 operation envelopes; preserve lean open and provenance.

**Tech Stack:** React, Tauri, `core::query`, existing analysis modules, policy engine, flamegraph/report renderers.

**Roadmap:** [M28 — Power-tool completeness](../specs/2026-09-15-ui-mat-maturity-roadmap-design.md#m28--power-tool-completeness)

## Global Constraints

- Keep named OQL deferrals explicit: `eval(...)`, arbitrary-depth nesting, class lists beyond the shipped eight-pattern bound, and other unscheduled grammar.
- Strings/collections/arrays/threads/referrers/classloaders remain opt-in with memory/cost disclosure.
- Reuse `AnalyzeRequest` flags and existing analyzer reports; no duplicate frontend analyzer.
- Preserve HTML escaping and provenance on every export.
- Use focused component/client tests; never mount the production route tree in WSL tests.

---

## File map

- OQL: `ui/src/features/heap-explorer/HeapQueryConsolePage.tsx`, `components/QueryConsolePanel.tsx`, `heap-explorer-query-client.ts`, `core/src/query/`.
- Analyzers: `ui/src/features/artifact-explorer/components/*Panel.tsx`, `tauri/src/commands.rs`, `tauri/session-ops/src/lib.rs`.
- Policy/compare controls: `ui/src/features/policy/`, `ui/src/features/comparison/`.
- Visualization/export: `ui/src/features/flamegraph/`, `core/src/report/`, existing desktop flamegraph command.

### M28.A — OQL workbench

**Owned files:**
- Modify: `ui/src/features/heap-explorer/components/QueryConsolePanel.tsx`
- Modify: `ui/src/features/heap-explorer/components/QueryConsolePanel.test.tsx`
- Modify: `ui/src/features/heap-explorer/heap-explorer-query-client.ts`
- Modify: `ui/src/features/heap-explorer/heap-explorer-query-client.test.ts`
- Modify: `ui/src/host/tauri-bridge.ts`
- Modify: `ui/src/host/tauri-bridge.test.ts`

**Interfaces:**
- `runHeapQuery(input)` keeps the existing ready/unavailable/error result union, with the error branch widened to include an optional structured `{ byteOffset, line, column }` location.
- Query result object IDs remain scalar cells; the panel recognizes only object-ID columns and commits navigation through `useInvestigationStore.getState().setObjectId(id, "inspector")`.
- The desktop bridge continues to use M26 `{ workspaceId, revision, operationId, data }` envelopes. The panel adds a workspace/revision guard for every browser-bridge response before committing UI state.
- Query text remains an invocation-only value. It is never added to progress payloads, persistence, or console logging.

#### Task 1: Structured query errors and privacy-safe invocation

- [ ] **Step 1: Add failing client tests for structured locations**

  Add focused tests to `heap-explorer-query-client.test.ts` proving that a bridge rejection ending in `at byte 14` returns an error result with `byteOffset: 14`, one-based `line`/`column`, and no copied query text in the error object. Add a multiline UTF-8 case so byte offsets are not treated as JavaScript character indexes.

- [ ] **Step 2: Run the client test and verify RED**

  Run: `cd ui && bun test src/features/heap-explorer/heap-explorer-query-client.test.ts --max-concurrency=1`

  Expected: FAIL because query errors expose only a flat `error` string.

- [ ] **Step 3: Implement the minimal structured error parser**

  In `heap-explorer-query-client.ts`, add:

  ```ts
  export type HeapQueryErrorLocation = {
    byteOffset: number;
    line: number;
    column: number;
  };
  ```

  Extract a terminal `at byte N` marker from the host error, map the UTF-8 byte prefix into one-based line/column coordinates, and return the location separately from the display message. Do not return or persist the query text.

- [ ] **Step 4: Add failing native bridge privacy test**

  In `tauri-bridge.test.ts`, reject `query_heap` with an error value containing a unique query sentinel and spy on `console.error`. Assert the sentinel is absent from all logged arguments and that emitted operation-progress payloads contain only the M26 context/kind/phase/progress fields.

- [ ] **Step 5: Run the bridge test and verify RED**

  Run: `cd ui && bun test src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: FAIL because the generic native invoke logger currently logs the raw rejection object.

- [ ] **Step 6: Redact raw query invocation failures**

  Add a privacy option to the internal `invokeOrThrow` helper and enable it only for `query_heap`. Keep the command-level failure marker, but never pass the raw rejection to `console.error`. Do not alter result envelopes or progress payloads.

- [ ] **Step 7: Run focused client/bridge tests**

  Run: `cd ui && bun test src/features/heap-explorer/heap-explorer-query-client.test.ts src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: PASS with no query sentinel in captured logs or progress.

#### Task 2: Bounded history, vetted examples, and syntax disclosure

- [ ] **Step 1: Add failing panel tests for bounded history**

  Extend `QueryConsolePanel.test.tsx` with focused component tests that submit more than the history limit, assert newest-first ordering, assert duplicate queries move to the front without duplication, and assert clicking a history item restores it to the editor.

- [ ] **Step 2: Add failing panel tests for examples and syntax**

  Assert that vetted examples can replace the editor text, the supported syntax list is visible beside the editor, and named deferrals explicitly include `eval(...)`, arbitrary-depth nesting, class lists beyond the shipped eight-pattern `FROM` bound, and traversal beyond the shipped three-hop `OBJECTS` bound.

- [ ] **Step 3: Run the panel test and verify RED**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx --max-concurrency=1`

  Expected: FAIL because the panel currently renders only a textarea, button, and raw table.

- [ ] **Step 4: Implement the workbench side rail**

  Add immutable vetted examples and supported/deferred syntax copy beside the editor. Keep history in component memory only, cap it at 10 unique entries, record non-empty submissions, and provide buttons that restore history/examples without auto-running them.

- [ ] **Step 5: Run the panel test and verify GREEN**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx --max-concurrency=1`

  Expected: PASS.

#### Task 3: Inspector navigation and stale-response rejection

- [ ] **Step 1: Add failing object-navigation test**

  Render only `QueryConsolePanel` under a memory router, return an `object_id`/`@objectId` cell, click it, and assert the investigation store contains that ID with `originPane: "inspector"` and the location is `/heap-explorer/object-inspector?objectId=<encoded-id>`. Assert non-object scalar cells remain plain text.

- [ ] **Step 2: Add failing stale-response tests**

  Use deferred bridge promises. Change the investigation revision before resolving ready, error, and unavailable responses, then assert none replace the currently rendered result/status. These tests mount only the focused panel, never production routes.

- [ ] **Step 3: Run the panel test and verify RED**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx --max-concurrency=1`

  Expected: FAIL because result cells are not navigable and panel responses are committed without a workspace/revision check.

- [ ] **Step 4: Implement navigation and response guards**

  Recognize case-insensitive `object_id`, `objectId`, and `@objectId` result columns. For valid hexadecimal object-ID cells, commit the ID to the investigation store with inspector origin and navigate to the object-inspector route. Capture `{ workspaceId, revision }` plus a local request sequence before every query; commit ready, error, or unavailable state only when both identities are still current.

- [ ] **Step 5: Render structured error coordinates**

  Display parser errors as an alert with a separate `Line N, column M` location. Keep location absent for execution/transport errors that do not provide a parser byte offset.

- [ ] **Step 6: Run all focused M28.A tests**

  Run: `cd ui && bun test src/features/heap-explorer/components/QueryConsolePanel.test.tsx src/features/heap-explorer/heap-explorer-query-client.test.ts src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: PASS.

#### Task 4: M28.A verification and implementation commits

- [ ] **Step 1: Run the related route adapter test**

  Run: `cd ui && bun test src/features/heap-explorer/HeapQueryConsolePage.test.tsx --max-concurrency=1`

  Expected: PASS without mounting the production route tree.

- [ ] **Step 2: Run TypeScript lint and production build**

  Run: `cd ui && bun run lint && bun run build`

  Expected: both commands exit 0.

- [ ] **Step 3: Review scope**

  Run: `git diff --check && git status --short`

  Expected: only M28.A-owned source/tests plus this plan are changed; untracked `.claude/skills/gitnexus-*` remain untouched.

- [ ] **Step 4: Commit implementation**

  ```bash
  git add ui/src/features/heap-explorer/components/QueryConsolePanel.tsx \
    ui/src/features/heap-explorer/components/QueryConsolePanel.test.tsx \
    ui/src/features/heap-explorer/heap-explorer-query-client.ts \
    ui/src/features/heap-explorer/heap-explorer-query-client.test.ts \
    ui/src/host/tauri-bridge.ts \
    ui/src/host/tauri-bridge.test.ts
  git commit -m "feat(ui): mature OQL workbench"
  ```

### M28.B — On-demand analyzers

**Owned files:**
- Create: `ui/src/features/artifact-explorer/analyzer-enrichment-client.ts`
- Create: `ui/src/features/artifact-explorer/analyzer-enrichment-client.test.ts`
- Create: `ui/src/features/artifact-explorer/components/AnalyzerEnrichmentPanel.tsx`
- Create: `ui/src/features/artifact-explorer/components/AnalyzerEnrichmentPanel.test.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.tsx`
- Modify: `ui/src/features/artifact-explorer/components/AnalyzerRail.tsx`
- Modify: `ui/src/features/artifact-explorer/ArtifactExplorerPage.test.tsx`
- Modify: `ui/src/features/policy/policy-bridge-client.ts`
- Modify: `ui/src/features/policy/PolicyCheckPage.tsx`
- Modify: `ui/src/features/policy/PolicyCheckPage.test.tsx`
- Modify: `ui/src/features/comparison/ComparisonPicker.tsx`
- Modify: `ui/src/features/comparison/ComparisonPicker.test.tsx`

**Interfaces:**
- `runAnalyzerEnrichment(selection)` sends exactly one `runDesktopAnalysis` host request with `mode: "custom"` and the existing `AnalyzeRequest`-backed analyzer flags. It accumulates already-present optional reports so a later enrichment does not silently discard prior analyzer results.
- The injected Tauri bridge remains the operation-ID acceptance boundary. The enrichment client additionally captures `{ workspaceId, revision }` and commits the returned artifact only while that workspace revision still matches.
- Field-data analyzers are exactly strings, collections, duplicate primitive arrays, and threads. Their preview must disclose a full host reparse with retained field/array bytes and potentially material peak-memory and duration cost. Referrers and classloaders remain deep graph analyzers but do not request field-data retention.
- The lean Home open remains unchanged: strings, collections, duplicate arrays, threads, and referrers stay `false`; classloaders and top instances retain their existing lean defaults.
- Analyzer outcomes never synthesize data. Missing bridge/source is `unavailable`; missing requested report after a successful response is `unavailable`; response-level `partial` and `fallback` provenance is rendered verbatim with its detail.
- Policy growth rules use the existing `baselineSourceId` contract. Baseline selection returns an opaque source ID plus display-safe name and must not replace the remembered current heap.
- M27 already shipped current/baseline snapshot pickers, identity strategy, top-N, leak cross-reference, match quality, and after-side navigation. M28.B adds only plain-language strategy/cost disclosure, especially that `FullFingerprint` reparses both heaps with retained field data.

#### Task 1: One revision-safe analyzer enrichment request

- [ ] **Step 1: Add failing client tests for one accumulated request**

  In `analyzer-enrichment-client.test.ts`, seed a remembered desktop source, a current artifact with `topInstances` and `classloaderReport`, and a bridge spy. Request strings, collections, and referrers together. Assert one host call with:

  ```ts
  {
    sourceId: "src-current",
    mode: "custom",
    enableClassloaders: true,
    enableThreads: false,
    enableStrings: true,
    enableCollections: true,
    enableTopInstances: true,
    enableByReferrer: true,
    enableDuplicateArrays: false,
  }
  ```

  Return a valid enriched artifact and assert it replaces the artifact facts without bumping the current investigation revision.

- [ ] **Step 2: Add failing stale/unavailable/provenance tests**

  Use a deferred bridge promise, change the investigation revision before resolving it, and assert the stale result does not replace the current artifact. Add missing-source and missing-bridge cases that return `unavailable`. Add a response with `Partial` and `Fallback` markers plus one missing requested report and assert the result preserves those markers and identifies the missing section as unavailable.

- [ ] **Step 3: Run the client test and verify RED**

  Run: `cd ui && bun test src/features/artifact-explorer/analyzer-enrichment-client.test.ts --max-concurrency=1`

  Expected: FAIL because no workspace analyzer-enrichment client exists.

- [ ] **Step 4: Implement the minimal enrichment client**

  Add:

  ```ts
  export type AnalyzerSelection = {
    strings: boolean;
    collections: boolean;
    duplicateArrays: boolean;
    threads: boolean;
    referrers: boolean;
    classloaders: boolean;
  };

  export type AnalyzerEnrichmentResult =
    | { status: "ready"; requested: string[]; unavailable: string[]; provenance: ArtifactProvenanceMarker[] }
    | { status: "stale" }
    | { status: "unavailable"; message: string }
    | { status: "error"; message: string };
  ```

  Read the remembered opaque source, current artifact/name, and current workspace/revision. Call `runDesktopAnalysis` once with the selected-or-already-present flags, parse with `parseAnalysisArtifact`, then re-read the investigation identity before calling `useArtifactStore.setState`. Do not bump the revision or mutate facts on stale, unavailable, or invalid responses.

- [ ] **Step 5: Run the client test and verify GREEN**

  Run: `cd ui && bun test src/features/artifact-explorer/analyzer-enrichment-client.test.ts --max-concurrency=1`

  Expected: PASS.

#### Task 2: Opt-in analyzer controls and honest outcomes

- [ ] **Step 1: Add failing focused panel tests**

  Render only `AnalyzerEnrichmentPanel`. Assert all six analyzer choices are opt-in and unselected initially. Selecting strings plus referrers must show that strings require retained field data while referrers do not. Before confirmation, assert the preview says the host reparses the current heap, retains field/array bytes, and may materially increase peak memory and duration.

- [ ] **Step 2: Add failing one-submit and outcome-label tests**

  Confirm once and assert the panel calls `runAnalyzerEnrichment` once with the complete selection. Cover `ready`, `stale`, `unavailable`, and `error` statuses. For ready responses, render requested-section availability and every returned `Partial`/`Fallback` marker and detail; never convert an unavailable report into an empty success.

- [ ] **Step 3: Run the panel test and verify RED**

  Run: `cd ui && bun test src/features/artifact-explorer/components/AnalyzerEnrichmentPanel.test.tsx --max-concurrency=1`

  Expected: FAIL because the opt-in panel does not exist.

- [ ] **Step 4: Implement and mount the focused panel**

  Add the panel above the analyzer rail in `ArtifactExplorerPage.tsx`. Keep the submit button disabled until at least one analyzer is selected and while a request is running. Field-data choices must require an explicit confirmation step after the preview; non-field choices may run directly. The page continues to mount its existing artifact-backed detail panels and never mounts a production route tree in tests.

- [ ] **Step 5: Prove lean open remains unchanged**

  Run: `cd ui && bun test src/features/artifact-loader/ArtifactLoaderPage.test.tsx --max-concurrency=1`

  Expected: PASS, including the existing assertion that first open sends `false` for threads, strings, collections, referrers, and duplicate arrays.

- [ ] **Step 6: Run focused explorer tests**

  Run: `cd ui && bun test src/features/artifact-explorer/components/AnalyzerEnrichmentPanel.test.tsx src/features/artifact-explorer/ArtifactExplorerPage.test.tsx --max-concurrency=1`

  Expected: PASS.

#### Task 3: Recommendation content and analyzer-state honesty

- [ ] **Step 1: Add failing analyzer-rail assertions**

  Extend `ArtifactExplorerPage.test.tsx` so the recommendation card renders the actual recommendation text, not only its count. Add response-level `Partial` and `Fallback` provenance fixtures and assert their labels/details are visible. Assert absent optional analyzer sections use an `UNAVAILABLE` label rather than implying that an analyzer ran and found zero rows.

- [ ] **Step 2: Run the explorer test and verify RED**

  Run: `cd ui && bun test src/features/artifact-explorer/ArtifactExplorerPage.test.tsx --max-concurrency=1`

  Expected: FAIL because recommendations are count-only and absent sections use `SECTION_ABSENT`.

- [ ] **Step 3: Render bounded recommendation content and provenance**

  In `AnalyzerRail.tsx`, render up to the first three recommendation strings as list content and disclose the remaining count. Render response-level partial/fallback provenance markers with their details. Rename the absent-state badge to `UNAVAILABLE`; preserve distinct `EMPTY` for analyzers that ran successfully and returned no findings.

- [ ] **Step 4: Run the explorer test and verify GREEN**

  Run: `cd ui && bun test src/features/artifact-explorer/ArtifactExplorerPage.test.tsx --max-concurrency=1`

  Expected: PASS.

#### Task 4: Policy baseline picker over opaque sources

- [ ] **Step 1: Add failing policy tests**

  In `PolicyCheckPage.test.tsx`, enter an `object_growth_threshold` policy and assert Run is blocked with an actionable baseline message until a baseline is selected. Mock the heap picker, select `baseline.hprof`, and assert `runCiCheck` receives `baselineSourceId: "src-baseline"` while the remembered current source remains `src-current`. Add cancelled and unavailable picker states.

- [ ] **Step 2: Run the policy test and verify RED**

  Run: `cd ui && bun test src/features/policy/PolicyCheckPage.test.tsx --max-concurrency=1`

  Expected: FAIL because the page never gathers or sends `baselineSourceId`.

- [ ] **Step 3: Add the baseline-source helper and picker**

  Add `pickDesktopBaselineSource()` to `policy-bridge-client.ts` as a thin wrapper around `pickHeapFile()` that returns only selected/cancelled/unavailable/error outcomes and never calls `rememberDesktopHeapSource`. In `PolicyCheckPage.tsx`, render the optional baseline picker, show only the host-provided display name, detect the exact `predicate = "object_growth_threshold"` declaration, require a baseline for that policy, and pass the opaque baseline source ID to `runCiCheck`.

- [ ] **Step 4: Run the policy test and verify GREEN**

  Run: `cd ui && bun test src/features/policy/PolicyCheckPage.test.tsx --max-concurrency=1`

  Expected: PASS.

#### Task 5: Compare identity-strategy disclosure only

- [ ] **Step 1: Add failing disclosure assertions**

  Extend `ComparisonPicker.test.tsx` to assert each existing identity-strategy choice has plain-language precision/trade-off copy. For `FullFingerprint`, require an explicit warning that both heaps are reparsed with retained field data and may use materially more memory and time.

- [ ] **Step 2: Run the picker test and verify RED**

  Run: `cd ui && bun test src/features/comparison/ComparisonPicker.test.tsx --max-concurrency=1`

  Expected: FAIL because the M27 control exists but does not explain strategy semantics or cost.

- [ ] **Step 3: Add disclosure without widening compare behavior**

  Add an immutable description map keyed by `IdentityStrategy` and render the active description below the existing select. Do not add another strategy control, change defaults, alter bridge inputs, or touch diff identity logic.

- [ ] **Step 4: Run the picker test and verify GREEN**

  Run: `cd ui && bun test src/features/comparison/ComparisonPicker.test.tsx --max-concurrency=1`

  Expected: PASS.

#### Task 6: M28.B verification and implementation commits

- [ ] **Step 1: Run all focused M28.B tests**

  Run:

  ```bash
  cd ui && bun test \
    src/features/artifact-explorer/analyzer-enrichment-client.test.ts \
    src/features/artifact-explorer/components/AnalyzerEnrichmentPanel.test.tsx \
    src/features/artifact-explorer/ArtifactExplorerPage.test.tsx \
    src/features/artifact-loader/ArtifactLoaderPage.test.tsx \
    src/features/policy/PolicyCheckPage.test.tsx \
    src/features/comparison/ComparisonPicker.test.tsx \
    src/host/tauri-bridge.test.ts \
    --max-concurrency=1
  ```

  Expected: PASS without mounting production routes.

- [ ] **Step 2: Run TypeScript lint and production build**

  Run: `cd ui && bun run lint && bun run build`

  Expected: both commands exit 0.

- [ ] **Step 3: Review scope**

  Run: `git diff --check && git status --short`

  Expected: only M28.B-owned source/tests plus this plan are changed; untracked `.claude/skills/gitnexus-*` remain untouched. GitNexus change detection is attempted only when its tools are available and never blocks this slice.

- [ ] **Step 4: Commit implementation**

  ```bash
  git add ui/src/features/artifact-explorer \
    ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx \
    ui/src/features/policy \
    ui/src/features/comparison/ComparisonPicker.tsx \
    ui/src/features/comparison/ComparisonPicker.test.tsx
  git commit -m "feat(ui): add on-demand analyzer enrichment"
  ```

### M28.C — Visualization and export

**Owned files:**
- Create: `ui/src/features/flamegraph/export-download.ts`
- Create: `ui/src/features/flamegraph/export-download.test.ts`
- Modify: `ui/src/features/flamegraph/FlamegraphPage.tsx`
- Modify: `ui/src/features/flamegraph/FlamegraphPage.test.tsx`
- Modify: `ui/src/features/artifact-loader/desktop-heap-client.ts`
- Modify: `ui/src/host/tauri-bridge.ts`
- Modify: `ui/src/host/tauri-bridge.test.ts`
- Modify: `tauri/src/state.rs`
- Modify: `tauri/src/commands.rs`
- Modify: `tauri/src/main.rs`

**Interfaces:**
- `DesktopHeapBridge.exportReport(input)` requests one of the existing core report formats (`text`, `markdown`, `html`, `toon`, `json`) for the active opaque heap source. The native host renders the most recently committed `AnalyzeResponse`; it does not accept a heap path or untrusted pre-rendered HTML from React.
- `generateFlamegraph(input)` keeps the existing command and result shape while the page exposes all shipped formats: `svg`, `folded-stack`, and `json`.
- Native export results use `{ format, content, mimeType, byteLength, mode, provenance }`. `content` is always a string at the React boundary, including JSON flamegraph output.
- `prepareExportDownload(...)` validates the allowlisted format, normalizes control characters, derives a basename-only filename, and returns a Blob download descriptor. HTML/SVG content is downloadable only; SVG preview remains an `<img src="blob:…">`, never `innerHTML`/`dangerouslySetInnerHTML`.
- Export labels show the active mode and every response-level provenance marker. Filenames contain only a sanitized heap basename, export kind, mode, and allowlisted extension.

#### Task 1: Safe filename/content and download contract

- [ ] **Step 1: Add failing filename and content tests**

  In `export-download.test.ts`, assert that a display name such as `../../<img src=x onerror=alert(1)>.hprof` produces a basename-only, extension-allowlisted filename with no `/`, `\`, `<`, `>`, quotes, or event-handler text. Cover empty/all-invalid names and a name longer than the exported filename limit.

  Assert that text control characters are removed except `\n`, `\r`, and `\t`; JSON object content is serialized to a string; and an unknown format is rejected rather than converted to a generic text file.

- [ ] **Step 2: Run the utility test and verify RED**

  Run: `cd ui && bun test src/features/flamegraph/export-download.test.ts --max-concurrency=1`

  Expected: FAIL because the export-download contract does not exist.

- [ ] **Step 3: Implement the minimal download contract**

  Add:

  ```ts
  export type ExportKind = "flamegraph" | "report";
  export type FlamegraphExportFormat = "svg" | "folded-stack" | "json";
  export type ReportExportFormat = "text" | "markdown" | "html" | "toon" | "json";

  export type ExportDownload = {
    filename: string;
    mimeType: string;
    content: string;
    blob: Blob;
  };
  ```

  Use an explicit kind/format → extension/MIME allowlist. Strip path components, remove a terminal `.hprof`/`.bin`/`.json`, normalize unsafe filename runs to `-`, trim leading/trailing dots/dashes, cap the basename, and fall back to `mnemosyne-heap`. Normalize content to a string and remove unsafe C0 controls without interpreting markup.

- [ ] **Step 4: Add and test the click-download helper**

  Add a helper that creates an object URL, assigns it to a temporary anchor with the sanitized `download` value, clicks it, removes it, and revokes the URL. Test those observable calls with DOM spies; do not use `innerHTML` or `dangerouslySetInnerHTML`.

- [ ] **Step 5: Run the utility test and verify GREEN**

  Run: `cd ui && bun test src/features/flamegraph/export-download.test.ts --max-concurrency=1`

  Expected: PASS.

#### Task 2: Native report-renderer export bridge

- [ ] **Step 1: Add failing bridge contract tests**

  In `tauri-bridge.test.ts`, inject a native invoke spy, call `exportReport({ sourceId, format })`, and assert one `export_desktop_report` invocation with the active M26 operation context. Assert the response is accepted only through the existing workspace/revision/operation-ID envelope.

- [ ] **Step 2: Run the bridge test and verify RED**

  Run: `cd ui && bun test src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: FAIL because `DesktopHeapBridge` and the injected host bridge do not expose `exportReport`.

- [ ] **Step 3: Add the native response cache and export command**

  Add `analysis: RwLock<Option<AnalyzeResponse>>` to `HeapSession`. Install the response only at the same guarded commit point that installs the current graph/dominator pair; clear it whenever the heap session is unloaded or replaced without an analysis response.

  Add `export_desktop_report` to `commands.rs` and register it in `main.rs`. Resolve the opaque source ID on the native side, verify it still matches the loaded heap, clone the last committed response, replace `summary.heap_path` with `display_name_for_path(...)`, and dispatch through existing `render_report(ReportRequest { analysis, format })`. Accept only the five built-in formats; reject custom/unknown values.

- [ ] **Step 4: Preserve mode/provenance and HTML escaping**

  Return the rendered content plus `mimeType`, `byteLength`, the response mode, and response-level provenance markers. Do not post-process HTML in React. Add focused Rust helper assertions in `commands.rs` proving an untrusted display name is escaped by the existing HTML renderer and the response metadata retains `mode` and provenance.

- [ ] **Step 5: Wire the typed desktop bridge**

  Extend `DesktopHeapBridge` in `desktop-heap-client.ts` and the Tauri injector in `tauri-bridge.ts`. Route export through `invokeOperation("analyze", ...)` so M26 workspace/revision/operation-ID rejection remains the acceptance boundary.

- [ ] **Step 6: Run focused bridge tests**

  Run: `cd ui && bun test src/host/tauri-bridge.test.ts --max-concurrency=1`

  Expected: PASS.

#### Task 3: Workspace flamegraph formats and safe report downloads

- [ ] **Step 1: Add failing flamegraph format tests**

  Extend `FlamegraphPage.test.tsx` to select each shipped format and assert the bridge receives exactly `svg`, `folded-stack`, or `json`. Assert SVG alone creates an `<img>` blob preview; folded-stack/JSON render metadata and a download action without mounting returned content.

- [ ] **Step 2: Add failing report export tests**

  Seed a remembered source and artifact, request HTML and JSON report exports, and assert `exportReport` receives only the opaque source ID and allowlisted format. Render a host result containing an HTML/script sentinel and assert it is absent from `container.innerHTML`, no script/event-handler node exists, and download uses the sanitized filename.

  Assert mode and each `Partial`/`Fallback` provenance detail are visible as text labels. Add missing-bridge and missing-current-analysis cases that stay explicitly unavailable.

- [ ] **Step 3: Run the focused page test and verify RED**

  Run: `cd ui && bun test src/features/flamegraph/FlamegraphPage.test.tsx --max-concurrency=1`

  Expected: FAIL because the page is SVG-only and has no report export controls.

- [ ] **Step 4: Implement flamegraph format selection**

  Add an allowlisted format selector beside the existing root selector. Normalize the host's JSON object payload into a downloadable string, preserve root/format/source metadata, preview SVG only through an object URL image, and revoke every replaced/unmounted URL.

- [ ] **Step 5: Implement report export controls**

  Add a separate report-format selector and generate/download flow backed by `exportReport`. Keep HTML and all other report bodies out of the DOM. Render mode and provenance metadata as React text nodes only.

- [ ] **Step 6: Run focused export tests and verify GREEN**

  Run:

  ```bash
  cd ui && bun test \
    src/features/flamegraph/export-download.test.ts \
    src/features/flamegraph/FlamegraphPage.test.tsx \
    src/host/tauri-bridge.test.ts \
    --max-concurrency=1
  ```

  Expected: PASS without mounting the production route tree.

#### Task 4: M28.C verification and implementation commit

- [ ] **Step 1: Run TypeScript lint and production build**

  Run: `cd ui && bun run lint && bun run build`

  Expected: both commands exit 0.

- [ ] **Step 2: Run focused native checks where this host permits**

  Run: `cargo test --manifest-path tauri/session-ops/Cargo.toml`

  Attempt: `cargo check --manifest-path tauri/Cargo.toml`

  Expected: session-operation tests pass. Record the desktop check as command-layer evidence only; if WSL lacks WebKitGTK/GTK system libraries, record that exact environment block and do not call the packaged GUI proven.

- [ ] **Step 3: Review scope**

  Run: `git diff --check && git status --short`

  Expected: only M28.C-owned source/tests plus this plan are changed; untracked `.claude/skills/gitnexus-*` remain untouched. GitNexus is non-blocking and omitted when unavailable.

- [ ] **Step 4: Commit implementation**

  ```bash
  git add tauri/src/state.rs tauri/src/commands.rs tauri/src/main.rs \
    ui/src/features/artifact-loader/desktop-heap-client.ts \
    ui/src/features/flamegraph/FlamegraphPage.tsx \
    ui/src/features/flamegraph/FlamegraphPage.test.tsx \
    ui/src/features/flamegraph/export-download.ts \
    ui/src/features/flamegraph/export-download.test.ts \
    ui/src/host/tauri-bridge.ts ui/src/host/tauri-bridge.test.ts
  git commit -m "feat(ui): add safe workspace exports"
  ```

#### Task 5: M28 closeout

- [ ] **Step 1: Create the evidence record**

  Create `docs/evidence/m28-power-tool-completeness.md` with exact M28.A (`856b89c`, `15f6a98`), M28.B (`2d1b4cc`, `7ca3e29`), and M28.C commit hashes; focused commands and results; export format/provenance/XSS assertions; and an explicit **NOT-PROVEN** row for packaged Windows/macOS/Linux GUI behavior.

- [ ] **Step 2: Synchronize status, matrix, and roadmap**

  Update `STATUS.md`, `docs/product/ui-capability-matrix.md`, and `docs/roadmap.md` to mark M28 closed with focused command/UI/native-contract evidence. Keep packaged GUI and native launch claims **NOT PROVEN** on WSL. Set the next roadmap milestone to M29 without starting or expanding M29 work.

- [ ] **Step 3: Check all M28 plan boxes**

  Mark M28.A, M28.B, and M28.C steps complete only where the referenced commits/evidence prove them. Leave any host-blocked launch proof described as a caveat rather than an unchecked hidden requirement.

- [ ] **Step 4: Verify documentation and scope**

  Run: `git diff --check && git status --short`

  Expected: closeout docs plus the plan are modified; untracked `.claude/skills/gitnexus-*` remain untouched.

- [ ] **Step 5: Commit closeout**

  ```bash
  git add docs/evidence/m28-power-tool-completeness.md \
    docs/superpowers/plans/2026-09-15-m28-power-tool-completeness.md \
    docs/product/ui-capability-matrix.md docs/roadmap.md STATUS.md
  git commit -m "docs: close M28 power-tool completeness"
  ```

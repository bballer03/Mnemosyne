# M6 — Tauri Desktop Packaging

> **Status:** ⚬ Pending  
> **Design Owner:** Design Consulting Agent  
> **Parent Milestone:** [Milestone 6 — Ecosystem, Community & UI Completion](milestone-6-ecosystem-and-community.md)  
> **Last Updated:** 2026-04-25

---

## Objective

Package the Mnemosyne browser-first UI as a native desktop application using Tauri v2. The Tauri backend calls `mnemosyne-core` functions directly (in-process Rust), exposes them to the frontend via `#[tauri::command]` IPC, and distributes platform-specific installers for macOS, Linux, and Windows through the existing GitHub Releases pipeline.

## Context

The `ui/` React frontend and the two host bridge contracts (`HeapExplorerHostBridge`, `LeakWorkspaceHostBridge`) are stable and shipped. In the browser, these bridges are wired to artifact loaders or MCP-backed providers. For the desktop app, the bridges instead call Tauri's `invoke()` IPC, which routes to in-process Rust commands that call `core::` APIs directly. No MCP stdio transport is involved — Tauri is in-process Rust, while MCP is an external-tool stdio JSON-RPC channel.

## Scope

- Tauri v2 project scaffolding under `tauri/`
- Tauri command implementations for all `HeapExplorerHostBridge` and `LeakWorkspaceHostBridge` methods
- `HeapSession` state management with `RwLock<Option<ObjectGraph>>`
- Heap load/unload lifecycle
- Bridge injection script that wires `window.__MNEMOSYNE_*_BRIDGE__` to `invoke()`
- Vite build integration
- CI release workflow for cross-platform builds

## Non-scope

- New analysis features or core API changes
- Code signing (Apple notarization, Windows Authenticode) — deferred
- Auto-update mechanism — deferred
- Custom window chrome, native menus, or file associations — deferred
- Changes to the existing browser-first UI flow

---

## 1. Architecture Decision: Direct Core Calls

Tauri commands call `core::` functions directly — not through the MCP stdio JSON-RPC transport.

**Rationale:**
- Tauri runs in-process Rust. Spawning a child process and serializing JSON-RPC over stdio would add latency, complexity, and fragility for zero benefit.
- MCP is designed for external tools (VS Code, Cursor, Zed) that communicate with Mnemosyne over stdio. The desktop app has direct access to the Rust API.
- Error handling is simpler: Tauri commands return `Result<T, String>` directly from core functions, rather than parsing wire-format error envelopes.

**State model:** The parsed `ObjectGraph`, `AppConfig`, and heap file path are held in Tauri managed state behind an `RwLock`, enabling concurrent read access from multiple frontend queries.

---

## 2. Tauri Project Structure

```
tauri/
├── Cargo.toml            # tauri app crate, depends on mnemosyne-core
├── tauri.conf.json        # window config, app identifier, build commands
├── src/
│   ├── main.rs            # tauri::Builder, state init, command registration
│   ├── commands.rs        # #[tauri::command] functions
│   └── state.rs           # HeapSession struct + RwLock management
└── icons/                 # platform app icons (png, ico, icns)
```

The `tauri/Cargo.toml` declares a dependency on `mnemosyne-core` via workspace-relative path:

```toml
[dependencies]
mnemosyne-core = { path = "../core" }
tauri = { version = "2", features = ["devtools"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

The root `Cargo.toml` workspace members list gains `"tauri"`.

---

## 3. Command-to-Bridge Mapping

### HeapExplorerHostBridge

| Bridge method | Tauri command | Core call |
|---|---|---|
| `queryHeap(input)` | `query_heap` | `core::query::execute_query()` on the loaded `ObjectGraph` |
| `getReferences(objectId)` | `get_references` | `ObjectGraph::get_references(id)` |
| `getReferrers(objectId)` | `get_referrers` | `ObjectGraph::get_referrers(id)` |

```rust
#[tauri::command]
fn query_heap(
    input: HeapQueryInput,
    state: State<'_, HeapSession>,
) -> Result<QueryResult, String> { ... }

#[tauri::command]
fn get_references(
    object_id: String,
    state: State<'_, HeapSession>,
) -> Result<ObjectReferencesResult, String> { ... }

#[tauri::command]
fn get_referrers(
    object_id: String,
    state: State<'_, HeapSession>,
) -> Result<ObjectReferrersResult, String> { ... }
```

### LeakWorkspaceHostBridge

| Bridge method | Tauri command | Core call |
|---|---|---|
| `explainLeak(input)` | `explain_leak` | `core::analysis::analyze_heap()` filtered to leak |
| `findGcPath(input)` | `find_gc_path` | `core::graph::find_gc_path()` |
| `mapToCode(input)` | `map_to_code` | `core::mapper::map_to_code()` |
| `proposeFix(input)` | `propose_fix` | `core::fix::propose_fix_with_config()` |
| `capabilities(heapPath)` | `capabilities` | Returns current feature flags |

### Lifecycle Commands

| Command | Purpose |
|---|---|
| `load_heap(path: String)` | Parse HPROF file via `core::hprof::parse_hprof_file()`, store `ObjectGraph` in state |
| `unload_heap()` | Clear state, drop `ObjectGraph` |

---

## 4. Bridge Injection Pattern

The UI's `window.__MNEMOSYNE_*_BRIDGE__` globals receive implementations that call Tauri's IPC. A Tauri-specific init script is injected before the app loads:

```ts
import { invoke } from "@tauri-apps/api/core";

window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
  queryHeap: (input) => invoke("query_heap", { input }),
  getReferences: (objectId) => invoke("get_references", { objectId }),
  getReferrers: (objectId) => invoke("get_referrers", { objectId }),
};

window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
  capabilities: { provider: "ready" },
  explainLeak: (input) => invoke("explain_leak", { input }),
  findGcPath: (input) => invoke("find_gc_path", { input }),
  mapToCode: (input) => invoke("map_to_code", { input }),
  proposeFix: (input) => invoke("propose_fix", { input }),
};
```

This script is loaded conditionally — only when running inside Tauri (detected via `window.__TAURI_INTERNALS__`). The browser-first artifact-loader flow remains untouched.

---

## 5. State Management

```rust
use std::sync::RwLock;
use mnemosyne_core::hprof::ObjectGraph;
use mnemosyne_core::config::AppConfig;

pub struct HeapSession {
    pub graph: RwLock<Option<ObjectGraph>>,
    pub config: RwLock<AppConfig>,
    pub heap_path: RwLock<Option<String>>,
}
```

**Lifecycle:**

1. **App start** — `HeapSession` initialized with all fields `None` / default.
2. **`load_heap(path)`** — Parses the HPROF file, acquires write lock, stores the `ObjectGraph` and path. Returns a summary (object count, class count, GC root count) to the frontend.
3. **Query commands** — Acquire read lock on the graph. If `None`, return a typed error: `"No heap loaded"`.
4. **`unload_heap()`** — Acquires write lock, drops the `ObjectGraph`, clears the path.

**Concurrency:** `RwLock` permits multiple concurrent reads (query, references, referrers) while blocking only during load/unload. Since HPROF parsing is CPU-bound and single-threaded, the write lock is held briefly only to swap the `Option`.

**Error contract:** All commands return `Result<T, String>`. The `String` error is displayed by the frontend's existing error handling. Structured error types can be added later if needed.

---

## 6. Vite / Build Integration

The existing `ui/` directory stays as the frontend source. Tauri's build config points to it:

```jsonc
// tauri/tauri.conf.json (relevant sections)
{
  "build": {
    "beforeDevCommand": "cd ../ui && bun run dev",
    "devUrl": "http://localhost:5173",
    "beforeBuildCommand": "cd ../ui && bun run build",
    "frontendDist": "../ui/dist"
  },
  "app": {
    "title": "Mnemosyne",
    "identifier": "com.mnemosyne.app"
  },
  "bundle": {
    "active": true,
    "targets": "all"
  }
}
```

`ui/package.json` gains two scripts:

```json
{
  "tauri:dev": "cd ../tauri && cargo tauri dev",
  "tauri:build": "cd ../tauri && cargo tauri build"
}
```

The `@tauri-apps/api` npm package is added as a dependency in `ui/package.json` for `invoke()` access.

---

## 7. CI Release Workflow

Use `tauri-apps/tauri-action@v0` in the existing release workflow:

```yaml
tauri-release:
  strategy:
    matrix:
      include:
        - platform: macos-latest
          target: aarch64-apple-darwin
        - platform: macos-latest
          target: x86_64-apple-darwin
        - platform: ubuntu-22.04
          target: x86_64-unknown-linux-gnu
        - platform: windows-latest
          target: x86_64-pc-windows-msvc
  runs-on: ${{ matrix.platform }}
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with:
        node-version: 20
    - name: Install Bun
      uses: oven-sh/setup-bun@v2
    - name: Install frontend deps
      run: cd ui && bun install
    - name: Install Linux deps
      if: matrix.platform == 'ubuntu-22.04'
      run: |
        sudo apt-get update
        sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev
    - uses: tauri-apps/tauri-action@v0
      with:
        projectPath: tauri
        tagName: v__VERSION__
        releaseName: "Mnemosyne v__VERSION__"
```

Tauri installers (`.dmg`, `.AppImage`, `.msi`) are attached to the GitHub Release alongside the existing CLI binaries and Docker image.

**Code signing:** Deferred. Initial releases ship unsigned. Apple notarization and Windows Authenticode signing are documented as future work in section 9.

---

## 8. Platform Notes

| Platform | WebView runtime | Additional build deps |
|---|---|---|
| **Linux** | WebKitGTK 4.1 | `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev` |
| **Windows** | WebView2 (pre-installed Win10+) | None (WebView2 bootstrapper bundled by Tauri) |
| **macOS** | System WebKit (WKWebView) | None |

All platforms require the Rust toolchain and the Tauri CLI (`cargo install tauri-cli`).

---

## 9. Deferred Concerns

| Concern | Notes |
|---|---|
| **Code signing** | Apple notarization requires a Developer ID certificate + `notarytool`. Windows Authenticode requires an EV code signing certificate. Both should be set up before public distribution but are not required for initial builds. |
| **Auto-update** | Tauri v2 has a built-in updater plugin. Requires a signed update manifest hosted at a stable URL. |
| **Custom window chrome** | Tauri supports `decorations: false` for frameless windows with custom title bars. Not needed initially. |
| **Native menus** | File → Open, recent files, keyboard shortcuts. Can be added incrementally. |
| **File associations** | Register `.hprof` file extension to open in Mnemosyne. Platform-specific configuration. |
| **Deep linking** | URL scheme (`mnemosyne://open?path=...`) for IDE integration. |

---

## Validation / Testing Strategy

- **Unit tests** for `commands.rs`: mock `HeapSession` state, verify command output shapes match bridge type contracts.
- **Integration test**: load a synthetic HPROF fixture via `load_heap`, run `query_heap`, `get_references`, `get_referrers`, verify correct results.
- **Manual smoke test**: build Tauri app, open a real heap dump, verify all bridge operations work end-to-end.
- **CI**: Tauri build matrix compiles on all three platforms. Build failures block the release.

## Risks and Open Questions

| Risk | Mitigation |
|---|---|
| Large heap dumps may cause UI freezes during `load_heap` | Run parsing on a background thread; show a loading indicator. Tauri async commands already run on a thread pool. |
| `ObjectGraph` memory usage for multi-GB dumps | Same constraints as CLI — the Rust core handles this. The desktop app inherits the core's memory characteristics. |
| WebView2 not installed on older Windows | Tauri v2 bundles the WebView2 bootstrapper by default. |
| Cross-compilation complexity | Use platform-native CI runners (matrix strategy) rather than cross-compilation. |

## Rollout / Implementation Phases

1. **Phase 1 — Scaffold** — Create `tauri/` directory, `Cargo.toml`, `tauri.conf.json`, `main.rs` with empty command set. Verify `cargo tauri dev` launches the existing UI.
2. **Phase 2 — State + load/unload** — Implement `HeapSession`, `load_heap`, `unload_heap`. Verify HPROF parsing works through Tauri.
3. **Phase 3 — HeapExplorerHostBridge commands** — Implement `query_heap`, `get_references`, `get_referrers`. Wire bridge injection script.
4. **Phase 4 — LeakWorkspaceHostBridge commands** — Implement `explain_leak`, `find_gc_path`, `map_to_code`, `propose_fix`, `capabilities`.
5. **Phase 5 — CI release** — Add Tauri build matrix to GitHub Actions. Attach installers to releases.
6. **Phase 6 — Polish** — Loading indicators, error toasts, file-open dialog integration.

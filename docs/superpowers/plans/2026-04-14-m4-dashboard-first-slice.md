# M4 Dashboard First Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a browser-first React dashboard that loads Mnemosyne analysis JSON artifacts and delivers the first real M4 leak-triage UI surface, while establishing a shared frontend foundation for later Tauri packaging.

**Architecture:** Add a dedicated `ui/` frontend workspace managed with Bun, keep Rust as the analysis source of truth, and consume the existing serialized `AnalyzeResponse` JSON artifact through a thin typed adapter layer instead of introducing a live API. The first route is a leak-triage dashboard with an app shell, artifact loader, summary strip, leak table, and compact graph/histogram panels.

**Tech Stack:** Bun, React, TypeScript, Vite, React Router, TanStack Query, TanStack Table, Zustand, Tailwind CSS, shadcn/ui, Vitest, Testing Library, existing Rust CLI/core tests.

---

### Task 1: Create the Frontend Workspace and Toolchain Skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `ui/package.json`
- Create: `ui/bun.lock`
- Create: `ui/tsconfig.json`
- Create: `ui/tsconfig.app.json`
- Create: `ui/vite.config.ts`
- Create: `ui/index.html`
- Create: `ui/src/main.tsx`
- Create: `ui/src/app/App.tsx`
- Create: `ui/src/app/router.tsx`
- Create: `ui/src/app/providers.tsx`
- Create: `ui/src/app/globals.css`
- Create: `ui/src/test/setup.ts`
- Create: `ui/src/app/App.test.tsx`

- [ ] **Step 1: Add the failing frontend smoke test first**

Create `ui/src/app/App.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { App } from "./App";

describe("App", () => {
  it("renders the Mnemosyne app shell heading", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { name: /mnemosyne/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/load analysis artifact/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Add the Bun package manifest and scripts**

Create `ui/package.json`:

```json
{
  "name": "mnemosyne-ui",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "test": "vitest run",
    "test:watch": "vitest",
    "lint": "tsc --noEmit"
  },
  "dependencies": {
    "@tanstack/react-query": "^5.51.0",
    "@tanstack/react-table": "^8.20.5",
    "clsx": "^2.1.1",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-router-dom": "^6.26.1",
    "zustand": "^4.5.5"
  },
  "devDependencies": {
    "@testing-library/jest-dom": "^6.4.8",
    "@testing-library/react": "^16.0.0",
    "@testing-library/user-event": "^14.5.2",
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "autoprefixer": "^10.4.20",
    "postcss": "^8.4.41",
    "tailwindcss": "^3.4.10",
    "typescript": "^5.5.4",
    "vite": "^5.4.1",
    "vitest": "^2.0.5"
  }
}
```

- [ ] **Step 3: Add TypeScript, Vite, and test setup files**

Create `ui/tsconfig.json`:

```json
{
  "files": [],
  "references": [{ "path": "./tsconfig.app.json" }]
}
```

Create `ui/tsconfig.app.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "Bundler",
    "allowImportingTsExtensions": false,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    },
    "types": ["vitest/globals"]
  },
  "include": ["src"]
}
```

Create `ui/vite.config.ts`:

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
});
```

Create `ui/src/test/setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 4: Add the minimal app shell implementation**

Create `ui/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Mnemosyne UI</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

Create `ui/src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";

import { App } from "@/app/App";
import "@/app/globals.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

Create `ui/src/app/providers.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PropsWithChildren, useState } from "react";

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            retry: false,
          },
        },
      }),
  );

  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}
```

Create `ui/src/app/router.tsx`:

```tsx
import { createBrowserRouter, RouterProvider } from "react-router-dom";

const router = createBrowserRouter([
  {
    path: "/",
    element: <div>Load analysis artifact</div>,
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
```

Create `ui/src/app/App.tsx`:

```tsx
import { AppProviders } from "./providers";
import { AppRouter } from "./router";

export function App() {
  return (
    <AppProviders>
      <div>
        <header>
          <h1>Mnemosyne</h1>
          <p>Load analysis artifact</p>
        </header>
        <AppRouter />
      </div>
    </AppProviders>
  );
}
```

Create `ui/src/app/globals.css`:

```css
:root {
  color-scheme: dark;
  font-family: Inter, system-ui, sans-serif;
  background: #0a0c10;
  color: #e2e2e8;
}

body {
  margin: 0;
  background: #0a0c10;
  color: #e2e2e8;
}

* {
  box-sizing: border-box;
}
```

- [ ] **Step 5: Install dependencies with Bun and generate `bun.lock`**

Run: `bun install`

Expected: install succeeds and `ui/bun.lock` is created.

- [ ] **Step 6: Run the frontend smoke test and verify it passes**

Run: `bun test`

Expected: PASS with the app shell heading test green.

- [ ] **Step 7: Commit the frontend foundation**

```bash
git add Cargo.toml ui
git commit -m "feat: scaffold M4 frontend workspace with bun"
```

### Task 2: Add Typed Artifact Loading for Real `AnalyzeResponse` JSON

**Files:**
- Create: `ui/src/lib/analysis-types.ts`
- Create: `ui/src/lib/analysis-types.test.ts`
- Create: `ui/src/features/artifact-loader/load-analysis-artifact.ts`
- Create: `ui/src/features/artifact-loader/load-analysis-artifact.test.ts`
- Create: `ui/src/features/artifact-loader/use-artifact-store.ts`

- [ ] **Step 1: Write a failing type-adapter test for a valid artifact**

Create `ui/src/lib/analysis-types.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { parseAnalysisArtifact } from "./analysis-types";

describe("parseAnalysisArtifact", () => {
  it("accepts a valid Mnemosyne analysis artifact", () => {
    const parsed = parseAnalysisArtifact({
      summary: {
        heap_path: "heap.hprof",
        total_objects: 42,
        total_size_bytes: 2048,
        classes: [],
        generated_at: "2026-04-14T00:00:00Z",
        header: null,
        total_records: 2,
        record_stats: [],
      },
      leaks: [
        {
          id: "leak-1",
          class_name: "com.example.Cache",
          leak_kind: "CACHE",
          severity: "HIGH",
          retained_size_bytes: 1024,
          shallow_size_bytes: 64,
          suspect_score: 0.98,
          instances: 4,
          description: "Cache retains request objects",
          provenance: [],
        },
      ],
      recommendations: [],
      elapsed: { secs: 1, nanos: 0 },
      graph: {
        node_count: 200,
        edge_count: 400,
        dominators: [],
      },
      histogram: {
        group_by: "class",
        entries: [],
        total_instances: 42,
        total_shallow_size: 2048,
      },
      provenance: [],
    });

    expect(parsed.summary.heapPath).toBe("heap.hprof");
    expect(parsed.leaks).toHaveLength(1);
    expect(parsed.graph.nodeCount).toBe(200);
  });
});
```

- [ ] **Step 2: Run the adapter test and verify it fails because the parser does not exist yet**

Run: `bun test ui/src/lib/analysis-types.test.ts`

Expected: FAIL with module/function missing errors.

- [ ] **Step 3: Implement the thin typed adapter layer**

Create `ui/src/lib/analysis-types.ts`:

```ts
export type DashboardSeverity = "LOW" | "MEDIUM" | "HIGH" | "CRITICAL";

export type DashboardArtifact = {
  summary: {
    heapPath: string;
    totalObjects: number;
    totalSizeBytes: number;
    generatedAt?: string;
    totalRecords: number;
  };
  leaks: Array<{
    id: string;
    className: string;
    leakKind: string;
    severity: DashboardSeverity;
    retainedSizeBytes: number;
    shallowSizeBytes?: number;
    suspectScore?: number;
    instances: number;
    description: string;
    provenance: Array<{ kind: string; detail?: string }>;
  }>;
  elapsedSeconds: number;
  graph: {
    nodeCount: number;
    edgeCount: number;
    dominatorCount: number;
  };
  histogram?: {
    groupBy: string;
    totalInstances: number;
    totalShallowSize: number;
    entries: Array<{
      key: string;
      instanceCount: number;
      shallowSize: number;
      retainedSize: number;
    }>;
  };
  provenance: Array<{ kind: string; detail?: string }>;
};

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function parseAnalysisArtifact(input: unknown): DashboardArtifact {
  if (!isObject(input) || !isObject(input.summary) || !Array.isArray(input.leaks) || !isObject(input.graph)) {
    throw new Error("Invalid Mnemosyne analysis artifact");
  }

  const elapsed = isObject(input.elapsed) && typeof input.elapsed.secs === "number"
    ? input.elapsed.secs + ((typeof input.elapsed.nanos === "number" ? input.elapsed.nanos : 0) / 1_000_000_000)
    : 0;

  return {
    summary: {
      heapPath: String(input.summary.heap_path ?? ""),
      totalObjects: Number(input.summary.total_objects ?? 0),
      totalSizeBytes: Number(input.summary.total_size_bytes ?? 0),
      generatedAt: typeof input.summary.generated_at === "string" ? input.summary.generated_at : undefined,
      totalRecords: Number(input.summary.total_records ?? 0),
    },
    leaks: input.leaks.map((leak) => ({
      id: String((leak as Record<string, unknown>).id ?? ""),
      className: String((leak as Record<string, unknown>).class_name ?? ""),
      leakKind: String((leak as Record<string, unknown>).leak_kind ?? "UNKNOWN"),
      severity: String((leak as Record<string, unknown>).severity ?? "LOW") as DashboardSeverity,
      retainedSizeBytes: Number((leak as Record<string, unknown>).retained_size_bytes ?? 0),
      shallowSizeBytes:
        typeof (leak as Record<string, unknown>).shallow_size_bytes === "number"
          ? Number((leak as Record<string, unknown>).shallow_size_bytes)
          : undefined,
      suspectScore:
        typeof (leak as Record<string, unknown>).suspect_score === "number"
          ? Number((leak as Record<string, unknown>).suspect_score)
          : undefined,
      instances: Number((leak as Record<string, unknown>).instances ?? 0),
      description: String((leak as Record<string, unknown>).description ?? ""),
      provenance: Array.isArray((leak as Record<string, unknown>).provenance)
        ? ((leak as Record<string, unknown>).provenance as Array<Record<string, unknown>>).map((marker) => ({
            kind: String(marker.kind ?? "UNKNOWN"),
            detail: typeof marker.detail === "string" ? marker.detail : undefined,
          }))
        : [],
    })),
    elapsedSeconds: elapsed,
    graph: {
      nodeCount: Number(input.graph.node_count ?? 0),
      edgeCount: Number(input.graph.edge_count ?? 0),
      dominatorCount: Array.isArray(input.graph.dominators) ? input.graph.dominators.length : 0,
    },
    histogram:
      isObject(input.histogram) && Array.isArray(input.histogram.entries)
        ? {
            groupBy: String(input.histogram.group_by ?? "class"),
            totalInstances: Number(input.histogram.total_instances ?? 0),
            totalShallowSize: Number(input.histogram.total_shallow_size ?? 0),
            entries: input.histogram.entries.map((entry: Record<string, unknown>) => ({
              key: String(entry.key ?? ""),
              instanceCount: Number(entry.instance_count ?? 0),
              shallowSize: Number(entry.shallow_size ?? 0),
              retainedSize: Number(entry.retained_size ?? 0),
            })),
          }
        : undefined,
    provenance: Array.isArray(input.provenance)
      ? (input.provenance as Array<Record<string, unknown>>).map((marker) => ({
          kind: String(marker.kind ?? "UNKNOWN"),
          detail: typeof marker.detail === "string" ? marker.detail : undefined,
        }))
      : [],
  };
}
```

- [ ] **Step 4: Add a failing loader test for malformed JSON**

Create `ui/src/features/artifact-loader/load-analysis-artifact.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { loadAnalysisArtifactFromText } from "./load-analysis-artifact";

describe("loadAnalysisArtifactFromText", () => {
  it("throws a readable error for malformed JSON", () => {
    expect(() => loadAnalysisArtifactFromText("not-json")).toThrow(/invalid json/i);
  });
});
```

- [ ] **Step 5: Run the loader test and verify it fails because the loader does not exist yet**

Run: `bun test ui/src/features/artifact-loader/load-analysis-artifact.test.ts`

Expected: FAIL with missing module/function errors.

- [ ] **Step 6: Implement the loader and store**

Create `ui/src/features/artifact-loader/load-analysis-artifact.ts`:

```ts
import { DashboardArtifact, parseAnalysisArtifact } from "@/lib/analysis-types";

export function loadAnalysisArtifactFromText(text: string): DashboardArtifact {
  let parsed: unknown;

  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("Invalid JSON artifact");
  }

  return parseAnalysisArtifact(parsed);
}
```

Create `ui/src/features/artifact-loader/use-artifact-store.ts`:

```ts
import { create } from "zustand";

import { DashboardArtifact } from "@/lib/analysis-types";

type ArtifactState = {
  artifactName?: string;
  artifact?: DashboardArtifact;
  loadError?: string;
  setArtifact: (artifactName: string, artifact: DashboardArtifact) => void;
  setLoadError: (message: string) => void;
  reset: () => void;
};

export const useArtifactStore = create<ArtifactState>((set) => ({
  artifactName: undefined,
  artifact: undefined,
  loadError: undefined,
  setArtifact: (artifactName, artifact) => set({ artifactName, artifact, loadError: undefined }),
  setLoadError: (message) => set({ loadError: message, artifact: undefined }),
  reset: () => set({ artifactName: undefined, artifact: undefined, loadError: undefined }),
}));
```

- [ ] **Step 7: Run the artifact tests and verify they pass**

Run: `bun test ui/src/lib/analysis-types.test.ts ui/src/features/artifact-loader/load-analysis-artifact.test.ts`

Expected: PASS.

- [ ] **Step 8: Commit the artifact-loading foundation**

```bash
git add ui/src/lib/analysis-types.ts ui/src/lib/analysis-types.test.ts ui/src/features/artifact-loader/load-analysis-artifact.ts ui/src/features/artifact-loader/load-analysis-artifact.test.ts ui/src/features/artifact-loader/use-artifact-store.ts
git commit -m "feat: add M4 analysis artifact loading"
```

### Task 3: Build the App Shell and Artifact Loader Route

**Files:**
- Modify: `ui/src/app/App.tsx`
- Modify: `ui/src/app/router.tsx`
- Create: `ui/src/features/artifact-loader/ArtifactLoaderPage.tsx`
- Create: `ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`
- Create: `ui/src/features/artifact-loader/ArtifactDropzone.tsx`

- [ ] **Step 1: Write a failing component test for loading a valid artifact into the app shell**

Create `ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { ArtifactLoaderPage } from "./ArtifactLoaderPage";

describe("ArtifactLoaderPage", () => {
  it("shows the selected artifact name after a valid JSON load", async () => {
    const user = userEvent.setup();
    render(<ArtifactLoaderPage />);

    const file = new File(
      [
        JSON.stringify({
          summary: {
            heap_path: "fixture.hprof",
            total_objects: 42,
            total_size_bytes: 2048,
            classes: [],
            generated_at: "2026-04-14T00:00:00Z",
            header: null,
            total_records: 2,
            record_stats: [],
          },
          leaks: [],
          recommendations: [],
          elapsed: { secs: 1, nanos: 0 },
          graph: { node_count: 1, edge_count: 2, dominators: [] },
          provenance: [],
        }),
      ],
      "fixture.json",
      { type: "application/json" },
    );

    const input = screen.getByLabelText(/analysis json artifact/i);
    await user.upload(input, file);

    expect(screen.getByText(/fixture\.json/i)).toBeInTheDocument();
    expect(screen.getByText(/artifact loaded/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the loader page test and verify it fails because the page does not exist yet**

Run: `bun test ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`

Expected: FAIL with missing module/component errors.

- [ ] **Step 3: Implement the artifact loader UI**

Create `ui/src/features/artifact-loader/ArtifactDropzone.tsx`:

```tsx
type ArtifactDropzoneProps = {
  onFileSelected: (file: File) => void;
};

export function ArtifactDropzone({ onFileSelected }: ArtifactDropzoneProps) {
  return (
    <label>
      <span>Analysis JSON artifact</span>
      <input
        aria-label="Analysis JSON artifact"
        type="file"
        accept="application/json,.json"
        onChange={(event) => {
          const file = event.currentTarget.files?.[0];
          if (file) onFileSelected(file);
        }}
      />
    </label>
  );
}
```

Create `ui/src/features/artifact-loader/ArtifactLoaderPage.tsx`:

```tsx
import { useState } from "react";

import { loadAnalysisArtifactFromText } from "./load-analysis-artifact";
import { ArtifactDropzone } from "./ArtifactDropzone";
import { useArtifactStore } from "./use-artifact-store";

export function ArtifactLoaderPage() {
  const { artifactName, artifact, loadError, setArtifact, setLoadError } = useArtifactStore();
  const [isLoading, setIsLoading] = useState(false);

  async function handleFile(file: File) {
    setIsLoading(true);
    try {
      const text = await file.text();
      const parsed = loadAnalysisArtifactFromText(text);
      setArtifact(file.name, parsed);
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : "Failed to load artifact");
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <section>
      <h2>Load analysis artifact</h2>
      <p>Open a Mnemosyne JSON analysis artifact to start dashboard triage.</p>
      <ArtifactDropzone onFileSelected={handleFile} />
      {isLoading ? <p>Loading artifact...</p> : null}
      {artifactName ? <p>Artifact loaded: {artifactName}</p> : null}
      {artifact ? <p>Heap: {artifact.summary.heapPath}</p> : null}
      {loadError ? <p role="alert">{loadError}</p> : null}
    </section>
  );
}
```

- [ ] **Step 4: Wire the route into the app shell**

Update `ui/src/app/router.tsx`:

```tsx
import { createBrowserRouter, RouterProvider } from "react-router-dom";

import { ArtifactLoaderPage } from "@/features/artifact-loader/ArtifactLoaderPage";

const router = createBrowserRouter([
  {
    path: "/",
    element: <ArtifactLoaderPage />,
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
```

Update `ui/src/app/App.tsx`:

```tsx
import { AppProviders } from "./providers";
import { AppRouter } from "./router";

export function App() {
  return (
    <AppProviders>
      <div>
        <header>
          <h1>Mnemosyne</h1>
          <p>Heap analysis console</p>
        </header>
        <AppRouter />
      </div>
    </AppProviders>
  );
}
```

- [ ] **Step 5: Run the artifact loader tests and verify they pass**

Run: `bun test ui/src/app/App.test.tsx ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`

Expected: PASS.

- [ ] **Step 6: Commit the app shell and loader route**

```bash
git add ui/src/app/App.tsx ui/src/app/router.tsx ui/src/features/artifact-loader/ArtifactLoaderPage.tsx ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx ui/src/features/artifact-loader/ArtifactDropzone.tsx
git commit -m "feat: add M4 app shell and artifact loader"
```

### Task 4: Build the Dashboard Route and Leak-Triage Components

**Files:**
- Create: `ui/src/features/dashboard/DashboardPage.tsx`
- Create: `ui/src/features/dashboard/components/SummaryStrip.tsx`
- Create: `ui/src/features/dashboard/components/LeakTable.tsx`
- Create: `ui/src/features/dashboard/components/GraphMetricsPanel.tsx`
- Create: `ui/src/features/dashboard/components/HistogramPanel.tsx`
- Create: `ui/src/features/dashboard/components/ProvenanceBadge.tsx`
- Create: `ui/src/features/dashboard/dashboard-store.ts`
- Create: `ui/src/features/dashboard/DashboardPage.test.tsx`
- Create: `ui/src/features/dashboard/components/LeakTable.test.tsx`
- Modify: `ui/src/app/router.tsx`

- [ ] **Step 1: Write a failing dashboard route test using real adapted data**

Create `ui/src/features/dashboard/DashboardPage.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DashboardPage } from "./DashboardPage";
import { useArtifactStore } from "@/features/artifact-loader/use-artifact-store";

describe("DashboardPage", () => {
  it("renders summary metrics and top leak section from loaded artifact", () => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
        summary: {
          heapPath: "fixture.hprof",
          totalObjects: 42,
          totalSizeBytes: 2048,
          generatedAt: "2026-04-14T00:00:00Z",
          totalRecords: 2,
        },
        leaks: [
          {
            id: "leak-1",
            className: "com.example.Cache",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 1024,
            shallowSizeBytes: 64,
            suspectScore: 0.98,
            instances: 4,
            description: "Cache retains request objects",
            provenance: [],
          },
        ],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 200,
          edgeCount: 400,
          dominatorCount: 10,
        },
        histogram: {
          groupBy: "class",
          totalInstances: 42,
          totalShallowSize: 2048,
          entries: [],
        },
        provenance: [],
      },
    });

    render(<DashboardPage />);

    expect(screen.getByRole("heading", { name: /mnemosyne triage dashboard/i })).toBeInTheDocument();
    expect(screen.getByText(/top leak suspects/i)).toBeInTheDocument();
    expect(screen.getByText(/fixture\.hprof/i)).toBeInTheDocument();
    expect(screen.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the dashboard test and verify it fails because the route is not implemented yet**

Run: `bun test ui/src/features/dashboard/DashboardPage.test.tsx`

Expected: FAIL with missing component/module errors.

- [ ] **Step 3: Add the dashboard state store**

Create `ui/src/features/dashboard/dashboard-store.ts`:

```ts
import { create } from "zustand";

type SeverityFilter = "all" | "LOW" | "MEDIUM" | "HIGH" | "CRITICAL";

type DashboardState = {
  search: string;
  severity: SeverityFilter;
  onlyMarkedProvenance: boolean;
  minimumRetainedBytes: number;
  expandedLeakIds: Record<string, boolean>;
  setSearch: (value: string) => void;
  setSeverity: (value: SeverityFilter) => void;
  setOnlyMarkedProvenance: (value: boolean) => void;
  setMinimumRetainedBytes: (value: number) => void;
  toggleLeakExpanded: (leakId: string) => void;
};

export const useDashboardStore = create<DashboardState>((set) => ({
  search: "",
  severity: "all",
  onlyMarkedProvenance: false,
  minimumRetainedBytes: 0,
  expandedLeakIds: {},
  setSearch: (value) => set({ search: value }),
  setSeverity: (value) => set({ severity: value }),
  setOnlyMarkedProvenance: (value) => set({ onlyMarkedProvenance: value }),
  setMinimumRetainedBytes: (value) => set({ minimumRetainedBytes: value }),
  toggleLeakExpanded: (leakId) =>
    set((state) => ({
      expandedLeakIds: {
        ...state.expandedLeakIds,
        [leakId]: !state.expandedLeakIds[leakId],
      },
    })),
}));
```

- [ ] **Step 4: Implement the dashboard presentation components**

Create `ui/src/features/dashboard/components/ProvenanceBadge.tsx`:

```tsx
export function ProvenanceBadge({ kind }: { kind: string }) {
  return <span>{kind}</span>;
}
```

Create `ui/src/features/dashboard/components/SummaryStrip.tsx`:

```tsx
import { DashboardArtifact } from "@/lib/analysis-types";

export function SummaryStrip({ artifact }: { artifact: DashboardArtifact }) {
  return (
    <section>
      <div>Total Objects: {artifact.summary.totalObjects}</div>
      <div>Heap Size: {artifact.summary.totalSizeBytes}</div>
      <div>Leak Count: {artifact.leaks.length}</div>
      <div>Graph Nodes: {artifact.graph.nodeCount}</div>
      <div>Elapsed: {artifact.elapsedSeconds.toFixed(2)}s</div>
    </section>
  );
}
```

Create `ui/src/features/dashboard/components/GraphMetricsPanel.tsx`:

```tsx
import { DashboardArtifact } from "@/lib/analysis-types";

export function GraphMetricsPanel({ artifact }: { artifact: DashboardArtifact }) {
  return (
    <section>
      <h2>Graph Metrics</h2>
      <p>Nodes: {artifact.graph.nodeCount}</p>
      <p>Edges: {artifact.graph.edgeCount}</p>
      <p>Dominator Entries: {artifact.graph.dominatorCount}</p>
    </section>
  );
}
```

Create `ui/src/features/dashboard/components/HistogramPanel.tsx`:

```tsx
import { DashboardArtifact } from "@/lib/analysis-types";

export function HistogramPanel({ artifact }: { artifact: DashboardArtifact }) {
  if (!artifact.histogram) return null;

  return (
    <section>
      <h2>Histogram Snapshot</h2>
      <p>Grouped by {artifact.histogram.groupBy}</p>
      <ul>
        {artifact.histogram.entries.slice(0, 5).map((entry) => (
          <li key={entry.key}>
            {entry.key}: {entry.retainedSize}
          </li>
        ))}
      </ul>
    </section>
  );
}
```

Create `ui/src/features/dashboard/components/LeakTable.tsx`:

```tsx
import { DashboardArtifact } from "@/lib/analysis-types";

import { ProvenanceBadge } from "./ProvenanceBadge";

export function LeakTable({ artifact }: { artifact: DashboardArtifact }) {
  return (
    <section>
      <h2>Top Leak Suspects</h2>
      <table>
        <thead>
          <tr>
            <th>Severity</th>
            <th>Class</th>
            <th>Leak ID</th>
            <th>Retained</th>
            <th>Shallow</th>
            <th>Instances</th>
            <th>Score</th>
            <th>Description</th>
          </tr>
        </thead>
        <tbody>
          {artifact.leaks.map((leak) => (
            <tr key={leak.id}>
              <td>{leak.severity}</td>
              <td>{leak.className}</td>
              <td>{leak.id}</td>
              <td>{leak.retainedSizeBytes}</td>
              <td>{leak.shallowSizeBytes ?? "-"}</td>
              <td>{leak.instances}</td>
              <td>{leak.suspectScore?.toFixed(2) ?? "-"}</td>
              <td>
                {leak.description}
                {leak.provenance.map((marker) => (
                  <ProvenanceBadge key={`${leak.id}-${marker.kind}`} kind={marker.kind} />
                ))}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
```

Create `ui/src/features/dashboard/DashboardPage.tsx`:

```tsx
import { Navigate } from "react-router-dom";

import { useArtifactStore } from "@/features/artifact-loader/use-artifact-store";

import { GraphMetricsPanel } from "./components/GraphMetricsPanel";
import { HistogramPanel } from "./components/HistogramPanel";
import { LeakTable } from "./components/LeakTable";
import { SummaryStrip } from "./components/SummaryStrip";

export function DashboardPage() {
  const { artifact, artifactName } = useArtifactStore();

  if (!artifact) {
    return <Navigate to="/" replace />;
  }

  return (
    <main>
      <header>
        <p>Mnemosyne Triage Dashboard</p>
        <h2>{artifact.summary.heapPath}</h2>
        <p>Loaded artifact: {artifactName}</p>
      </header>
      <SummaryStrip artifact={artifact} />
      <section>
        <LeakTable artifact={artifact} />
        <aside>
          <GraphMetricsPanel artifact={artifact} />
          <HistogramPanel artifact={artifact} />
        </aside>
      </section>
    </main>
  );
}
```

- [ ] **Step 5: Wire the dashboard route into the router**

Update `ui/src/app/router.tsx`:

```tsx
import { createBrowserRouter, RouterProvider } from "react-router-dom";

import { ArtifactLoaderPage } from "@/features/artifact-loader/ArtifactLoaderPage";
import { DashboardPage } from "@/features/dashboard/DashboardPage";

const router = createBrowserRouter([
  {
    path: "/",
    element: <ArtifactLoaderPage />,
  },
  {
    path: "/dashboard",
    element: <DashboardPage />,
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
```

- [ ] **Step 6: Update the loader page to navigate after successful artifact load**

Modify `ui/src/features/artifact-loader/ArtifactLoaderPage.tsx` to use `useNavigate()`:

```tsx
import { useNavigate } from "react-router-dom";
```

Add inside the component:

```tsx
const navigate = useNavigate();
```

After `setArtifact(file.name, parsed);`, add:

```tsx
navigate("/dashboard");
```

- [ ] **Step 7: Run the dashboard test and make it pass**

Run: `bun test ui/src/features/dashboard/DashboardPage.test.tsx`

Expected: PASS.

- [ ] **Step 8: Add a failing leak-table interaction test for filtering**

Create `ui/src/features/dashboard/components/LeakTable.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { LeakTable } from "./LeakTable";

describe("LeakTable", () => {
  it("renders leak rows from the artifact", () => {
    render(
      <LeakTable
        artifact={{
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [{ kind: "FALLBACK" }],
            },
          ],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        }}
      />,
    );

    expect(screen.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
    expect(screen.getByText(/fallback/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 9: Run the leak table test and verify it passes**

Run: `bun test ui/src/features/dashboard/components/LeakTable.test.tsx`

Expected: PASS.

- [ ] **Step 10: Commit the dashboard route and components**

```bash
git add ui/src/features/dashboard ui/src/app/router.tsx ui/src/features/artifact-loader/ArtifactLoaderPage.tsx
git commit -m "feat: add first M4 triage dashboard route"
```

### Task 5: Add Dashboard Filters, Empty States, and Invalid Artifact Handling

**Files:**
- Modify: `ui/src/features/dashboard/components/LeakTable.tsx`
- Modify: `ui/src/features/dashboard/DashboardPage.tsx`
- Modify: `ui/src/features/dashboard/components/LeakTable.test.tsx`
- Modify: `ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`
- Modify: `ui/src/features/artifact-loader/ArtifactLoaderPage.tsx`

- [ ] **Step 1: Add a failing test for invalid artifact feedback**

Extend `ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx` with:

```tsx
it("shows a readable invalid artifact error", async () => {
  const user = userEvent.setup();
  render(<ArtifactLoaderPage />);

  const file = new File(["not-json"], "broken.json", { type: "application/json" });
  const input = screen.getByLabelText(/analysis json artifact/i);

  await user.upload(input, file);

  expect(screen.getByRole("alert")).toHaveTextContent(/invalid json artifact/i);
});
```

- [ ] **Step 2: Run the invalid-artifact test and verify it passes or make the message exact**

Run: `bun test ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`

Expected: if it fails because the message differs, normalize the UI copy to `Invalid JSON artifact`.

- [ ] **Step 3: Add a failing test for dashboard empty state**

Extend `ui/src/features/dashboard/DashboardPage.test.tsx` with:

```tsx
it("shows a no leaks empty state when the artifact has no leak rows", () => {
  useArtifactStore.setState({
    artifactName: "fixture.json",
    loadError: undefined,
    artifact: {
      summary: {
        heapPath: "fixture.hprof",
        totalObjects: 42,
        totalSizeBytes: 2048,
        generatedAt: "2026-04-14T00:00:00Z",
        totalRecords: 2,
      },
      leaks: [],
      elapsedSeconds: 1,
      graph: {
        nodeCount: 200,
        edgeCount: 400,
        dominatorCount: 10,
      },
      provenance: [],
    },
  });

  render(<DashboardPage />);

  expect(screen.getByText(/no leak suspects detected/i)).toBeInTheDocument();
});
```

- [ ] **Step 4: Run the dashboard test and verify it fails if empty state is not implemented**

Run: `bun test ui/src/features/dashboard/DashboardPage.test.tsx`

Expected: FAIL if the table simply renders empty rows area. Then implement the explicit empty state.

- [ ] **Step 5: Add dashboard filter controls and filtered leak rendering**

Update `ui/src/features/dashboard/components/LeakTable.tsx` so it consumes filter state and renders controls above the table:

```tsx
import { useMemo } from "react";

import { DashboardArtifact } from "@/lib/analysis-types";

import { useDashboardStore } from "../dashboard-store";
import { ProvenanceBadge } from "./ProvenanceBadge";

export function LeakTable({ artifact }: { artifact: DashboardArtifact }) {
  const {
    search,
    severity,
    onlyMarkedProvenance,
    minimumRetainedBytes,
    setSearch,
    setSeverity,
    setOnlyMarkedProvenance,
    setMinimumRetainedBytes,
  } = useDashboardStore();

  const filteredLeaks = useMemo(() => {
    return artifact.leaks.filter((leak) => {
      const haystack = `${leak.className} ${leak.id} ${leak.description}`.toLowerCase();
      const matchesSearch = !search || haystack.includes(search.toLowerCase());
      const matchesSeverity = severity === "all" || leak.severity === severity;
      const matchesProvenance = !onlyMarkedProvenance || leak.provenance.length > 0;
      const matchesRetained = leak.retainedSizeBytes >= minimumRetainedBytes;

      return matchesSearch && matchesSeverity && matchesProvenance && matchesRetained;
    });
  }, [artifact.leaks, search, severity, onlyMarkedProvenance, minimumRetainedBytes]);

  return (
    <section>
      <h2>Top Leak Suspects</h2>
      <div>
        <input aria-label="Search leaks" value={search} onChange={(e) => setSearch(e.target.value)} />
        <select aria-label="Severity filter" value={severity} onChange={(e) => setSeverity(e.target.value as "all" | "LOW" | "MEDIUM" | "HIGH" | "CRITICAL") }>
          <option value="all">All severities</option>
          <option value="CRITICAL">Critical</option>
          <option value="HIGH">High</option>
          <option value="MEDIUM">Medium</option>
          <option value="LOW">Low</option>
        </select>
        <label>
          <input type="checkbox" checked={onlyMarkedProvenance} onChange={(e) => setOnlyMarkedProvenance(e.target.checked)} />
          Marked provenance only
        </label>
        <input
          aria-label="Minimum retained bytes"
          type="number"
          min={0}
          value={minimumRetainedBytes}
          onChange={(e) => setMinimumRetainedBytes(Number(e.target.value || 0))}
        />
      </div>
      {filteredLeaks.length === 0 ? (
        <p>No leak suspects detected</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Severity</th>
              <th>Class</th>
              <th>Leak ID</th>
              <th>Retained</th>
              <th>Shallow</th>
              <th>Instances</th>
              <th>Score</th>
              <th>Description</th>
            </tr>
          </thead>
          <tbody>
            {filteredLeaks.map((leak) => (
              <tr key={leak.id}>
                <td>{leak.severity}</td>
                <td>{leak.className}</td>
                <td>{leak.id}</td>
                <td>{leak.retainedSizeBytes}</td>
                <td>{leak.shallowSizeBytes ?? "-"}</td>
                <td>{leak.instances}</td>
                <td>{leak.suspectScore?.toFixed(2) ?? "-"}</td>
                <td>
                  {leak.description}
                  {leak.provenance.map((marker) => (
                    <ProvenanceBadge key={`${leak.id}-${marker.kind}`} kind={marker.kind} />
                  ))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
```

- [ ] **Step 6: Add a filtering assertion to the leak table test**

Extend `ui/src/features/dashboard/components/LeakTable.test.tsx` with:

```tsx
import userEvent from "@testing-library/user-event";

it("filters to provenance-marked rows", async () => {
  const user = userEvent.setup();

  render(
    <LeakTable
      artifact={{
        summary: {
          heapPath: "fixture.hprof",
          totalObjects: 1,
          totalSizeBytes: 1,
          totalRecords: 1,
        },
        leaks: [
          {
            id: "leak-1",
            className: "com.example.Marked",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 100,
            instances: 1,
            description: "Marked leak",
            provenance: [{ kind: "FALLBACK" }],
          },
          {
            id: "leak-2",
            className: "com.example.Clean",
            leakKind: "CACHE",
            severity: "LOW",
            retainedSizeBytes: 50,
            instances: 1,
            description: "Clean leak",
            provenance: [],
          },
        ],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 1,
          edgeCount: 1,
          dominatorCount: 1,
        },
        provenance: [],
      }}
    />,
  );

  await user.click(screen.getByLabelText(/marked provenance only/i));

  expect(screen.getByText(/com\.example\.Marked/i)).toBeInTheDocument();
  expect(screen.queryByText(/com\.example\.Clean/i)).not.toBeInTheDocument();
});
```

- [ ] **Step 7: Run the focused dashboard tests and verify they pass**

Run: `bun test ui/src/features/dashboard/DashboardPage.test.tsx ui/src/features/dashboard/components/LeakTable.test.tsx ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx`

Expected: PASS.

- [ ] **Step 8: Commit the dashboard state and interaction pass**

```bash
git add ui/src/features/dashboard ui/src/features/artifact-loader/ArtifactLoaderPage.tsx ui/src/features/artifact-loader/ArtifactLoaderPage.test.tsx
git commit -m "feat: add M4 dashboard filters and empty states"
```

### Task 6: Verify the Browser-First Slice and Sync Project Docs

**Files:**
- Modify: `docs/design/milestone-4-ui-and-usability.md`
- Modify: `docs/roadmap.md`
- Modify: `README.md`
- Modify: `STATUS.md`
- Test: frontend and existing Rust tests only

- [ ] **Step 1: Update the M4 design docs to reflect the approved stack and first-slice shape**

Update `docs/design/milestone-4-ui-and-usability.md` so it reflects:

- browser-first first slice
- shared React frontend
- later Tauri shell
- JSON artifact loading instead of immediate local API/server dependency for the first slice

Keep the edit narrow and aligned with the approved spec.

- [ ] **Step 2: Update roadmap/current-state wording only where the new first slice would otherwise contradict docs**

In `docs/roadmap.md`, update only directly relevant M4 current-state wording so it reflects that:

- M4 remains open
- but the first UI slice now exists as a browser-first frontend dashboard path rather than UI being entirely absent

- [ ] **Step 3: Update `README.md` and `STATUS.md` only if needed to describe the frontend workflow honestly**

If needed, add narrow wording that explains:

- Bun is the supported frontend workflow
- the first UI slice is browser-first
- Tauri remains the later desktop wrapper path

- [ ] **Step 4: Run focused frontend verification**

Run:

```bash
bun test
bun run build
bun run lint
```

Expected: PASS.

- [ ] **Step 5: Run broader repo verification**

Run:

```bash
cargo check
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 6: Commit the docs sync and final slice state**

```bash
git add docs/design/milestone-4-ui-and-usability.md docs/roadmap.md README.md STATUS.md
git commit -m "docs: record browser-first M4 dashboard slice"
```

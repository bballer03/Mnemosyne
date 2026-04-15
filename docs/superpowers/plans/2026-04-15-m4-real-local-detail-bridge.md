# M4 Real Local Detail Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the leak workspace's placeholder explain/source-map/fix behavior with a real workspace-local host bridge that can surface honest `ready`, `fallback`, and `unavailable` states from local runtime integrations.

**Architecture:** Keep the bridge feature-local to `ui/src/features/leak-workspace/`. The UI reads an optional host bridge from `window`, adapts raw local-runtime payloads into the existing workspace result types, and only marks the bridge `ready` when the required methods exist. If the bridge is absent, the workspace remains usable and the affected panels render explicit unavailable states instead of synthetic success placeholders.

**Tech Stack:** React, TypeScript, React Router 6, Zustand, Bun, Testing Library

---

## File Structure

### Existing files to modify

- `ui/src/features/leak-workspace/live-detail-client.ts`
  - replace placeholder explain/source-map/fix logic with a host bridge contract, bridge-status helpers, and raw payload parsing
- `ui/src/features/leak-workspace/live-detail-client.test.ts`
  - cover unavailable-without-bridge and ready/fallback normalization with an injected host bridge
- `ui/src/features/leak-workspace/LeakExplainPage.tsx`
  - render honest unavailable state when the local explain bridge is absent and preserve ready/fallback/error handling
- `ui/src/features/leak-workspace/LeakExplainPage.test.tsx`
  - cover unavailable and bridge-backed ready behavior
- `ui/src/features/leak-workspace/LeakSourceMapPage.tsx`
  - treat bridge absence as unavailable instead of returning a synthetic mapped placeholder
- `ui/src/features/leak-workspace/LeakSourceMapPage.test.tsx`
  - cover unavailable and bridge-backed source locations
- `ui/src/features/leak-workspace/LeakFixPage.tsx`
  - treat bridge absence as unavailable while preserving fallback/error rendering when a bridge exists
- `ui/src/features/leak-workspace/LeakFixPage.test.tsx`
  - cover unavailable, ready, and fallback bridge-backed fix flows
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`
  - reflect bridge/provider readiness from the real host bridge status rather than hardcoded values
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`
  - cover bridge/provider readiness when the host bridge is injected

### Files intentionally left unchanged

- `ui/src/features/leak-workspace/LeakGcPathPage.tsx`
- `ui/src/features/leak-workspace/LeakGcPathPage.test.tsx`
- `ui/src/features/leak-workspace/leak-workspace-store.ts`

This slice prioritizes `source-map`, `fix`, and `explain`. GC-path workflow hardening stays for the next slice.

---

### Task 1: Define The Workspace-Local Host Bridge Contract

**Files:**
- Modify: `ui/src/features/leak-workspace/live-detail-client.ts`
- Modify: `ui/src/features/leak-workspace/live-detail-client.test.ts`

- [ ] **Step 1: Write the failing bridge-client tests**

Add tests that prove:

```ts
it("marks explain/source-map/fix as unavailable when no host bridge is installed", async () => {
  const explain = await explainLeak({ leakId: "leak-1", heapPath: "fixture.hprof" });
  const sourceMap = await resolveLeakSourceMap({
    leakId: "leak-1",
    className: "com.example.Cache",
    projectRoot: "D:/repo",
  });
  const fix = await proposeLeakFix({
    leakId: "leak-1",
    heapPath: "fixture.hprof",
    projectRoot: "D:/repo",
  });

  expect(explain.status).toBe("unavailable");
  expect(sourceMap.status).toBe("unavailable");
  expect(fix.status).toBe("unavailable");
});

it("normalizes bridge-backed explain, source-map, and fix payloads", async () => {
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    capabilities: { provider: "ready" },
    explainLeak: async () => ({ summary: "Bridge explanation." }),
    mapToCode: async () => ({
      leak_id: "leak-1",
      locations: [
        {
          file: "D:/repo/src/main/java/com/example/Cache.java",
          line: 42,
          symbol: "com.example.Cache",
          code_snippet: "return cache;",
          git: { commit: "abc123" },
        },
      ],
    }),
    proposeFix: async () => ({
      suggestions: [
        {
          target_file: "D:/repo/src/main/java/com/example/Cache.java",
          description: "Release entries sooner.",
          diff: "@@ -1 +1 @@",
          confidence: 0.8,
          style: "Minimal",
        },
      ],
    }),
  };

  const explain = await explainLeak({ leakId: "leak-1", heapPath: "fixture.hprof" });
  const sourceMap = await resolveLeakSourceMap({
    leakId: "leak-1",
    className: "com.example.Cache",
    projectRoot: "D:/repo",
  });
  const fix = await proposeLeakFix({
    leakId: "leak-1",
    heapPath: "fixture.hprof",
    projectRoot: "D:/repo",
  });

  expect(explain.status).toBe("ready");
  expect(explain.data?.summary).toBe("Bridge explanation.");
  expect(sourceMap.status).toBe("ready");
  expect(sourceMap.data?.locations[0]?.file).toBe("D:/repo/src/main/java/com/example/Cache.java");
  expect(fix.status).toBe("ready");
  expect(fix.data?.suggestions[0]?.target_file).toBe("D:/repo/src/main/java/com/example/Cache.java");
});
```

- [ ] **Step 2: Run the bridge-client tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts"`
Expected: FAIL because the client still returns placeholder success/fallback responses with no host bridge.

- [ ] **Step 3: Implement the bridge contract and payload parsing**

Update `live-detail-client.ts` so it:

```ts
export type LeakWorkspaceHostBridge = {
  capabilities?: {
    provider?: "ready" | "unknown" | "unavailable";
  };
  explainLeak?: (input: ExplainLeakInput) => Promise<unknown>;
  mapToCode?: (input: ResolveLeakSourceMapInput) => Promise<unknown>;
  proposeFix?: (input: ProposeLeakFixInput) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__?: LeakWorkspaceHostBridge;
  }
}
```

Add helpers that:

- read the optional bridge from `window`
- report `bridge: "ready" | "unavailable"`
- report `provider: "ready" | "unknown" | "unavailable"`
- parse raw bridge payloads into the existing `ExplainResult`, `SourceMapResult`, and `FixResult` shapes

Replace placeholder success/fallback returns for `explainLeak`, `resolveLeakSourceMap`, and `proposeLeakFix` with:

- `unavailable` when the host bridge or required method is missing
- `ready` / `fallback` when a bridge method returns a valid payload
- `error` when a bridge method throws or returns malformed data

Keep `findLeakGcPath()` unchanged in this slice.

- [ ] **Step 4: Re-run the bridge-client tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts"`
Expected: PASS

---

### Task 2: Switch Explain, Source Map, And Fix Views To Honest Bridge States

**Files:**
- Modify: `ui/src/features/leak-workspace/LeakExplainPage.tsx`
- Modify: `ui/src/features/leak-workspace/LeakExplainPage.test.tsx`
- Modify: `ui/src/features/leak-workspace/LeakSourceMapPage.tsx`
- Modify: `ui/src/features/leak-workspace/LeakSourceMapPage.test.tsx`
- Modify: `ui/src/features/leak-workspace/LeakFixPage.tsx`
- Modify: `ui/src/features/leak-workspace/LeakFixPage.test.tsx`

- [ ] **Step 1: Write the failing subview tests**

Add targeted cases proving:

```tsx
it("renders explain unavailable when the host bridge is absent", async () => {
  seedArtifact();
  const router = createMemoryRouter([{ path: "/leaks/:leakId/explain", element: <LeakExplainPage /> }], {
    initialEntries: ["/leaks/leak-1/explain"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByText(/explain unavailable: local explain bridge is unavailable\./i)).toBeInTheDocument();
});

it("renders bridge-backed source map locations", async () => {
  seedArtifact();
  act(() => {
    useLeakWorkspaceStore.getState().setSelection({ projectRoot: "D:/repo" });
  });
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    mapToCode: async () => ({
      leak_id: "leak-1",
      locations: [
        {
          file: "D:/repo/src/main/java/com/example/Cache.java",
          line: 42,
          symbol: "com.example.Cache",
          code_snippet: "return cache;",
          git: { commit: "abc123" },
        },
      ],
    }),
  };

  const router = createMemoryRouter([{ path: "/leaks/:leakId/source-map", element: <LeakSourceMapPage /> }], {
    initialEntries: ["/leaks/leak-1/source-map"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByText(/d:\/repo\/src\/main\/java\/com\/example\/cache\.java/i)).toBeInTheDocument();
  expect(view.getByText(/git metadata: present/i)).toBeInTheDocument();
});
```

Add the equivalent fix-page cases for:

- unavailable when no bridge exists
- ready when a bridge returns a suggestion with no fallback provenance
- fallback when a bridge returns provenance containing `FALLBACK`

- [ ] **Step 2: Run the targeted subview tests to verify they fail**

Run:

`npx --yes bun test "src/features/leak-workspace/LeakExplainPage.test.tsx" "src/features/leak-workspace/LeakSourceMapPage.test.tsx" "src/features/leak-workspace/LeakFixPage.test.tsx"`

Expected: FAIL because the pages currently assume placeholder success/fallback behavior instead of bridge-backed unavailable/ready states.

- [ ] **Step 3: Update the subviews**

Implement the minimal view changes:

- `LeakExplainPage.tsx`
  - add `showUnavailable`
  - render `Explain unavailable: ...` when `explain.status === "unavailable"`
- `LeakSourceMapPage.tsx`
  - keep `projectRoot` as a prerequisite
  - render `Source map unavailable: ...` when the bridge client returns `unavailable`
  - keep fallback text only for bridge-returned fallback provenance or unmapped results
- `LeakFixPage.tsx`
  - keep `projectRoot` as a prerequisite
  - render `Fix proposal unavailable: ...` when the bridge client returns `unavailable`
  - keep fallback copy only for bridge-returned fallback provenance

- [ ] **Step 4: Re-run the targeted subview tests to verify they pass**

Run:

`npx --yes bun test "src/features/leak-workspace/LeakExplainPage.test.tsx" "src/features/leak-workspace/LeakSourceMapPage.test.tsx" "src/features/leak-workspace/LeakFixPage.test.tsx"`

Expected: PASS

---

### Task 3: Reflect Real Bridge And Provider Readiness In Overview

**Files:**
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`

- [ ] **Step 1: Write the failing overview readiness tests**

Add coverage for both bridge-absent and bridge-present states:

```tsx
it("shows bridge and provider as unavailable when no host bridge is installed", () => {
  seedArtifact();
  const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
    initialEntries: ["/leaks/leak-1/overview"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/bridge: unavailable/i)).toBeInTheDocument();
  expect(view.getByText(/provider: unavailable/i)).toBeInTheDocument();
});

it("shows bridge ready and provider ready when the host bridge reports those capabilities", () => {
  seedArtifact();
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    capabilities: { provider: "ready" },
    explainLeak: async () => ({ summary: "Bridge explanation." }),
    mapToCode: async () => ({ leak_id: "leak-1", locations: [] }),
    proposeFix: async () => ({ suggestions: [] }),
  };

  const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
    initialEntries: ["/leaks/leak-1/overview"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/bridge: ready/i)).toBeInTheDocument();
  expect(view.getByText(/provider: ready/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the overview tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceOverview.test.tsx"`
Expected: FAIL because overview still hardcodes bridge/provider readiness.

- [ ] **Step 3: Update overview readiness**

Use the bridge-status helper from `live-detail-client.ts` so the dependency matrix becomes:

```ts
const bridgeStatus = getLeakWorkspaceBridgeStatus();

const dependencyStatus: LeakWorkspaceDependencyStatus = {
  bridge: bridgeStatus.bridge,
  projectRoot: projectRoot ? "present" : "missing",
  objectTarget: objectId ? "present" : "missing",
  provider: bridgeStatus.provider,
};
```

- [ ] **Step 4: Re-run the overview tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceOverview.test.tsx"`
Expected: PASS

---

### Task 4: Verify Slice 2 End-To-End

**Files:**
- Verify only

- [ ] **Step 1: Run the full frontend test suite**

Run: `npx --yes bun test`
Expected: PASS

- [ ] **Step 2: Run the frontend build**

Run: `npx --yes bun run build`
Expected: PASS

- [ ] **Step 3: Run the frontend lint/type check**

Run: `npx --yes bun run lint`
Expected: PASS

- [ ] **Step 4: Run repo-standard Rust verification**

Run: `cargo test`
Expected: PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

Run: `cargo fmt --all -- --check`
Expected: PASS

- [ ] **Step 5: Inspect final local scope**

Run: `git status --short`
Expected: intended leak-workspace UI files, the new plan file, and any unrelated pre-existing worktree changes

---

## Slice Queue After This Plan

After this slice lands, continue in this order:

1. GC-path workflow hardening around real object targets
2. artifact-driven explorer breadth (histogram + analyzer views)
3. competitive explorer surfaces (dominator/object/query)

## Execution Note

This repository is already being worked from an isolated worktree. Do not create commits unless the user explicitly asks for them.

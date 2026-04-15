# M4 Investigation Context Activation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Activate the existing leak workspace routes by seeding route-backed leak identity into the workspace store and adding shell-level controls for `projectRoot` and `objectId` so the current `source-map`, `fix`, and `gc-path` tabs can be used intentionally.

**Architecture:** Keep the leak workspace browser-first and feature-local. The shell owns route-seeded leak identity plus explicit context controls, the workspace store remains the source of truth for selected leak context, and the existing subviews keep their honest `ready` / `fallback` / `unavailable` semantics without adding a generalized app-wide browser RPC layer.

**Tech Stack:** React, TypeScript, React Router 6, Zustand, Bun, Testing Library

---

## File Structure

### Existing files to modify

- `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
  - seed `leakId` / `heapPath` into the workspace store and render context controls for `projectRoot` and `objectId`
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`
  - cover context seeding and operator-controlled activation from the shell
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`
  - reflect current store-backed readiness for `projectRoot` and `objectId`
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`
  - cover readiness when the shell context is present
- `ui/src/features/leak-workspace/leak-workspace-store.ts`
  - keep route identity seeding and dependent reset semantics stable for the new shell-driven flow

### Files intentionally left unchanged

- `ui/src/features/leak-workspace/LeakExplainPage.tsx`
- `ui/src/features/leak-workspace/LeakSourceMapPage.tsx`
- `ui/src/features/leak-workspace/LeakFixPage.tsx`
- `ui/src/features/leak-workspace/LeakGcPathPage.tsx`

Those pages already respond correctly once `projectRoot` or `objectId` exists in the store.

---

### Task 1: Seed Leak Identity And Add Shell Context Controls

**Files:**
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`

- [ ] **Step 1: Write the failing shell-context test**

Add a test near the existing layout tests:

```tsx
it("seeds leak identity into the workspace store and lets the user apply local context", async () => {
  const user = userEvent.setup();

  act(() => {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
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
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 1,
          edgeCount: 1,
          dominatorCount: 1,
        },
        provenance: [],
      },
    });
  });

  const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1/overview"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await view.findByRole("heading", { name: /overview/i });

  expect(useLeakWorkspaceStore.getState().leakId).toBe("leak-1");
  expect(useLeakWorkspaceStore.getState().heapPath).toBe("fixture.hprof");

  await user.clear(view.getByLabelText(/project root/i));
  await user.type(view.getByLabelText(/project root/i), "D:/repo");
  await user.click(view.getByRole("button", { name: /apply project root/i }));

  await user.clear(view.getByLabelText(/object target id/i));
  await user.type(view.getByLabelText(/object target id/i), "0x1234");
  await user.click(view.getByRole("button", { name: /apply object target/i }));

  expect(useLeakWorkspaceStore.getState().projectRoot).toBe("D:/repo");
  expect(useLeakWorkspaceStore.getState().objectId).toBe("0x1234");
});
```

- [ ] **Step 2: Run the layout test to verify it fails**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`
Expected: FAIL because the shell does not yet seed store identity or render these controls.

- [ ] **Step 3: Update the shell implementation**

Add the minimal logic to `LeakWorkspaceLayout.tsx`:

```tsx
import { useEffect, useState } from "react";

import { useLeakWorkspaceStore } from "./leak-workspace-store";

const inputStyle = {
  borderRadius: 12,
  border: "1px solid #334155",
  background: "rgba(2, 6, 23, 0.82)",
  color: "#e2e8f0",
  padding: "0.65rem 0.8rem",
} as const;

export function LeakWorkspaceLayout() {
  const { artifact, artifactName } = useArtifactStore();
  const setSelection = useLeakWorkspaceStore((state) => state.setSelection);
  const projectRoot = useLeakWorkspaceStore((state) => state.projectRoot);
  const objectId = useLeakWorkspaceStore((state) => state.objectId);
  const [projectRootDraft, setProjectRootDraft] = useState(projectRoot ?? "");
  const [objectIdDraft, setObjectIdDraft] = useState(objectId ?? "");

  useEffect(() => {
    setProjectRootDraft(projectRoot ?? "");
  }, [projectRoot]);

  useEffect(() => {
    setObjectIdDraft(objectId ?? "");
  }, [objectId]);

  useEffect(() => {
    if (!artifact || !leak) {
      return;
    }

    setSelection({
      leakId: leak.id,
      heapPath: artifact.summary.heapPath,
    });
  }, [artifact, leak, setSelection]);

  // inside the shell body, render a context section with:
  // - Project root input + Apply/Clear buttons
  // - Object target ID input + Apply/Clear buttons
  // - helper text that says what each field unlocks
}
```

Use explicit button handlers:

```tsx
onClick={() => setSelection({ projectRoot: projectRootDraft.trim() || undefined })}
onClick={() => setSelection({ projectRoot: undefined })}
onClick={() => setSelection({ objectId: objectIdDraft.trim() || undefined })}
onClick={() => setSelection({ objectId: undefined })}
```

- [ ] **Step 4: Re-run the layout test to verify it passes**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`
Expected: PASS

---

### Task 2: Reflect Current Context In Overview And Route Activation

**Files:**
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceOverview.test.tsx`
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`

- [ ] **Step 1: Write the failing overview readiness test**

Add a test to `LeakWorkspaceOverview.test.tsx`:

```tsx
it("shows project root and object target as present when the workspace store has been activated", () => {
  seedArtifact();

  act(() => {
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      projectRoot: "D:/repo",
      objectId: "0x1234",
    });
  });

  const router = createMemoryRouter([{ path: "/leaks/:leakId/overview", element: <LeakWorkspaceOverview /> }], {
    initialEntries: ["/leaks/leak-1/overview"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(view.getByText(/project root: present/i)).toBeInTheDocument();
  expect(view.getByText(/object target: present/i)).toBeInTheDocument();
  expect(view.getByText(/gc path can be loaded from a concrete object target/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Add the route-activation test for source-map and gc-path**

Add one layout-route test proving the shell controls activate the existing routes:

```tsx
it("activates source-map and gc-path routes after the operator applies local context", async () => {
  const user = userEvent.setup();
  // seed the same single-leak artifact as in the shell-context test

  const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1/overview"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await view.findByRole("heading", { name: /overview/i });

  await user.clear(view.getByLabelText(/project root/i));
  await user.type(view.getByLabelText(/project root/i), "D:/repo");
  await user.click(view.getByRole("button", { name: /apply project root/i }));
  await user.click(view.getByRole("link", { name: /source map/i }));

  expect(await view.findByText(/mapping fell back to an unmapped placeholder/i)).toBeInTheDocument();

  await user.clear(view.getByLabelText(/object target id/i));
  await user.type(view.getByLabelText(/object target id/i), "0x1234");
  await user.click(view.getByRole("button", { name: /apply object target/i }));
  await user.click(view.getByRole("link", { name: /gc path/i }));

  expect(await view.findByText(/gc path includes backend-reported fallback provenance/i)).toBeInTheDocument();
});
```

- [ ] **Step 3: Run the overview/layout tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceOverview.test.tsx" "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`
Expected: FAIL until the overview and shell reflect current context.

- [ ] **Step 4: Update the overview to read current context from the store**

In `LeakWorkspaceOverview.tsx`, replace the hardcoded readiness values with store-backed values:

```tsx
import { useLeakWorkspaceStore } from "./leak-workspace-store";

const projectRoot = useLeakWorkspaceStore((state) => state.projectRoot);
const objectId = useLeakWorkspaceStore((state) => state.objectId);

const dependencyStatus: LeakWorkspaceDependencyStatus = {
  bridge: "unavailable",
  projectRoot: projectRoot ? "present" : "missing",
  objectTarget: objectId ? "present" : "missing",
  provider: "unknown",
};
```

Keep the other overview behavior unchanged.

- [ ] **Step 5: Re-run the targeted tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/LeakWorkspaceOverview.test.tsx" "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`
Expected: PASS

---

### Task 3: Verify The Slice End-To-End

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

- [ ] **Step 4: Re-run workspace Rust verification because the repo standard expects full validation before claiming completion**

Run: `cargo test`
Expected: PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

Run: `cargo fmt --all -- --check`
Expected: PASS

- [ ] **Step 5: Inspect final local scope**

Run: `git status --short`
Expected: only the intended UI/docs files for this slice plus any unrelated pre-existing worktree changes

---

## Slice Queue After This Plan

After this slice lands, continue in this order:

1. real source/fix/explain bridge for the leak workspace only
2. explicit GC-path workflow hardening around real object targets
3. artifact-driven explorer breadth (histogram + analyzer views)
4. dominator/object/query competitive surfaces

## Execution Note

This repository is already being worked from an isolated worktree. Do not create commits unless the user explicitly asks for them.

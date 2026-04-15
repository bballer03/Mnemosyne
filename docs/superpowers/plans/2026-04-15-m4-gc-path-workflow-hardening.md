# M4 GC Path Workflow Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the leak workspace GC-path placeholder flow with a real bridge-backed workflow and add lightweight object-target recall/refresh controls so operators can revisit real targets safely.

**Architecture:** Keep all changes feature-local to `ui/src/features/leak-workspace/`. Extend the narrow host bridge with a `findGcPath` method, parse the real `find_gc_path` payload into the existing workspace result model, and stop fabricating synthetic GC paths in the browser. Add a small per-leak recent-object-target history plus a GC-path refresh trigger in the workspace store so the shell and the GC-path page can support explicit recall and reruns without guessing object IDs from artifact provenance.

**Tech Stack:** React, TypeScript, React Router 6, Zustand, Bun, Testing Library

---

## File Structure

### Existing files to modify

- `ui/src/features/leak-workspace/live-detail-client.ts`
  - extend the host bridge with `findGcPath`
  - parse raw GC-path payloads from the local runtime
  - require real bridge access instead of returning a browser-side synthetic placeholder
- `ui/src/features/leak-workspace/live-detail-client.test.ts`
  - cover bridge-backed GC-path ready/fallback/unavailable behavior
  - cover recent-object-target store behavior because this file already owns store-selection tests
- `ui/src/features/leak-workspace/leak-workspace-store.ts`
  - preserve a small recent-object-target history per leak
  - add a GC-path refresh trigger that the page can bump explicitly
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
  - surface recent object-target recall buttons in the shell
- `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`
  - prove recent object-target recall is visible and reusable from the shell
- `ui/src/features/leak-workspace/LeakGcPathPage.tsx`
  - render honest bridge-backed unavailable/ready/fallback/error states
  - add explicit current-target and refresh controls
  - render backend path/provenance details instead of the browser placeholder path
- `ui/src/features/leak-workspace/LeakGcPathPage.test.tsx`
  - cover bridge absence, bridge-backed paths, fallback provenance, and refresh behavior

### Files intentionally left unchanged

- `ui/src/features/leak-workspace/LeakExplainPage.tsx`
- `ui/src/features/leak-workspace/LeakSourceMapPage.tsx`
- `ui/src/features/leak-workspace/LeakFixPage.tsx`
- `ui/src/features/leak-workspace/LeakWorkspaceOverview.tsx`

Slice 3 is intentionally narrow: harden `gc-path` around real object targets and real backend data before widening into new explorer routes.

---

### Task 1: Replace The Browser GC-Path Placeholder With A Real Bridge Contract

**Files:**
- Modify: `ui/src/features/leak-workspace/live-detail-client.ts`
- Modify: `ui/src/features/leak-workspace/live-detail-client.test.ts`

- [ ] **Step 1: Write the failing bridge-client tests**

Add tests that prove the GC-path adapter is now bridge-backed rather than browser-synthesized:

```ts
it("marks gc path as unavailable when no host bridge is installed", async () => {
  const gcPath = await findLeakGcPath({
    leakId: "leak-1",
    heapPath: "fixture.hprof",
    objectId: "0x1000",
  });

  expect(gcPath.status).toBe("unavailable");
  expect(gcPath.error).toBe("Local GC path bridge is unavailable.");
});

it("normalizes a bridge-backed gc path payload", async () => {
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    findGcPath: async () => ({
      object_id: "0x0000000000001000",
      path_length: 2,
      path: [
        {
          object_id: "0x0000000000000001",
          class_name: "java.lang.Thread",
          field: "ROOT",
          is_root: true,
        },
        {
          object_id: "0x0000000000001000",
          class_name: "com.example.Cache",
          field: "entries",
          is_root: false,
        },
      ],
      provenance: [],
    }),
  };

  const gcPath = await findLeakGcPath({
    leakId: "leak-1",
    heapPath: "fixture.hprof",
    objectId: "0x1000",
  });

  expect(gcPath.status).toBe("ready");
  expect(gcPath.data?.object_id).toBe("0x0000000000001000");
  expect(gcPath.data?.path[0]).toEqual({
    object_id: "0x0000000000000001",
    class_name: "java.lang.Thread",
    via: "ROOT",
    is_root: true,
  });
});

it("marks bridge-backed synthetic/fallback gc paths as fallback", async () => {
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    findGcPath: async () => ({
      object_id: "0x1000",
      path_length: 2,
      path: [
        {
          object_id: "GC_ROOT_thread",
          class_name: "java.lang.Thread",
          field: "ROOT",
          is_root: true,
        },
        {
          object_id: "0x1000",
          class_name: "com.example.Cache",
          field: "entries",
          is_root: false,
        },
      ],
      provenance: [
        { kind: "SYNTHETIC", detail: "GC path was synthesized from summary-level heap information." },
        { kind: "FALLBACK", detail: "No real GC root chain could be resolved; best-effort fallback path returned." },
      ],
    }),
  };

  const gcPath = await findLeakGcPath({
    leakId: "leak-1",
    heapPath: "fixture.hprof",
    objectId: "0x1000",
  });

  expect(gcPath.status).toBe("fallback");
  expect(gcPath.data?.provenance?.map((marker) => marker.kind)).toEqual(["SYNTHETIC", "FALLBACK"]);
});
```

- [ ] **Step 2: Run the bridge-client tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts"`
Expected: FAIL because `findLeakGcPath()` still fabricates a browser-side placeholder and does not require a bridge or `heapPath`.

- [ ] **Step 3: Implement the real GC-path bridge parsing**

Update `live-detail-client.ts` so the GC-path adapter follows the same narrow-bridge rules as the other live subviews:

```ts
export type FindLeakGcPathInput = {
  leakId: string;
  heapPath: string;
  objectId?: string;
};

export type LeakWorkspaceHostBridge = {
  capabilities?: {
    provider?: "ready" | "unknown" | "unavailable";
  };
  explainLeak?: (input: ExplainLeakInput) => Promise<unknown>;
  findGcPath?: (input: FindLeakGcPathInput) => Promise<unknown>;
  mapToCode?: (input: ResolveLeakSourceMapInput) => Promise<unknown>;
  proposeFix?: (input: ProposeLeakFixInput) => Promise<unknown>;
};

type RawGcPathNode = {
  object_id: string;
  class_name: string;
  field?: string;
  is_root?: boolean;
};

export type GcPathNode = {
  object_id: string;
  class_name: string;
  via?: string;
  is_root?: boolean;
};

export type GcPathResult = {
  leak_id: string;
  object_id: string;
  path: GcPathNode[];
  path_length: number;
  provenance?: ArtifactProvenanceMarker[];
};

function parseGcPathResult(value: unknown, leakId: string): GcPathResult {
  // validate object_id, path, path_length, provenance
}

export async function findLeakGcPath(input: FindLeakGcPathInput): Promise<LiveDetailResult<GcPathResult>> {
  if (!input.objectId) {
    return {
      status: "unavailable",
      data: {
        leak_id: input.leakId,
        object_id: "",
        path: [],
        path_length: 0,
      },
    };
  }

  const bridge = getLeakWorkspaceHostBridge();

  if (!bridge?.findGcPath) {
    return {
      status: "unavailable",
      error: "Local GC path bridge is unavailable.",
    };
  }

  try {
    const raw = await bridge.findGcPath(input);
    const parsed = parseGcPathResult(raw, input.leakId);
    return normalizeGcPathResult(parsed);
  } catch (error: unknown) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "GC path bridge request failed.",
    };
  }
}
```

Keep `normalizeGcPathResult()` honest:

- `ready` when no fallback/synthetic provenance is present
- `fallback` when provenance includes `FALLBACK` or `SYNTHETIC`
- never fabricate a browser-only synthetic path for a real object target

- [ ] **Step 4: Re-run the bridge-client tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts"`
Expected: PASS

---

### Task 2: Persist Recent Object Targets And Surface Recall Controls In The Workspace Shell

**Files:**
- Modify: `ui/src/features/leak-workspace/leak-workspace-store.ts`
- Modify: `ui/src/features/leak-workspace/live-detail-client.test.ts`
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceLayout.tsx`
- Modify: `ui/src/features/leak-workspace/LeakWorkspaceLayout.test.tsx`

- [ ] **Step 1: Write the failing store and shell tests**

Extend the existing store-selection tests with a recent-target history case:

```ts
it("records recent object targets per leak and moves duplicates to the front", () => {
  useLeakWorkspaceStore.getState().reset();
  useLeakWorkspaceStore.getState().setSelection({ leakId: "leak-1", heapPath: "fixture.hprof" });

  useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
  useLeakWorkspaceStore.getState().setSelection({ objectId: "0x2000" });
  useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });

  expect(useLeakWorkspaceStore.getState().recentObjectTargetsByLeak["leak-1"]).toEqual(["0x1000", "0x2000"]);
});
```

Add a layout test proving the shell surfaces and reuses that history:

```tsx
it("shows recent object targets and reapplies one from the shell", async () => {
  seedArtifact();
  const user = userEvent.setup();

  act(() => {
    useLeakWorkspaceStore.getState().setSelection({
      leakId: "leak-1",
      heapPath: "fixture.hprof",
      objectId: "0x1000",
    });
    useLeakWorkspaceStore.getState().setSelection({ objectId: undefined });
  });

  const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1/overview"] });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByRole("button", { name: /reuse 0x1000/i })).toBeInTheDocument();

  await user.click(view.getByRole("button", { name: /reuse 0x1000/i }));

  expect(useLeakWorkspaceStore.getState().objectId).toBe("0x1000");
  expect(view.getByLabelText(/object target id/i)).toHaveValue("0x1000");
});
```

- [ ] **Step 2: Run the store and shell tests to verify they fail**

Run:

`npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts" "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`

Expected: FAIL because the store does not preserve recent targets and the shell does not render recall actions.

- [ ] **Step 3: Implement recent-target history and recall controls**

Keep the store additions small and leak-scoped:

```ts
type LeakWorkspaceState = WorkspaceSelection & SubviewStateByKey & {
  recentObjectTargetsByLeak: Record<string, string[]>;
  gcPathRefreshNonce: number;
  requestGcPathRefresh: () => void;
  setSelection: (selection: Partial<WorkspaceSelection>) => void;
  setSubviewState: <K extends LiveDetailKey>(key: K, value: SubviewStateByKey[K]) => void;
  reset: () => void;
};

function rememberObjectTarget(
  history: Record<string, string[]>,
  leakId: string | undefined,
  objectId: string | undefined,
) {
  if (!leakId || !objectId) {
    return history;
  }

  const current = history[leakId] ?? [];
  return {
    ...history,
    [leakId]: [objectId, ...current.filter((entry) => entry !== objectId)].slice(0, 5),
  };
}
```

Render the recall buttons in `LeakWorkspaceLayout.tsx` near the object-target controls:

```tsx
const recentObjectTargets = useLeakWorkspaceStore(
  (state) => (leak?.id ? state.recentObjectTargetsByLeak[leak.id] ?? [] : []),
);

{recentObjectTargets.length ? (
  <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
    {recentObjectTargets.map((target) => (
      <button
        key={target}
        type="button"
        style={buttonStyle}
        onClick={() => {
          setObjectIdDraft(target);
          setSelection({ objectId: target });
        }}
      >
        Reuse {target}
      </button>
    ))}
  </div>
) : null}
```

Only record explicitly applied, non-empty object IDs. Clearing the current target should not erase the recent-target history.

- [ ] **Step 4: Re-run the store and shell tests to verify they pass**

Run:

`npx --yes bun test "src/features/leak-workspace/live-detail-client.test.ts" "src/features/leak-workspace/LeakWorkspaceLayout.test.tsx"`

Expected: PASS

---

### Task 3: Harden The GC-Path Page Around Real Targets, Refresh, And Provenance

**Files:**
- Modify: `ui/src/features/leak-workspace/LeakGcPathPage.tsx`
- Modify: `ui/src/features/leak-workspace/LeakGcPathPage.test.tsx`

- [ ] **Step 1: Write the failing GC-path page tests**

Add targeted cases proving the page now depends on the real bridge and explicit refresh state:

```tsx
it("renders gc-path unavailable when the host bridge is absent", async () => {
  seedArtifact();
  act(() => {
    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
  });

  const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
    initialEntries: ["/leaks/leak-1/gc-path"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByText(/gc path unavailable: local gc path bridge is unavailable\./i)).toBeInTheDocument();
});

it("renders a bridge-backed gc path with root and edge details", async () => {
  seedArtifact();
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    findGcPath: async () => ({
      object_id: "0x0000000000001000",
      path_length: 2,
      path: [
        {
          object_id: "0x0000000000000001",
          class_name: "java.lang.Thread",
          field: "ROOT",
          is_root: true,
        },
        {
          object_id: "0x0000000000001000",
          class_name: "com.example.Cache",
          field: "entries",
          is_root: false,
        },
      ],
      provenance: [],
    }),
  };

  act(() => {
    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
  });

  const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
    initialEntries: ["/leaks/leak-1/gc-path"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByText(/current object target: 0x1000/i)).toBeInTheDocument();
  expect(view.getByText(/root node/i)).toBeInTheDocument();
  expect(view.getByText(/via: entries/i)).toBeInTheDocument();
});

it("renders fallback provenance details for a bridge-backed fallback gc path", async () => {
  seedArtifact();
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    findGcPath: async () => ({
      object_id: "0x1000",
      path_length: 2,
      path: [
        {
          object_id: "GC_ROOT_thread",
          class_name: "java.lang.Thread",
          field: "ROOT",
          is_root: true,
        },
        {
          object_id: "0x1000",
          class_name: "com.example.Cache",
          field: "entries",
          is_root: false,
        },
      ],
      provenance: [
        { kind: "SYNTHETIC", detail: "GC path was synthesized from summary-level heap information." },
        { kind: "FALLBACK", detail: "No real GC root chain could be resolved; best-effort fallback path returned." },
      ],
    }),
  };

  act(() => {
    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
  });

  const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
    initialEntries: ["/leaks/leak-1/gc-path"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  expect(await view.findByText(/gc path includes backend-reported fallback provenance\./i)).toBeInTheDocument();
  expect(view.getByText(/gc path was synthesized from summary-level heap information\./i)).toBeInTheDocument();
  expect(view.getByText(/no real gc root chain could be resolved; best-effort fallback path returned\./i)).toBeInTheDocument();
});

it("refreshes the current gc path when the operator requests it", async () => {
  seedArtifact();
  const calls: string[] = [];
  window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    findGcPath: async ({ objectId }: { objectId: string }) => {
      calls.push(objectId);
      return {
        object_id: objectId,
        path_length: 1,
        path: [
          {
            object_id: objectId,
            class_name: "com.example.Cache",
            field: "ROOT",
            is_root: true,
          },
        ],
        provenance: [],
      };
    },
  };

  act(() => {
    useLeakWorkspaceStore.getState().setSelection({ objectId: "0x1000" });
  });

  const router = createMemoryRouter([{ path: "/leaks/:leakId/gc-path", element: <LeakGcPathPage /> }], {
    initialEntries: ["/leaks/leak-1/gc-path"],
  });
  const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

  await view.findByText(/current object target: 0x1000/i);
  await userEvent.setup().click(view.getByRole("button", { name: /refresh gc path/i }));

  expect(calls).toEqual(["0x1000", "0x1000"]);
});
```

- [ ] **Step 2: Run the targeted GC-path page tests to verify they fail**

Run: `npx --yes bun test "src/features/leak-workspace/LeakGcPathPage.test.tsx"`
Expected: FAIL because the page still depends on the browser placeholder flow and has no explicit refresh control.

- [ ] **Step 3: Implement the hardened GC-path page**

Update `LeakGcPathPage.tsx` to use the real bridge-backed client and explicit refresh state:

```tsx
const gcPathRefreshNonce = useLeakWorkspaceStore((state) => state.gcPathRefreshNonce);
const requestGcPathRefresh = useLeakWorkspaceStore((state) => state.requestGcPathRefresh);
const requestKey = leakId && objectId ? `${leakId}:${objectId}:${gcPathRefreshNonce}` : undefined;

useEffect(() => {
  if (!artifact || !leakId || !objectId) {
    requestedKeyRef.current = undefined;
    return;
  }

  let cancelled = false;
  requestedKeyRef.current = requestKey;
  setSubviewState("gcPath", { status: "loading" });

  void findLeakGcPath({
    leakId,
    heapPath: artifact.summary.heapPath,
    objectId,
  })
    .then((result) => {
      if (!cancelled) {
        setSubviewState("gcPath", result);
      }
    })
    .catch((error: unknown) => {
      if (!cancelled) {
        setSubviewState("gcPath", {
          status: "error",
          error: error instanceof Error ? error.message : "GC path request failed.",
        });
      }
    });

  return () => {
    cancelled = true;
  };
}, [artifact, leakId, objectId, requestKey, setSubviewState]);
```

Render these states explicitly:

- missing target: `GC path is unavailable for this leak until an object target is present.`
- bridge missing/method missing: `GC path unavailable: ...`
- fallback: `GC path includes backend-reported fallback provenance.` plus each provenance detail line
- ready: path cards showing root/edge details from the real backend payload
- refresh: `Refresh GC path` button that bumps the store nonce via `requestGcPathRefresh()`

Keep the page honest: do not render any synthetic node or inferred target when the bridge has not provided one.

- [ ] **Step 4: Re-run the targeted GC-path page tests to verify they pass**

Run: `npx --yes bun test "src/features/leak-workspace/LeakGcPathPage.test.tsx"`
Expected: PASS

---

### Task 4: Verify Slice 3 End-To-End

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
Expected: intended leak-workspace files, the new Slice 3 plan file, and any unrelated pre-existing worktree changes

---

## Slice Queue After This Plan

After this slice lands, continue in this order:

1. artifact-driven explorer breadth (histogram + analyzer views)
2. competitive explorer surfaces (dominator/object/query)

## Execution Note

This repository is already being worked from an isolated worktree. Do not create commits unless the user explicitly asks for them.

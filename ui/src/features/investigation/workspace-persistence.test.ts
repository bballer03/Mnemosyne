import { describe, expect, it } from "bun:test";

import {
  WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
  createWorkspacePersistence,
  parsePersistedWorkspace,
  restoreCompatibleWorkspace,
  type PersistedWorkspaceV1,
  type WorkspacePersistenceIdentity,
} from "./workspace-persistence";

const identity: WorkspacePersistenceIdentity = {
  kind: "workspace",
  key: "source-opaque-a",
};

const validRecord: PersistedWorkspaceV1 = {
  schemaVersion: WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
  identity,
  revision: 3,
  layout: { activePane: "inspector" },
  filters: {
    histogram: {
      searchText: "cache",
      groupBy: "class",
      sortKey: "retained",
      sortDirection: "desc",
      pageOffset: 100,
    },
  },
  selection: {
    revision: 3,
    objectId: "object-current",
    classKey: "class-current",
    leakId: "leak-stale",
  },
  notes: [
    {
      id: "note-1",
      target: { kind: "object", id: "object-current" },
      text: "Inspect this retained object.",
    },
  ],
  bookmarks: [
    {
      id: "bookmark-1",
      target: { kind: "class", id: "class-current" },
      label: "Cache class",
    },
  ],
};

class MemoryStorage implements Storage {
  readonly values = new Map<string, string>();

  get length() {
    return this.values.size;
  }

  clear() {
    this.values.clear();
  }

  getItem(key: string) {
    return this.values.get(key) ?? null;
  }

  key(index: number) {
    return [...this.values.keys()][index] ?? null;
  }

  removeItem(key: string) {
    this.values.delete(key);
  }

  setItem(key: string, value: string) {
    this.values.set(key, value);
  }
}

describe("workspace persistence schema", () => {
  it("accepts and reconstructs versioned display-safe metadata", () => {
    expect(parsePersistedWorkspace(validRecord)).toEqual({
      status: "ready",
      record: validRecord,
    });
  });

  it("rejects unknown path, graph, and artifact payloads", () => {
    for (const candidate of [
      { ...validRecord, heapPath: "/srv/heaps/prod.hprof" },
      { ...validRecord, graph: { nodes: [{ fieldValue: "secret" }] } },
      { ...validRecord, artifact: { summary: {} } },
    ]) {
      expect(parsePersistedWorkspace(candidate).status).toBe("rejected");
    }
  });

  it("rejects absolute paths embedded in persisted display text", () => {
    const candidate = {
      ...validRecord,
      notes: [
        {
          id: "note-absolute",
          target: { kind: "workspace", id: "workspace-note" },
          text: "Heap was copied from /srv/heaps/prod.hprof",
        },
      ],
    };

    expect(parsePersistedWorkspace(candidate)).toEqual({
      status: "rejected",
      reason: "absolute paths are not display-safe",
    });
  });

  it("reports unsupported schema versions without guessing a migration", () => {
    expect(parsePersistedWorkspace({ ...validRecord, schemaVersion: 2 })).toEqual({
      status: "unsupported-schema",
      schemaVersion: 2,
    });
  });

  it("accepts only display-safe revision-bound workflow metadata", () => {
    const withWorkflow = {
      ...validRecord,
      workflow: {
        workflowId: "wf-internal",
        kind: "tune_gc" as const,
        currentStep: "thread_local_review",
        revision: validRecord.revision,
      },
    };
    expect(parsePersistedWorkspace(withWorkflow)).toEqual({
      status: "ready",
      record: withWorkflow,
    });

    for (const workflow of [
      { ...withWorkflow.workflow, workflowId: "/secret/workflow" },
      { ...withWorkflow.workflow, kind: "unknown" },
      { ...withWorkflow.workflow, currentStep: "/secret/heap.hprof" },
      { ...withWorkflow.workflow, revision: -1 },
    ]) {
      expect(parsePersistedWorkspace({ ...validRecord, workflow }).status).toBe("rejected");
    }
  });

  it("rebinds compatible IDs to the current revision and reports stale IDs", () => {
    const restored = restoreCompatibleWorkspace(validRecord, {
      identity,
      revision: 9,
      objectIds: new Set(["object-current"]),
      classKeys: new Set(["class-current"]),
      leakIds: new Set<string>(),
    });

    expect(restored.selection).toEqual({
      revision: 9,
      objectId: "object-current",
      classKey: "class-current",
    });
    expect(restored.droppedSelectionIds).toEqual([{ kind: "leak", id: "leak-stale" }]);
    expect(restored.notes).toEqual(validRecord.notes);
    expect(restored.bookmarks).toEqual(validRecord.bookmarks);
  });

  it("drops all selection IDs when no reopened-revision compatibility exists", () => {
    const restored = restoreCompatibleWorkspace(validRecord, {
      identity,
      revision: 10,
      objectIds: new Set<string>(),
      classKeys: new Set<string>(),
      leakIds: new Set<string>(),
    });

    expect(restored.selection).toEqual({ revision: 10 });
    expect(restored.layout).toEqual({});
    expect(restored.droppedSelectionIds).toEqual([
      { kind: "object", id: "object-current" },
      { kind: "class", id: "class-current" },
      { kind: "leak", id: "leak-stale" },
    ]);
  });

  it("restores a workflow only when it matched the persisted workspace revision", () => {
    const workflow = {
      workflowId: "wf-internal",
      kind: "tune_gc" as const,
      currentStep: "thread_local_review",
      revision: validRecord.revision,
    };
    const compatible = restoreCompatibleWorkspace(
      { ...validRecord, workflow },
      {
        identity,
        revision: 10,
        objectIds: new Set<string>(),
        classKeys: new Set<string>(),
        leakIds: new Set<string>(),
      },
    );
    expect(compatible.workflow).toEqual({ ...workflow, revision: 10 });

    const stale = restoreCompatibleWorkspace(
      { ...validRecord, workflow: { ...workflow, revision: validRecord.revision - 1 } },
      {
        identity,
        revision: 10,
        objectIds: new Set<string>(),
        classKeys: new Set<string>(),
        leakIds: new Set<string>(),
      },
    );
    expect(stale.workflow).toBeUndefined();
  });
});

describe("workspace persistence storage", () => {
  it("round-trips one record under its opaque identity", () => {
    const storage = new MemoryStorage();
    const persistence = createWorkspacePersistence(storage);

    expect(persistence.save(validRecord)).toEqual({ status: "saved" });
    expect(persistence.load(identity)).toEqual({
      status: "ready",
      record: validRecord,
    });
    expect([...storage.values.values()].join("\n")).not.toContain("/srv/");
  });

  it("rejects unsafe values before writing storage", () => {
    const storage = new MemoryStorage();
    const persistence = createWorkspacePersistence(storage);
    const unsafe = { ...validRecord, graph: { objects: ["secret"] } };

    expect(persistence.save(unsafe as PersistedWorkspaceV1)).toEqual({
      status: "rejected",
      reason: "unexpected field graph",
    });
    expect(storage.length).toBe(0);
  });

  it("does not load a record stored under a different embedded identity", () => {
    const storage = new MemoryStorage();
    const persistence = createWorkspacePersistence(storage);
    persistence.save(validRecord);
    const rawKey = storage.key(0);
    expect(rawKey).not.toBeNull();
    storage.setItem(
      rawKey!,
      JSON.stringify({
        ...validRecord,
        identity: { kind: "workspace", key: "source-opaque-b" },
      }),
    );

    expect(persistence.load(identity)).toEqual({
      status: "rejected",
      reason: "stored identity does not match requested identity",
    });
  });

  it("returns unavailable instead of throwing when storage access fails", () => {
    const failingStorage = {
      getItem() {
        throw new Error("disabled");
      },
      setItem() {
        throw new Error("disabled");
      },
      removeItem() {
        throw new Error("disabled");
      },
    } as unknown as Storage;
    const persistence = createWorkspacePersistence(failingStorage);

    expect(persistence.save(validRecord)).toEqual({ status: "unavailable" });
    expect(persistence.load(identity)).toEqual({ status: "unavailable" });
    expect(() => persistence.remove(identity)).not.toThrow();
  });
});

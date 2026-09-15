import { create } from "zustand";

import {
  createOperationId,
  createWorkspaceId,
  type OperationContext,
  type OperationKind,
  type OperationPhase,
  type OperationProgress,
} from "../../host/operation-protocol";
import type { HistogramGroupByMode } from "../heap-explorer/heap-explorer-query-client";
import {
  WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
  createWorkspacePersistence,
  restoreCompatibleWorkspace,
  type PersistedWorkspaceV1,
  type WorkspaceBookmark,
  type WorkspaceCompatibility,
  type WorkspaceNote,
  type WorkspacePersistenceIdentity,
  type WorkspaceRestoreResult,
} from "./workspace-persistence";

export type InvestigationOriginPane =
  | "histogram"
  | "dominators"
  | "inspector"
  | "gc-path"
  | "leak"
  | "findings";

export type InvestigationSelection = {
  revision: number;
  objectId?: string;
  classKey?: string;
  leakId?: string;
  originPane?: InvestigationOriginPane;
};

export type HistogramSortKey = "retained" | "shallow" | "instances" | "class";
export type HistogramSortDirection = "asc" | "desc";

export type HistogramViewState = {
  searchText: string;
  groupBy: HistogramGroupByMode;
  sortKey: HistogramSortKey;
  sortDirection: HistogramSortDirection;
  pageOffset: number;
};

export type ActiveOperation = OperationContext & {
  kind: OperationKind;
  status: OperationPhase;
  cancellationRequestedFrom?: OperationPhase;
  completed?: number;
  total?: number;
  unit?: string;
  indeterminate: boolean;
  elapsedMs: number;
};

type InvestigationState = InvestigationSelection & {
  workspaceId: string;
  activeOperation?: ActiveOperation;
  histogramView: HistogramViewState;
  persistenceIdentity?: WorkspacePersistenceIdentity;
  notes: WorkspaceNote[];
  bookmarks: WorkspaceBookmark[];
  lastPersistenceNotice?: string;
  beginOperation: (kind: OperationKind) => OperationContext;
  acceptOperationResult: (context: OperationContext) => boolean;
  updateOperationProgress: (progress: OperationProgress) => boolean;
  applyOperationResult: (context: OperationContext, apply: () => void) => boolean;
  requestOperationCancellation: (context: OperationContext) => boolean;
  rejectOperationCancellation: (context: OperationContext) => boolean;
  finishOperation: (context: OperationContext, status: OperationPhase) => boolean;
  setHistogramView: (patch: Partial<HistogramViewState>) => void;
  setObjectId: (objectId: string | undefined, originPane: InvestigationOriginPane) => void;
  setClassKey: (classKey: string | undefined, originPane: InvestigationOriginPane) => void;
  setLeakId: (leakId: string | undefined, originPane: InvestigationOriginPane) => void;
  activatePersistence: (
    identity: WorkspacePersistenceIdentity,
    compatibility: WorkspaceCompatibility,
  ) => WorkspaceRestoreResult | undefined;
  deactivatePersistence: () => void;
  upsertNote: (note: WorkspaceNote) => void;
  removeNote: (noteId: string) => void;
  upsertBookmark: (bookmark: WorkspaceBookmark) => void;
  removeBookmark: (bookmarkId: string) => void;
  clearSelection: () => void;
  bumpRevisionOnArtifactChange: () => void;
};

const clearedSelection = {
  objectId: undefined,
  classKey: undefined,
  leakId: undefined,
  originPane: undefined,
};

const defaultHistogramView: HistogramViewState = {
  searchText: "",
  groupBy: "class",
  sortKey: "retained",
  sortDirection: "desc",
  pageOffset: 0,
};

const terminalOperationPhases = new Set<OperationPhase>(["cancelled", "complete", "failed"]);

function workspacePersistence() {
  return createWorkspacePersistence();
}

function buildPersistedWorkspace(state: InvestigationState): PersistedWorkspaceV1 | undefined {
  if (!state.persistenceIdentity) {
    return undefined;
  }
  return {
    schemaVersion: WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
    identity: state.persistenceIdentity,
    revision: state.revision,
    layout: state.originPane ? { activePane: state.originPane } : {},
    filters: { histogram: { ...state.histogramView } },
    selection: {
      revision: state.revision,
      ...(state.objectId ? { objectId: state.objectId } : {}),
      ...(state.classKey ? { classKey: state.classKey } : {}),
      ...(state.leakId ? { leakId: state.leakId } : {}),
    },
    notes: state.notes,
    bookmarks: state.bookmarks,
  };
}

function savePersistedWorkspace(state: InvestigationState) {
  const record = buildPersistedWorkspace(state);
  if (record) {
    workspacePersistence().save(record);
  }
}

function formatDroppedSelectionNotice(
  dropped: WorkspaceRestoreResult["droppedSelectionIds"],
): string | undefined {
  if (dropped.length === 0) {
    return undefined;
  }
  return `Dropped stale workspace selections: ${dropped
    .map((entry) => `${entry.kind} ${entry.id}`)
    .join(", ")}`;
}

function persistenceMetadataChanged(
  current: InvestigationState,
  previous: InvestigationState,
): boolean {
  return (
    current.persistenceIdentity !== previous.persistenceIdentity ||
    current.revision !== previous.revision ||
    current.objectId !== previous.objectId ||
    current.classKey !== previous.classKey ||
    current.leakId !== previous.leakId ||
    current.originPane !== previous.originPane ||
    current.histogramView !== previous.histogramView ||
    current.notes !== previous.notes ||
    current.bookmarks !== previous.bookmarks
  );
}

function operationMatches(
  state: Pick<InvestigationState, "workspaceId" | "revision" | "activeOperation">,
  context: OperationContext,
): boolean {
  const active = state.activeOperation;
  return (
    active !== undefined &&
    context.workspaceId === state.workspaceId &&
    context.revision === state.revision &&
    context.workspaceId === active.workspaceId &&
    context.revision === active.revision &&
    context.operationId === active.operationId
  );
}

function operationAcceptsResult(state: InvestigationState, context: OperationContext): boolean {
  return (
    operationMatches(state, context) &&
    state.activeOperation?.status !== "cancelling" &&
    state.activeOperation?.status !== "cancelled"
  );
}

export const useInvestigationStore = create<InvestigationState>((set, get) => ({
  workspaceId: createWorkspaceId(),
  revision: 0,
  activeOperation: undefined,
  ...clearedSelection,
  histogramView: { ...defaultHistogramView },
  persistenceIdentity: undefined,
  notes: [],
  bookmarks: [],
  lastPersistenceNotice: undefined,
  beginOperation: (kind) => {
    const state = get();
    const context: OperationContext = {
      workspaceId: state.workspaceId,
      revision: state.revision,
      operationId: createOperationId(),
    };
    set({
      activeOperation: {
        ...context,
        kind,
        status: "accepted",
        indeterminate: true,
        elapsedMs: 0,
      },
    });
    return context;
  },
  acceptOperationResult: (context) => operationAcceptsResult(get(), context),
  updateOperationProgress: (progress) => {
    const state = get();
    if (
      !operationMatches(state, progress.context) ||
      state.activeOperation?.kind !== progress.kind
    ) {
      return false;
    }

    if (
      state.activeOperation.status === "cancelled" ||
      (state.activeOperation.status === "cancelling" && progress.phase !== "cancelled")
    ) {
      return false;
    }

    set({
      activeOperation: {
        ...progress.context,
        kind: progress.kind,
        status: progress.phase,
        cancellationRequestedFrom: undefined,
        completed: progress.completed,
        total: progress.total,
        unit: progress.unit,
        indeterminate: progress.indeterminate,
        elapsedMs: progress.elapsedMs,
      },
    });
    return true;
  },
  applyOperationResult: (context, apply) => {
    if (!operationAcceptsResult(get(), context)) {
      return false;
    }
    apply();
    return true;
  },
  requestOperationCancellation: (context) => {
    const state = get();
    const active = state.activeOperation;
    if (
      !operationMatches(state, context) ||
      active === undefined ||
      active.status === "cancelling" ||
      terminalOperationPhases.has(active.status)
    ) {
      return false;
    }
    set({
      activeOperation: {
        ...active,
        cancellationRequestedFrom: active.status,
        status: "cancelling",
      },
    });
    return true;
  },
  rejectOperationCancellation: (context) => {
    const state = get();
    const active = state.activeOperation;
    if (!operationMatches(state, context) || active?.status !== "cancelling") {
      return false;
    }
    set({
      activeOperation: {
        ...active,
        status: active.cancellationRequestedFrom ?? "accepted",
        cancellationRequestedFrom: undefined,
      },
    });
    return true;
  },
  finishOperation: (context, status) => {
    const state = get();
    if (!terminalOperationPhases.has(status) || !operationMatches(state, context)) {
      return false;
    }
    if (state.activeOperation?.status === "cancelling" && status === "complete") {
      return false;
    }
    if (status === "cancelled" && state.activeOperation) {
      set({
        activeOperation: {
          ...state.activeOperation,
          status: "cancelled",
          cancellationRequestedFrom: undefined,
        },
      });
    } else {
      set({ activeOperation: undefined });
    }
    return true;
  },
  setHistogramView: (patch) =>
    set((state) => {
      const resetsPage =
        (patch.searchText !== undefined &&
          patch.searchText !== state.histogramView.searchText) ||
        (patch.groupBy !== undefined && patch.groupBy !== state.histogramView.groupBy) ||
        (patch.sortKey !== undefined && patch.sortKey !== state.histogramView.sortKey) ||
        (patch.sortDirection !== undefined &&
          patch.sortDirection !== state.histogramView.sortDirection);

      return {
        histogramView: {
          ...state.histogramView,
          ...patch,
          ...(resetsPage ? { pageOffset: 0 } : {}),
        },
      };
    }),
  setObjectId: (objectId, originPane) => set({ objectId, originPane }),
  setClassKey: (classKey, originPane) => set({ classKey, originPane }),
  setLeakId: (leakId, originPane) => set({ leakId, originPane }),
  activatePersistence: (identity, compatibility) => {
    const loadResult = workspacePersistence().load(identity);
    if (loadResult.status !== "ready") {
      set({
        persistenceIdentity: identity,
        revision: compatibility.revision,
        notes: [],
        bookmarks: [],
        lastPersistenceNotice:
          loadResult.status === "missing" || loadResult.status === "unavailable"
            ? undefined
            : `Workspace metadata was not restored: ${loadResult.status}`,
      });
      return undefined;
    }

    const restored = restoreCompatibleWorkspace(loadResult.record, {
      ...compatibility,
      identity,
    });
    set({
      persistenceIdentity: identity,
      revision: restored.revision,
      activeOperation: undefined,
      ...restored.selection,
      originPane: restored.layout.activePane,
      histogramView: { ...restored.filters.histogram },
      notes: restored.notes,
      bookmarks: restored.bookmarks,
      lastPersistenceNotice: formatDroppedSelectionNotice(
        restored.droppedSelectionIds,
      ),
    });
    return restored;
  },
  deactivatePersistence: () => {
    savePersistedWorkspace(get());
    set({
      persistenceIdentity: undefined,
      notes: [],
      bookmarks: [],
      lastPersistenceNotice: undefined,
    });
  },
  upsertNote: (note) =>
    set((state) => ({
      notes: [note, ...state.notes.filter((entry) => entry.id !== note.id)].slice(
        0,
        100,
      ),
    })),
  removeNote: (noteId) =>
    set((state) => ({
      notes: state.notes.filter((entry) => entry.id !== noteId),
    })),
  upsertBookmark: (bookmark) =>
    set((state) => ({
      bookmarks: [
        bookmark,
        ...state.bookmarks.filter((entry) => entry.id !== bookmark.id),
      ].slice(0, 100),
    })),
  removeBookmark: (bookmarkId) =>
    set((state) => ({
      bookmarks: state.bookmarks.filter((entry) => entry.id !== bookmarkId),
    })),
  clearSelection: () => set(clearedSelection),
  bumpRevisionOnArtifactChange: () => {
    savePersistedWorkspace(get());
    set((state) => ({
      revision: state.revision + 1,
      activeOperation: undefined,
      ...clearedSelection,
      histogramView: { ...defaultHistogramView },
      persistenceIdentity: undefined,
      notes: [],
      bookmarks: [],
      lastPersistenceNotice: undefined,
    }));
  },
}));

useInvestigationStore.subscribe((state, previousState) => {
  if (state.persistenceIdentity && persistenceMetadataChanged(state, previousState)) {
    savePersistedWorkspace(state);
  }
});

import { create } from "zustand";

import {
  createOperationId,
  createWorkspaceId,
  type OperationContext,
  type OperationKind,
  type OperationPhase,
} from "../../host/operation-protocol";
import type { HistogramGroupByMode } from "../heap-explorer/heap-explorer-query-client";

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
};

type InvestigationState = InvestigationSelection & {
  workspaceId: string;
  activeOperation?: ActiveOperation;
  histogramView: HistogramViewState;
  beginOperation: (kind: OperationKind) => OperationContext;
  acceptOperationResult: (context: OperationContext) => boolean;
  applyOperationResult: (context: OperationContext, apply: () => void) => boolean;
  finishOperation: (context: OperationContext, status: OperationPhase) => boolean;
  setHistogramView: (patch: Partial<HistogramViewState>) => void;
  setObjectId: (objectId: string | undefined, originPane: InvestigationOriginPane) => void;
  setClassKey: (classKey: string | undefined, originPane: InvestigationOriginPane) => void;
  setLeakId: (leakId: string | undefined, originPane: InvestigationOriginPane) => void;
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

export const useInvestigationStore = create<InvestigationState>((set, get) => ({
  workspaceId: createWorkspaceId(),
  revision: 0,
  activeOperation: undefined,
  ...clearedSelection,
  histogramView: { ...defaultHistogramView },
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
      },
    });
    return context;
  },
  acceptOperationResult: (context) => operationMatches(get(), context),
  applyOperationResult: (context, apply) => {
    if (!operationMatches(get(), context)) {
      return false;
    }
    apply();
    return true;
  },
  finishOperation: (context, status) => {
    if (!terminalOperationPhases.has(status) || !operationMatches(get(), context)) {
      return false;
    }
    set({ activeOperation: undefined });
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
  clearSelection: () => set(clearedSelection),
  bumpRevisionOnArtifactChange: () =>
    set((state) => ({
      revision: state.revision + 1,
      activeOperation: undefined,
      ...clearedSelection,
      histogramView: { ...defaultHistogramView },
    })),
}));

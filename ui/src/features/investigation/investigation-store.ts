import { create } from "zustand";

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

type InvestigationState = InvestigationSelection & {
  histogramView: HistogramViewState;
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

export const useInvestigationStore = create<InvestigationState>((set) => ({
  revision: 0,
  ...clearedSelection,
  histogramView: { ...defaultHistogramView },
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
      ...clearedSelection,
      histogramView: { ...defaultHistogramView },
    })),
}));

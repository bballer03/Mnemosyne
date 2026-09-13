import { create } from "zustand";

import type { ObjectDiffReport } from "../../lib/diff-types";

export type ComparisonSourceKind = "file" | "live";

export type ComparisonLoadStatus = "idle" | "loading" | "ready" | "error" | "unavailable";

type ComparisonState = {
  diffReport?: ObjectDiffReport;
  sourceLabel?: string;
  sourceKind?: ComparisonSourceKind;
  loadStatus: ComparisonLoadStatus;
  loadError?: string;
  liveBeforeKey: string;
  liveAfterKey: string;
  setDiffReport: (report: ObjectDiffReport, sourceLabel: string, sourceKind: ComparisonSourceKind) => void;
  setLoadStatus: (status: ComparisonLoadStatus, error?: string) => void;
  setLiveBeforeKey: (value: string) => void;
  setLiveAfterKey: (value: string) => void;
  reset: () => void;
};

const initialState = {
  diffReport: undefined,
  sourceLabel: undefined,
  sourceKind: undefined,
  loadStatus: "idle" as ComparisonLoadStatus,
  loadError: undefined,
  liveBeforeKey: "",
  liveAfterKey: "",
};

export const useComparisonStore = create<ComparisonState>((set) => ({
  ...initialState,
  setDiffReport: (report, sourceLabel, sourceKind) =>
    set({
      diffReport: report,
      sourceLabel,
      sourceKind,
      loadStatus: "ready",
      loadError: undefined,
    }),
  setLoadStatus: (status, error) =>
    set({
      loadStatus: status,
      loadError: error,
    }),
  setLiveBeforeKey: (value) => set({ liveBeforeKey: value }),
  setLiveAfterKey: (value) => set({ liveAfterKey: value }),
  reset: () => set(initialState),
}));

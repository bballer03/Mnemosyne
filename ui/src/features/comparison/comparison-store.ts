import { create } from "zustand";

import type { IdentityStrategy, ObjectDiffReport } from "../../lib/diff-types";

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
  identityStrategy: IdentityStrategy;
  topN: number;
  crossReferenceLeaks: boolean;
  setDiffReport: (report: ObjectDiffReport, sourceLabel: string, sourceKind: ComparisonSourceKind) => void;
  setLoadStatus: (status: ComparisonLoadStatus, error?: string) => void;
  setLiveBeforeKey: (value: string) => void;
  setLiveAfterKey: (value: string) => void;
  setIdentityStrategy: (value: IdentityStrategy) => void;
  setTopN: (value: number) => void;
  setCrossReferenceLeaks: (value: boolean) => void;
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
  identityStrategy: "ClassDominator" as IdentityStrategy,
  topN: 50,
  crossReferenceLeaks: false,
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
  setIdentityStrategy: (value) => set({ identityStrategy: value }),
  setTopN: (value) => set({ topN: Math.min(500, Math.max(1, Math.trunc(value) || 1)) }),
  setCrossReferenceLeaks: (value) => set({ crossReferenceLeaks: value }),
  reset: () => set(initialState),
}));

import { create } from "zustand";

type DashboardState = {
  search: string;
  severity: string;
  provenanceFilter: string;
  minimumRetainedBytes?: number;
  expandedLeakIds: Record<string, boolean>;
  setSearch: (value: string) => void;
  setSeverity: (value: string) => void;
  setProvenanceFilter: (value: string) => void;
  setMinimumRetainedBytes: (value?: number) => void;
  toggleLeakExpanded: (leakId: string) => void;
  reset: () => void;
};

const initialState = {
  search: "",
  severity: "all",
  provenanceFilter: "all",
  minimumRetainedBytes: undefined,
  expandedLeakIds: {},
};

export const useDashboardStore = create<DashboardState>((set) => ({
  ...initialState,
  setSearch: (value) => set({ search: value }),
  setSeverity: (value) => set({ severity: value }),
  setProvenanceFilter: (value) => set({ provenanceFilter: value }),
  setMinimumRetainedBytes: (value) => set({ minimumRetainedBytes: value }),
  toggleLeakExpanded: (leakId) =>
    set((state) => ({
      expandedLeakIds: {
        ...state.expandedLeakIds,
        [leakId]: !state.expandedLeakIds[leakId],
      },
    })),
  reset: () => set(initialState),
}));

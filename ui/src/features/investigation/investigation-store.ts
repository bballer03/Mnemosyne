import { create } from "zustand";

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

type InvestigationState = InvestigationSelection & {
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

export const useInvestigationStore = create<InvestigationState>((set) => ({
  revision: 0,
  ...clearedSelection,
  setObjectId: (objectId, originPane) => set({ objectId, originPane }),
  setClassKey: (classKey, originPane) => set({ classKey, originPane }),
  setLeakId: (leakId, originPane) => set({ leakId, originPane }),
  clearSelection: () => set(clearedSelection),
  bumpRevisionOnArtifactChange: () =>
    set((state) => ({
      revision: state.revision + 1,
      ...clearedSelection,
    })),
}));

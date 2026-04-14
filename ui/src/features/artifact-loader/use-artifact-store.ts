import { create } from "zustand";

import type { AnalysisArtifact } from "../../lib/analysis-types";

type ArtifactState = {
  artifactName?: string;
  artifact?: AnalysisArtifact;
  loadError?: string;
  recentLoads: Array<{
    fileName: string;
    sizeLabel: string;
    loadedAtLabel: string;
    heapPath: string;
  }>;
  setArtifact: (artifactName: string, artifact: AnalysisArtifact) => void;
  setLoadError: (message: string) => void;
  addRecentLoad: (entry: {
    fileName: string;
    sizeLabel: string;
    loadedAtLabel: string;
    heapPath: string;
  }) => void;
  reset: () => void;
};

const initialState = {
  artifactName: undefined,
  artifact: undefined,
  loadError: undefined,
  recentLoads: [],
};

export const useArtifactStore = create<ArtifactState>((set) => ({
  ...initialState,
  setArtifact: (artifactName, artifact) =>
    set({
      artifactName,
      artifact,
      loadError: undefined,
    }),
  setLoadError: (message) =>
    set({
      artifactName: undefined,
      artifact: undefined,
      loadError: message,
    }),
  addRecentLoad: (entry) =>
    set((state) => ({
      recentLoads: [entry, ...state.recentLoads.filter((item) => item.fileName !== entry.fileName)].slice(
        0,
        4,
      ),
    })),
  reset: () => set(initialState),
}));

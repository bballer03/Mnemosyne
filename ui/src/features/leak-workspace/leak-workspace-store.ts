import { create } from "zustand";

import type {
  ExplainResult,
  FixResult,
  GcPathResult,
  LiveDetailKey,
  LiveDetailResult,
  SourceMapResult,
} from "./live-detail-client";

type LiveDetailDataByKey = {
  explain: ExplainResult;
  gcPath: GcPathResult;
  sourceMap: SourceMapResult;
  fix: FixResult;
};

type SubviewStateByKey = {
  [K in LiveDetailKey]: LiveDetailResult<LiveDetailDataByKey[K]>;
};

type WorkspaceSelection = {
  leakId?: string;
  heapPath?: string;
  objectId?: string;
  projectRoot?: string;
};

type LeakWorkspaceState = WorkspaceSelection & SubviewStateByKey & {
  recentObjectTargetsByLeak: Record<string, string[]>;
  gcPathRefreshNonce: number;
  requestGcPathRefresh: () => void;
  setSelection: (selection: Partial<WorkspaceSelection>) => void;
  setSubviewState: <K extends LiveDetailKey>(key: K, value: SubviewStateByKey[K]) => void;
  reset: () => void;
};

const idleSubview = { status: "idle" } as const;

const initialState = {
  leakId: undefined,
  heapPath: undefined,
  objectId: undefined,
  projectRoot: undefined,
  recentObjectTargetsByLeak: {},
  gcPathRefreshNonce: 0,
  explain: idleSubview,
  gcPath: idleSubview,
  sourceMap: idleSubview,
  fix: idleSubview,
};

function buildIdleSubviewState(): SubviewStateByKey {
  return {
    explain: { status: "idle" },
    gcPath: { status: "idle" },
    sourceMap: { status: "idle" },
    fix: { status: "idle" },
  };
}

function hasOwnSelectionField<K extends keyof WorkspaceSelection>(
  selection: Partial<WorkspaceSelection>,
  key: K,
) {
  return Object.prototype.hasOwnProperty.call(selection, key);
}

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

export const useLeakWorkspaceStore = create<LeakWorkspaceState>((set) => ({
  ...initialState,
  setSelection: (selection) =>
    set((state) => {
      const hasLeakId = hasOwnSelectionField(selection, "leakId");
      const hasHeapPath = hasOwnSelectionField(selection, "heapPath");
      const hasObjectId = hasOwnSelectionField(selection, "objectId");
      const hasProjectRoot = hasOwnSelectionField(selection, "projectRoot");

      const nextLeakId = hasLeakId ? selection.leakId : state.leakId;
      const nextHeapPath = hasHeapPath ? selection.heapPath : state.heapPath;
      const leakIdentityChanged =
        nextLeakId !== state.leakId || nextHeapPath !== state.heapPath;
      const nextRecentObjectTargetsByLeak =
        hasObjectId && selection.objectId
          ? rememberObjectTarget(state.recentObjectTargetsByLeak, nextLeakId, selection.objectId)
          : state.recentObjectTargetsByLeak;

      if (leakIdentityChanged) {
        return {
          leakId: nextLeakId,
          heapPath: nextHeapPath,
          objectId: hasObjectId ? selection.objectId : undefined,
          projectRoot: hasProjectRoot ? selection.projectRoot : undefined,
          recentObjectTargetsByLeak: nextRecentObjectTargetsByLeak,
          gcPathRefreshNonce: state.gcPathRefreshNonce,
          ...buildIdleSubviewState(),
        };
      }

      const nextObjectId = hasObjectId ? selection.objectId : state.objectId;
      const nextProjectRoot = hasProjectRoot ? selection.projectRoot : state.projectRoot;
      const dependentFieldsChanged =
        nextObjectId !== state.objectId || nextProjectRoot !== state.projectRoot;
      const historyChanged = nextRecentObjectTargetsByLeak !== state.recentObjectTargetsByLeak;

      return dependentFieldsChanged || historyChanged
        ? {
            leakId: state.leakId,
            heapPath: state.heapPath,
            objectId: nextObjectId,
            projectRoot: nextProjectRoot,
            recentObjectTargetsByLeak: nextRecentObjectTargetsByLeak,
            gcPathRefreshNonce: state.gcPathRefreshNonce,
            ...buildIdleSubviewState(),
          }
        : state;
    }),
  requestGcPathRefresh: () => set((state) => ({ gcPathRefreshNonce: state.gcPathRefreshNonce + 1 })),
  setSubviewState: (key, value) => set({ [key]: value } as Pick<SubviewStateByKey, typeof key>),
  reset: () => set({ ...initialState, ...buildIdleSubviewState() }),
}));

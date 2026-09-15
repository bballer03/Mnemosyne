import { parseAnalysisArtifact, type AnalysisArtifact } from "../../lib/analysis-types";
import { formatHostError } from "../../host/format-host-error";
import {
  pickHeapFile,
  runDesktopAnalysis,
  unloadHeap,
} from "../artifact-loader/desktop-heap-client";
import {
  clearRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useDashboardStore } from "../dashboard/dashboard-store";
import { useComparisonStore } from "../comparison/comparison-store";
import { useLeakWorkspaceStore } from "../leak-workspace/leak-workspace-store";
import {
  runOpenSnapshot,
  type SnapshotWorkspaceHydrate,
  type WorkflowBridgeResult,
} from "../workflow-landing/workflow-bridge-client";
import { closeOrDetachWorkspaceWorkflow } from "../workflow-landing/workflow-binding";
import { useInvestigationStore } from "./investigation-store";
import type { WorkspaceCompatibility } from "./workspace-persistence";

export type OpenHeapPhase = "idle" | "picking" | "analyzing";

export type OpenDesktopHeapResult =
  | { status: "cancelled" }
  | { status: "unavailable"; message: string }
  | { status: "error"; message: string }
  | {
      status: "ready";
      displayName: string;
      sourceId: string;
      artifact: AnalysisArtifact;
    };

function buildArtifactCompatibility(
  artifact: AnalysisArtifact,
  revision: number,
): Omit<WorkspaceCompatibility, "identity"> {
  return {
    revision,
    objectIds: new Set(artifact.graph.dominators.map((entry) => entry.objectId)),
    classKeys: new Set(artifact.histogram?.entries.map((entry) => entry.key) ?? []),
    leakIds: new Set(artifact.leaks.map((entry) => entry.id)),
  };
}

function recentTimestamp() {
  return new Intl.DateTimeFormat("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date());
}

function displayNameForPath(path: string) {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

async function analyzeDesktopHeapSource(
  sourceId: string,
  displayName: string,
): Promise<OpenDesktopHeapResult> {
  try {
    const raw = await runDesktopAnalysis({
      sourceId,
      mode: "incident",
      enableClassloaders: true,
      enableTopInstances: true,
      enableThreads: false,
      enableStrings: false,
      enableCollections: false,
      enableByReferrer: false,
      enableDuplicateArrays: false,
    });
    const artifact = parseAnalysisArtifact(raw);
    rememberDesktopHeapSource(sourceId, displayName);
    return { status: "ready", displayName, sourceId, artifact };
  } catch (error) {
    return {
      status: "error",
      message: formatHostError(error, "Failed to open heap dump"),
    };
  }
}

/** Reopen a session-local recent heap via its opaque host source id. */
export async function openDesktopHeapFromSource(
  sourceId: string,
  displayName: string,
  onPhase?: (phase: OpenHeapPhase) => void,
): Promise<OpenDesktopHeapResult> {
  onPhase?.("analyzing");
  try {
    return await analyzeDesktopHeapSource(sourceId, displayName);
  } finally {
    onPhase?.("idle");
  }
}

/** Lean first-open / open-another: remember source only after success. */
export async function openDesktopHeapLean(
  onPhase?: (phase: OpenHeapPhase) => void,
): Promise<OpenDesktopHeapResult> {
  onPhase?.("picking");
  try {
    const picked = await pickHeapFile();
    if (picked.status === "cancelled") {
      return { status: "cancelled" };
    }
    if (picked.status === "unavailable") {
      const inTauri =
        typeof globalThis !== "undefined" && "__TAURI_INTERNALS__" in globalThis;
      return {
        status: "unavailable",
        message: inTauri
          ? "Desktop host is running but the heap bridge failed to load. Restart the app, or import an analysis JSON artifact."
          : "Open heap dump needs the desktop app. In the browser, import an analysis JSON artifact instead.",
      };
    }

    onPhase?.("analyzing");
    return await analyzeDesktopHeapSource(picked.sourceId, picked.displayName);
  } catch (error) {
    return {
      status: "error",
      message: formatHostError(error, "Failed to open heap dump"),
    };
  } finally {
    onPhase?.("idle");
  }
}

export function applyOpenedHeap(displayName: string, artifact: AnalysisArtifact, sourceId?: string) {
  void closeOrDetachWorkspaceWorkflow();
  useInvestigationStore.getState().bumpRevisionOnArtifactChange();
  useArtifactStore.getState().setArtifact(displayName, artifact);
  if (sourceId) {
    const persistenceIdentity = { kind: "workspace" as const, key: sourceId };
    useInvestigationStore.getState().activatePersistence(persistenceIdentity, {
      ...buildArtifactCompatibility(
        artifact,
        useInvestigationStore.getState().revision,
      ),
      identity: persistenceIdentity,
    });
  }
  useDashboardStore.getState().reset();
  useArtifactStore.getState().addRecentLoad({
    fileName: displayName,
    sizeLabel: `${artifact.summary.totalObjects.toLocaleString()} objects`,
    loadedAtLabel: recentTimestamp(),
    heapPath: displayNameForPath(artifact.summary.heapPath),
    sourceId,
  });
}

export function applySnapshotWorkspaceHydrate(hydrate: SnapshotWorkspaceHydrate) {
  void closeOrDetachWorkspaceWorkflow();
  const { snapshot, analysis, mode, capabilities } = hydrate;
  const nextRevision = useInvestigationStore.getState().revision + 1;
  const recentLoad = {
    fileName: snapshot.displayName,
    sizeLabel: `${analysis.summary.totalObjects.toLocaleString()} objects`,
    loadedAtLabel: recentTimestamp(),
    heapPath: snapshot.displayName,
    sourceId: snapshot.sourceId,
  };

  useArtifactStore.setState((state) => ({
    artifactName: snapshot.displayName,
    artifact: analysis,
    loadError: undefined,
    recentLoads: [
      recentLoad,
      ...state.recentLoads.filter((item) => item.fileName !== snapshot.displayName),
    ].slice(0, 4),
  }));
  useDashboardStore.getState().reset();
  useComparisonStore.getState().reset();
  useLeakWorkspaceStore.getState().reset();
  const persistenceIdentity = {
    kind: "snapshot" as const,
    key: snapshot.key,
  };
  useInvestigationStore.getState().commitSnapshotHydrate(
    persistenceIdentity,
    buildArtifactCompatibility(analysis, nextRevision),
    mode,
    capabilities,
  );
  rememberDesktopHeapSource(snapshot.sourceId, snapshot.displayName);
}

export async function openSnapshotWorkspace(
  key: string,
): Promise<WorkflowBridgeResult<SnapshotWorkspaceHydrate>> {
  const result = await runOpenSnapshot(key);
  if (result.status === "ready") {
    applySnapshotWorkspaceHydrate(result.data);
  }
  return result;
}

/** Close investigation: unload host graph (if any) and clear heap-bound UI state. */
export async function closeInvestigationWorkspace(): Promise<void> {
  await closeOrDetachWorkspaceWorkflow();
  try {
    await unloadHeap();
  } catch {
    // Host may be browser-only or already unloaded; still clear UI.
  }
  clearRememberedDesktopHeapSource();
  useArtifactStore.getState().reset();
  useDashboardStore.getState().reset();
  useComparisonStore.getState().reset();
  useLeakWorkspaceStore.getState().reset();
  useInvestigationStore.getState().bumpRevisionOnArtifactChange();
}

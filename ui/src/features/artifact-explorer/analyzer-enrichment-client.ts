import {
  parseAnalysisArtifact,
  type AnalysisArtifact,
  type ArtifactProvenanceMarker,
} from "../../lib/analysis-types";
import {
  getDesktopHeapBridge,
  runDesktopAnalysis,
  type DesktopAnalysisInput,
} from "../artifact-loader/desktop-heap-client";
import { getRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useInvestigationStore } from "../investigation/investigation-store";

export type AnalyzerSelection = {
  strings: boolean;
  collections: boolean;
  duplicateArrays: boolean;
  threads: boolean;
  referrers: boolean;
  classloaders: boolean;
};

export type AnalyzerEnrichmentResult =
  | {
      status: "ready";
      requested: string[];
      unavailable: string[];
      provenance: ArtifactProvenanceMarker[];
    }
  | { status: "stale" }
  | { status: "unavailable"; message: string }
  | { status: "error"; message: string };

const requestedSections = [
  ["strings", "Strings", "stringReport"],
  ["collections", "Collections", "collectionReport"],
  ["duplicateArrays", "Duplicate arrays", "arrayReport"],
  ["threads", "Threads", "threadReport"],
  ["referrers", "Referrers", "referrerReport"],
  ["classloaders", "Classloaders", "classloaderReport"],
] as const satisfies ReadonlyArray<
  readonly [keyof AnalyzerSelection, string, keyof AnalysisArtifact]
>;

function buildAnalysisInput(
  sourceId: string,
  selection: AnalyzerSelection,
  artifact: AnalysisArtifact,
): DesktopAnalysisInput {
  return {
    sourceId,
    mode: "custom",
    enableClassloaders: selection.classloaders || artifact.classloaderReport !== undefined,
    enableThreads: selection.threads || artifact.threadReport !== undefined,
    enableStrings: selection.strings || artifact.stringReport !== undefined,
    enableCollections: selection.collections || artifact.collectionReport !== undefined,
    enableTopInstances: artifact.topInstances !== undefined,
    enableByReferrer: selection.referrers || artifact.referrerReport !== undefined,
    enableDuplicateArrays: selection.duplicateArrays || artifact.arrayReport !== undefined,
  };
}

function relevantProvenance(
  provenance: ArtifactProvenanceMarker[],
): ArtifactProvenanceMarker[] {
  return provenance.filter((marker) => {
    const kind = marker.kind.toLowerCase();
    return kind === "partial" || kind === "fallback";
  });
}

export async function runAnalyzerEnrichment(
  selection: AnalyzerSelection,
): Promise<AnalyzerEnrichmentResult> {
  const source = getRememberedDesktopHeapSource();
  const { artifact, artifactName } = useArtifactStore.getState();
  if (!source || !artifact || !artifactName) {
    return {
      status: "unavailable",
      message: "Analyzer enrichment needs an open desktop heap investigation.",
    };
  }
  if (!getDesktopHeapBridge()?.runDesktopAnalysis) {
    return {
      status: "unavailable",
      message: "Analyzer enrichment is unavailable because no desktop host bridge is connected.",
    };
  }

  const { workspaceId, revision } = useInvestigationStore.getState();
  try {
    const raw = await runDesktopAnalysis(buildAnalysisInput(source.sourceId, selection, artifact));
    const enrichedArtifact = parseAnalysisArtifact(raw);
    const current = useInvestigationStore.getState();
    if (current.workspaceId !== workspaceId || current.revision !== revision) {
      return { status: "stale" };
    }

    useArtifactStore.getState().setArtifact(artifactName, enrichedArtifact);
    const requested = requestedSections
      .filter(([selectionKey]) => selection[selectionKey])
      .map(([, label]) => label);
    const unavailable = requestedSections
      .filter(
        ([selectionKey, , reportKey]) =>
          selection[selectionKey] && enrichedArtifact[reportKey] === undefined,
      )
      .map(([, label]) => label);

    return {
      status: "ready",
      requested,
      unavailable,
      provenance: relevantProvenance(enrichedArtifact.provenance),
    };
  } catch (error) {
    return {
      status: "error",
      message: error instanceof Error ? error.message : "Analyzer enrichment failed.",
    };
  }
}

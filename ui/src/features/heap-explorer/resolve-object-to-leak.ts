import type { AnalysisArtifact } from "../../lib/analysis-types";

export function resolveObjectToLeak(
  objectId: string | undefined,
  artifact: AnalysisArtifact,
): string | undefined {
  if (!objectId) {
    return undefined;
  }

  const dominatorRow = artifact.graph.dominators.find((row) => row.objectId === objectId);
  if (!dominatorRow) {
    return undefined;
  }

  const matchingLeak = artifact.leaks.find((leak) => leak.className === dominatorRow.className);
  return matchingLeak?.id;
}
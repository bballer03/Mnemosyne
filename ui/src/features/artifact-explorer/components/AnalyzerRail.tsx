import type { ReactNode } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

type AnalyzerRailProps = {
  artifact: AnalysisArtifact;
};

type AnalyzerCardProps = {
  title: string;
  state: "present" | "empty" | "absent";
  children: ReactNode;
};

function AnalyzerCard({ title, state, children }: AnalyzerCardProps) {
  return (
    <section
      style={{
        display: "grid",
        gap: "0.45rem",
        borderRadius: 16,
        border: "1px solid #1e293b",
        background: "rgba(2, 6, 23, 0.75)",
        padding: "0.85rem 0.9rem",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "0.75rem", alignItems: "start" }}>
        <strong>{title}</strong>
        <span
          style={{
            color: state === "present" ? "#86efac" : state === "empty" ? "#facc15" : "#94a3b8",
            fontSize: "0.72rem",
            letterSpacing: "0.08em",
            textTransform: "uppercase",
          }}
        >
          {state === "absent" ? "SECTION_ABSENT" : state.toUpperCase()}
        </span>
      </div>
      <div style={{ color: "#cbd5e1", fontSize: "0.92rem", lineHeight: 1.5 }}>{children}</div>
    </section>
  );
}

export function AnalyzerRail({ artifact }: AnalyzerRailProps) {
  const recommendationState = artifact.recommendations.length > 0 ? "present" : "empty";
  const stringState = artifact.stringReport
    ? artifact.stringReport.duplicateGroups.length > 0 || artifact.stringReport.topStringsBySize.length > 0
      ? "present"
      : "empty"
    : "absent";
  const collectionState = artifact.collectionReport
    ? artifact.collectionReport.oversizedCollections.length > 0 || Object.keys(artifact.collectionReport.summaryByType).length > 0
      ? "present"
      : "empty"
    : "absent";
  const topInstancesState = artifact.topInstances
    ? artifact.topInstances.instances.length > 0
      ? "present"
      : "empty"
    : "absent";
  const classloaderState = artifact.classloaderReport
    ? artifact.classloaderReport.loaders.length > 0 || artifact.classloaderReport.potentialLeaks.length > 0
      ? "present"
      : "empty"
    : "absent";
  const unreachableState = artifact.unreachable
    ? artifact.unreachable.totalCount > 0
      ? "present"
      : "empty"
    : "absent";

  return (
    <div style={{ display: "grid", gap: "0.85rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Analyzer Rail</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Optional artifact-backed summaries from the loaded analysis snapshot.
        </p>
      </div>

      <AnalyzerCard title="Artifact Recommendations" state={recommendationState}>
        {recommendationState === "present"
          ? `${artifact.recommendations.length} recommendation${artifact.recommendations.length === 1 ? "" : "s"} available.`
          : "No artifact recommendations were included in this snapshot."}
      </AnalyzerCard>

      <AnalyzerCard title="String Deduplication" state={stringState}>
        {stringState === "present"
          ? `${artifact.stringReport?.duplicateGroups.length ?? 0} duplicate groups with ${artifact.stringReport?.totalDuplicateWaste ?? 0} bytes of duplicate waste.`
          : stringState === "empty"
            ? "String analysis is present but has no duplicate groups or top-string rows to show."
            : "String analysis was not serialized into this artifact."}
      </AnalyzerCard>

      <AnalyzerCard title="Collections" state={collectionState}>
        {collectionState === "present"
          ? `${artifact.collectionReport?.oversizedCollections.length ?? 0} oversized collections across ${artifact.collectionReport?.totalCollections ?? 0} inspected collections.`
          : collectionState === "empty"
            ? "Collection analysis is present but currently empty."
            : "Collection analysis is not available in this artifact."}
      </AnalyzerCard>

      <AnalyzerCard title="Top Instances" state={topInstancesState}>
        {topInstancesState === "present"
          ? `${artifact.topInstances?.instances.length ?? 0} ranked instances out of ${artifact.topInstances?.totalCount ?? 0} total objects.`
          : topInstancesState === "empty"
            ? "Top-instance analysis is present but contains no ranked rows."
            : "Top-instance analysis is not available in this artifact."}
      </AnalyzerCard>

      <AnalyzerCard title="Classloaders" state={classloaderState}>
        {classloaderState === "present"
          ? `${artifact.classloaderReport?.loaders.length ?? 0} loaders tracked with ${artifact.classloaderReport?.potentialLeaks.length ?? 0} potential leak candidates.`
          : classloaderState === "empty"
            ? "Classloader analysis is present but has no loaders or leak candidates to show."
            : "Classloader analysis is not available in this artifact."}
      </AnalyzerCard>

      <AnalyzerCard title="Unreachable Summary" state={unreachableState}>
        {unreachableState === "present"
          ? `${artifact.unreachable?.totalCount ?? 0} unreachable objects totaling ${artifact.unreachable?.totalShallowSize ?? 0} shallow bytes.`
          : unreachableState === "empty"
            ? "Unreachable-object analysis is present but reports zero unreachable objects."
            : "Unreachable-object analysis is not available in this artifact."}
      </AnalyzerCard>
    </div>
  );
}

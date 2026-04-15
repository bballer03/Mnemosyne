import type { AnalysisArtifact } from "../../lib/analysis-types";

export type LeakWorkspaceDependencyStatus = {
  bridge: "ready" | "unavailable";
  projectRoot: "present" | "missing";
  objectTarget: "present" | "missing";
  provider: "ready" | "unknown" | "unavailable";
};

export type LiveSubviewStatus = "idle" | "loading" | "ready" | "error" | "unavailable" | "fallback";

export type SelectedLeak = AnalysisArtifact["leaks"][number];

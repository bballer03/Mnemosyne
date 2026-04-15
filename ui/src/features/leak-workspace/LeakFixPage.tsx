import { useEffect, useRef } from "react";
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { proposeLeakFix } from "./live-detail-client";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

const sectionStyle = {
  display: "grid",
  gap: "0.75rem",
} as const;

const cardStyle = {
  border: "1px solid #334155",
  borderRadius: 12,
  padding: "0.75rem",
  display: "grid",
  gap: "0.35rem",
} as const;

export function LeakFixPage() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const projectRoot = useLeakWorkspaceStore((state) => state.projectRoot);
  const fix = useLeakWorkspaceStore((state) => state.fix);
  const setSubviewState = useLeakWorkspaceStore((state) => state.setSubviewState);
  const requestedFixKeyRef = useRef<string | undefined>(undefined);

  const leak = artifact?.leaks.find((entry) => entry.id === leakId);
  const requestKey = leakId ? `${leakId}::${projectRoot ?? ""}` : undefined;
  const hasRequestedCurrentFix = requestedFixKeyRef.current === requestKey;
  const showLoading = !hasRequestedCurrentFix || fix.status === "loading" || fix.status === "idle";
  const currentFix = hasRequestedCurrentFix ? fix.data : undefined;
  const showUnavailable = hasRequestedCurrentFix && fix.status === "unavailable";
  const showError = hasRequestedCurrentFix && fix.status === "error";
  const showFallback = hasRequestedCurrentFix && fix.status === "fallback";

  useEffect(() => {
    if (!artifact || !leakId || !leak) {
      return;
    }

    let cancelled = false;
    requestedFixKeyRef.current = requestKey;
    setSubviewState("fix", { status: "loading" });

    void proposeLeakFix({ leakId, heapPath: artifact.summary.heapPath, projectRoot })
      .then((result) => {
        if (!cancelled) {
          setSubviewState("fix", result);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setSubviewState("fix", {
            status: "error",
            error: error instanceof Error ? error.message : "Fix request failed.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [artifact, leak, leakId, projectRoot, requestKey, setSubviewState]);

  if (!artifact || !leakId || !leak) {
    return null;
  }

  return (
    <section style={sectionStyle}>
      <h3 style={{ margin: 0 }}>Fix Proposal</h3>
      {showLoading ? <div>Loading fix proposal...</div> : null}
      {showUnavailable ? <div>Fix proposal unavailable: {fix.error ?? "Required local context is missing."}</div> : null}
      {showError ? <div>Fix proposal failed: {fix.error ?? "Unknown error."}</div> : null}
      {showFallback ? <div>Fallback: provider-backed generation was unavailable, showing heuristic guidance.</div> : null}
      {currentFix?.suggestions.map((suggestion, index) => (
        <article key={`${suggestion.target_file ?? "suggestion"}:${index}`} style={cardStyle}>
          <div>{suggestion.description ?? "No description available."}</div>
          <div>Target file: {suggestion.target_file ?? "Unknown target"}</div>
          <div>Style: {suggestion.style ?? "Unknown"}</div>
          <div>Confidence: {suggestion.confidence ?? 0}</div>
          <pre style={{ margin: 0, whiteSpace: "pre-wrap" }}>{suggestion.diff ?? "No diff preview available."}</pre>
        </article>
      ))}
    </section>
  );
}

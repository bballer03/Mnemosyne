import { useEffect, useRef } from "react";
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { explainLeak } from "./live-detail-client";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

export function LeakExplainPage() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const explain = useLeakWorkspaceStore((state) => state.explain);
  const setSubviewState = useLeakWorkspaceStore((state) => state.setSubviewState);
  const requestedLeakIdRef = useRef<string | undefined>(undefined);

  const hasRequestedCurrentLeak = requestedLeakIdRef.current === leakId;
  const showLoading = !hasRequestedCurrentLeak || explain.status === "loading" || explain.status === "idle";
  const currentExplain = hasRequestedCurrentLeak && explain.data?.leak_id === leakId ? explain.data : undefined;
  const showError = hasRequestedCurrentLeak && explain.status === "error";
  const showUnavailable = hasRequestedCurrentLeak && explain.status === "unavailable";
  const showFallback = hasRequestedCurrentLeak && explain.status === "fallback";

  useEffect(() => {
    if (!artifact || !leakId) {
      return;
    }

    let cancelled = false;
    requestedLeakIdRef.current = leakId;
    setSubviewState("explain", { status: "loading" });

    void explainLeak({ leakId, heapPath: artifact.summary.heapPath })
      .then((result) => {
        if (!cancelled) {
          setSubviewState("explain", result);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setSubviewState("explain", {
            status: "error",
            error: error instanceof Error ? error.message : "Explanation request failed.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [artifact, leakId, setSubviewState]);

  if (!artifact || !leakId) {
    return null;
  }

  return (
    <section style={{ display: "grid", gap: "0.75rem" }}>
      <h3 style={{ margin: 0 }}>Explain</h3>
      {showLoading ? <div>Loading explanation...</div> : null}
      {showUnavailable ? <div>Explain unavailable: {explain.error ?? "No local explain bridge is available."}</div> : null}
      {showError ? <div>Explanation failed: {explain.error ?? "Unknown error."}</div> : null}
      {currentExplain ? <div>{currentExplain.summary}</div> : null}
      {showFallback ? <div>Explanation includes backend-reported fallback provenance.</div> : null}
    </section>
  );
}

import { useEffect, useRef } from "react";
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { resolveLeakSourceMap } from "./live-detail-client";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

const sectionStyle = {
  display: "grid",
  gap: "0.5rem",
} as const;

export function LeakSourceMapPage() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const projectRoot = useLeakWorkspaceStore((state) => state.projectRoot);
  const sourceMap = useLeakWorkspaceStore((state) => state.sourceMap);
  const setSubviewState = useLeakWorkspaceStore((state) => state.setSubviewState);
  const requestedLeakIdRef = useRef<string | undefined>(undefined);

  const leak = artifact?.leaks.find((entry) => entry.id === leakId);
  const hasRequestedCurrentLeak = requestedLeakIdRef.current === leakId;
  const showLoading = !hasRequestedCurrentLeak || sourceMap.status === "loading" || sourceMap.status === "idle";
  const currentSourceMap = hasRequestedCurrentLeak && sourceMap.data?.leak_id === leakId ? sourceMap.data : undefined;
  const showError = hasRequestedCurrentLeak && sourceMap.status === "error";
  const showUnavailable = hasRequestedCurrentLeak && sourceMap.status === "unavailable";
  const showFallback = hasRequestedCurrentLeak && sourceMap.status === "fallback";

  useEffect(() => {
    if (!leakId || !leak || !projectRoot) {
      return;
    }

    let cancelled = false;
    requestedLeakIdRef.current = leakId;
    setSubviewState("sourceMap", { status: "loading" });

    void resolveLeakSourceMap({
      leakId,
      className: leak.className,
      projectRoot,
    })
      .then((result) => {
        if (!cancelled) {
          setSubviewState("sourceMap", result);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setSubviewState("sourceMap", {
            status: "error",
            error: error instanceof Error ? error.message : "Source map request failed.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [leak, leakId, projectRoot, setSubviewState]);

  if (!artifact || !leakId || !leak) {
    return null;
  }

  if (!projectRoot) {
    return (
      <section style={sectionStyle}>
        <h3 style={{ margin: 0 }}>Source Map</h3>
        <div>Source map is unavailable until a project root is configured.</div>
      </section>
    );
  }

  return (
    <section style={sectionStyle}>
      <h3 style={{ margin: 0 }}>Source Map</h3>
      {showLoading ? <div>Loading source map...</div> : null}
      {showError ? <div>Source map failed: {sourceMap.error ?? "Unknown error."}</div> : null}
      {showUnavailable ? <div>Source map unavailable: {sourceMap.error ?? "No source mapping is available for this leak."}</div> : null}
      {showFallback ? <div>Mapping fell back to an unmapped placeholder.</div> : null}
      {currentSourceMap?.locations.map((location) => (
        <article
          key={`${location.file}:${location.line}:${location.symbol}`}
          style={{ border: "1px solid #334155", borderRadius: 12, padding: "0.75rem", display: "grid", gap: "0.35rem" }}
        >
          <div>{location.file}</div>
          <div>
            Line {location.line} - {location.symbol}
          </div>
          <div>{location.code_snippet || "No snippet available."}</div>
          <div>Git metadata: {location.git ? "present" : "missing"}</div>
        </article>
      ))}
    </section>
  );
}

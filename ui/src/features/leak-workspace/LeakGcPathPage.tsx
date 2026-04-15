import { useEffect, useRef } from "react";
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { findLeakGcPath } from "./live-detail-client";
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

export function LeakGcPathPage() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const objectId = useLeakWorkspaceStore((state) => state.objectId);
  const gcPath = useLeakWorkspaceStore((state) => state.gcPath);
  const gcPathRefreshNonce = useLeakWorkspaceStore((state) => state.gcPathRefreshNonce);
  const requestGcPathRefresh = useLeakWorkspaceStore((state) => state.requestGcPathRefresh);
  const setSubviewState = useLeakWorkspaceStore((state) => state.setSubviewState);
  const requestedKeyRef = useRef<string | undefined>(undefined);
  const heapPath = artifact?.summary.heapPath;

  const leak = artifact?.leaks.find((entry) => entry.id === leakId);
  const requestKey = leakId && heapPath && objectId ? `${leakId}:${heapPath}:${objectId}:${gcPathRefreshNonce}` : undefined;
  const hasRequestedCurrentPath = requestedKeyRef.current === requestKey;
  const showLoading = Boolean(requestKey) && (!hasRequestedCurrentPath || gcPath.status === "loading" || gcPath.status === "idle");
  const currentPath = hasRequestedCurrentPath && gcPath.data?.leak_id === leakId ? gcPath.data : undefined;
  const showUnavailable = Boolean(requestKey) && hasRequestedCurrentPath && gcPath.status === "unavailable";
  const showFallback = Boolean(requestKey) && hasRequestedCurrentPath && gcPath.status === "fallback";
  const showError = Boolean(requestKey) && hasRequestedCurrentPath && gcPath.status === "error";

  useEffect(() => {
    if (!artifact || !leakId || !leak || !objectId) {
      requestedKeyRef.current = undefined;
      return;
    }

    let cancelled = false;
    requestedKeyRef.current = requestKey;
    setSubviewState("gcPath", { status: "loading" });

    void findLeakGcPath({ leakId, heapPath: artifact.summary.heapPath, objectId })
      .then((result) => {
        if (!cancelled) {
          setSubviewState("gcPath", result);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setSubviewState("gcPath", {
            status: "error",
            error: error instanceof Error ? error.message : "GC path request failed.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [artifact, leak, leakId, objectId, requestKey, setSubviewState]);

  if (!artifact || !leakId || !leak) {
    return null;
  }

  if (!objectId) {
    return (
      <section style={sectionStyle}>
        <h3 style={{ margin: 0 }}>GC Path</h3>
        <div>GC path is unavailable for this leak until an object target is present.</div>
      </section>
    );
  }

  return (
    <section style={sectionStyle}>
      <h3 style={{ margin: 0 }}>GC Path</h3>
      <div>Current object target: {objectId}</div>
      <button type="button" onClick={() => requestGcPathRefresh()}>
        Refresh GC path
      </button>
      {showLoading ? <div>Loading GC path...</div> : null}
      {showUnavailable ? <div>GC path unavailable: {gcPath.error ?? "Unknown error."}</div> : null}
      {showError ? <div>GC path failed: {gcPath.error ?? "Unknown error."}</div> : null}
      {showFallback ? <div>GC path includes backend-reported fallback provenance.</div> : null}
      {showFallback
        ? currentPath?.provenance?.map((marker, index) => (
            <div key={`${marker.kind}:${marker.detail ?? index}`}>{marker.detail ?? marker.kind}</div>
          ))
        : null}
      {currentPath?.path.map((node) => (
        <article key={`${node.object_id}:${node.class_name}`} style={cardStyle}>
          <div>{node.is_root ? "Root node" : "Path node"}</div>
          <div>{node.class_name}</div>
          <div>Object ID: {node.object_id}</div>
          <div>Via: {node.via ?? "Direct root path"}</div>
        </article>
      ))}
    </section>
  );
}

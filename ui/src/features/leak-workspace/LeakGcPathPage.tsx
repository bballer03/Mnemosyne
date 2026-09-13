import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import {
  findAllLeakGcPaths,
  findLeakGcPath,
  isFindAllGcPathsAvailable,
  type AllGcPathsResult,
  type LiveDetailResult,
} from "./live-detail-client";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

const PATH_COUNT_OPTIONS = [1, 3, 5, 10, 20] as const;
const DEFAULT_MAX_PATHS = 5;

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

  // M14 Slice 14.B: multi-path view, additive alongside the single-path
  // state above. `findAllAvailable` gates every new piece of state/effect
  // below -- when it is false (today's default, no bridge wired up yet),
  // none of this runs and the render path below falls straight back to the
  // original single-path JSX, unchanged.
  const findAllAvailable = isFindAllGcPathsAvailable();
  const [maxPaths, setMaxPaths] = useState<number>(DEFAULT_MAX_PATHS);
  const [multiPathState, setMultiPathState] = useState<LiveDetailResult<AllGcPathsResult>>({ status: "idle" });
  const multiRequestedKeyRef = useRef<string | undefined>(undefined);

  const leak = artifact?.leaks.find((entry) => entry.id === leakId);
  const requestKey = leakId && heapPath && objectId ? `${leakId}:${heapPath}:${objectId}:${gcPathRefreshNonce}` : undefined;
  const hasRequestedCurrentPath = requestedKeyRef.current === requestKey;
  const showLoading = Boolean(requestKey) && (!hasRequestedCurrentPath || gcPath.status === "loading" || gcPath.status === "idle");
  const currentPath = hasRequestedCurrentPath && gcPath.data?.leak_id === leakId ? gcPath.data : undefined;
  const showUnavailable = Boolean(requestKey) && hasRequestedCurrentPath && gcPath.status === "unavailable";
  const showFallback = Boolean(requestKey) && hasRequestedCurrentPath && gcPath.status === "fallback";
  const showError = Boolean(requestKey) && hasRequestedCurrentPath && gcPath.status === "error";

  const multiRequestKey = findAllAvailable && leakId && heapPath && objectId
    ? `${leakId}:${heapPath}:${objectId}:${maxPaths}:${gcPathRefreshNonce}`
    : undefined;
  const hasRequestedMultiPath = multiRequestedKeyRef.current === multiRequestKey;
  const multiCurrent = hasRequestedMultiPath && multiPathState.data?.leak_id === leakId ? multiPathState.data : undefined;
  const showMultiLoading = Boolean(multiRequestKey) && (!hasRequestedMultiPath || multiPathState.status === "loading" || multiPathState.status === "idle");
  const showMultiUnavailable = Boolean(multiRequestKey) && hasRequestedMultiPath && multiPathState.status === "unavailable";
  const showMultiFallback = Boolean(multiRequestKey) && hasRequestedMultiPath && multiPathState.status === "fallback";
  const showMultiError = Boolean(multiRequestKey) && hasRequestedMultiPath && multiPathState.status === "error";

  useEffect(() => {
    if (findAllAvailable) {
      requestedKeyRef.current = undefined;
      return;
    }

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
  }, [artifact, findAllAvailable, leak, leakId, objectId, requestKey, setSubviewState]);

  useEffect(() => {
    if (!findAllAvailable || !artifact || !leakId || !leak || !objectId) {
      multiRequestedKeyRef.current = undefined;
      return;
    }

    let cancelled = false;
    multiRequestedKeyRef.current = multiRequestKey;
    setMultiPathState({ status: "loading" });

    void findAllLeakGcPaths({ leakId, heapPath: artifact.summary.heapPath, objectId, maxPaths })
      .then((result) => {
        if (!cancelled) {
          setMultiPathState(result);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setMultiPathState({
            status: "error",
            error: error instanceof Error ? error.message : "GC path request failed.",
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [artifact, findAllAvailable, leak, leakId, maxPaths, multiRequestKey, objectId]);

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

  if (findAllAvailable) {
    return (
      <section style={sectionStyle}>
        <h3 style={{ margin: 0 }}>GC Path</h3>
        <div>Current object target: {objectId}</div>
        <div>
          <label htmlFor="gc-path-max-paths">Path count</label>{" "}
          <select
            id="gc-path-max-paths"
            value={maxPaths}
            onChange={(event) => setMaxPaths(Number(event.target.value))}
          >
            {PATH_COUNT_OPTIONS.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </div>
        <button type="button" onClick={() => requestGcPathRefresh()}>
          Refresh GC path
        </button>
        {showMultiLoading ? <div>Loading GC paths...</div> : null}
        {showMultiUnavailable ? <div>GC paths unavailable: {multiPathState.error ?? "Unknown error."}</div> : null}
        {showMultiError ? <div>GC paths failed: {multiPathState.error ?? "Unknown error."}</div> : null}
        {showMultiFallback ? <div>GC path includes backend-reported fallback provenance.</div> : null}
        {showMultiFallback
          ? multiCurrent?.provenance?.map((marker, index) => (
              <div key={`${marker.kind}:${marker.detail ?? index}`}>{marker.detail ?? marker.kind}</div>
            ))
          : null}
        {multiCurrent?.truncated ? <div>Path enumeration was truncated by the shared path budget.</div> : null}
        {multiCurrent?.all_paths.map((path, pathIndex) => (
          <div key={`path-${pathIndex}`} style={sectionStyle}>
            <h4 style={{ margin: 0 }}>
              Path {pathIndex + 1} of {multiCurrent.all_paths.length}
            </h4>
            {path.map((node) => (
              <article key={`${pathIndex}:${node.object_id}:${node.class_name}`} style={cardStyle}>
                <div>{node.is_root ? "Root node" : "Path node"}</div>
                <div>{node.class_name}</div>
                <div>Object ID: {node.object_id}</div>
                <div>Via: {node.via ?? "Direct root path"}</div>
              </article>
            ))}
          </div>
        ))}
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

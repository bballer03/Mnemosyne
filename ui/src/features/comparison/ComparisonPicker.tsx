import { useEffect, useRef, useState } from "react";

import { parseHeapDiffArtifact, type IdentityStrategy } from "../../lib/diff-types";
import { useInvestigationStore } from "../investigation/investigation-store";
import {
  isListSnapshotsAvailable,
  runListSnapshots,
  type SnapshotManifest,
} from "../workflow-landing/workflow-bridge-client";

import { isDiffObjectsAvailable, runDiffObjects } from "./comparison-bridge-client";
import { useComparisonStore } from "./comparison-store";

const inputStyle = {
  width: "100%",
  borderRadius: 12,
  border: "1px solid #334155",
  background: "rgba(2, 6, 23, 0.82)",
  color: "#e2e8f0",
  padding: "0.65rem 0.8rem",
} as const;

const buttonStyle = {
  border: "1px solid #38bdf8",
  borderRadius: 999,
  background: "#082f49",
  color: "#e0f2fe",
  padding: "0.65rem 1rem",
  cursor: "pointer",
} as const;

const visuallyHiddenInputStyle = {
  position: "absolute",
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: "hidden",
  clip: "rect(0, 0, 0, 0)",
  whiteSpace: "nowrap",
  border: 0,
} as const;

type SnapshotListState =
  | { status: "idle" | "loading" | "unavailable" }
  | { status: "ready"; snapshots: SnapshotManifest[] }
  | { status: "error"; message: string };

export function ComparisonPicker() {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [isDragActive, setIsDragActive] = useState(false);
  const [isRunningLiveDiff, setIsRunningLiveDiff] = useState(false);
  const [snapshotList, setSnapshotList] = useState<SnapshotListState>({ status: "idle" });
  const persistenceIdentity = useInvestigationStore((state) => state.persistenceIdentity);
  const {
    sourceLabel,
    loadStatus,
    loadError,
    liveBeforeKey,
    liveAfterKey,
    identityStrategy,
    topN,
    crossReferenceLeaks,
    setDiffReport,
    setLoadStatus,
    setLiveBeforeKey,
    setLiveAfterKey,
    setIdentityStrategy,
    setTopN,
    setCrossReferenceLeaks,
  } = useComparisonStore();
  const [topNInput, setTopNInput] = useState(String(topN));

  const liveDiffAvailable = isDiffObjectsAvailable();
  const snapshotListAvailable = isListSnapshotsAvailable();

  useEffect(() => {
    if (!liveDiffAvailable || !snapshotListAvailable) {
      setSnapshotList({ status: "unavailable" });
      return;
    }

    let active = true;
    setSnapshotList({ status: "loading" });
    void runListSnapshots().then((result) => {
      if (!active) {
        return;
      }
      if (result.status === "ready") {
        setSnapshotList({ status: "ready", snapshots: result.data });
      } else if (result.status === "error") {
        setSnapshotList({ status: "error", message: result.error });
      } else {
        setSnapshotList({ status: "unavailable" });
      }
    });

    return () => {
      active = false;
    };
  }, [liveDiffAvailable, snapshotListAvailable]);

  useEffect(() => {
    if (persistenceIdentity?.kind === "snapshot") {
      setLiveAfterKey(persistenceIdentity.key);
    }
  }, [persistenceIdentity, setLiveAfterKey]);

  async function loadFile(file: File | undefined) {
    if (!file) {
      return;
    }

    setLoadStatus("loading");

    try {
      const text = await file.text();
      const parsed: unknown = JSON.parse(text);
      const artifact = parseHeapDiffArtifact(parsed);

      setDiffReport(artifact.objectDiff, file.name, "file");
    } catch (error) {
      setLoadStatus("error", error instanceof Error ? error.message : "Failed to load diff report.");
    }
  }

  async function runLiveDiff() {
    setIsRunningLiveDiff(true);
    setLoadStatus("loading");

    try {
      const result = await runDiffObjects({
        beforeKey: liveBeforeKey,
        afterKey: liveAfterKey,
        strategy: identityStrategy,
        topN,
        crossReferenceLeaks,
      });

      if (result.status === "ready") {
        setDiffReport(result.data, `live: ${liveBeforeKey} -> ${liveAfterKey}`, "live");
      } else if (result.status === "unavailable") {
        setLoadStatus("unavailable", "Live diff bridge is unavailable.");
      } else {
        setLoadStatus("error", result.error);
      }
    } finally {
      setIsRunningLiveDiff(false);
    }
  }

  return (
    <section
      aria-label="Comparison picker"
      style={{
        border: "1px solid #1e293b",
        borderRadius: 20,
        background: "rgba(15, 23, 42, 0.92)",
        padding: "1.3rem",
        display: "grid",
        gap: "1.1rem",
      }}
    >
      <div>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Comparison basket</h2>
        <p style={{ margin: "0.4rem 0 0", color: "#94a3b8", lineHeight: 1.6 }}>
          Load a precomputed object-level diff report, or run a live diff when a comparison bridge is connected.
        </p>
      </div>

      <div
        onDragEnter={(event) => {
          event.preventDefault();
          setIsDragActive(true);
        }}
        onDragOver={(event) => {
          event.preventDefault();
        }}
        onDragLeave={(event) => {
          event.preventDefault();
          setIsDragActive(false);
        }}
        onDrop={(event) => {
          event.preventDefault();
          setIsDragActive(false);
          void loadFile(event.dataTransfer.files?.[0]);
        }}
        style={{
          border: `1px dashed ${isDragActive ? "#7dd3fc" : "#334155"}`,
          borderRadius: 16,
          background: isDragActive ? "rgba(14, 165, 233, 0.12)" : "rgba(2, 6, 23, 0.7)",
          padding: "1rem",
          display: "grid",
          gap: "0.6rem",
        }}
      >
        <label
          htmlFor="comparison-diff-report-input"
          style={{ fontSize: "0.95rem", fontWeight: 600, color: "#e2e8f0" }}
        >
          Diff report JSON
        </label>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Drop the output of <code>mnemosyne diff --mode object --format json</code> here, or browse your filesystem.
        </p>
        <div style={{ display: "flex", gap: "0.75rem", alignItems: "center", flexWrap: "wrap" }}>
          <button type="button" style={buttonStyle} onClick={() => inputRef.current?.click()}>
            Select diff report JSON
          </button>
          {sourceLabel ? <span style={{ color: "#64748b", fontSize: "0.88rem" }}>Loaded: {sourceLabel}</span> : null}
        </div>
        <input
          ref={inputRef}
          id="comparison-diff-report-input"
          aria-label="Diff report JSON"
          type="file"
          accept="application/json,.json"
          onChange={(event) => {
            void loadFile(event.currentTarget.files?.[0]);
            event.currentTarget.value = "";
          }}
          style={visuallyHiddenInputStyle}
        />
      </div>

      <div
        style={{
          border: "1px solid #1e293b",
          borderRadius: 16,
          background: "rgba(2, 6, 23, 0.7)",
          padding: "1rem",
          display: "grid",
          gap: "0.6rem",
        }}
      >
        <div style={{ fontSize: "0.95rem", fontWeight: 600, color: "#e2e8f0" }}>Live diff</div>

        {!liveDiffAvailable ? (
          <p role="status" style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            Live diff unavailable: no comparison bridge is connected. Load a precomputed diff report JSON above instead.
          </p>
        ) : !snapshotListAvailable || snapshotList.status === "unavailable" ? (
          <p role="status" style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            Snapshot picker unavailable: no snapshot-list bridge is connected. Load a precomputed diff report JSON above
            instead.
          </p>
        ) : snapshotList.status === "loading" || snapshotList.status === "idle" ? (
          <p role="status" style={{ margin: 0, color: "#94a3b8" }}>
            Loading snapshots...
          </p>
        ) : snapshotList.status === "error" ? (
          <p role="alert" style={{ margin: 0, color: "#fca5a5" }}>
            Failed to list snapshots: {snapshotList.message}
          </p>
        ) : snapshotList.snapshots.length === 0 ? (
          <p role="status" style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            No cached snapshots are available. Save snapshots before running a live comparison.
          </p>
        ) : (
          <>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0.75rem" }}>
              <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
                <span>Current snapshot</span>
                <select value={liveAfterKey} onChange={(event) => setLiveAfterKey(event.target.value)} style={inputStyle}>
                  <option value="">Select current snapshot</option>
                  {snapshotList.snapshots.map((snapshot) => (
                    <option key={`current-${snapshot.heapSha256}`} value={snapshot.heapSha256}>
                      {snapshot.heapPath}
                    </option>
                  ))}
                </select>
              </label>
              <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
                <span>Baseline snapshot</span>
                <select
                  value={liveBeforeKey}
                  onChange={(event) => setLiveBeforeKey(event.target.value)}
                  style={inputStyle}
                >
                  <option value="">Select baseline snapshot</option>
                  {snapshotList.snapshots.map((snapshot) => (
                    <option key={`baseline-${snapshot.heapSha256}`} value={snapshot.heapSha256}>
                      {snapshot.heapPath}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "minmax(180px, 1fr) minmax(100px, 0.4fr)",
                gap: "0.75rem",
              }}
            >
              <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
                <span>Identity strategy</span>
                <select
                  value={identityStrategy}
                  onChange={(event) => setIdentityStrategy(event.target.value as IdentityStrategy)}
                  style={inputStyle}
                >
                  <option value="ClassRetained">Class + retained size</option>
                  <option value="ClassDominator">Class + dominator chain</option>
                  <option value="FullFingerprint">Full fingerprint</option>
                </select>
              </label>
              <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
                <span>Top N</span>
                <input
                  type="number"
                  min={1}
                  max={500}
                  value={topNInput}
                  onChange={(event) => {
                    setTopNInput(event.target.value);
                    if (event.target.value !== "") {
                      setTopN(event.target.valueAsNumber);
                    }
                  }}
                  onBlur={() => setTopNInput(String(useComparisonStore.getState().topN))}
                  style={inputStyle}
                />
              </label>
            </div>
            <label
              style={{
                display: "flex",
                gap: "0.55rem",
                alignItems: "center",
                color: "#cbd5e1",
                fontSize: "0.9rem",
              }}
            >
              <input
                type="checkbox"
                checked={crossReferenceLeaks}
                onChange={(event) => setCrossReferenceLeaks(event.target.checked)}
              />
              Cross-reference leaks on the current snapshot
            </label>
            <button
              type="button"
              style={buttonStyle}
              disabled={isRunningLiveDiff || !liveBeforeKey || !liveAfterKey}
              onClick={() => void runLiveDiff()}
            >
              {isRunningLiveDiff ? "Running diff..." : "Run live diff"}
            </button>
          </>
        )}
      </div>

      {loadStatus === "error" ? (
        <p role="alert" style={{ margin: 0, color: "#fca5a5" }}>
          Failed to load diff: {loadError ?? "Unknown error."}
        </p>
      ) : null}
    </section>
  );
}

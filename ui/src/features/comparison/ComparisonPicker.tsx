import { useRef, useState } from "react";

import { parseHeapDiffArtifact } from "../../lib/diff-types";

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

export function ComparisonPicker() {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [isDragActive, setIsDragActive] = useState(false);
  const [isRunningLiveDiff, setIsRunningLiveDiff] = useState(false);
  const {
    sourceLabel,
    loadStatus,
    loadError,
    liveBeforeKey,
    liveAfterKey,
    setDiffReport,
    setLoadStatus,
    setLiveBeforeKey,
    setLiveAfterKey,
  } = useComparisonStore();

  const liveDiffAvailable = isDiffObjectsAvailable();

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
      const result = await runDiffObjects({ beforeKey: liveBeforeKey, afterKey: liveAfterKey });

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
        ) : (
          <>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0.75rem" }}>
              <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
                <span>Before snapshot key</span>
                <input
                  type="text"
                  value={liveBeforeKey}
                  onChange={(event) => setLiveBeforeKey(event.target.value)}
                  style={inputStyle}
                />
              </label>
              <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
                <span>After snapshot key</span>
                <input
                  type="text"
                  value={liveAfterKey}
                  onChange={(event) => setLiveAfterKey(event.target.value)}
                  style={inputStyle}
                />
              </label>
            </div>
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

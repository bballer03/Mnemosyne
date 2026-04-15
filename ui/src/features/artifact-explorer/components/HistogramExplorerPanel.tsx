import { useEffect, useMemo, useState } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

const searchInputStyle = {
  width: "100%",
  borderRadius: 12,
  border: "1px solid #334155",
  background: "rgba(2, 6, 23, 0.82)",
  color: "#e2e8f0",
  padding: "0.65rem 0.8rem",
} as const;

type HistogramExplorerPanelProps = {
  artifact: AnalysisArtifact;
  selectedKey?: string;
  onSelectKey: (key: string | undefined) => void;
};

export function HistogramExplorerPanel({
  artifact,
  selectedKey,
  onSelectKey,
}: HistogramExplorerPanelProps) {
  const [searchText, setSearchText] = useState("");

  const filteredEntries = useMemo(() => {
    if (!artifact.histogram) {
      return [];
    }

    const normalizedSearch = searchText.trim().toLowerCase();

    return artifact.histogram.entries
      .filter((entry) => entry.key.toLowerCase().includes(normalizedSearch))
      .slice()
      .sort((left, right) => {
        return right.retainedSize - left.retainedSize || right.shallowSize - left.shallowSize || left.key.localeCompare(right.key);
      });
  }, [artifact.histogram, searchText]);

  useEffect(() => {
    if (!artifact.histogram) {
      return;
    }

    if (filteredEntries.length === 0) {
      return;
    }

    if (!selectedKey || !filteredEntries.some((entry) => entry.key === selectedKey)) {
      onSelectKey(filteredEntries[0]?.key);
    }
  }, [artifact.histogram, filteredEntries, onSelectKey, selectedKey]);

  if (!artifact.histogram) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Histogram Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Histogram data is absent from this artifact.
        </p>
      </div>
    );
  }

  if (artifact.histogram.entries.length === 0) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Histogram Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          This artifact includes histogram metadata, but no grouped entries are available.
        </p>
      </div>
    );
  }

  const maxRetainedSize = filteredEntries[0]?.retainedSize ?? 0;

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", flexWrap: "wrap" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Histogram Explorer</h2>
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            Artifact-backed retained and shallow comparison across {artifact.histogram.groupBy} buckets.
          </p>
        </div>
        <div
          style={{
            color: "#38bdf8",
            fontSize: "0.78rem",
            letterSpacing: "0.08em",
            textTransform: "uppercase",
          }}
        >
          {artifact.histogram.groupBy}
        </div>
      </div>

      <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
        <span>Search histogram</span>
        <input
          aria-label="Search histogram"
          type="text"
          value={searchText}
          onChange={(event) => setSearchText(event.target.value)}
          placeholder="class or package"
          style={searchInputStyle}
        />
      </label>

      {filteredEntries.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          No histogram buckets match the current search.
        </p>
      ) : (
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ color: "#94a3b8", fontSize: "0.9rem" }}>Retained vs shallow</div>
          {filteredEntries.map((entry) => {
            const retainedWidth = maxRetainedSize > 0 ? Math.max((entry.retainedSize / maxRetainedSize) * 100, 8) : 0;
            const shallowWidth = maxRetainedSize > 0 ? Math.max((entry.shallowSize / maxRetainedSize) * 100, 4) : 0;
            const isSelected = entry.key === selectedKey;

            return (
              <button
                key={entry.key}
                type="button"
                aria-pressed={isSelected}
                aria-label={`Select ${entry.key}`}
                onClick={() => onSelectKey(entry.key)}
                style={{
                  display: "grid",
                  gap: "0.65rem",
                  textAlign: "left",
                  borderRadius: 16,
                  border: isSelected ? "1px solid #38bdf8" : "1px solid #1e293b",
                  background: isSelected ? "rgba(14, 116, 144, 0.18)" : "rgba(2, 6, 23, 0.75)",
                  padding: "0.9rem",
                  color: "#e2e8f0",
                  cursor: "pointer",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "start" }}>
                  <strong style={{ overflowWrap: "anywhere" }}>{entry.key}</strong>
                  <span style={{ color: "#94a3b8", whiteSpace: "nowrap" }}>
                    {entry.instanceCount.toLocaleString()} instances
                  </span>
                </div>

                <div style={{ display: "grid", gap: "0.35rem" }}>
                  <div style={{ display: "grid", gap: "0.25rem" }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
                      <span>Retained</span>
                      <span>{formatBytes(entry.retainedSize)}</span>
                    </div>
                    <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
                      <div
                        style={{
                          width: `${retainedWidth}%`,
                          height: "100%",
                          borderRadius: 999,
                          background: "linear-gradient(90deg, #38bdf8, #67e8f9)",
                        }}
                      />
                    </div>
                  </div>

                  <div style={{ display: "grid", gap: "0.25rem" }}>
                    <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
                      <span>Shallow</span>
                      <span>{formatBytes(entry.shallowSize)}</span>
                    </div>
                    <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
                      <div
                        style={{
                          width: `${shallowWidth}%`,
                          height: "100%",
                          borderRadius: 999,
                          background: "linear-gradient(90deg, #22c55e, #86efac)",
                        }}
                      />
                    </div>
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

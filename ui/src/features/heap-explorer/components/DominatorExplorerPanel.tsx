import { useMemo, useState } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

const searchInputStyle = {
  width: "100%",
  borderRadius: 12,
  border: "1px solid #334155",
  background: "rgba(2, 6, 23, 0.82)",
  color: "#e2e8f0",
  padding: "0.65rem 0.8rem",
} as const;

type DominatorExplorerPanelProps = {
  rows: AnalysisArtifact["graph"]["dominators"];
  selectedRowIndex?: number;
  onSelectRowIndex: (rowIndex: number) => void;
};

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

export function DominatorExplorerPanel({ rows, selectedRowIndex, onSelectRowIndex }: DominatorExplorerPanelProps) {
  const [searchText, setSearchText] = useState("");
  const normalizedSearch = searchText.trim().toLowerCase();

  const filteredRows = useMemo(() => {
    return rows.flatMap((row, index) => {
      if (normalizedSearch === "") {
        return [{ row, index }];
      }

      return [row.className, row.objectId, row.name].some((value) => value.toLowerCase().includes(normalizedSearch))
        ? [{ row, index }]
        : [];
    });
  }, [rows, normalizedSearch]);

  const maxRetainedSize = filteredRows.length > 0 ? Math.max(...filteredRows.map(({ row }) => row.retainedSize)) : 0;
  const maxDominates = filteredRows.length > 0 ? Math.max(...filteredRows.map(({ row }) => row.dominates)) : 0;

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.4rem)", lineHeight: 1.08 }}>Dominator Explorer</h1>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7 }}>
          Search retained heap roots by class, object id, or dominator label.
        </p>
      </div>

      <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
        <span>Search dominators</span>
        <input
          aria-label="Search dominators"
          type="text"
          value={searchText}
          onChange={(event) => setSearchText(event.target.value)}
          placeholder="class, id, or label"
          style={searchInputStyle}
        />
      </label>

      {filteredRows.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          {normalizedSearch === ""
            ? "No dominator rows are available in this artifact."
            : "No dominator rows match the current search."}
        </p>
      ) : (
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ color: "#94a3b8", fontSize: "0.9rem" }}>Retained vs dominates</div>
          {filteredRows.map(({ row, index }) => {
            const retainedWidth =
              maxRetainedSize > 0 && row.retainedSize > 0 ? Math.max((row.retainedSize / maxRetainedSize) * 100, 8) : 0;
            const dominatesWidth =
              maxDominates > 0 && row.dominates > 0 ? Math.max((row.dominates / maxDominates) * 100, 8) : 0;
            const isSelected = index === selectedRowIndex;
            const objectIdLabel = row.objectId || "artifact-only row";
            const selectionLabel = row.objectId
              ? `Select ${row.className} ${row.objectId}`
              : `Select ${row.className} artifact-only row ${row.name} row ${index + 1}`;

            return (
              <button
                key={index}
                type="button"
                aria-pressed={isSelected}
                aria-label={selectionLabel}
                onClick={() => onSelectRowIndex(index)}
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
                  <div style={{ display: "grid", gap: "0.2rem" }}>
                    <strong style={{ overflowWrap: "anywhere" }}>{row.className}</strong>
                    <span style={{ color: "#94a3b8", overflowWrap: "anywhere" }}>{row.name}</span>
                  </div>
                  <span style={{ color: "#94a3b8", whiteSpace: "nowrap" }}>{objectIdLabel}</span>
                </div>

                <div style={{ display: "grid", gap: "0.35rem" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
                    <span>Retained</span>
                    <span>{formatBytes(row.retainedSize)}</span>
                  </div>
                  <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
                    <div
                      data-testid="retained-bar"
                      style={{
                        width: `${retainedWidth}%`,
                        height: "100%",
                        borderRadius: 999,
                        background: "linear-gradient(90deg, #38bdf8, #67e8f9)",
                      }}
                    />
                  </div>
                </div>

                <div style={{ display: "grid", gap: "0.35rem" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
                    <span>Dominates</span>
                    <span>{row.dominates.toLocaleString()} rows</span>
                  </div>
                  <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
                    <div
                      data-testid="dominates-bar"
                      style={{
                        width: `${dominatesWidth}%`,
                        height: "100%",
                        borderRadius: 999,
                        background: "linear-gradient(90deg, #22c55e, #86efac)",
                      }}
                    />
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

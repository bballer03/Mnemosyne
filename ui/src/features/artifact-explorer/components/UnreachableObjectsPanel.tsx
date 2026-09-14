import { useMemo, useState } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

const INITIAL_ROWS = 25;

function formatBytes(bytes: number | undefined) {
  if (bytes === undefined) {
    return "-";
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

export function UnreachableObjectsPanel({ artifact }: { artifact: AnalysisArtifact }) {
  const report = artifact.unreachable;
  const [expanded, setExpanded] = useState(false);
  const rows = useMemo(() => report?.byClass ?? [], [report]);
  const visible = expanded ? rows : rows.slice(0, INITIAL_ROWS);

  if (!report) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Unreachable Objects</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Unreachable-object analysis is absent from this artifact.
        </p>
      </div>
    );
  }

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Unreachable Objects</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          {report.totalCount.toLocaleString()} unreachable objects ·{" "}
          {formatBytes(report.totalShallowSize)} shallow
        </p>
      </div>

      {rows.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Unreachable analysis is present but reports zero class rows.
        </p>
      ) : (
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr style={{ textAlign: "left", color: "#94a3b8" }}>
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Class</th>
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Count</th>
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Shallow</th>
              </tr>
            </thead>
            <tbody>
              {visible.map((row) => (
                <tr key={row.className}>
                  <td
                    style={{
                      padding: "0.55rem 0.6rem 0.55rem 0",
                      borderTop: "1px solid #1e293b",
                      overflowWrap: "anywhere",
                    }}
                  >
                    {row.className}
                  </td>
                  <td style={{ padding: "0.55rem 0.6rem 0.55rem 0", borderTop: "1px solid #1e293b" }}>
                    {row.count.toLocaleString()}
                  </td>
                  <td style={{ padding: "0.55rem 0.6rem 0.55rem 0", borderTop: "1px solid #1e293b" }}>
                    {formatBytes(row.shallowSize)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {rows.length > INITIAL_ROWS ? (
            <button
              type="button"
              onClick={() => setExpanded((value) => !value)}
              style={{
                marginTop: "0.75rem",
                border: "1px solid #334155",
                borderRadius: 999,
                background: "transparent",
                color: "#cbd5e1",
                padding: "0.4rem 0.8rem",
                cursor: "pointer",
              }}
            >
              {expanded ? "Show fewer" : `Show all ${rows.length} classes`}
            </button>
          ) : null}
        </div>
      )}
    </div>
  );
}

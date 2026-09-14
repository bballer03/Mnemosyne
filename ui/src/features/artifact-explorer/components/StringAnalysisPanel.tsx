import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

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

export function StringAnalysisPanel({ artifact }: { artifact: AnalysisArtifact }) {
  const report = artifact.stringReport;
  const [expanded, setExpanded] = useState(false);

  const duplicates = useMemo(() => report?.duplicateGroups ?? [], [report]);
  const visible = expanded ? duplicates : duplicates.slice(0, INITIAL_ROWS);

  if (!report) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>String Deduplication</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          String analysis is absent from this artifact. Re-analyze with <code>--strings</code> to
          populate <code>string_report</code>.
        </p>
      </div>
    );
  }

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>String Deduplication</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          {report.totalStrings.toLocaleString()} strings / {report.uniqueStrings.toLocaleString()} unique
          · duplicate waste {formatBytes(report.totalDuplicateWaste)}
        </p>
      </div>

      {duplicates.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          String analysis is present but has no duplicate groups.
        </p>
      ) : (
        <div style={{ display: "grid", gap: "0.65rem" }}>
          {visible.map((group, index) => (
            <div
              key={`${group.value}-${index}`}
              style={{
                display: "grid",
                gap: "0.25rem",
                borderRadius: 12,
                border: "1px solid #1e293b",
                background: "rgba(2, 6, 23, 0.75)",
                padding: "0.75rem",
              }}
            >
              <strong style={{ overflowWrap: "anywhere" }}>{group.value}</strong>
              <span style={{ color: "#94a3b8", fontSize: "0.88rem" }}>
                {group.count.toLocaleString()} copies · wasted {formatBytes(group.totalWastedBytes)}
              </span>
            </div>
          ))}
          {duplicates.length > INITIAL_ROWS ? (
            <button
              type="button"
              onClick={() => setExpanded((value) => !value)}
              style={{
                justifySelf: "start",
                border: "1px solid #334155",
                borderRadius: 999,
                background: "transparent",
                color: "#cbd5e1",
                padding: "0.4rem 0.8rem",
                cursor: "pointer",
              }}
            >
              {expanded ? "Show fewer" : `Show all ${duplicates.length} groups`}
            </button>
          ) : null}
        </div>
      )}

      {report.topStringsBySize.length > 0 ? (
        <div style={{ display: "grid", gap: "0.5rem" }}>
          <strong>Largest strings</strong>
          {report.topStringsBySize.slice(0, 10).map((entry) => (
            <div key={entry.objectId} style={{ color: "#cbd5e1", fontSize: "0.9rem" }}>
              <Link
                to={`/heap-explorer/object-inspector?objectId=${encodeURIComponent(`0x${entry.objectId.toString(16)}`)}`}
              >{`0x${entry.objectId.toString(16)}`}</Link>
              {" · "}
              {formatBytes(entry.byteLength)} · {entry.value.slice(0, 80)}
              {entry.value.length > 80 ? "…" : ""}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}

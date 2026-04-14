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

export function HistogramPanel({ artifact }: { artifact: AnalysisArtifact }) {
  if (!artifact.histogram) {
    return null;
  }

  const entries = artifact.histogram.entries.slice(0, 5);

  return (
    <section
      style={{
        border: "1px solid #1e293b",
        borderRadius: 20,
        background: "rgba(15, 23, 42, 0.88)",
        padding: "1.1rem 1.2rem",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", marginBottom: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Histogram Snapshot</h2>
        <span style={{ color: "#38bdf8", fontSize: "0.78rem", letterSpacing: "0.08em", textTransform: "uppercase" }}>
          {artifact.histogram.groupBy}
        </span>
      </div>
      <p style={{ margin: "0 0 0.9rem", color: "#94a3b8", lineHeight: 1.6 }}>
        Top retained groups from the current artifact snapshot.
      </p>
      {entries.length === 0 ? (
        <p style={{ margin: 0, color: "#64748b" }}>No grouped histogram entries available in this artifact.</p>
      ) : (
        <div style={{ display: "grid", gap: "0.7rem" }}>
          {entries.map((entry) => (
            <div
              key={entry.key}
              style={{
                borderRadius: 16,
                border: "1px solid #1e293b",
                background: "rgba(2, 6, 23, 0.75)",
                padding: "0.8rem 0.9rem",
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem" }}>
                <strong style={{ overflowWrap: "anywhere" }}>{entry.key}</strong>
                <span style={{ color: "#94a3b8" }}>{entry.instanceCount.toLocaleString()} instances</span>
              </div>
              <div style={{ marginTop: "0.35rem", color: "#67e8f9", fontSize: "0.92rem" }}>
                {formatBytes(entry.retainedSize)} retained
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

import type { AnalysisArtifact } from "../../../lib/analysis-types";

function formatMetricNumber(value: number) {
  return value.toLocaleString();
}

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }

  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

const cardStyle = {
  border: "1px solid #1e293b",
  borderRadius: 18,
  background: "rgba(15, 23, 42, 0.88)",
  padding: "1rem 1.1rem",
} as const;

export function SummaryStrip({ artifact }: { artifact: AnalysisArtifact }) {
  const items = [
    ["Total Objects", formatMetricNumber(artifact.summary.totalObjects)],
    ["Heap Size", formatBytes(artifact.summary.totalSizeBytes)],
    ["Leak Count", formatMetricNumber(artifact.leaks.length)],
    ["Graph Nodes", formatMetricNumber(artifact.graph.nodeCount)],
    ["Elapsed", `${artifact.elapsedSeconds.toFixed(2)}s`],
  ];

  return (
    <section
      aria-label="Summary strip"
      style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
        gap: "0.9rem",
      }}
    >
      {items.map(([label, value]) => (
        <div key={label} style={cardStyle}>
          <div
            style={{
              marginBottom: "0.45rem",
              color: "#94a3b8",
              fontSize: "0.78rem",
              textTransform: "uppercase",
              letterSpacing: "0.08em",
            }}
          >
            {label}
          </div>
          <div style={{ fontSize: "1.4rem", fontWeight: 600 }}>{value}</div>
        </div>
      ))}
    </section>
  );
}

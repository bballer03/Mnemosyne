import type { AnalysisArtifact } from "../../../lib/analysis-types";

function metricRow(label: string, value: string) {
  return (
    <div
      key={label}
      style={{
        display: "flex",
        justifyContent: "space-between",
        gap: "1rem",
        padding: "0.8rem 0",
        borderTop: "1px solid #1e293b",
      }}
    >
      <span style={{ color: "#94a3b8" }}>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function GraphMetricsPanel({ artifact }: { artifact: AnalysisArtifact }) {
  const averageDegree = artifact.graph.nodeCount === 0
    ? 0
    : artifact.graph.edgeCount / artifact.graph.nodeCount;

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
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Graph Metrics</h2>
        <span style={{ color: "#38bdf8", fontSize: "0.78rem", letterSpacing: "0.08em", textTransform: "uppercase" }}>
          Support
        </span>
      </div>
      <p style={{ margin: "0 0 0.4rem", color: "#94a3b8", lineHeight: 1.6 }}>
        Compact topology context from the loaded artifact only.
      </p>
      {metricRow("Nodes", artifact.graph.nodeCount.toLocaleString())}
      {metricRow("Edges", artifact.graph.edgeCount.toLocaleString())}
      {metricRow("Dominator Entries", artifact.graph.dominatorCount.toLocaleString())}
      {metricRow("Avg Degree", averageDegree.toFixed(2))}
    </section>
  );
}

import type { AnalysisArtifact } from "../../../lib/analysis-types";

type ObjectInspectorPanelProps = {
  artifact: AnalysisArtifact;
  selectedRowIndex?: number;
};

const fieldLabelStyle = {
  fontSize: "0.78rem",
  letterSpacing: "0.08em",
  textTransform: "uppercase",
  color: "#64748b",
} as const;

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

export function ObjectInspectorPanel({ artifact, selectedRowIndex }: ObjectInspectorPanelProps) {
  const selectedRow = selectedRowIndex !== undefined ? artifact.graph.dominators[selectedRowIndex] : undefined;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.4rem)", lineHeight: 1.08 }}>Object Inspector</h1>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7 }}>
          Live references and referrers are not yet available; this inspector currently shows artifact-backed dominator details only.
        </p>
      </div>

      {selectedRow ? (
        <dl style={{ display: "grid", gap: "0.9rem", margin: 0 }}>
          <div style={{ display: "grid", gap: "0.2rem" }}>
            <dt style={fieldLabelStyle}>Class name</dt>
            <dd style={{ margin: 0, color: "#e2e8f0", overflowWrap: "anywhere" }}>{selectedRow.className}</dd>
          </div>
          <div style={{ display: "grid", gap: "0.2rem" }}>
            <dt style={fieldLabelStyle}>Object id</dt>
            <dd style={{ margin: 0, color: "#e2e8f0", overflowWrap: "anywhere" }}>{selectedRow.objectId || "Artifact-only row"}</dd>
          </div>
          <div style={{ display: "grid", gap: "0.2rem" }}>
            <dt style={fieldLabelStyle}>Shallow size</dt>
            <dd style={{ margin: 0, color: "#e2e8f0" }}>{formatBytes(selectedRow.shallowSize)}</dd>
          </div>
          <div style={{ display: "grid", gap: "0.2rem" }}>
            <dt style={fieldLabelStyle}>Retained size</dt>
            <dd style={{ margin: 0, color: "#e2e8f0" }}>{formatBytes(selectedRow.retainedSize)}</dd>
          </div>
          <div style={{ display: "grid", gap: "0.2rem" }}>
            <dt style={fieldLabelStyle}>Dominates count</dt>
            <dd style={{ margin: 0, color: "#e2e8f0" }}>{selectedRow.dominates.toLocaleString()} objects</dd>
          </div>
          <div style={{ display: "grid", gap: "0.2rem" }}>
            <dt style={fieldLabelStyle}>Immediate dominator</dt>
            <dd style={{ margin: 0, color: "#e2e8f0", overflowWrap: "anywhere" }}>
              {selectedRow.immediateDominator ?? "Not present in the artifact"}
            </dd>
          </div>
        </dl>
      ) : (
        <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>
          Select a dominator row to inspect its artifact-backed details.
        </p>
      )}
    </section>
  );
}

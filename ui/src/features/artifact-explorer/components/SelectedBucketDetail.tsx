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

type SelectedBucketDetailProps = {
  artifact: AnalysisArtifact;
  selectedKey?: string;
};

function deriveLeakHints(artifact: AnalysisArtifact, selectedKey?: string) {
  if (!selectedKey || !artifact.histogram) {
    return [];
  }

  if (artifact.histogram.groupBy === "class") {
    return artifact.leaks.filter((leak) => leak.className === selectedKey);
  }

  if (artifact.histogram.groupBy === "package") {
    return artifact.leaks.filter((leak) => leak.className.startsWith(`${selectedKey}.`) || leak.className === selectedKey);
  }

  return [];
}

export function SelectedBucketDetail({ artifact, selectedKey }: SelectedBucketDetailProps) {
  const selectedEntry = artifact.histogram?.entries.find((entry) => entry.key === selectedKey);
  const leakHints = deriveLeakHints(artifact, selectedKey);

  return (
    <div style={{ display: "grid", gap: "0.9rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Selected Bucket</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Detail derived only from the currently selected histogram bucket and artifact leak rows.
        </p>
      </div>

      {selectedEntry ? (
        <>
          <section
            style={{
              display: "grid",
              gap: "0.5rem",
              borderRadius: 16,
              border: "1px solid #1e293b",
              background: "rgba(2, 6, 23, 0.75)",
              padding: "0.9rem",
            }}
          >
            <strong style={{ overflowWrap: "anywhere" }}>{selectedEntry.key}</strong>
            <div style={{ color: "#94a3b8", fontSize: "0.92rem" }}>Grouped by {artifact.histogram?.groupBy ?? "unknown"}</div>
            <div style={{ color: "#cbd5e1", fontSize: "0.92rem" }}>
              {selectedEntry.instanceCount.toLocaleString()} instances
            </div>
            <div style={{ color: "#cbd5e1", fontSize: "0.92rem" }}>
              {formatBytes(selectedEntry.shallowSize)} shallow
            </div>
            <div style={{ color: "#67e8f9", fontSize: "0.92rem" }}>
              {formatBytes(selectedEntry.retainedSize)} retained
            </div>
          </section>

          <section
            style={{
              display: "grid",
              gap: "0.5rem",
              borderRadius: 16,
              border: "1px solid #1e293b",
              background: "rgba(2, 6, 23, 0.75)",
              padding: "0.9rem",
            }}
          >
            <strong>Artifact-backed leak hints</strong>
            {leakHints.length > 0 ? (
              leakHints.map((leak) => (
                <div key={leak.id} style={{ display: "grid", gap: "0.2rem", color: "#cbd5e1", fontSize: "0.92rem" }}>
                  <span>{leak.className}</span>
                  <span style={{ color: "#94a3b8" }}>{leak.description}</span>
                </div>
              ))
            ) : (
              <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
                No direct leak relationship is proven by this artifact bucket.
              </p>
            )}
          </section>
        </>
      ) : (
        <section
          style={{
            display: "grid",
            gap: "0.5rem",
            borderRadius: 16,
            border: "1px solid #1e293b",
            background: "rgba(2, 6, 23, 0.75)",
            padding: "0.9rem",
          }}
        >
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            Select a histogram bucket to inspect bucket-level artifact detail.
          </p>
        </section>
      )}
    </div>
  );
}

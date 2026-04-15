import { useEffect, useState } from "react";
import { Link, Navigate } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { AnalyzerRail } from "./components/AnalyzerRail";
import { HistogramExplorerPanel } from "./components/HistogramExplorerPanel";
import { SelectedBucketDetail } from "./components/SelectedBucketDetail";

const panelStyle = {
  border: "1px solid #1e293b",
  borderRadius: 24,
  background: "linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(2, 6, 23, 0.96))",
  padding: "1.3rem",
} as const;

export function ArtifactExplorerPage() {
  const { artifact, artifactName } = useArtifactStore();
  const [selectedHistogramKey, setSelectedHistogramKey] = useState<string | undefined>(
    artifact?.histogram?.entries[0]?.key,
  );

  useEffect(() => {
    setSelectedHistogramKey(artifact?.histogram?.entries[0]?.key);
  }, [artifact]);

  if (!artifact) {
    return <Navigate to="/" replace />;
  }

  return (
    <main style={{ display: "grid", gap: "1rem" }}>
      <section style={panelStyle}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
            <Link to="/dashboard">Dashboard</Link>
            <Link to="/artifacts/explorer" aria-current="page">
              Artifact Explorer
            </Link>
          </div>
          <div style={{ color: "#38bdf8", fontSize: "0.78rem", letterSpacing: "0.16em", textTransform: "uppercase" }}>
            Artifact Explorer
          </div>
          <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.6rem)", lineHeight: 1.08 }}>
            Artifact Explorer
          </h1>
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
            Dedicated artifact-backed exploration surface for histogram breadth and analyzer modules.
          </p>
          <div style={{ color: "#cbd5e1", overflowWrap: "anywhere" }}>{artifact.summary.heapPath}</div>
          <div style={{ color: "#94a3b8", overflowWrap: "anywhere" }}>
            Artifact: {artifactName ?? "Unnamed artifact"}
          </div>
        </header>
      </section>

      <section
        style={{
          display: "grid",
          gridTemplateColumns: "280px minmax(0, 1fr) 320px",
          gap: "1rem",
          alignItems: "start",
        }}
      >
        <aside aria-label="Analyzer rail" style={panelStyle}>
          <AnalyzerRail artifact={artifact} />
        </aside>
        <section aria-label="Histogram explorer" style={panelStyle}>
          <HistogramExplorerPanel
            artifact={artifact}
            selectedKey={selectedHistogramKey}
            onSelectKey={setSelectedHistogramKey}
          />
        </section>
        <aside aria-label="Selected bucket detail" style={panelStyle}>
          <SelectedBucketDetail artifact={artifact} selectedKey={selectedHistogramKey} />
        </aside>
      </section>
    </main>
  );
}

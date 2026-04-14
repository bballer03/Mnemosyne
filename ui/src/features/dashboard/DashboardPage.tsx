import { useEffect, useState } from "react";
import { Navigate, useInRouterContext } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { GraphMetricsPanel } from "./components/GraphMetricsPanel";
import { HistogramPanel } from "./components/HistogramPanel";
import { LeakTable } from "./components/LeakTable";
import { SummaryStrip } from "./components/SummaryStrip";

function formatGeneratedAt(value?: string) {
  if (!value) {
    return "Timestamp unavailable";
  }

  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("en-US", {
    year: "numeric",
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

const panelStyle = {
  border: "1px solid #1e293b",
  borderRadius: 24,
  background: "linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(2, 6, 23, 0.96))",
  padding: "1.3rem",
} as const;

function formatProvenanceSummary(kinds: Array<{ kind: string }>) {
  if (kinds.length === 0) {
    return "Provenance: none attached";
  }

  return `Provenance: ${kinds.map((entry) => entry.kind.toLowerCase()).join(", ")}`;
}

export function DashboardPage() {
  const { artifact, artifactName } = useArtifactStore();
  const isInRouterContext = useInRouterContext();
  const [isCompactLayout, setIsCompactLayout] = useState(() =>
    typeof window !== "undefined" ? window.innerWidth < 980 : false,
  );

  useEffect(() => {
    if (typeof window === "undefined") {
      return undefined;
    }

    function handleResize() {
      setIsCompactLayout(window.innerWidth < 980);
    }

    handleResize();
    window.addEventListener("resize", handleResize);

    return () => window.removeEventListener("resize", handleResize);
  }, []);

  if (!artifact) {
    return isInRouterContext ? <Navigate to="/" replace /> : null;
  }

  return (
    <main style={{ display: "grid", gap: "1.25rem" }}>
      <section style={panelStyle}>
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "start",
            gap: "1rem",
            flexWrap: "wrap",
          }}
        >
          <div style={{ display: "grid", gap: "0.55rem" }}>
            <p
              style={{
                margin: 0,
                fontSize: "0.78rem",
                letterSpacing: "0.16em",
                textTransform: "uppercase",
                color: "#38bdf8",
              }}
            >
              JVM Engine 01
            </p>
            <h2 style={{ margin: 0, fontSize: "clamp(1.9rem, 4vw, 3rem)", lineHeight: 1.08 }}>
              Mnemosyne Triage Dashboard
            </h2>
            <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
              Browser-first triage surface powered entirely by the loaded analysis artifact.
            </p>
          </div>
          <div
            style={{
              minWidth: "min(100%, 280px)",
              display: "grid",
              gap: "0.6rem",
              borderRadius: 20,
              border: "1px solid #1e293b",
              background: "rgba(2, 6, 23, 0.75)",
              padding: "0.95rem 1rem",
            }}
          >
            <div style={{ color: "#64748b", fontSize: "0.78rem", letterSpacing: "0.08em", textTransform: "uppercase" }}>
              Loaded Artifact Context
            </div>
            <div style={{ fontSize: "1.05rem", fontWeight: 600, overflowWrap: "anywhere" }}>{artifact.summary.heapPath}</div>
            <div style={{ color: "#94a3b8", overflowWrap: "anywhere" }}>Artifact: {artifactName ?? "Unnamed artifact"}</div>
            <div style={{ color: "#94a3b8" }}>Generated: {formatGeneratedAt(artifact.summary.generatedAt)}</div>
            <div style={{ color: "#cbd5e1", fontSize: "0.9rem" }}>{formatProvenanceSummary(artifact.provenance)}</div>
            <div style={{ color: "#86efac", fontSize: "0.9rem" }}>Status: artifact loaded locally</div>
          </div>
        </div>
      </section>

      <SummaryStrip artifact={artifact} />

      <section
        style={{
          display: "grid",
          gridTemplateColumns: isCompactLayout
            ? "minmax(0, 1fr)"
            : "minmax(0, 1.7fr) minmax(280px, 0.95fr)",
          gap: "1.25rem",
          alignItems: "start",
        }}
      >
        <LeakTable artifact={artifact} />
        <aside style={{ display: "grid", gap: "1rem" }}>
          <GraphMetricsPanel artifact={artifact} />
          <HistogramPanel artifact={artifact} />
        </aside>
      </section>
    </main>
  );
}

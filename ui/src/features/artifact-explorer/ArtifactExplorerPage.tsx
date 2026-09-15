import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, Navigate } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import {
  normalizeHistogramGroupBy,
  regroupHistogram,
  type HistogramResultView,
} from "../heap-explorer/heap-explorer-query-client";
import { useInvestigationStore } from "../investigation/investigation-store";

import { AnalyzerRail } from "./components/AnalyzerRail";
import { ClassInstancesPanel } from "./components/ClassInstancesPanel";
import { ClassloaderExplorerPanel } from "./components/ClassloaderExplorerPanel";
import { CollectionAnalysisPanel } from "./components/CollectionAnalysisPanel";
import { DuplicateArrayPanel } from "./components/DuplicateArrayPanel";
import { HistogramExplorerPanel } from "./components/HistogramExplorerPanel";
import { PluginFindingsPanel } from "./components/PluginFindingsPanel";
import { ReferrerPanel } from "./components/ReferrerPanel";
import { SelectedBucketDetail } from "./components/SelectedBucketDetail";
import { StringAnalysisPanel } from "./components/StringAnalysisPanel";
import { TopInstancesPanel } from "./components/TopInstancesPanel";
import { UnreachableObjectsPanel } from "./components/UnreachableObjectsPanel";

const panelStyle = {
  border: "1px solid #1e293b",
  borderRadius: 24,
  background: "linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(2, 6, 23, 0.96))",
  padding: "1.3rem",
} as const;

export function ArtifactExplorerPage() {
  const { artifact, artifactName } = useArtifactStore();
  const selectedClassKey = useInvestigationStore((state) => state.classKey);
  const setClassKey = useInvestigationStore((state) => state.setClassKey);
  const histogramView = useInvestigationStore((state) => state.histogramView);
  const setHistogramView = useInvestigationStore((state) => state.setHistogramView);
  const [selectedHistogramKey, setSelectedHistogramKey] = useState<string | undefined>(
    artifact?.histogram?.entries[0]?.key,
  );
  const [liveHistogram, setLiveHistogram] = useState<HistogramResultView | undefined>();
  const [histogramSource, setHistogramSource] = useState<"artifact" | "live">("artifact");

  useEffect(() => {
    setSelectedHistogramKey(artifact?.histogram?.entries[0]?.key);
    setLiveHistogram(undefined);
    setHistogramSource("artifact");
  }, [artifact]);

  useEffect(() => {
    if (selectedClassKey) {
      setSelectedHistogramKey(selectedClassKey);
    }
  }, [selectedClassKey]);

  useEffect(() => {
    if (!selectedClassKey && selectedHistogramKey) {
      setClassKey(selectedHistogramKey, "histogram");
    }
  }, [selectedClassKey, selectedHistogramKey, setClassKey]);

  const handleSelectHistogramKey = useCallback(
    (key: string | undefined) => {
      setSelectedHistogramKey(key);
      setClassKey(key, "histogram");
    },
    [setClassKey],
  );

  const explorerArtifact = useMemo(() => {
    if (!artifact) {
      return undefined;
    }

    if (!liveHistogram) {
      return artifact;
    }

    return {
      ...artifact,
      histogram: liveHistogram,
    };
  }, [artifact, liveHistogram]);

  const activeHistogramGroupBy =
    normalizeHistogramGroupBy(explorerArtifact?.histogram?.groupBy) ?? histogramView.groupBy;

  const handleRegroupToClass = useCallback(async () => {
    const artifactGroupBy = normalizeHistogramGroupBy(artifact?.histogram?.groupBy);
    if (artifactGroupBy === "class") {
      setHistogramView({ groupBy: "class" });
      setLiveHistogram(undefined);
      setHistogramSource("artifact");
      return;
    }

    const result = await regroupHistogram("class");
    if (result.status === "ready") {
      setHistogramView({ groupBy: "class" });
      setLiveHistogram(result.data);
      setHistogramSource("live");
    }
  }, [artifact, setHistogramView]);

  if (!artifact || !explorerArtifact) {
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
            liveHistogram={liveHistogram}
            histogramSource={histogramSource}
            onLiveHistogramChange={(histogram, source) => {
              setLiveHistogram(histogram);
              setHistogramSource(source);
            }}
            selectedKey={selectedHistogramKey}
            onSelectKey={handleSelectHistogramKey}
            histogramView={histogramView}
            setHistogramView={setHistogramView}
          />
        </section>
        <aside aria-label="Selected bucket detail" style={panelStyle}>
          <SelectedBucketDetail artifact={explorerArtifact} selectedKey={selectedHistogramKey} />
          <div style={{ borderTop: "1px solid #1e293b", marginTop: "1rem", paddingTop: "1rem" }}>
            <ClassInstancesPanel
              groupBy={activeHistogramGroupBy}
              onRegroupToClass={handleRegroupToClass}
            />
          </div>
        </aside>
      </section>

      <section
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)",
          gap: "1rem",
          alignItems: "start",
        }}
      >
        <section aria-label="Referrer panel" style={panelStyle}>
          <ReferrerPanel artifact={artifact} />
        </section>
        <section id="classloader-explorer" aria-label="Classloader explorer panel" style={panelStyle}>
          <ClassloaderExplorerPanel artifact={artifact} />
        </section>
      </section>

      <section
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)",
          gap: "1rem",
          alignItems: "start",
        }}
      >
        <section id="string-analysis" aria-label="String analysis panel" style={panelStyle}>
          <StringAnalysisPanel artifact={artifact} />
        </section>
        <section id="duplicate-arrays" aria-label="Duplicate array panel" style={panelStyle}>
          <DuplicateArrayPanel artifact={artifact} />
        </section>
      </section>

      <section
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)",
          gap: "1rem",
          alignItems: "start",
        }}
      >
        <section id="collections" aria-label="Collection analysis panel" style={panelStyle}>
          <CollectionAnalysisPanel artifact={artifact} />
        </section>
        <section id="top-instances" aria-label="Top instances panel" style={panelStyle}>
          <TopInstancesPanel artifact={artifact} />
        </section>
      </section>

      <section id="unreachable-objects" aria-label="Unreachable objects panel" style={panelStyle}>
        <UnreachableObjectsPanel artifact={artifact} />
      </section>

      <section aria-label="Static plugin findings panel" style={panelStyle}>
        <PluginFindingsPanel artifact={artifact} />
      </section>
    </main>
  );
}

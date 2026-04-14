import { useEffect, useRef, useState } from "react";
import { useInRouterContext, useNavigate } from "react-router-dom";

import { loadAnalysisArtifactFromText } from "./load-analysis-artifact";
import { ArtifactDropzone } from "./ArtifactDropzone";
import { useArtifactStore } from "./use-artifact-store";
import { useDashboardStore } from "../dashboard/dashboard-store";

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

function formatTimestamp(date: Date) {
  return new Intl.DateTimeFormat("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function panelStyle() {
  return {
    border: "1px solid #1e293b",
    borderRadius: 20,
    background: "rgba(15, 23, 42, 0.88)",
    padding: "1.25rem",
  } as const;
}

export function ArtifactLoaderPage() {
  const {
    artifactName,
    artifact,
    loadError,
    recentLoads,
    setArtifact,
    setLoadError,
    addRecentLoad,
  } = useArtifactStore();
  const resetDashboardState = useDashboardStore((state) => state.reset);
  const [isLoading, setIsLoading] = useState(false);
  const [isCompactLayout, setIsCompactLayout] = useState(() =>
    typeof window !== "undefined" ? window.innerWidth < 980 : false,
  );
  const [statusLines, setStatusLines] = useState<string[]>([
    "[00:00:00] system initialized",
    "[00:00:00] ready for local artifact input",
    "[00:00:01] waiting for analysis json selection",
  ]);
  const latestRequestId = useRef(0);
  const [shouldNavigateToDashboard, setShouldNavigateToDashboard] = useState(false);
  const isInRouterContext = useInRouterContext();

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

  async function handleFile(file: File) {
    const requestId = ++latestRequestId.current;
    setIsLoading(true);
    setStatusLines((current) => [
      `[${formatTimestamp(new Date())}] reading ${file.name}`,
      ...current,
    ]);

    try {
      const text = await file.text();
      const parsed = loadAnalysisArtifactFromText(text);
      const loadedAt = new Date();

      if (requestId !== latestRequestId.current) {
        return;
      }

      setArtifact(file.name, parsed);
      resetDashboardState();
      setShouldNavigateToDashboard(true);
      addRecentLoad({
        fileName: file.name,
        sizeLabel: formatBytes(file.size),
        loadedAtLabel: formatTimestamp(loadedAt),
        heapPath: parsed.summary.heapPath,
      });
      setStatusLines((current) => [
        `[${formatTimestamp(loadedAt)}] artifact validated: ${file.name}`,
        `[${formatTimestamp(loadedAt)}] heap path ready: ${parsed.summary.heapPath}`,
        ...current,
      ]);
    } catch (error) {
      if (requestId !== latestRequestId.current) {
        return;
      }

      const message = error instanceof Error ? error.message : "Failed to load artifact";
      setLoadError(message);
      setStatusLines((current) => [
        `[${formatTimestamp(new Date())}] validation error: ${message}`,
        ...current,
      ]);
    } finally {
      if (requestId === latestRequestId.current) {
        setIsLoading(false);
      }
    }
  }

  const previewItems = [
    {
      title: "Summary",
      description: artifact
        ? `${artifact.summary.totalObjects.toLocaleString()} objects across ${artifact.summary.totalRecords.toLocaleString()} records.`
        : "High-level heap health metrics and snapshot summary.",
    },
    {
      title: "Leak Triage",
      description: artifact
        ? `${artifact.leaks.length.toLocaleString()} leak suspects prepared for review.`
        : "Automated detection of suspicious retained-object patterns.",
    },
    {
      title: "Graph Metrics",
      description: artifact
        ? `${artifact.graph.nodeCount.toLocaleString()} nodes and ${artifact.graph.edgeCount.toLocaleString()} edges available.`
        : "Node connectivity, dominators, and graph health indicators.",
    },
    {
      title: "Histogram",
      description: artifact?.histogram
        ? `${artifact.histogram.totalInstances.toLocaleString()} grouped instances by ${artifact.histogram.groupBy}.`
        : "Object count distribution by class and allocation grouping.",
    },
  ];

  return (
    <main
      style={{
        display: "grid",
        gap: "1.5rem",
      }}
    >
      <section
        style={{
          display: "grid",
          gap: "0.75rem",
        }}
      >
        <p
          style={{
            margin: 0,
            fontSize: "0.78rem",
            letterSpacing: "0.16em",
            textTransform: "uppercase",
            color: "#38bdf8",
          }}
        >
          Artifact Loader
        </p>
        <h2
          style={{
            margin: 0,
            fontSize: "clamp(1.9rem, 4vw, 3rem)",
            lineHeight: 1.1,
          }}
        >
          Load analysis artifact
        </h2>
        <p
          style={{
            margin: 0,
            maxWidth: "64ch",
            color: "#94a3b8",
            lineHeight: 1.7,
          }}
        >
          Choose an analysis artifact to begin. Load a Mnemosyne analysis JSON derived from
          AnalyzeResponse to inspect summary metrics, leak triage hints, graph counts, and
          histogram coverage entirely in the browser.
        </p>
      </section>

      <section
        style={{
          display: "grid",
          gridTemplateColumns: isCompactLayout
            ? "minmax(0, 1fr)"
            : "minmax(0, 1.65fr) minmax(280px, 0.95fr)",
          gap: "1.5rem",
          alignItems: "start",
        }}
      >
        <div style={{ display: "grid", gap: "1.5rem" }}>
          <ArtifactDropzone onFileSelected={handleFile} />

          <section style={panelStyle()}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                gap: "1rem",
                alignItems: "center",
                flexWrap: "wrap",
                marginBottom: "0.75rem",
              }}
            >
              <h3 style={{ margin: 0 }}>Validation Console</h3>
              <span
                role="status"
                style={{
                  color: loadError ? "#fca5a5" : artifactName ? "#86efac" : "#facc15",
                  fontSize: "0.9rem",
                }}
              >
                {isLoading
                  ? "Reading local artifact..."
                  : loadError
                    ? "Validation error"
                    : artifactName
                      ? "Artifact loaded"
                      : "Ready for input"}
              </span>
            </div>

            <div
              style={{
                borderRadius: 14,
                background: "#020617",
                border: "1px solid #0f172a",
                padding: "1rem",
                fontFamily: '"IBM Plex Mono", "SFMono-Regular", Consolas, monospace',
                fontSize: "0.88rem",
                lineHeight: 1.7,
                color: loadError ? "#fca5a5" : "#cbd5e1",
              }}
            >
              {statusLines.slice(0, 5).map((line) => (
                <div key={line}>{line}</div>
              ))}
            </div>

            {artifact ? (
              <div
                style={{
                  display: "grid",
                  gap: "0.35rem",
                  marginTop: "0.9rem",
                  color: "#cbd5e1",
                }}
              >
                <div>Artifact loaded: {artifactName}</div>
                <div>Heap path: {artifact.summary.heapPath}</div>
              </div>
            ) : null}

            {loadError ? (
              <p role="alert" style={{ marginBottom: 0, color: "#fca5a5" }}>
                {loadError}
              </p>
            ) : null}
          </section>

          <section style={panelStyle()}>
            <h3 style={{ marginTop: 0 }}>Recent Loads</h3>
            <p style={{ marginTop: 0, color: "#94a3b8" }}>
              Local metadata only for this browser session.
            </p>
            {recentLoads.length === 0 ? (
              <p style={{ marginBottom: 0, color: "#64748b" }}>No local artifacts loaded yet.</p>
            ) : (
              <div style={{ overflowX: "auto" }}>
                <table style={{ width: "100%", borderCollapse: "collapse" }}>
                  <thead>
                    <tr style={{ textAlign: "left", color: "#94a3b8" }}>
                      <th style={{ padding: "0 0 0.6rem" }}>Filename</th>
                      <th style={{ padding: "0 0 0.6rem" }}>Size</th>
                      <th style={{ padding: "0 0 0.6rem" }}>Timestamp</th>
                    </tr>
                  </thead>
                  <tbody>
                    {recentLoads.map((entry) => (
                      <tr key={`${entry.fileName}-${entry.loadedAtLabel}`}>
                        <td style={{ padding: "0.65rem 0", borderTop: "1px solid #1e293b" }}>
                          <div>{entry.fileName}</div>
                          <div style={{ color: "#64748b", fontSize: "0.85rem" }}>{entry.heapPath}</div>
                        </td>
                        <td style={{ padding: "0.65rem 0", borderTop: "1px solid #1e293b" }}>
                          {entry.sizeLabel}
                        </td>
                        <td style={{ padding: "0.65rem 0", borderTop: "1px solid #1e293b" }}>
                          {entry.loadedAtLabel}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        </div>

        <aside style={panelStyle()}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              gap: "1rem",
              alignItems: "center",
              marginBottom: "1rem",
            }}
          >
            <h3 style={{ margin: 0 }}>Dashboard Preview</h3>
            <span style={{ color: "#64748b", fontSize: "0.9rem" }}>
              {artifactName ? "Local artifact ready" : "Awaiting local file"}
            </span>
          </div>

          <div style={{ display: "grid", gap: "0.85rem" }}>
            {previewItems.map((item) => (
              <section
                key={item.title}
                style={{
                  borderRadius: 16,
                  border: "1px solid #1e293b",
                  background: "rgba(2, 6, 23, 0.75)",
                  padding: "0.9rem 1rem",
                }}
              >
                <h4 style={{ margin: "0 0 0.45rem", fontSize: "1rem" }}>{item.title}</h4>
                <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>{item.description}</p>
              </section>
            ))}
          </div>
        </aside>
      </section>

      {isInRouterContext ? (
        <NavigateToDashboardOnSuccess active={shouldNavigateToDashboard} onNavigated={() => setShouldNavigateToDashboard(false)} />
      ) : null}
    </main>
  );
}

function NavigateToDashboardOnSuccess({
  active,
  onNavigated,
}: {
  active: boolean;
  onNavigated: () => void;
}) {
  const navigate = useNavigate();

  useEffect(() => {
    if (!active) {
      return;
    }

    navigate("/dashboard");
    onNavigated();
  }, [active, navigate, onNavigated]);

  return null;
}

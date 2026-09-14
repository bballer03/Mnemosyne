import { useEffect, useRef, useState } from "react";
import { useInRouterContext, useNavigate } from "react-router-dom";

import { loadAnalysisArtifactFromText } from "./load-analysis-artifact";
import { ArtifactDropzone } from "./ArtifactDropzone";
import { getDesktopLogPath } from "./desktop-heap-client";
import { formatHostError } from "../../host/format-host-error";
import { useArtifactStore } from "./use-artifact-store";
import { useDashboardStore } from "../dashboard/dashboard-store";
import { GuidedLanding } from "../workflow-landing/GuidedLanding";
import { TopNav } from "../../app/TopNav";
import {
  applyOpenedHeap,
  openDesktopHeapLean,
} from "../investigation/workspace-actions";

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

// M14 Slice 14.D landing-page-restructure decision (design doc §5's "Files
// owned" note leaves this to implementation): this page keeps ALL of its
// existing markup, copy, and post-load behavior completely unchanged --
// including the auto-navigate-to-`/dashboard` effect below
// (`NavigateToDashboardOnSuccess`), which several regression tests in
// `ArtifactLoaderPage.test.tsx` assert on directly (e.g. the "mnemosyne
// triage dashboard" heading appearing after upload). Rather than replacing
// this page's content with the guided landing (design doc §5's diagram
// shows both living at `/`), the guided-workflow content
// (`GuidedLanding` -- triage summary card, NL input bar, workflow cards,
// "Recent heaps") is appended as a new section below the existing
// dropzone/validation-console/recent-loads layout. In practice this section
// is most visible before any artifact is loaded (an always-present
// accelerator hub) and again if the user navigates back to `/` with an
// artifact already in `useArtifactStore` (the auto-navigate effect only
// fires once, right after a *new* successful load) -- both are legitimate
// uses of "the landing route", and neither requires touching the
// drop-flow's own tested behavior. This was judged safer than an in-place
// rewrite per this slice's explicit instruction to prioritize not breaking
// the existing drop-flow tests over any particular implementation shape.
//
// `TopNav` (persistent top-level navigation to every power route) is
// rendered here, at the top of the landing page, rather than as a shared
// layout route wrapping every entry in `router.tsx`'s `routes` array --
// that was tried first and reverted (see `router.tsx`'s own comment): it
// collided with several existing pages' own in-page cross-navigation links
// that already share the same accessible names ("Dashboard", "Artifact
// Explorer", etc.), breaking a number of *pre-existing* tests that query
// those pages' own nav by role/name. Rendering `TopNav` only on `/` still
// satisfies this slice's concrete requirement -- every power route
// reachable from the landing page without any workflow step first -- while
// touching zero other routes. Guarded by `isInRouterContext` exactly like
// `NavigateToDashboardOnSuccess` below, since `<NavLink>` requires a
// `<Router>` ancestor and this page is rendered without one in several of
// its own existing tests.
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
  const [heapOpenPhase, setHeapOpenPhase] = useState<"idle" | "picking" | "analyzing">("idle");
  const [desktopHeapMessage, setDesktopHeapMessage] = useState<string | undefined>();
  const [desktopLogPath, setDesktopLogPath] = useState<string | undefined>();
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

  useEffect(() => {
    let cancelled = false;
    void getDesktopLogPath()
      .then((path) => {
        if (!cancelled && path) {
          setDesktopLogPath(path);
          setStatusLines((current) => [
            `[${formatTimestamp(new Date())}] desktop log: ${path}`,
            ...current,
          ]);
        }
      })
      .catch(() => {
        // Browser / missing bridge — leave unset.
      });
    return () => {
      cancelled = true;
    };
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

      const message = formatHostError(error, "Failed to load artifact");
      console.error("[mnemosyne] artifact load failed", error);
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

  async function handleOpenHeapDump() {
    setDesktopHeapMessage(undefined);
    setStatusLines((current) => [
      `[${formatTimestamp(new Date())}] opening heap dump picker`,
      ...current,
    ]);

    const result = await openDesktopHeapLean(setHeapOpenPhase);
    if (result.status === "cancelled") {
      setDesktopHeapMessage("Heap dump selection cancelled.");
      setStatusLines((current) => [
        `[${formatTimestamp(new Date())}] heap dump selection cancelled`,
        ...current,
      ]);
      return;
    }

    if (result.status === "unavailable" || result.status === "error") {
      setDesktopHeapMessage(result.message);
      setStatusLines((current) => [
        `[${formatTimestamp(new Date())}] heap open: ${result.message}`,
        ...current,
      ]);
      return;
    }

    const loadedAt = new Date();
    applyOpenedHeap(result.displayName, result.artifact);
    setShouldNavigateToDashboard(true);
    setDesktopHeapMessage(
      `Analyzed ${result.displayName}: ${result.artifact.summary.totalObjects.toLocaleString()} objects in artifact view.`,
    );
    setStatusLines((current) => [
      `[${formatTimestamp(loadedAt)}] desktop analysis ready: ${result.displayName}`,
      ...current,
    ]);
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
    <>
      {isInRouterContext ? <TopNav /> : null}
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
          Choose an analysis artifact to begin, or open a heap dump in the desktop app.
          Browser mode keeps JSON artifact import; desktop mode can pick `.hprof` / `.bin`
          without exposing the absolute path to the UI.
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
          <section style={panelStyle()}>
            <div style={{ display: "grid", gap: "0.75rem" }}>
              <h3 style={{ margin: 0 }}>Open heap dump</h3>
              <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
                Desktop first-run path: select a local `.hprof` or `.bin` file. The absolute path
                stays in the native session; React only sees the filename and an opaque source id.
              </p>
              <button
                type="button"
                onClick={() => {
                  void handleOpenHeapDump();
                }}
                disabled={heapOpenPhase !== "idle" || isLoading}
                style={{
                  justifySelf: "start",
                  border: "1px solid #38bdf8",
                  borderRadius: 999,
                  background: "rgba(56, 189, 248, 0.12)",
                  color: "#e0f2fe",
                  padding: "0.55rem 1rem",
                  cursor: heapOpenPhase !== "idle" || isLoading ? "wait" : "pointer",
                }}
              >
                {heapOpenPhase === "picking"
                  ? "Opening…"
                  : heapOpenPhase === "analyzing"
                    ? "Analyzing…"
                    : "Open heap dump"}
              </button>
              {desktopHeapMessage ? (
                <p role="status" style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.6 }}>
                  {desktopHeapMessage}
                </p>
              ) : null}
            </div>
          </section>

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
            <p style={{ margin: "0.55rem 0 0", color: "#64748b", fontSize: "0.82rem" }}>
              Recent host errors also appear here. Desktop: DevTools (F12) for{" "}
              <code>[mnemosyne]</code>
              {desktopLogPath ? (
                <>
                  ; host file log: <code>{desktopLogPath}</code>
                </>
              ) : (
                <>
                  {" "}
                  (host file log under LocalAppData/mnemosyne/logs when running in the desktop app)
                </>
              )}
              .
            </p>

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

      <GuidedLanding heapPath={artifact?.summary.heapPath} />

      {isInRouterContext ? (
        <NavigateToDashboardOnSuccess active={shouldNavigateToDashboard} onNavigated={() => setShouldNavigateToDashboard(false)} />
      ) : null}
      </main>
    </>
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

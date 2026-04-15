import { useEffect, useState } from "react";
import { Link, Navigate, NavLink, Outlet, useLocation } from "react-router-dom";

import type { AnalysisArtifact } from "../../lib/analysis-types";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { ModeRail } from "./components/ModeRail";
import { ObjectInspectorPanel } from "./components/ObjectInspectorPanel";

const panelStyle = {
  border: "1px solid #1e293b",
  borderRadius: 24,
  background: "linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(2, 6, 23, 0.96))",
  padding: "1.3rem",
} as const;

export type HeapExplorerOutletContext = {
  artifact: AnalysisArtifact;
  selectedObject?: {
    objectId: string;
    className: string;
    name: string;
  };
  selectedRowIndex?: number;
  setSelectedRowIndex: (rowIndex: number | undefined) => void;
};

export function HeapExplorerLayout() {
  const { artifact, artifactName } = useArtifactStore();
  const location = useLocation();
  const [selectedRowIndex, setSelectedRowIndex] = useState<number | undefined>(artifact?.graph.dominators[0] ? 0 : undefined);
  const [seededSearch, setSeededSearch] = useState<string | undefined>();
  const [hasUnmatchedSeededObject, setHasUnmatchedSeededObject] = useState(false);
  const [isCompactLayout, setIsCompactLayout] = useState(() =>
    typeof window !== "undefined" ? window.innerWidth < 980 : false,
  );

  function handleSelectedRowIndexChange(rowIndex: number | undefined) {
    setHasUnmatchedSeededObject(false);
    setSelectedRowIndex(rowIndex);
  }

  useEffect(() => {
    setSelectedRowIndex(artifact?.graph.dominators[0] ? 0 : undefined);
    setSeededSearch(undefined);
    setHasUnmatchedSeededObject(false);
  }, [artifact]);

  useEffect(() => {
    if (!artifact) {
      return;
    }

    if (seededSearch === location.search) {
      return;
    }

    const objectId = new URLSearchParams(location.search).get("objectId");
    if (!objectId) {
      setHasUnmatchedSeededObject(false);
      setSeededSearch(location.search);
      return;
    }

    const matchingRowIndex = artifact.graph.dominators.findIndex((row) => row.objectId === objectId);
    if (matchingRowIndex >= 0) {
      setSelectedRowIndex(matchingRowIndex);
      setHasUnmatchedSeededObject(false);
    } else {
      setSelectedRowIndex(undefined);
      setHasUnmatchedSeededObject(true);
    }

    setSeededSearch(location.search);
  }, [artifact, location.search, seededSearch]);

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
    return <Navigate to="/" replace />;
  }

  const selectedObject = hasUnmatchedSeededObject
    ? undefined
    : (selectedRowIndex !== undefined ? artifact.graph.dominators[selectedRowIndex] : undefined) ?? artifact.graph.dominators[0];
  const showInspectorPane = location.pathname !== "/heap-explorer/object-inspector";

  return (
    <main style={{ display: "grid", gap: "1rem" }}>
      <section style={panelStyle}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
            <Link to="/dashboard">Dashboard</Link>
            <Link to="/artifacts/explorer">Artifact Explorer</Link>
            <NavLink to="/heap-explorer" end={false}>
              Heap Explorer
            </NavLink>
          </div>
          <div style={{ color: "#38bdf8", fontSize: "0.78rem", letterSpacing: "0.16em", textTransform: "uppercase" }}>
            Heap Explorer
          </div>
          <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.6rem)", lineHeight: 1.08 }}>Heap Explorer</h1>
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
            Heap graph shell for dominator-driven navigation and object inspection.
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
          gridTemplateColumns: isCompactLayout
            ? "minmax(0, 1fr)"
            : showInspectorPane
            ? "260px minmax(0, 1fr) 320px"
            : "260px minmax(0, 1fr)",
          gap: "1rem",
          alignItems: "start",
        }}
      >
        <aside aria-label="Mode rail" style={panelStyle}>
          <ModeRail selectedObject={selectedObject} />
        </aside>
        <section aria-label="Heap explorer workspace" style={panelStyle}>
          <Outlet
            context={{
              artifact,
              selectedObject,
              selectedRowIndex,
              setSelectedRowIndex: handleSelectedRowIndexChange,
            } satisfies HeapExplorerOutletContext}
          />
        </section>
        {showInspectorPane ? (
          <aside aria-label="Object inspector panel" style={panelStyle}>
            <ObjectInspectorPanel artifact={artifact} selectedRowIndex={selectedRowIndex} />
          </aside>
        ) : null}
      </section>
    </main>
  );
}

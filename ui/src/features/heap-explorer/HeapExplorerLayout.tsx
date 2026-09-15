import { useEffect, useRef, useState } from "react";
import { Link, Navigate, NavLink, Outlet, useLocation } from "react-router-dom";

import { InvestigationBreadcrumbs } from "../../app/InvestigationBreadcrumbs";
import type { AnalysisArtifact } from "../../lib/analysis-types";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useInvestigationStore } from "../investigation/investigation-store";

import { ModeRail } from "./components/ModeRail";
import { ObjectInspectorPanel } from "./components/ObjectInspectorPanel";
import { resolveObjectToLeak } from "./resolve-object-to-leak";

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
  resolvedLeakId?: string;
  selectedRowIndex?: number;
  setSelectedRowIndex: (rowIndex: number | undefined) => void;
};

export function HeapExplorerLayout() {
  const { artifact, artifactName } = useArtifactStore();
  const selectionRevision = useInvestigationStore((state) => state.revision);
  const selectedObjectId = useInvestigationStore((state) => state.objectId);
  const setObjectId = useInvestigationStore((state) => state.setObjectId);
  const bumpRevisionOnArtifactChange = useInvestigationStore(
    (state) => state.bumpRevisionOnArtifactChange,
  );
  const location = useLocation();
  const previousArtifactRef = useRef(artifact);
  const observedRevisionRef = useRef(selectionRevision);
  const [selectedRowIndex, setSelectedRowIndex] = useState<number | undefined>(artifact?.graph.dominators[0] ? 0 : undefined);
  const [seededSearch, setSeededSearch] = useState<string | undefined>();
  const [isCompactLayout, setIsCompactLayout] = useState(() =>
    typeof window !== "undefined" ? window.innerWidth < 980 : false,
  );

  function handleSelectedRowIndexChange(rowIndex: number | undefined) {
    setSelectedRowIndex(rowIndex);
    setObjectId(
      rowIndex === undefined ? undefined : artifact?.graph.dominators[rowIndex]?.objectId || undefined,
      "dominators",
    );
  }

  useEffect(() => {
    const artifactChanged = previousArtifactRef.current !== artifact;
    const revisionAlreadyBumped = observedRevisionRef.current !== selectionRevision;
    previousArtifactRef.current = artifact;
    observedRevisionRef.current = selectionRevision;

    if (artifactChanged && artifact && !revisionAlreadyBumped) {
      bumpRevisionOnArtifactChange();
    }
    // Only reset local row selection when the artifact identity changes — not on every
    // selection-revision bump (artifact-only rows keep index-based selection).
    if (artifactChanged) {
      setSelectedRowIndex(artifact?.graph.dominators[0] ? 0 : undefined);
      setSeededSearch(undefined);
    }
  }, [artifact, bumpRevisionOnArtifactChange, selectionRevision]);

  useEffect(() => {
    if (!artifact) {
      return;
    }

    if (seededSearch === location.search) {
      return;
    }

    const objectId = new URLSearchParams(location.search).get("objectId");
    if (!objectId) {
      setSeededSearch(location.search);
      return;
    }

    setObjectId(objectId, "inspector");
    const matchingRowIndex = artifact.graph.dominators.findIndex((row) => row.objectId === objectId);
    if (matchingRowIndex >= 0) {
      setSelectedRowIndex(matchingRowIndex);
    } else {
      // Explicit URL seed that is not in the table: clear row so unmatched seed wins.
      setSelectedRowIndex(undefined);
    }

    setSeededSearch(location.search);
  }, [artifact, location.search, seededSearch, setObjectId]);

  useEffect(() => {
    if (!artifact || !selectedObjectId) {
      return;
    }

    const matchingRowIndex = artifact.graph.dominators.findIndex(
      (row) => row.objectId === selectedObjectId,
    );
    // Only sync row index when the shared objectId maps to a dominator row.
    // Artifact-only rows (empty objectId) must keep index-based selection.
    if (matchingRowIndex >= 0) {
      setSelectedRowIndex(matchingRowIndex);
    }
  }, [artifact, selectedObjectId]);

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

  const storedSelectedObject = selectedObjectId
    ? artifact.graph.dominators.find((row) => row.objectId === selectedObjectId)
    : undefined;
  const rowSelectedObject =
    selectedRowIndex !== undefined ? artifact.graph.dominators[selectedRowIndex] : undefined;
  // Prefer a concrete dominator row over an unmatched shared id so stale store
  // objectIds (or cross-pane seeds) cannot steal leak/cross-nav identity.
  const unmatchedSelectedObject =
    selectedObjectId && !storedSelectedObject && !rowSelectedObject
      ? {
          objectId: selectedObjectId,
          className: "Object not present in dominator artifact",
          name: "Shared object selection",
        }
      : undefined;
  const selectedObject =
    storedSelectedObject ??
    rowSelectedObject ??
    unmatchedSelectedObject ??
    artifact.graph.dominators[0];
  const resolvedLeakId = resolveObjectToLeak(selectedObject?.objectId, artifact);
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
          <InvestigationBreadcrumbs />
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
              resolvedLeakId,
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

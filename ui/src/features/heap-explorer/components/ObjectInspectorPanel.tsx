import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import type { AnalysisArtifact } from "../../../lib/analysis-types";
import {
  getObjectReferences,
  getObjectReferrers,
  isReferencesAvailable,
  isReferrersAvailable,
  type ObjectReferenceEntry,
  type ObjectReferencesResult,
  type ObjectReferrersResult,
} from "../heap-explorer-query-client";

type ObjectInspectorPanelProps = Readonly<{
  artifact: AnalysisArtifact;
  selectedRowIndex?: number;
}>;

type LiveLookupState<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "unavailable" }
  | { status: "ready"; data: T }
  | { status: "error"; error: string };

const fieldLabelStyle = {
  fontSize: "0.78rem",
  letterSpacing: "0.08em",
  textTransform: "uppercase",
  color: "#64748b",
} as const;

const liveReferenceListStyle = {
  display: "grid",
  gap: "0.65rem",
  listStyle: "none",
  padding: 0,
  margin: 0,
} as const;

const liveReferenceLinkStyle = {
  display: "grid",
  gap: "0.2rem",
  padding: "0.85rem 0.95rem",
  borderRadius: 16,
  border: "1px solid rgba(148, 163, 184, 0.22)",
  background: "rgba(15, 23, 42, 0.7)",
  color: "#e2e8f0",
  textDecoration: "none",
} as const;

const liveReferenceMetaStyle = {
  margin: 0,
  color: "#94a3b8",
  fontSize: "0.92rem",
  overflowWrap: "anywhere",
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

function renderReferenceList(entries: ObjectReferenceEntry[]) {
  return (
    <ul style={liveReferenceListStyle}>
      {entries.map((entry) => (
        <li key={`${entry.objectId}:${entry.className}`}>
          <Link to={`/heap-explorer/object-inspector?objectId=${encodeURIComponent(entry.objectId)}`} style={liveReferenceLinkStyle}>
            <strong style={{ overflowWrap: "anywhere" }}>{entry.className}</strong>
            {entry.displayName ? <p style={liveReferenceMetaStyle}>{entry.displayName}</p> : null}
            <p style={liveReferenceMetaStyle}>{entry.objectId}</p>
            <p style={{ ...liveReferenceMetaStyle, color: "#cbd5e1" }}>{formatBytes(entry.shallowSize)}</p>
          </Link>
        </li>
      ))}
    </ul>
  );
}

export function ObjectInspectorPanel({ artifact, selectedRowIndex }: ObjectInspectorPanelProps) {
  const selectedRow = selectedRowIndex === undefined ? undefined : artifact.graph.dominators[selectedRowIndex];
  const selectedObjectId = selectedRow?.objectId ? selectedRow.objectId : undefined;
  const referencesAvailable = isReferencesAvailable();
  const referrersAvailable = isReferrersAvailable();
  const [referencesState, setReferencesState] = useState<LiveLookupState<ObjectReferencesResult>>({ status: "idle" });
  const [referrersState, setReferrersState] = useState<LiveLookupState<ObjectReferrersResult>>({ status: "idle" });

  useEffect(() => {
    let cancelled = false;

    if (!selectedObjectId) {
      setReferencesState({ status: "idle" });
      setReferrersState({ status: "idle" });
      return () => {
        cancelled = true;
      };
    }

    setReferencesState(referencesAvailable ? { status: "loading" } : { status: "unavailable" });
    setReferrersState(referrersAvailable ? { status: "loading" } : { status: "unavailable" });

    async function loadLiveRelations() {
      const [referencesResult, referrersResult] = await Promise.all([
        getObjectReferences(selectedObjectId),
        getObjectReferrers(selectedObjectId),
      ]);

      if (cancelled) {
        return;
      }

      setReferencesState(referencesResult);
      setReferrersState(referrersResult);
    }

    void loadLiveRelations();

    return () => {
      cancelled = true;
    };
  }, [referrersAvailable, referencesAvailable, selectedObjectId]);

  const liveBridgeUnavailable = !referencesAvailable && !referrersAvailable;

  function renderReferenceSection(
    title: string,
    available: boolean,
    state: LiveLookupState<ObjectReferencesResult | ObjectReferrersResult>,
    entries: ObjectReferenceEntry[],
    unavailableMessage: string,
    emptyMessage: string,
    loadingMessage: string,
  ) {
    let content;

    if (!available || state.status === "unavailable") {
      content = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>{unavailableMessage}</p>;
    } else if (!selectedObjectId) {
      content = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>Live object navigation requires an object id in the artifact.</p>;
    } else if (state.status === "idle" || state.status === "loading") {
      content = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>{loadingMessage}</p>;
    } else if (state.status === "error") {
      content = <p style={{ margin: 0, color: "#fda4af", lineHeight: 1.7 }}>{state.error}</p>;
    } else if (entries.length === 0) {
      content = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>{emptyMessage}</p>;
    } else {
      content = renderReferenceList(entries);
    }

    return (
      <section style={{ display: "grid", gap: "0.6rem" }}>
        <h2 style={{ margin: 0, fontSize: "1rem", color: "#e2e8f0" }}>{title}</h2>
        {content}
      </section>
    );
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.4rem)", lineHeight: 1.08 }}>Object Inspector</h1>
        {liveBridgeUnavailable ? (
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7 }}>
            Live references and referrers require a host bridge connection.
          </p>
        ) : null}
      </div>

      {selectedRow ? (
        <div style={{ display: "grid", gap: "1.25rem" }}>
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

          {renderReferenceSection(
            "References (outgoing)",
            referencesAvailable,
            referencesState,
            referencesState.status === "ready" ? referencesState.data.references : [],
            "Live references are not available — no host bridge connected.",
            "No outgoing references.",
            "Loading outgoing references...",
          )}

          {renderReferenceSection(
            "Referrers (incoming)",
            referrersAvailable,
            referrersState,
            referrersState.status === "ready" ? referrersState.data.referrers : [],
            "Live referrers are not available — no host bridge connected.",
            "No incoming referrers.",
            "Loading incoming referrers...",
          )}
        </div>
      ) : (
        <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>
          Select a dominator row to inspect its artifact-backed details.
        </p>
      )}
    </section>
  );
}

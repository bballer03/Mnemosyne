import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import type { AnalysisArtifact } from "../../../lib/analysis-types";
import {
  getObjectReferences,
  getObjectReferrers,
  inspectObject,
  isInspectObjectAvailable,
  isReferencesAvailable,
  isReferrersAvailable,
  type ObjectInspection,
  type ObjectInspectionRef,
  type ObjectReferenceEntry,
  type ObjectReferencesResult,
  type ObjectReferrersResult,
} from "../heap-explorer-query-client";

type ObjectInspectorPanelProps = Readonly<{
  artifact: AnalysisArtifact;
  objectId?: string;
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

/// M14 Slice 14.B: dominator parent/children navigation chip, built from the
/// leaner `inspectObject` `ObjectInspectionRef` shape (`{ objectId,
/// className }`, no `shallowSize`/`displayName`) -- same
/// `/heap-explorer/object-inspector?objectId=...` navigation target as
/// `renderReferenceList` above, just a smaller card since there is less
/// data to show.
function renderInspectionRefChip(entry: ObjectInspectionRef) {
  return (
    <Link
      key={`${entry.objectId}:${entry.className}`}
      to={`/heap-explorer/object-inspector?objectId=${encodeURIComponent(entry.objectId)}`}
      style={liveReferenceLinkStyle}
    >
      <strong style={{ overflowWrap: "anywhere" }}>{entry.className}</strong>
      <p style={liveReferenceMetaStyle}>{entry.objectId}</p>
    </Link>
  );
}

function renderDominatorChildrenList(entries: ObjectInspectionRef[]) {
  return (
    <ul style={liveReferenceListStyle}>
      {entries.map((entry) => (
        <li key={`${entry.objectId}:${entry.className}`}>{renderInspectionRefChip(entry)}</li>
      ))}
    </ul>
  );
}

export function ObjectInspectorPanel({ artifact, objectId, selectedRowIndex }: ObjectInspectorPanelProps) {
  const indexedRow = selectedRowIndex === undefined ? undefined : artifact.graph.dominators[selectedRowIndex];
  const selectedRow = objectId
    ? artifact.graph.dominators.find((row) => row.objectId === objectId)
    : indexedRow;
  const selectedObjectId = objectId ?? (selectedRow?.objectId || undefined);
  const referencesAvailable = isReferencesAvailable();
  const referrersAvailable = isReferrersAvailable();
  const inspectObjectAvailable = isInspectObjectAvailable();
  const [referencesState, setReferencesState] = useState<LiveLookupState<ObjectReferencesResult>>({ status: "idle" });
  const [referrersState, setReferrersState] = useState<LiveLookupState<ObjectReferrersResult>>({ status: "idle" });
  const [inspectionState, setInspectionState] = useState<LiveLookupState<ObjectInspection>>({ status: "idle" });
  const [fieldInspectionState, setFieldInspectionState] = useState<LiveLookupState<ObjectInspection>>({
    status: "idle",
  });

  useEffect(() => {
    let cancelled = false;

    if (!selectedObjectId) {
      setReferencesState({ status: "idle" });
      setReferrersState({ status: "idle" });
      return () => {
        cancelled = true;
      };
    }

    const objectId: string = selectedObjectId;
    const shouldLoadLiveRelations = referencesAvailable || referrersAvailable;

    setReferencesState(referencesAvailable ? { status: "loading" } : { status: "unavailable" });
    setReferrersState(referrersAvailable ? { status: "loading" } : { status: "unavailable" });

    if (!shouldLoadLiveRelations) {
      return () => {
        cancelled = true;
      };
    }

    async function loadLiveRelations() {
      const [referencesResult, referrersResult] = await Promise.all([
        getObjectReferences(objectId),
        getObjectReferrers(objectId),
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

  // M14 Slice 14.B: independent effect for the newer `inspectObject` bridge
  // method, kept separate from the references/referrers effect above so
  // that effect's behavior (and every existing test asserting against it)
  // stays byte-identical when the bridge lacks `inspectObject` -- this
  // effect simply never fires anything user-visible in that case.
  useEffect(() => {
    let cancelled = false;

    if (!selectedObjectId) {
      setInspectionState({ status: "idle" });
      setFieldInspectionState({ status: "idle" });
      return () => {
        cancelled = true;
      };
    }

    const objectId: string = selectedObjectId;

    setFieldInspectionState({ status: "idle" });
    setInspectionState(inspectObjectAvailable ? { status: "loading" } : { status: "unavailable" });

    if (!inspectObjectAvailable) {
      return () => {
        cancelled = true;
      };
    }

    async function loadInspection() {
      const result = await inspectObject(objectId, false);

      if (cancelled) {
        return;
      }

      setInspectionState(result);
    }

    void loadInspection();

    return () => {
      cancelled = true;
    };
  }, [inspectObjectAvailable, selectedObjectId]);

  const liveBridgeUnavailable = !referencesAvailable && !referrersAvailable;
  const liveInspection = inspectionState.status === "ready" ? inspectionState.data : undefined;

  async function requestFieldData() {
    if (!selectedObjectId || !inspectObjectAvailable) {
      return;
    }

    const requestedObjectId = selectedObjectId;
    setFieldInspectionState({ status: "loading" });
    const result = await inspectObject(requestedObjectId, true);
    setFieldInspectionState(result);
  }

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

  // M14 Slice 14.B: live dominator parent/children chips, sourced from the
  // new `inspectObject` bridge method. Renders nothing at all when the
  // bridge does not support `inspectObject` -- a hard regression gate: the
  // rest of the panel must render byte-identical to pre-14.B output for
  // every host that has not yet wired the new bridge method up.
  function renderDominatorContextSection() {
    if (!inspectObjectAvailable) {
      return null;
    }

    let content;

    if (!selectedObjectId) {
      content = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>Live dominator navigation requires an object id in the artifact.</p>;
    } else if (inspectionState.status === "idle" || inspectionState.status === "loading") {
      content = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>Loading dominator context...</p>;
    } else if (inspectionState.status === "error") {
      content = <p style={{ margin: 0, color: "#fda4af", lineHeight: 1.7 }}>{inspectionState.error}</p>;
    } else if (inspectionState.status === "unavailable") {
      content = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>Live dominator context is not available.</p>;
    } else {
      const { dominatorParent, dominatorChildren } = inspectionState.data;

      content = (
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ display: "grid", gap: "0.4rem" }}>
            <h3 style={{ margin: 0, fontSize: "0.9rem", color: "#cbd5e1" }}>Parent</h3>
            {dominatorParent ? (
              <ul style={liveReferenceListStyle}>
                <li>{renderInspectionRefChip(dominatorParent)}</li>
              </ul>
            ) : (
              <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>No dominator parent (GC root or none).</p>
            )}
          </div>
          <div style={{ display: "grid", gap: "0.4rem" }}>
            <h3 style={{ margin: 0, fontSize: "0.9rem", color: "#cbd5e1" }}>Children</h3>
            {dominatorChildren.length === 0 ? (
              <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>No dominator children.</p>
            ) : (
              renderDominatorChildrenList(dominatorChildren)
            )}
          </div>
        </div>
      );
    }

    return (
      <section style={{ display: "grid", gap: "0.6rem" }}>
        <h2 style={{ margin: 0, fontSize: "1rem", color: "#e2e8f0" }}>Dominator context (live)</h2>
        {content}
      </section>
    );
  }

  function renderFieldDataSection() {
    if (!selectedObjectId) {
      return null;
    }

    let resultContent;
    if (!inspectObjectAvailable || fieldInspectionState.status === "unavailable") {
      resultContent = (
        <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>
          Field data is unavailable — the object inspection host bridge is not connected.
        </p>
      );
    } else if (fieldInspectionState.status === "loading") {
      resultContent = (
        <button type="button" disabled>
          Requesting field data...
        </button>
      );
    } else if (fieldInspectionState.status === "error") {
      resultContent = (
        <div style={{ display: "grid", gap: "0.6rem" }}>
          <p style={{ margin: 0, color: "#fda4af", lineHeight: 1.7 }}>{fieldInspectionState.error}</p>
          <button type="button" onClick={() => void requestFieldData()}>
            Retry field data request
          </button>
        </div>
      );
    } else if (fieldInspectionState.status === "ready") {
      const { fields } = fieldInspectionState.data;
      if (fields === undefined) {
        resultContent = (
          <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>
            Field bytes were unavailable after the opt-in request.
          </p>
        );
      } else if (fields.length === 0) {
        resultContent = <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>No decoded fields.</p>;
      } else {
        resultContent = (
          <dl style={{ display: "grid", gap: "0.75rem", margin: 0 }}>
            {fields.map((field, index) => (
              <div
                key={`${field.name}:${field.typeName}:${index}`}
                style={{
                  display: "grid",
                  gap: "0.25rem",
                  padding: "0.75rem",
                  borderRadius: 12,
                  border: "1px solid rgba(148, 163, 184, 0.22)",
                }}
              >
                <dt style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", color: "#e2e8f0" }}>
                  <strong>{field.name}</strong>
                  <span style={{ color: "#94a3b8" }}>{field.typeName}</span>
                </dt>
                <dd
                  style={{
                    margin: 0,
                    color: "#cbd5e1",
                    overflowWrap: "anywhere",
                    whiteSpace: "pre-wrap",
                  }}
                >
                  {field.value}
                </dd>
              </div>
            ))}
          </dl>
        );
      }
    } else {
      resultContent = (
        <button type="button" onClick={() => void requestFieldData()}>
          Request field data
        </button>
      );
    }

    return (
      <section style={{ display: "grid", gap: "0.6rem" }}>
        <h2 style={{ margin: 0, fontSize: "1rem", color: "#e2e8f0" }}>Fields</h2>
        {inspectObjectAvailable ? (
          <p style={{ margin: 0, color: "#fbbf24", lineHeight: 1.7 }}>
            May reparse the heap and retain field bytes; memory use can increase.
          </p>
        ) : null}
        {resultContent}
      </section>
    );
  }

  const displayClassName =
    selectedRow?.className ?? liveInspection?.className ?? "Loading live object details...";
  const displayObjectId = selectedObjectId ?? selectedRow?.objectId;
  const displayShallowSize = selectedRow?.shallowSize ?? liveInspection?.shallowSize;
  const displayRetainedSize = selectedRow?.retainedSize ?? liveInspection?.retainedSize;

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

      {selectedRow || selectedObjectId ? (
        <div style={{ display: "grid", gap: "1.25rem" }}>
          <dl style={{ display: "grid", gap: "0.9rem", margin: 0 }}>
            <div style={{ display: "grid", gap: "0.2rem" }}>
              <dt style={fieldLabelStyle}>Class name</dt>
              <dd style={{ margin: 0, color: "#e2e8f0", overflowWrap: "anywhere" }}>{displayClassName}</dd>
            </div>
            <div style={{ display: "grid", gap: "0.2rem" }}>
              <dt style={fieldLabelStyle}>Object id</dt>
              <dd style={{ margin: 0, color: "#e2e8f0", overflowWrap: "anywhere" }}>{displayObjectId || "Artifact-only row"}</dd>
            </div>
            <div style={{ display: "grid", gap: "0.2rem" }}>
              <dt style={fieldLabelStyle}>Shallow size</dt>
              <dd style={{ margin: 0, color: "#e2e8f0" }}>
                {displayShallowSize === undefined ? "Loading live object details..." : formatBytes(displayShallowSize)}
              </dd>
            </div>
            <div style={{ display: "grid", gap: "0.2rem" }}>
              <dt style={fieldLabelStyle}>Retained size</dt>
              <dd style={{ margin: 0, color: "#e2e8f0" }}>
                {displayRetainedSize === undefined ? "Unavailable" : formatBytes(displayRetainedSize)}
              </dd>
            </div>
            {selectedRow ? (
              <>
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
              </>
            ) : null}
          </dl>

          {renderFieldDataSection()}

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

          {renderDominatorContextSection()}
        </div>
      ) : (
        <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.7 }}>
          Select a dominator row to inspect its artifact-backed details.
        </p>
      )}
    </section>
  );
}

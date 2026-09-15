import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { ProvenanceBadge } from "../dashboard/components/ProvenanceBadge";
import {
  buildArtifactFindingFacts,
  findingHref,
} from "./finding-adapters";
import {
  useInvestigationStore,
  type FindingFact,
  type FindingTarget,
} from "./investigation-store";

const MAX_VISIBLE_FINDINGS = 8;

function syncFindingSelection(target: FindingTarget) {
  const investigation = useInvestigationStore.getState();
  investigation.clearSelection();
  if (target.kind === "leak") {
    investigation.setLeakId(target.leakId, "findings");
    if (target.classKey) {
      investigation.setClassKey(target.classKey, "findings");
    }
    if (target.objectId) {
      investigation.setObjectId(target.objectId, "findings");
    }
    return;
  }
  if (target.kind === "class") {
    investigation.setClassKey(target.classKey, "findings");
    return;
  }
  if (target.classKey) {
    investigation.setClassKey(target.classKey, "findings");
  }
  investigation.setObjectId(target.objectId, "findings");
}

function FindingRow({
  finding,
  status,
}: {
  finding: FindingFact;
  status: "open" | "resolved" | "deferred";
}) {
  const setFindingStatus = useInvestigationStore(
    (state) => state.setFindingStatus,
  );
  return (
    <li
      style={{
        border: "1px solid var(--mn-border)",
        borderRadius: 10,
        background: "var(--mn-surface-raised)",
        padding: "0.65rem 0.75rem",
        display: "grid",
        gap: "0.35rem",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: "0.75rem",
          flexWrap: "wrap",
        }}
      >
        <Link
          to={findingHref(finding.target)}
          onClick={() => syncFindingSelection(finding.target)}
          style={{ color: "var(--mn-text-primary)", fontWeight: 650 }}
        >
          {finding.title}
        </Link>
        <span style={{ color: "var(--mn-warning-text)", fontSize: "0.78rem" }}>
          {finding.severity}
        </span>
      </div>
      <span style={{ color: "var(--mn-text-muted)", fontSize: "0.84rem" }}>
        {finding.description}
      </span>
      <div
        style={{
          display: "flex",
          gap: "0.4rem",
          alignItems: "center",
          flexWrap: "wrap",
        }}
      >
        <span
          style={{
            color: "var(--mn-text-muted)",
            fontSize: "0.72rem",
            textTransform: "uppercase",
            letterSpacing: "0.06em",
          }}
        >
          {finding.source} finding · {finding.kind}
        </span>
        {finding.provenance.map((marker) => (
          <ProvenanceBadge
            key={`${finding.id}-${marker.kind}-${marker.detail ?? ""}`}
            kind={marker.kind}
          />
        ))}
      </div>
      <label
        style={{
          display: "flex",
          alignItems: "center",
          gap: "0.45rem",
          color: "var(--mn-text-muted)",
          fontSize: "0.8rem",
        }}
      >
        <span>Status</span>
        <select
          aria-label={`Status for ${finding.title}`}
          value={status}
          onChange={(event) =>
            setFindingStatus(
              finding.id,
              event.target.value as "open" | "resolved" | "deferred",
            )
          }
        >
          <option value="open">open</option>
          <option value="resolved">resolved</option>
          <option value="deferred">deferred</option>
        </select>
      </label>
    </li>
  );
}

/**
 * M24.D workbench-adjacent queue over already-loaded artifact findings.
 *
 * This pane performs no provider call and defaults to offline Rules guidance.
 * It only prioritizes existing artifact facts, preserving their provenance,
 * and sends stable identifiers into deterministic workbench routes.
 */
export function FindingsAdvisoryPane() {
  const artifact = useArtifactStore((state) => state.artifact);
  const [expanded, setExpanded] = useState(true);
  const workspaceId = useInvestigationStore((state) => state.workspaceId);
  const revision = useInvestigationStore((state) => state.revision);
  const findingFacts = useInvestigationStore((state) => state.findingFacts);
  const findingStatuses = useInvestigationStore(
    (state) => state.findingStatuses,
  );
  const replaceFindings = useInvestigationStore(
    (state) => state.replaceFindings,
  );
  const artifactFacts = useMemo(
    () => (artifact ? buildArtifactFindingFacts(artifact) : []),
    [artifact],
  );
  const findings = findingFacts.slice(0, MAX_VISIBLE_FINDINGS);

  useEffect(() => {
    replaceFindings(
      { workspaceId, revision },
      "artifact",
      artifactFacts,
    );
  }, [artifactFacts, replaceFindings, revision, workspaceId]);

  if (!artifact) {
    return null;
  }

  return (
    <section
      role="region"
      aria-label="Findings advisory"
      style={{
        border: "1px solid var(--mn-advisory-border)",
        borderRadius: 14,
        background: "var(--mn-advisory-surface)",
        color: "var(--mn-text-primary)",
        padding: "0.85rem 1rem",
        display: "grid",
        gap: "0.75rem",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "0.75rem",
          flexWrap: "wrap",
        }}
      >
        <div style={{ display: "grid", gap: "0.2rem" }}>
          <div style={{ display: "flex", gap: "0.55rem", alignItems: "center", flexWrap: "wrap" }}>
            <strong>Findings queue</strong>
            <span
              style={{
                border: "1px solid var(--mn-provenance-rules-border)",
                borderRadius: 999,
                color: "var(--mn-provenance-rules-text)",
                padding: "0.15rem 0.5rem",
                fontSize: "0.72rem",
                letterSpacing: "0.06em",
                textTransform: "uppercase",
              }}
            >
              Advisory provenance: Rules · offline
            </span>
          </div>
          <span style={{ color: "var(--mn-text-muted)", fontSize: "0.82rem" }}>
            The loaded artifact remains authoritative; this queue never replaces measured facts.
          </span>
        </div>
        <div style={{ display: "flex", gap: "0.65rem", alignItems: "center" }}>
          <Link
            to="/assistant"
            style={{ color: "var(--mn-advisory-link)", fontSize: "0.86rem" }}
          >
            Open full Assistant
          </Link>
          <button
            type="button"
            aria-expanded={expanded}
            aria-label={`${expanded ? "Collapse" : "Expand"} findings advisory`}
            onClick={() => setExpanded((value) => !value)}
            style={{
              border: "1px solid var(--mn-border)",
              borderRadius: 8,
              background: "var(--mn-surface-raised)",
              color: "var(--mn-text-primary)",
              padding: "0.3rem 0.6rem",
              cursor: "pointer",
            }}
          >
            {expanded ? "Collapse" : "Expand"}
          </button>
        </div>
      </div>

      {expanded ? (
        <div style={{ display: "grid", gap: "0.6rem" }}>
          <p style={{ margin: 0, color: "var(--mn-text-muted)", fontSize: "0.8rem" }}>
            AI/provider content appears only in Assistant and is labelled on every turn.
          </p>
          {findings.length === 0 ? (
            <div style={{ color: "var(--mn-text-muted)", fontSize: "0.88rem" }}>
              No actionable findings are present in this artifact.{" "}
              <Link to="/dashboard" style={{ color: "var(--mn-advisory-link)" }}>
                Review measured dashboard facts
              </Link>
              .
            </div>
          ) : (
            <ol
              aria-label="Actionable findings"
              style={{ margin: 0, padding: 0, listStyle: "none", display: "grid", gap: "0.55rem" }}
            >
              {findings.map((finding) => (
                <FindingRow
                  key={finding.id}
                  finding={finding}
                  status={findingStatuses[finding.id] ?? "open"}
                />
              ))}
            </ol>
          )}
        </div>
      ) : null}
    </section>
  );
}

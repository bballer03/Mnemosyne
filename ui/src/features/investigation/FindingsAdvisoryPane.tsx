import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import type { AnalysisArtifact } from "../../lib/analysis-types";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { ProvenanceBadge } from "../dashboard/components/ProvenanceBadge";
import { useInvestigationStore } from "./investigation-store";

const MAX_VISIBLE_FINDINGS = 8;

type LeakFinding = AnalysisArtifact["leaks"][number] & {
  objectId?: string;
};

function findObjectId(
  leak: AnalysisArtifact["leaks"][number],
  artifact: AnalysisArtifact,
): string | undefined {
  return artifact.graph.dominators.find(
    (entry) => entry.className === leak.className && entry.objectId.length > 0,
  )?.objectId;
}

function buildLeakFindings(artifact: AnalysisArtifact): LeakFinding[] {
  return artifact.leaks
    .map((leak) => ({
      ...leak,
      objectId: findObjectId(leak, artifact),
    }))
    .sort(
      (left, right) =>
        (right.suspectScore ?? 0) - (left.suspectScore ?? 0) ||
        right.retainedSizeBytes - left.retainedSizeBytes,
    )
    .slice(0, MAX_VISIBLE_FINDINGS);
}

function syncFindingSelection(finding: LeakFinding) {
  const investigation = useInvestigationStore.getState();
  investigation.clearSelection();
  investigation.setLeakId(finding.id, "findings");
  investigation.setClassKey(finding.className, "findings");
  if (finding.objectId) {
    investigation.setObjectId(finding.objectId, "findings");
  }
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
  const findings = useMemo(
    () => (artifact ? buildLeakFindings(artifact) : []),
    [artifact],
  );

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
              No leak findings are present in this artifact.{" "}
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
              {findings.map((finding) => {
                const primaryTarget = finding.objectId
                  ? `/heap-explorer/object-inspector?objectId=${encodeURIComponent(finding.objectId)}`
                  : `/leaks/${encodeURIComponent(finding.id)}/overview`;

                return (
                  <li
                    key={finding.id}
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
                        to={primaryTarget}
                        onClick={() => syncFindingSelection(finding)}
                        style={{ color: "var(--mn-text-primary)", fontWeight: 650 }}
                      >
                        Inspect {finding.className}
                      </Link>
                      <span style={{ color: "var(--mn-warning-text)", fontSize: "0.78rem" }}>
                        {finding.severity}
                      </span>
                    </div>
                    <span style={{ color: "var(--mn-text-muted)", fontSize: "0.84rem" }}>
                      {finding.description}
                    </span>
                    <div style={{ display: "flex", gap: "0.4rem", flexWrap: "wrap" }}>
                      <span
                        style={{
                          color: "var(--mn-text-muted)",
                          fontSize: "0.72rem",
                          textTransform: "uppercase",
                          letterSpacing: "0.06em",
                        }}
                      >
                        Artifact finding
                      </span>
                      {finding.provenance.map((marker) => (
                        <ProvenanceBadge
                          key={`${finding.id}-${marker.kind}-${marker.detail ?? ""}`}
                          kind={marker.kind}
                        />
                      ))}
                    </div>
                    <nav
                      aria-label={`Finding actions for ${finding.className}`}
                      style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap", fontSize: "0.8rem" }}
                    >
                      <Link
                        to={`/leaks/${encodeURIComponent(finding.id)}/overview`}
                        onClick={() => syncFindingSelection(finding)}
                      >
                        Open leak workspace
                      </Link>
                      <Link
                        to="/artifacts/explorer"
                        onClick={() => syncFindingSelection(finding)}
                      >
                        Show class in histogram
                      </Link>
                      {finding.objectId ? (
                        <Link
                          to={`/heap-explorer/object-inspector?objectId=${encodeURIComponent(finding.objectId)}`}
                          onClick={() => syncFindingSelection(finding)}
                        >
                          Inspect object {finding.objectId}
                        </Link>
                      ) : null}
                    </nav>
                  </li>
                );
              })}
            </ol>
          )}
        </div>
      ) : null}
    </section>
  );
}

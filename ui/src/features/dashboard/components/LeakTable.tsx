import { Fragment } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { useDashboardStore } from "../dashboard-store";
import { ProvenanceBadge } from "./ProvenanceBadge";

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

function severityTone(severity: string) {
  if (severity === "CRITICAL") {
    return { text: "#fecaca", border: "#7f1d1d", background: "rgba(127, 29, 29, 0.22)" };
  }

  if (severity === "HIGH") {
    return { text: "#fdba74", border: "#7c2d12", background: "rgba(124, 45, 18, 0.22)" };
  }

  return { text: "#cbd5e1", border: "#334155", background: "rgba(30, 41, 59, 0.7)" };
}

export function LeakTable({
  artifact,
  onTraceLeak,
}: {
  artifact: AnalysisArtifact;
  onTraceLeak?: (leakId: string) => void;
}) {
  const {
    search,
    severity,
    provenanceFilter,
    minimumRetainedBytes,
    expandedLeakIds,
    setSearch,
    setSeverity,
    setProvenanceFilter,
    setMinimumRetainedBytes,
    toggleLeakExpanded,
  } = useDashboardStore();
  const normalizedSearch = search.trim().toLowerCase();
  const severityOptions = Array.from(new Set(artifact.leaks.map((leak) => leak.severity)));
  const filteredLeaks = artifact.leaks.filter((leak) => {
    if (normalizedSearch.length > 0) {
      const searchHaystack = [leak.className, leak.id, leak.description].join(" ").toLowerCase();

      if (!searchHaystack.includes(normalizedSearch)) {
        return false;
      }
    }

    if (severity !== "all" && leak.severity !== severity) {
      return false;
    }

    if (provenanceFilter === "present" && leak.provenance.length === 0) {
      return false;
    }

    if (provenanceFilter === "none" && leak.provenance.length > 0) {
      return false;
    }

    if (
      typeof minimumRetainedBytes === "number" &&
      Number.isFinite(minimumRetainedBytes) &&
      leak.retainedSizeBytes < minimumRetainedBytes
    ) {
      return false;
    }

    return true;
  });

  return (
    <section
      style={{
        border: "1px solid #1e293b",
        borderRadius: 22,
        background: "rgba(15, 23, 42, 0.88)",
        padding: "1.2rem",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: "1rem",
          alignItems: "end",
          flexWrap: "wrap",
          marginBottom: "1rem",
        }}
      >
        <div>
          <h2 style={{ margin: 0 }}>Top Leak Suspects</h2>
          <p style={{ margin: "0.4rem 0 0", color: "#94a3b8", lineHeight: 1.6 }}>
            Dominant retained-memory candidates from the loaded artifact.
          </p>
        </div>
        <div style={{ color: "#64748b", fontSize: "0.9rem" }}>
          Displaying {filteredLeaks.length.toLocaleString()} of {artifact.leaks.length.toLocaleString()} potential leaks
        </div>
      </div>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
          gap: "0.85rem",
          marginBottom: "1rem",
          alignItems: "end",
        }}
      >
        <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
          <span>Search leaks</span>
          <input
            type="text"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="class, id, or description"
            style={{
              borderRadius: 12,
              border: "1px solid #334155",
              background: "rgba(2, 6, 23, 0.82)",
              color: "#e2e8f0",
              padding: "0.65rem 0.8rem",
            }}
          />
        </label>

        <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
          <span>Severity filter</span>
          <select
            value={severity}
            onChange={(event) => setSeverity(event.target.value)}
            style={{
              borderRadius: 12,
              border: "1px solid #334155",
              background: "rgba(2, 6, 23, 0.82)",
              color: "#e2e8f0",
              padding: "0.65rem 0.8rem",
            }}
          >
            <option value="all">all</option>
            {severityOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>

        <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
          <span>Provenance filter</span>
          <select
            value={provenanceFilter}
            onChange={(event) => setProvenanceFilter(event.target.value)}
            style={{
              borderRadius: 12,
              border: "1px solid #334155",
              background: "rgba(2, 6, 23, 0.82)",
              color: "#e2e8f0",
              padding: "0.65rem 0.8rem",
            }}
          >
            <option value="all">all</option>
            <option value="present">present</option>
            <option value="none">none</option>
          </select>
        </label>

        <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", fontSize: "0.9rem" }}>
          <span>Minimum retained bytes</span>
          <input
            type="number"
            inputMode="numeric"
            min={0}
            value={typeof minimumRetainedBytes === "number" ? minimumRetainedBytes : ""}
            onChange={(event) => {
              const nextValue = event.target.value;
              setMinimumRetainedBytes(nextValue === "" ? undefined : Number(nextValue));
            }}
            placeholder="0"
            style={{
              borderRadius: 12,
              border: "1px solid #334155",
              background: "rgba(2, 6, 23, 0.82)",
              color: "#e2e8f0",
              padding: "0.65rem 0.8rem",
            }}
          />
        </label>

      </div>

      {filteredLeaks.length === 0 ? (
        <div
          style={{
            borderRadius: 18,
            border: "1px solid #1e293b",
            background: "rgba(2, 6, 23, 0.78)",
            padding: "1rem 1.1rem",
            color: "#94a3b8",
          }}
        >
          <div style={{ color: "#e2e8f0", fontWeight: 600 }}>
            {artifact.leaks.length === 0
              ? "No leak suspects detected."
              : "No leak suspects match the current filters."}
          </div>
          <div style={{ marginTop: "0.35rem", lineHeight: 1.6 }}>
            {artifact.leaks.length === 0
              ? "Load an artifact with retained-memory findings to continue triage."
              : "Adjust or clear the current filters to restore matching rows."}
          </div>
        </div>
      ) : null}

      <div style={{ overflowX: "auto" }}>
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "#94a3b8" }}>
              <th style={{ padding: "0 0 0.75rem" }}>Sev</th>
              <th style={{ padding: "0 0 0.75rem" }}>Class Name</th>
              <th style={{ padding: "0 0 0.75rem" }}>Leak ID</th>
              <th style={{ padding: "0 0 0.75rem" }}>Retained</th>
              <th style={{ padding: "0 0 0.75rem" }}>Score</th>
              <th style={{ padding: "0 0 0.75rem" }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {filteredLeaks.map((leak) => {
              const expanded = Boolean(expandedLeakIds[leak.id]);
              const severity = severityTone(leak.severity);

              return (
                <Fragment key={leak.id}>
                  <tr key={leak.id}>
                    <td style={{ padding: "0.9rem 0.4rem 0.9rem 0", borderTop: "1px solid #1e293b" }}>
                      <span
                        style={{
                          display: "inline-flex",
                          minWidth: 56,
                          justifyContent: "center",
                          borderRadius: 999,
                          border: `1px solid ${severity.border}`,
                          color: severity.text,
                          background: severity.background,
                          padding: "0.2rem 0.5rem",
                          fontSize: "0.78rem",
                          letterSpacing: "0.08em",
                          textTransform: "uppercase",
                        }}
                      >
                        {leak.severity}
                      </span>
                    </td>
                    <td style={{ padding: "0.9rem 0.4rem", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      <div style={{ fontWeight: 600, overflowWrap: "anywhere" }}>{leak.className}</div>
                      <div style={{ color: "#64748b", fontSize: "0.86rem", marginTop: "0.25rem" }}>
                        {leak.instances.toLocaleString()} instances
                      </div>
                      {leak.provenance.length > 0 ? (
                        <div style={{ display: "flex", gap: "0.35rem", flexWrap: "wrap", marginTop: "0.45rem" }}>
                          {leak.provenance.map((marker) => (
                            <ProvenanceBadge key={`${leak.id}-${marker.kind}-inline`} kind={marker.kind} />
                          ))}
                        </div>
                      ) : null}
                    </td>
                    <td style={{ padding: "0.9rem 0.4rem", borderTop: "1px solid #1e293b", color: "#94a3b8", verticalAlign: "top" }}>
                      {leak.id}
                    </td>
                    <td style={{ padding: "0.9rem 0.4rem", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      <div>{formatBytes(leak.retainedSizeBytes)}</div>
                      <div style={{ color: "#64748b", fontSize: "0.86rem", marginTop: "0.25rem" }}>
                        Shallow {leak.shallowSizeBytes ? formatBytes(leak.shallowSizeBytes) : "-"}
                      </div>
                    </td>
                    <td style={{ padding: "0.9rem 0.4rem", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      {typeof leak.suspectScore === "number" ? leak.suspectScore.toFixed(2) : "-"}
                    </td>
                    <td style={{ padding: "0.9rem 0 0.9rem 0.4rem", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
                        <button
                          type="button"
                          onClick={() => toggleLeakExpanded(leak.id)}
                          style={{
                            borderRadius: 999,
                            border: "1px solid #334155",
                            background: "transparent",
                            color: "#cbd5e1",
                            padding: "0.35rem 0.7rem",
                            cursor: "pointer",
                          }}
                        >
                          {expanded ? "Hide" : "Inspect"}
                        </button>
                        <button
                          type="button"
                          disabled={typeof onTraceLeak !== "function"}
                          onClick={() => onTraceLeak?.(leak.id)}
                          style={{
                            borderRadius: 999,
                            border: "1px solid #334155",
                            background: "rgba(15, 23, 42, 0.8)",
                            color: "#cbd5e1",
                            padding: "0.35rem 0.7rem",
                            cursor: "pointer",
                          }}
                        >
                          Trace
                        </button>
                      </div>
                    </td>
                  </tr>
                  {expanded ? (
                    <tr key={`${leak.id}-details`}>
                      <td colSpan={6} style={{ padding: "0 0 1rem", borderTop: "1px solid #0f172a" }}>
                        <div
                          style={{
                            display: "grid",
                            gap: "0.75rem",
                            borderRadius: 16,
                            background: "rgba(2, 6, 23, 0.76)",
                            padding: "0.95rem 1rem",
                          }}
                        >
                          <div style={{ color: "#cbd5e1", lineHeight: 1.7 }}>{leak.description}</div>
                          {leak.provenance.length > 0 ? (
                            <div style={{ display: "flex", gap: "0.45rem", flexWrap: "wrap" }}>
                              {leak.provenance.map((marker) => (
                                <ProvenanceBadge key={`${leak.id}-${marker.kind}`} kind={marker.kind} />
                              ))}
                            </div>
                          ) : (
                            <div style={{ color: "#64748b" }}>No provenance markers attached to this suspect.</div>
                          )}
                          <div style={{ color: "#64748b", fontSize: "0.88rem" }}>
                            Inline drilldown is intentionally restrained in this slice. Deep trace and object pages remain a later task.
                          </div>
                        </div>
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

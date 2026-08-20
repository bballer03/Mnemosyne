import { Link } from "react-router-dom";

import { ComparisonPicker } from "./ComparisonPicker";
import { useComparisonStore } from "./comparison-store";
import { MatchQualityBadge } from "./MatchQualityBadge";
import { ObjectDeltaTable } from "./ObjectDeltaTable";

const panelStyle = {
  border: "1px solid #1e293b",
  borderRadius: 24,
  background: "linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(2, 6, 23, 0.96))",
  padding: "1.3rem",
} as const;

export function ComparisonPage() {
  const { diffReport, loadStatus } = useComparisonStore();

  const hasNoDifferences =
    diffReport !== undefined &&
    diffReport.added.length === 0 &&
    diffReport.removed.length === 0 &&
    diffReport.retainedChanged.length === 0;

  return (
    <main style={{ display: "grid", gap: "1rem" }}>
      <section style={panelStyle}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
            <Link to="/dashboard">Dashboard</Link>
            <Link to="/compare" aria-current="page">
              Compare
            </Link>
          </div>
          <div style={{ color: "#38bdf8", fontSize: "0.78rem", letterSpacing: "0.16em", textTransform: "uppercase" }}>
            Comparison basket
          </div>
          <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.6rem)", lineHeight: 1.08 }}>
            Compare two heaps
          </h1>
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
            Object-level diff between a before and after heap dump: added, removed, and retained-size-changed classes,
            ranked with a match-quality indicator for the identity strategy used.
          </p>
        </header>
      </section>

      <ComparisonPicker />

      {loadStatus === "loading" ? (
        <section style={panelStyle}>
          <p style={{ margin: 0, color: "#94a3b8" }}>Loading diff report...</p>
        </section>
      ) : null}

      {diffReport ? (
        <section style={{ display: "grid", gap: "1rem" }}>
          <MatchQualityBadge matchQuality={diffReport.matchQuality} />

          {hasNoDifferences ? (
            <section style={panelStyle}>
              <p style={{ margin: 0, color: "#e2e8f0", fontWeight: 600 }}>No differences found.</p>
              <p style={{ margin: "0.4rem 0 0", color: "#94a3b8", lineHeight: 1.6 }}>
                The before and after heaps produced no added, removed, or retained-size-changed classes under the
                current identity strategy and thresholds.
              </p>
            </section>
          ) : (
            <>
              <ObjectDeltaTable kind="Added" deltas={diffReport.added} />
              <ObjectDeltaTable kind="Removed" deltas={diffReport.removed} />
              <ObjectDeltaTable kind="RetainedChanged" deltas={diffReport.retainedChanged} />
            </>
          )}
        </section>
      ) : null}
    </main>
  );
}

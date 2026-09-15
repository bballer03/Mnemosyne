import { workbenchPanelStyle } from "../../app/theme-tokens";
import { useComparisonStore } from "./comparison-store";
import { MatchQualityBadge } from "./MatchQualityBadge";
import { ObjectDeltaTable } from "./ObjectDeltaTable";

const panelStyle = {
  border: workbenchPanelStyle.border,
  borderRadius: 18,
  background: "rgba(2, 6, 23, 0.78)",
  padding: "1rem",
} as const;

export function ComparisonResults() {
  const { diffReport, loadStatus } = useComparisonStore();
  const hasNoDifferences =
    diffReport !== undefined &&
    diffReport.added.length === 0 &&
    diffReport.removed.length === 0 &&
    diffReport.retainedChanged.length === 0;

  if (loadStatus === "loading") {
    return (
      <section style={panelStyle}>
        <p style={{ margin: 0, color: "#94a3b8" }}>Loading diff report...</p>
      </section>
    );
  }

  if (!diffReport) {
    return null;
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <MatchQualityBadge matchQuality={diffReport.matchQuality} />

      {hasNoDifferences ? (
        <section style={panelStyle}>
          <p style={{ margin: 0, color: "#e2e8f0", fontWeight: 600 }}>No differences found.</p>
          <p style={{ margin: "0.4rem 0 0", color: "#94a3b8", lineHeight: 1.6 }}>
            The baseline and current snapshots produced no added, removed, or retained-size-changed classes under the
            selected identity strategy and thresholds.
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
  );
}

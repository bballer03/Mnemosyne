import { useState } from "react";

import { workbenchPanelStyle } from "../../app/theme-tokens";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { ComparisonPicker } from "./ComparisonPicker";
import { ComparisonResults } from "./ComparisonResults";

const chromeStyle = {
  ...workbenchPanelStyle,
  borderRadius: 12,
  background: "rgba(15, 23, 42, 0.9)",
  padding: "0.75rem",
  display: "grid",
  gap: "0.75rem",
} as const;

const toggleStyle = {
  justifySelf: "end",
  border: "1px solid #334155",
  borderRadius: 8,
  background: "rgba(15, 23, 42, 0.9)",
  color: "#e2e8f0",
  padding: "0.4rem 0.8rem",
  cursor: "pointer",
} as const;

export function ComparisonWorkbenchPanel({ variant }: { variant: "chrome" | "route" }) {
  const artifact = useArtifactStore((state) => state.artifact);
  const [expanded, setExpanded] = useState(variant === "route");

  if (variant === "chrome" && !artifact) {
    return null;
  }

  if (variant === "route") {
    return (
      <section aria-label="Standalone comparison adapter" style={{ display: "grid", gap: "1rem" }}>
        <ComparisonPicker />
        <ComparisonResults />
      </section>
    );
  }

  return (
    <section aria-label="Workbench comparison" style={chromeStyle}>
      <button
        type="button"
        aria-expanded={expanded}
        style={toggleStyle}
        onClick={() => setExpanded((current) => !current)}
      >
        {expanded ? "Hide current to baseline compare" : "Compare current to baseline"}
      </button>
      {expanded ? (
        <div style={{ display: "grid", gap: "1rem" }}>
          <ComparisonPicker />
          <ComparisonResults />
        </div>
      ) : null}
    </section>
  );
}

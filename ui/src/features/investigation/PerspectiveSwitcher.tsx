import { useNavigate } from "react-router-dom";

import { useInvestigationStore } from "./investigation-store";
import { WORKBENCH_PERSPECTIVES } from "./perspectives";

const shellStyle = {
  border: "1px solid var(--mn-border-subtle)",
  borderRadius: "var(--mn-radius-panel)",
  background: "var(--mn-surface-panel)",
  padding: "0.75rem",
} as const;

const listStyle = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fit, minmax(11rem, 1fr))",
  gap: "0.5rem",
} as const;

const buttonStyle = {
  border: "1px solid var(--mn-border)",
  borderRadius: "var(--mn-radius-control)",
  background: "transparent",
  color: "var(--mn-text-primary)",
  cursor: "pointer",
  padding: "0.55rem 0.65rem",
  textAlign: "left",
} as const;

const selectedButtonStyle = {
  ...buttonStyle,
  border: "1px solid var(--mn-border-accent)",
  background: "rgba(56, 189, 248, 0.12)",
} as const;

export function PerspectiveSwitcher() {
  const navigate = useNavigate();
  const perspectiveId = useInvestigationStore((state) => state.perspectiveId);
  const setPerspectiveId = useInvestigationStore((state) => state.setPerspectiveId);

  return (
    <section aria-labelledby="workbench-perspectives-title" style={shellStyle}>
      <div
        style={{
          alignItems: "baseline",
          display: "flex",
          flexWrap: "wrap",
          gap: "0.5rem",
          justifyContent: "space-between",
          marginBottom: "0.5rem",
        }}
      >
        <h2 id="workbench-perspectives-title" style={{ fontSize: "0.9rem", margin: 0 }}>
          Workbench perspective
        </h2>
        <span style={{ color: "var(--mn-text-muted)", fontSize: "0.75rem" }}>
          Fixed layouts · no free-form docking
        </span>
      </div>
      <div style={listStyle}>
        {WORKBENCH_PERSPECTIVES.map((perspective) => {
          const selected = perspective.id === perspectiveId;
          return (
            <button
              aria-pressed={selected}
              key={perspective.id}
              onClick={() => {
                setPerspectiveId(perspective.id);
                navigate(perspective.route);
              }}
              style={selected ? selectedButtonStyle : buttonStyle}
              title={`Panes: ${perspective.panes.join(", ")}`}
              type="button"
            >
              <strong style={{ display: "block", fontSize: "0.82rem" }}>
                {perspective.name}
              </strong>
              <span
                style={{
                  color: "var(--mn-text-muted)",
                  display: "block",
                  fontSize: "0.72rem",
                  marginTop: "0.2rem",
                }}
              >
                {perspective.description}
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

import { NavLink } from "react-router-dom";

type SelectedObject = {
  objectId: string;
  className: string;
  name: string;
};

const linkStyle = {
  display: "block",
  borderRadius: 14,
  border: "1px solid #334155",
  padding: "0.7rem 0.85rem",
  textDecoration: "none",
  color: "#cbd5e1",
  background: "rgba(15, 23, 42, 0.55)",
} as const;

export function ModeRail({ selectedObject }: { selectedObject?: SelectedObject }) {
  const selectedObjectLabel = selectedObject?.objectId === "" ? "Artifact-only row" : selectedObject?.objectId;

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <nav aria-label="Heap explorer modes" style={{ display: "grid", gap: "0.65rem" }}>
        <NavLink to="/heap-explorer/dominators" style={linkStyle}>
          Dominators
        </NavLink>
        <NavLink to="/heap-explorer/object-inspector" style={linkStyle}>
          Object Inspector
        </NavLink>
        <NavLink to="/heap-explorer/query-console" style={linkStyle}>
          Query Console
        </NavLink>
      </nav>

      <section style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
        <div style={{ fontSize: "0.78rem", letterSpacing: "0.08em", textTransform: "uppercase", color: "#64748b" }}>
          Selected target
        </div>
        <div style={{ fontWeight: 600 }}>{selectedObject?.className ?? "No object selected"}</div>
        <div style={{ color: "#94a3b8", overflowWrap: "anywhere" }}>{selectedObjectLabel ?? "Awaiting selection"}</div>
      </section>

      <section style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
        <div style={{ fontSize: "0.78rem", letterSpacing: "0.08em", textTransform: "uppercase", color: "#64748b" }}>
          Recent targets
        </div>
        <div style={{ color: "#94a3b8", lineHeight: 1.6 }}>Recent targets appear here after selection.</div>
      </section>
    </div>
  );
}

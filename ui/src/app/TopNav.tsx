import { NavLink } from "react-router-dom";

import { useArtifactStore } from "../features/artifact-loader/use-artifact-store";

const navStyle = {
  display: "flex",
  flexWrap: "wrap" as const,
  gap: "0.5rem",
  padding: "0.75rem 1.5rem",
  borderBottom: "1px solid #0f172a",
  background: "rgba(2, 6, 23, 0.6)",
};

const linkBaseStyle = {
  border: "1px solid transparent",
  borderRadius: 999,
  padding: "0.35rem 0.85rem",
  fontSize: "0.85rem",
  color: "#94a3b8",
  textDecoration: "none",
};

const activeLinkStyle = {
  ...linkBaseStyle,
  border: "1px solid #38bdf8",
  color: "#e0f2fe",
  background: "rgba(56, 189, 248, 0.12)",
};

type NavItem = {
  to: string;
  label: string;
};

/** Exported for tests — keep nav contract in one place. */
export const POWER_ROUTES: NavItem[] = [
  { to: "/", label: "Home" },
  { to: "/dashboard", label: "Dashboard" },
  { to: "/artifacts/explorer", label: "Artifact Explorer" },
  { to: "/heap-explorer/dominators", label: "Dominators" },
  { to: "/heap-explorer/object-inspector", label: "Object Inspector" },
  { to: "/heap-explorer/query-console", label: "Query Console" },
  { to: "/heap-explorer/threads", label: "Threads" },
  { to: "/compare", label: "Compare" },
  { to: "/workbench/policies", label: "Policies" },
  { to: "/workbench/snapshots", label: "Snapshots" },
  { to: "/workbench/flamegraphs", label: "Flamegraphs" },
  { to: "/assistant", label: "Assistant" },
];

/**
 * Design doc §4 item 9 / §5's "persistent top-nav to every power route" —
 * this is the concrete enforcement of the brainstorming session's "don't
 * dumb down the product" constraint (design doc §2): every existing
 * MAT-equivalent power route stays one click away from the guided landing,
 * never gated behind a workflow step.
 *
 * `/leaks/:leakId/overview` is included only "when a leak context exists"
 * (per this slice's task brief) -- there is no dedicated "current leak"
 * store anywhere in this codebase yet (checked `dashboard-store.ts` and
 * `leak-workspace-store.ts`), so the heuristic here is the simplest one that
 * uses state that already exists: the loaded artifact's highest
 * `suspectScore` leak (falling back to the first leak when no entry has a
 * score), i.e. "the leak the AI would triage first". This is a judgment
 * call for this slice, documented here and in the commit body, not a
 * pre-existing convention.
 */
export function TopNav() {
  const artifact = useArtifactStore((state) => state.artifact);
  const topLeak = artifact?.leaks.length
    ? [...artifact.leaks].sort((a, b) => (b.suspectScore ?? 0) - (a.suspectScore ?? 0))[0]
    : undefined;

  const items: NavItem[] = topLeak
    ? [...POWER_ROUTES, { to: `/leaks/${topLeak.id}/overview`, label: "Leak Workspace" }]
    : POWER_ROUTES;

  return (
    <nav aria-label="Power routes" style={navStyle}>
      {items.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          style={({ isActive }) => (isActive ? activeLinkStyle : linkBaseStyle)}
        >
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}

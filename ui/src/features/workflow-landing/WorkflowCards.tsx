import { Link, useInRouterContext } from "react-router-dom";

import { WorkflowCard } from "./WorkflowCard";

const gridStyle = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))",
  gap: "1rem",
} as const;

const cardStyle = {
  border: "1px solid #1e293b",
  borderRadius: 16,
  background: "rgba(2, 6, 23, 0.75)",
  padding: "1rem 1.1rem",
  display: "grid",
  gap: "0.6rem",
} as const;

const linkButtonStyle = {
  border: "1px solid #38bdf8",
  borderRadius: 999,
  background: "#082f49",
  color: "#e0f2fe",
  padding: "0.5rem 0.9rem",
  fontSize: "0.88rem",
  justifySelf: "start",
  textDecoration: "none",
} as const;

export type WorkflowCardsProps = {
  heapPath?: string;
};

/**
 * The three additional workflow-kind cards from design doc §4 item 9,
 * beyond the primary triage summary card (`TriageSummaryCard`).
 * `compare_snapshots` deep-links to the existing `/compare` route shipped in
 * Slice 14.A rather than driving `start_workflow` itself from the landing
 * page -- that workflow's first step needs a before/after heap-path or
 * snapshot-key pair (see `core::mcp::server`'s `start_workflow` match arm),
 * which the landing page has no natural source for; the comparison picker
 * already built in Slice 14.A is exactly that UI. `tune_gc` and
 * `traverse_object_graph` reuse the same generic `WorkflowCard` the triage
 * summary uses.
 */
export function WorkflowCards({ heapPath }: WorkflowCardsProps) {
  // `Link` requires a `<Router>` ancestor. `ArtifactLoaderPage` (this
  // component's host) is itself already rendered both inside and outside a
  // router context across its own test suite (see its `isInRouterContext`
  // guard around `NavigateToDashboardOnSuccess`), so this card follows the
  // same pattern rather than assuming a router is always present.
  const isInRouterContext = useInRouterContext();

  return (
    <div style={gridStyle}>
      <WorkflowCard
        kind="tune_gc"
        title="Tune GC"
        description="Reviews thread-local allocation patterns and GC settings for tuning opportunities."
        heapPath={heapPath}
      />
      <WorkflowCard
        kind="traverse_object_graph"
        title="Traverse Object Graph"
        description="Walks the reference graph outward from a starting object."
        heapPath={heapPath}
        showObjectIdInput
      />
      <article style={cardStyle} aria-label="Compare snapshots workflow card">
        <h4 style={{ margin: 0 }}>Compare Snapshots</h4>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.55 }}>
          Pick two heap snapshots and see a ranked added/removed/retained-changed diff.
        </p>
        {isInRouterContext ? (
          <Link to="/compare" style={linkButtonStyle}>
            Open comparison view
          </Link>
        ) : (
          <span style={{ ...linkButtonStyle, opacity: 0.6 }}>Open comparison view</span>
        )}
      </article>
    </div>
  );
}

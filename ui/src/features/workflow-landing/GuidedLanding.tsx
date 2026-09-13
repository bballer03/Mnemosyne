import { NaturalLanguageInputBar } from "./NaturalLanguageInputBar";
import { RecentHeapsList } from "./RecentHeapsList";
import { TriageSummaryCard } from "./TriageSummaryCard";
import { WorkflowCards } from "./WorkflowCards";

export type GuidedLandingProps = {
  heapPath?: string;
};

const sectionStyle = {
  display: "grid",
  gap: "1.25rem",
} as const;

/**
 * Design doc §3.3 / §4 item 9: the AI-native layer added on top of the
 * unchanged artifact-drop flow (see `ArtifactLoaderPage`'s own doc comment
 * for the placement decision). Composes:
 *   - the primary triage summary card (`TriageSummaryCard`)
 *   - the natural-language input bar (`NaturalLanguageInputBar`)
 *   - the three remaining workflow-kind cards (`WorkflowCards`)
 *   - the optional "Recent heaps" snapshot list (`RecentHeapsList`, renders
 *     nothing at all when the bridge lacks `listSnapshots`)
 *
 * Every sub-component here independently degrades to an explicit
 * unavailable/placeholder state when its bridge method is absent -- this
 * component does no bridge-availability gating of its own beyond passing
 * `heapPath` through.
 */
export function GuidedLanding({ heapPath }: GuidedLandingProps) {
  return (
    <section style={sectionStyle} aria-label="AI-guided workflows">
      <div>
        <p
          style={{
            margin: 0,
            fontSize: "0.78rem",
            letterSpacing: "0.16em",
            textTransform: "uppercase",
            color: "#38bdf8",
          }}
        >
          Guided Workflows
        </p>
        <h3 style={{ margin: "0.35rem 0 0" }}>Ask a question or start a workflow</h3>
        <p style={{ margin: "0.35rem 0 0", color: "#94a3b8", maxWidth: "64ch", lineHeight: 1.6 }}>
          These accelerators compose the same underlying analysis every power route in the top navigation
          exposes directly -- nothing here is gated or exclusive.
        </p>
      </div>

      <TriageSummaryCard heapPath={heapPath} />
      <NaturalLanguageInputBar heapPath={heapPath} />
      <WorkflowCards heapPath={heapPath} />
      <RecentHeapsList />
    </section>
  );
}

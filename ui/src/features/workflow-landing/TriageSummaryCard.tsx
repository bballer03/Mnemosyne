import { WorkflowCard } from "./WorkflowCard";

const sectionStyle = {
  border: "1px solid #38bdf8",
  borderRadius: 20,
  background: "rgba(8, 47, 73, 0.35)",
  padding: "1.25rem",
  display: "grid",
  gap: "0.75rem",
} as const;

export type TriageSummaryCardProps = {
  heapPath?: string;
};

/**
 * The guided landing's primary card (design doc §4 item 9 / §5 diagram):
 * composes M11's `triage_memory_leak` workflow (`detect` ->
 * `investigate_suspect` -> `explain` -> `propose_fix` -> `complete`, per
 * `core::workflow::triage_memory_leak`) via `start_workflow`/`next_step`.
 * Thin wrapper around the generic `WorkflowCard` (shared with the other
 * workflow-kind cards) rather than a bespoke implementation -- the step
 * sequence itself is opaque to this UI layer by design (see
 * `WorkflowCard`'s doc comment): it renders whatever `current_step`/
 * `step_result` the backend returns, without hard-coding knowledge of
 * triage's four step names.
 */
export function TriageSummaryCard({ heapPath }: TriageSummaryCardProps) {
  return (
    <section style={sectionStyle} aria-label="Triage summary">
      <p
        style={{
          margin: 0,
          fontSize: "0.78rem",
          letterSpacing: "0.16em",
          textTransform: "uppercase",
          color: "#38bdf8",
        }}
      >
        AI Triage Summary
      </p>
      <WorkflowCard
        kind="triage_memory_leak"
        title="Triage memory leak"
        description="Runs leak detection, investigates the top suspect, explains it, and proposes a fix -- one step at a time."
        heapPath={heapPath}
      />
    </section>
  );
}

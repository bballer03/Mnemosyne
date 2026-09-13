import { useState, type FormEvent } from "react";

import { isHeapQueryAvailable, runHeapQuery, type HeapQueryResult } from "../heap-explorer/heap-explorer-query-client";
import { looksLikeOqlQuery, routeFreeTextToWorkflow } from "./natural-language-router";
import { isStartWorkflowAvailable, runStartWorkflow, type WorkflowStepResult } from "./workflow-bridge-client";

export type NaturalLanguageInputBarProps = {
  heapPath?: string;
};

type SubmitOutcome =
  | { kind: "idle" }
  | { kind: "submitting" }
  | { kind: "query-result"; data: HeapQueryResult }
  | { kind: "workflow-result"; workflowKindLabel: string; data: WorkflowStepResult }
  | { kind: "unavailable"; reason: string }
  | { kind: "error"; message: string };

const barStyle = {
  border: "1px solid #1e293b",
  borderRadius: 20,
  background: "rgba(15, 23, 42, 0.88)",
  padding: "1.25rem",
  display: "grid",
  gap: "0.75rem",
} as const;

const inputRowStyle = {
  display: "flex",
  gap: "0.6rem",
  flexWrap: "wrap" as const,
};

const inputStyle = {
  flex: "1 1 260px",
  borderRadius: 999,
  border: "1px solid #334155",
  background: "#020617",
  color: "#e2e8f0",
  padding: "0.6rem 1rem",
};

const submitButtonStyle = {
  border: "1px solid #38bdf8",
  borderRadius: 999,
  background: "#082f49",
  color: "#e0f2fe",
  padding: "0.6rem 1.1rem",
  cursor: "pointer",
} as const;

const WORKFLOW_LABELS: Record<string, string> = {
  triage_memory_leak: "Triage memory leak",
  tune_gc: "Tune GC",
  traverse_object_graph: "Traverse object graph",
  compare_snapshots: "Compare snapshots",
};

/**
 * Design doc §4 item 9's natural-language input bar. Disambiguation
 * heuristic lives in `./natural-language-router.ts` (documented there in
 * full): OQL-shaped input reuses `runHeapQuery` (the same execution path
 * `HeapQueryConsolePage`/`QueryConsolePanel` already call), everything else
 * routes to `startWorkflow` for a best-guess `WorkflowKind`.
 */
export function NaturalLanguageInputBar({ heapPath }: NaturalLanguageInputBarProps) {
  const [value, setValue] = useState("");
  const [outcome, setOutcome] = useState<SubmitOutcome>({ kind: "idle" });

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const trimmed = value.trim();

    if (trimmed.length === 0) {
      return;
    }

    setOutcome({ kind: "submitting" });

    if (looksLikeOqlQuery(trimmed)) {
      if (!isHeapQueryAvailable() || !heapPath) {
        setOutcome({
          kind: "unavailable",
          reason: !heapPath
            ? "Load an artifact before running a query."
            : "Query execution is unavailable without a connected heap-explorer bridge.",
        });
        return;
      }

      const result = await runHeapQuery({ heapPath, query: trimmed });

      if (result.status === "unavailable") {
        setOutcome({ kind: "unavailable", reason: "Query execution is unavailable in this session." });
      } else if (result.status === "error") {
        setOutcome({ kind: "error", message: result.error });
      } else {
        setOutcome({ kind: "query-result", data: result.data });
      }

      return;
    }

    const kind = routeFreeTextToWorkflow(trimmed);

    if (kind === "compare_snapshots") {
      setOutcome({
        kind: "unavailable",
        reason: "This reads as a comparison request -- use the \"Compare Snapshots\" card below to open /compare.",
      });
      return;
    }

    if (!isStartWorkflowAvailable() || !heapPath) {
      setOutcome({
        kind: "unavailable",
        reason: !heapPath
          ? "Load an artifact before starting a workflow."
          : "Workflow execution is unavailable without a connected workflow bridge.",
      });
      return;
    }

    const result = await runStartWorkflow(kind, { heapPath });

    if (result.status === "unavailable") {
      setOutcome({ kind: "unavailable", reason: "Workflow execution is unavailable in this session." });
    } else if (result.status === "error") {
      setOutcome({ kind: "error", message: result.error });
    } else {
      setOutcome({ kind: "workflow-result", workflowKindLabel: WORKFLOW_LABELS[kind] ?? kind, data: result.data });
    }
  }

  return (
    <section style={barStyle} aria-label="Natural language input">
      <h3 style={{ margin: 0 }}>Ask a question</h3>
      <p style={{ margin: 0, color: "#94a3b8" }}>
        Type an OQL query (e.g. <code>SELECT * FROM objects WHERE ...</code>) or ask a plain-language question --
        we'll route it to the right tool.
      </p>
      <form onSubmit={(event) => void handleSubmit(event)}>
        <div style={inputRowStyle}>
          <input
            aria-label="Natural language or OQL input"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            placeholder="e.g. why is my heap growing, or SELECT * FROM objects WHERE ..."
            style={inputStyle}
          />
          <button type="submit" style={submitButtonStyle}>
            Ask
          </button>
        </div>
      </form>

      {outcome.kind === "submitting" ? <p style={{ margin: 0, color: "#94a3b8" }}>Working...</p> : null}
      {outcome.kind === "unavailable" ? (
        <p style={{ margin: 0, color: "#facc15" }}>{outcome.reason}</p>
      ) : null}
      {outcome.kind === "error" ? (
        <p role="alert" style={{ margin: 0, color: "#fca5a5" }}>
          {outcome.message}
        </p>
      ) : null}

      {outcome.kind === "query-result" ? (
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr style={{ textAlign: "left", color: "#94a3b8" }}>
                {outcome.data.columns.map((column) => (
                  <th key={column} style={{ padding: "0 0.6rem 0.4rem 0" }}>
                    {column}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {outcome.data.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {row.map((cell, cellIndex) => (
                    <td key={cellIndex} style={{ padding: "0.4rem 0.6rem 0.4rem 0", borderTop: "1px solid #1e293b" }}>
                      {String(cell)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}

      {outcome.kind === "workflow-result" ? (
        <div style={{ display: "grid", gap: "0.4rem" }}>
          <div style={{ color: "#67e8f9", fontSize: "0.85rem" }}>
            Started {outcome.workflowKindLabel} -- current step: {outcome.data.currentStep}
          </div>
          <pre
            style={{
              margin: 0,
              background: "#020617",
              border: "1px solid #0f172a",
              borderRadius: 10,
              padding: "0.6rem",
              fontSize: "0.78rem",
              overflowX: "auto",
              color: "#cbd5e1",
            }}
          >
            {JSON.stringify(outcome.data.stepResult, null, 2)}
          </pre>
        </div>
      ) : null}
    </section>
  );
}

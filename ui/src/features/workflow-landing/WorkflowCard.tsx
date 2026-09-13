import { useState } from "react";

import {
  isStartWorkflowAvailable,
  runNextStep,
  runStartWorkflow,
  WORKFLOW_COMPLETE_STEP,
  type WorkflowKindId,
  type WorkflowStepResult,
} from "./workflow-bridge-client";

export type WorkflowCardProps = {
  kind: WorkflowKindId;
  title: string;
  description: string;
  heapPath?: string;
  /**
   * `traverse_object_graph` is the only kind whose first step takes a
   * caller-supplied `object_id` (see `core::mcp::server`'s `start_workflow`
   * match arm). The landing page has no "currently selected object" concept
   * of its own (that lives in the heap-explorer/object-inspector routes), so
   * this card exposes a plain text field for it rather than inventing a
   * cross-feature selection store just for this optional input.
   */
  showObjectIdInput?: boolean;
};

type CardState =
  | { phase: "idle" }
  | { phase: "starting" }
  | { phase: "advancing" }
  | { phase: "in-progress"; result: WorkflowStepResult }
  | { phase: "complete"; result: WorkflowStepResult }
  | { phase: "error"; message: string };

const cardStyle = {
  border: "1px solid #1e293b",
  borderRadius: 16,
  background: "rgba(2, 6, 23, 0.75)",
  padding: "1rem 1.1rem",
  display: "grid",
  gap: "0.6rem",
} as const;

const buttonStyle = {
  border: "1px solid #38bdf8",
  borderRadius: 999,
  background: "#082f49",
  color: "#e0f2fe",
  padding: "0.5rem 0.9rem",
  cursor: "pointer",
  fontSize: "0.88rem",
  justifySelf: "start",
} as const;

export function WorkflowCard({ kind, title, description, heapPath, showObjectIdInput = false }: WorkflowCardProps) {
  const [objectId, setObjectId] = useState("");
  const [state, setState] = useState<CardState>({ phase: "idle" });
  const bridgeAvailable = isStartWorkflowAvailable();

  async function handleStart() {
    setState({ phase: "starting" });

    const result = await runStartWorkflow(kind, {
      heapPath,
      objectId: showObjectIdInput && objectId.trim().length > 0 ? objectId.trim() : undefined,
    });

    if (result.status === "unavailable") {
      setState({ phase: "idle" });
      return;
    }

    if (result.status === "error") {
      setState({ phase: "error", message: result.error });
      return;
    }

    setState({
      phase: result.data.currentStep === WORKFLOW_COMPLETE_STEP ? "complete" : "in-progress",
      result: result.data,
    });
  }

  async function handleContinue(workflowId: string) {
    setState({ phase: "advancing" });

    const result = await runNextStep(workflowId);

    if (result.status === "unavailable") {
      setState({ phase: "idle" });
      return;
    }

    if (result.status === "error") {
      setState({ phase: "error", message: result.error });
      return;
    }

    setState({
      phase: result.data.currentStep === WORKFLOW_COMPLETE_STEP ? "complete" : "in-progress",
      result: result.data,
    });
  }

  return (
    <article style={cardStyle} aria-label={`${title} workflow card`}>
      <h4 style={{ margin: 0 }}>{title}</h4>
      <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.55 }}>{description}</p>

      {!bridgeAvailable ? (
        <p style={{ margin: 0, color: "#64748b", fontSize: "0.88rem" }}>
          Workflow bridge unavailable in this session. Connect a live Mnemosyne host to run this workflow here.
        </p>
      ) : (
        <>
          {showObjectIdInput ? (
            <label style={{ display: "grid", gap: "0.3rem", fontSize: "0.85rem", color: "#cbd5e1" }}>
              Starting object ID (optional)
              <input
                aria-label={`${title} starting object ID`}
                value={objectId}
                onChange={(event) => setObjectId(event.target.value)}
                style={{
                  borderRadius: 8,
                  border: "1px solid #334155",
                  background: "#020617",
                  color: "#e2e8f0",
                  padding: "0.4rem 0.6rem",
                }}
              />
            </label>
          ) : null}

          {state.phase === "idle" || state.phase === "error" ? (
            <button type="button" style={buttonStyle} onClick={() => void handleStart()} disabled={!heapPath}>
              Start {title}
            </button>
          ) : null}

          {!heapPath && state.phase === "idle" ? (
            <p style={{ margin: 0, color: "#64748b", fontSize: "0.82rem" }}>Load an artifact first.</p>
          ) : null}

          {state.phase === "starting" || state.phase === "advancing" ? (
            <p style={{ margin: 0, color: "#94a3b8" }}>Running...</p>
          ) : null}

          {state.phase === "error" ? (
            <p role="alert" style={{ margin: 0, color: "#fca5a5" }}>
              {state.message}
            </p>
          ) : null}

          {state.phase === "in-progress" || state.phase === "complete" ? (
            <div style={{ display: "grid", gap: "0.4rem" }}>
              <div style={{ color: "#67e8f9", fontSize: "0.85rem" }}>
                Current step: {state.result.currentStep}
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
                {JSON.stringify(state.result.stepResult, null, 2)}
              </pre>
              {state.phase === "in-progress" ? (
                <button
                  type="button"
                  style={buttonStyle}
                  onClick={() => void handleContinue(state.result.workflowId)}
                >
                  Continue
                </button>
              ) : (
                <div style={{ color: "#86efac", fontSize: "0.85rem" }}>Workflow complete.</div>
              )}
            </div>
          ) : null}
        </>
      )}
    </article>
  );
}

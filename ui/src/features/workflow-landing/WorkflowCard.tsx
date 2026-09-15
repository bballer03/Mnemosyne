import { useEffect, useRef, useState } from "react";
import { Link, useInRouterContext } from "react-router-dom";

import {
  isCloseWorkflowAvailable,
  isGetWorkflowAvailable,
  isStartWorkflowAvailable,
  WORKFLOW_COMPLETE_STEP,
  type WorkflowKindId,
  type WorkflowStepResult,
} from "./workflow-bridge-client";
import { useInvestigationStore } from "../investigation/investigation-store";
import {
  advanceWorkspaceWorkflow,
  closeWorkspaceWorkflow,
  recoverWorkspaceWorkflow,
  startWorkspaceWorkflow,
} from "./workflow-binding";

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
  | { phase: "resuming" }
  | { phase: "advancing" }
  | { phase: "closing" }
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

const linkStyle = {
  ...buttonStyle,
  textDecoration: "none",
  display: "inline-block",
} as const;

function firstDuplicateClassName(stepResult: unknown): string | undefined {
  if (typeof stepResult !== "object" || stepResult === null) {
    return undefined;
  }

  const names = (stepResult as { duplicate_class_names?: unknown }).duplicate_class_names;
  if (!Array.isArray(names) || typeof names[0] !== "string") {
    return undefined;
  }

  return names[0];
}

type StepLink = { to: string; label: string };

/** Deterministic workbench deep-links for the active workflow step. */
export function workflowStepLinks(kind: WorkflowKindId, currentStep: string): StepLink[] {
  if (kind === "classloader_leak") {
    if (currentStep === "detect" || currentStep === "select") {
      return [{ to: "/artifacts/explorer", label: "Open classloader explorer" }];
    }
    if (currentStep === "inspect_retention" || currentStep === "explain") {
      return [
        { to: "/heap-explorer/object-inspector", label: "Open object inspector" },
        { to: "/artifacts/explorer", label: "Open classloader explorer" },
        { to: "/leaks/focus/gc-path", label: "Open GC paths" },
      ];
    }
    if (currentStep === WORKFLOW_COMPLETE_STEP) {
      return [
        { to: "/artifacts/explorer", label: "Open classloader explorer" },
        { to: "/heap-explorer/object-inspector", label: "Open object inspector" },
      ];
    }
  }

  if (kind === "compare_snapshots") {
    return [{ to: "/compare", label: "Open compare" }];
  }

  if (kind === "traverse_object_graph" || kind === "triage_memory_leak") {
    if (currentStep === "investigate_suspect" || currentStep === "inspect" || currentStep === "choose_direction") {
      return [
        { to: "/heap-explorer/object-inspector", label: "Open object inspector" },
        { to: "/leaks/focus/gc-path", label: "Open GC paths" },
      ];
    }
  }

  if (kind === "tune_gc") {
    return [{ to: "/artifacts/explorer", label: "Open artifact explorer" }];
  }

  return [];
}

export function WorkflowCard({ kind, title, description, heapPath, showObjectIdInput = false }: WorkflowCardProps) {
  const [objectId, setObjectId] = useState("");
  const [state, setState] = useState<CardState>({ phase: "idle" });
  const activeWorkflow = useInvestigationStore((store) =>
    store.activeWorkflow?.kind === kind ? store.activeWorkflow : undefined,
  );
  const workflowNeedsRecovery = useInvestigationStore((store) => store.workflowNeedsRecovery);
  const recoveryAttempted = useRef<string | undefined>(undefined);
  const canStart = isStartWorkflowAvailable();
  const canResume = isGetWorkflowAvailable();
  const canClose = isCloseWorkflowAvailable();
  const bridgeAvailable = canStart || canResume;
  const isInRouterContext = useInRouterContext();

  useEffect(() => {
    if (
      !activeWorkflow ||
      !workflowNeedsRecovery ||
      !canResume ||
      recoveryAttempted.current === activeWorkflow.workflowId
    ) {
      return;
    }
    recoveryAttempted.current = activeWorkflow.workflowId;
    setState({ phase: "resuming" });
    void recoverWorkspaceWorkflow().then((result) => {
      if (result.status === "ready") {
        setState({
          phase: result.data.currentStep === WORKFLOW_COMPLETE_STEP ? "complete" : "in-progress",
          result: result.data,
        });
      } else if (result.status === "error") {
        setState({ phase: "error", message: result.error });
      } else if (result.status === "incompatible") {
        setState({ phase: "error", message: "Saved workflow is not compatible with this workspace." });
      } else {
        setState({ phase: "idle" });
      }
    });
  }, [activeWorkflow, canResume, workflowNeedsRecovery]);

  async function handleStart() {
    setState({ phase: "starting" });

    const result = await startWorkspaceWorkflow(kind, {
      heapPath,
      objectId: showObjectIdInput && objectId.trim().length > 0 ? objectId.trim() : undefined,
    });

    if (result.status === "unavailable" || result.status === "stale" || result.status === "idle") {
      setState({ phase: "idle" });
      return;
    }

    if (result.status === "error" || result.status === "incompatible") {
      setState({
        phase: "error",
        message:
          result.status === "error"
            ? result.error
            : "Saved workflow is not compatible with this workspace.",
      });
      return;
    }

    setState({
      phase: result.data.currentStep === WORKFLOW_COMPLETE_STEP ? "complete" : "in-progress",
      result: result.data,
    });
  }

  async function handleContinue(current: WorkflowStepResult) {
    setState({ phase: "advancing" });

    let input: unknown;
    if (kind === "classloader_leak" && current.currentStep === "select") {
      const className = firstDuplicateClassName(current.stepResult);
      if (!className) {
        setState({
          phase: "error",
          message:
            "No duplicate class was returned by detect. Open Artifact Explorer → Classloaders to inspect manually.",
        });
        return;
      }
      input = { class_name: className };
    }

    const result = await advanceWorkspaceWorkflow(input);

    if (result.status === "unavailable" || result.status === "stale" || result.status === "idle") {
      setState({ phase: "idle" });
      return;
    }

    if (result.status === "error" || result.status === "incompatible") {
      setState({
        phase: "error",
        message:
          result.status === "error"
            ? result.error
            : "Saved workflow is not compatible with this workspace.",
      });
      return;
    }

    setState({
      phase: result.data.currentStep === WORKFLOW_COMPLETE_STEP ? "complete" : "in-progress",
      result: result.data,
    });
  }

  async function handleClose() {
    setState({ phase: "closing" });
    const result = await closeWorkspaceWorkflow();

    if (result.status === "unavailable") {
      setState({ phase: "idle" });
      return;
    }

    if (result.status === "error") {
      setState({ phase: "error", message: result.error });
      return;
    }

    setState({ phase: "idle" });
  }

  const activeResult =
    state.phase === "in-progress" || state.phase === "complete"
      ? state.result
      : activeWorkflow && !workflowNeedsRecovery && state.phase !== "closing"
        ? {
            workflowId: activeWorkflow.workflowId,
            currentStep: activeWorkflow.currentStep,
            stepResult: null,
            nextExpectedInput: [],
          }
        : undefined;
  const stepLinks =
    activeResult && isInRouterContext
      ? workflowStepLinks(kind, activeResult.currentStep)
      : [];

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
          {showObjectIdInput && canStart ? (
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

          {(state.phase === "idle" || state.phase === "error") && !activeResult ? (
            <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
              {canStart ? (
                <button type="button" style={buttonStyle} onClick={() => void handleStart()} disabled={!heapPath}>
                  Start {title}
                </button>
              ) : null}
            </div>
          ) : null}

          {!heapPath && canStart && state.phase === "idle" ? (
            <p style={{ margin: 0, color: "#64748b", fontSize: "0.82rem" }}>Load an artifact first.</p>
          ) : null}

          {state.phase === "starting" ||
          state.phase === "resuming" ||
          state.phase === "advancing" ||
          state.phase === "closing" ? (
            <p style={{ margin: 0, color: "#94a3b8" }}>Running...</p>
          ) : null}

          {state.phase === "error" ? (
            <p role="alert" style={{ margin: 0, color: "#fca5a5" }}>
              {state.message}
            </p>
          ) : null}

          {activeResult ? (
            <div style={{ display: "grid", gap: "0.4rem" }}>
              <div style={{ color: "#67e8f9", fontSize: "0.85rem" }}>
                Current step: {activeResult.currentStep}
              </div>
              {activeResult.currentStep !== WORKFLOW_COMPLETE_STEP ? (
                <button
                  type="button"
                  style={buttonStyle}
                  onClick={() => void handleContinue(activeResult)}
                >
                  Continue
                </button>
              ) : (
                <div style={{ color: "#86efac", fontSize: "0.85rem" }}>Workflow complete.</div>
              )}
              {canClose ? (
                <button type="button" style={buttonStyle} onClick={() => void handleClose()}>
                  Close workflow
                </button>
              ) : null}
              {stepLinks.length > 0 ? (
                <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
                  {stepLinks.map((link) => (
                    <Link key={`${link.to}-${link.label}`} to={link.to} style={linkStyle}>
                      {link.label}
                    </Link>
                  ))}
                </div>
              ) : null}
            </div>
          ) : null}
        </>
      )}
    </article>
  );
}

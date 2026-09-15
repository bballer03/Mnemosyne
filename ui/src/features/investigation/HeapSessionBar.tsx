import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { getRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import {
  applyOpenedHeap,
  closeInvestigationWorkspace,
  openDesktopHeapLean,
  type OpenHeapPhase,
} from "./workspace-actions";
import { useInvestigationStore, type ActiveOperation } from "./investigation-store";

const phaseLabels: Record<ActiveOperation["status"], string> = {
  accepted: "Accepted",
  opening: "Opening",
  parsing: "Parsing",
  "building-graph": "Building graph",
  "computing-dominators": "Computing dominators",
  analyzing: "Analyzing",
  rendering: "Rendering",
  committing: "Committing",
  cancelling: "Cancelling",
  cancelled: "Cancelled",
  complete: "Complete",
  failed: "Failed",
};

function formatElapsed(elapsedMs: number): string {
  if (elapsedMs < 1_000) {
    return `${elapsedMs}ms`;
  }
  return `${(elapsedMs / 1_000).toFixed(1)}s`;
}

function operationPercent(operation: ActiveOperation): number | undefined {
  if (
    operation.indeterminate ||
    operation.completed === undefined ||
    operation.total === undefined ||
    operation.total <= 0
  ) {
    return undefined;
  }
  return Math.round((operation.completed / operation.total) * 100);
}

function OperationStatus({ operation }: { operation: ActiveOperation }) {
  const percent = operationPercent(operation);
  const shortId = operation.operationId.slice(0, 8);
  const progressText =
    percent === undefined
      ? "Indeterminate"
      : `${percent}% · ${operation.completed?.toLocaleString()} / ${operation.total?.toLocaleString()}${operation.unit ? ` ${operation.unit}` : ""}`;

  return (
    <div style={{ minWidth: "min(360px, 100%)", display: "grid", gap: 4 }}>
      <div style={{ color: "#67e8f9", fontSize: "0.78rem" }}>
        {phaseLabels[operation.status]} · {operation.kind} · op {shortId} ·{" "}
        {formatElapsed(operation.elapsedMs)}
      </div>
      <div
        role="progressbar"
        aria-label={`${operation.kind} operation progress`}
        aria-valuemin={percent === undefined ? undefined : 0}
        aria-valuemax={percent === undefined ? undefined : 100}
        aria-valuenow={percent}
        aria-valuetext={progressText}
        aria-busy={operation.indeterminate ? true : undefined}
        style={{
          height: 6,
          overflow: "hidden",
          borderRadius: 999,
          background: "#1e293b",
        }}
      >
        <div
          style={{
            width: percent === undefined ? "35%" : `${percent}%`,
            height: "100%",
            borderRadius: 999,
            background: "#22d3ee",
          }}
        />
      </div>
      <div style={{ color: "#94a3b8", fontSize: "0.72rem" }}>{progressText}</div>
    </div>
  );
}

function buttonStyle(primary?: boolean) {
  return {
    border: primary ? "1px solid #22d3ee" : "1px solid #334155",
    borderRadius: 8,
    background: primary ? "rgba(34, 211, 238, 0.12)" : "rgba(15, 23, 42, 0.9)",
    color: primary ? "#a5f3fc" : "#e2e8f0",
    padding: "0.35rem 0.75rem",
    fontSize: "0.85rem",
    cursor: "pointer",
  } as const;
}

/**
 * Persistent investigation chrome: heap identity + Open another + Close.
 * Rendered from App whenever a heap/artifact is loaded (all routes).
 */
export function HeapSessionBar() {
  const artifactName = useArtifactStore((s) => s.artifactName);
  const artifact = useArtifactStore((s) => s.artifact);
  const activeOperation = useInvestigationStore((s) => s.activeOperation);
  const navigate = useNavigate();
  const [openPhase, setOpenPhase] = useState<OpenHeapPhase>("idle");
  const [message, setMessage] = useState<string | undefined>();
  const busy = openPhase !== "idle" || activeOperation !== undefined;
  const hasHeap = Boolean(artifactName && artifact);

  if (!hasHeap && !activeOperation) {
    return null;
  }

  const remembered = getRememberedDesktopHeapSource();
  const label = remembered?.displayName ?? artifactName ?? "Opening heap dump";

  async function handleOpenAnother() {
    setMessage(undefined);
    const result = await openDesktopHeapLean(setOpenPhase);
    if (result.status === "cancelled") {
      setMessage("Heap dump selection cancelled.");
      return;
    }
    if (result.status === "unavailable" || result.status === "error") {
      setMessage(result.message);
      return;
    }
    applyOpenedHeap(result.displayName, result.artifact, result.sourceId);
    setMessage(`Opened ${result.displayName}.`);
    navigate("/dashboard");
  }

  async function handleClose() {
    setMessage(undefined);
    setOpenPhase("analyzing");
    try {
      await closeInvestigationWorkspace();
      navigate("/");
    } finally {
      setOpenPhase("idle");
    }
  }

  return (
    <div
      role="region"
      aria-label="Current heap session"
      style={{
        display: "flex",
        flexWrap: "wrap",
        alignItems: "center",
        gap: "0.75rem",
        width: "100%",
        justifyContent: "flex-end",
      }}
    >
      {hasHeap ? (
        <div style={{ textAlign: "right", minWidth: 0 }}>
          <div style={{ fontSize: "0.75rem", color: "#94a3b8", letterSpacing: "0.04em" }}>
            CURRENT HEAP
          </div>
          <div
            style={{
              color: "#e2e8f0",
              fontWeight: 600,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              maxWidth: "280px",
            }}
            title={label}
          >
            {label}
          </div>
          {message ? (
            <div style={{ color: "#fcd34d", fontSize: "0.75rem", marginTop: 2 }}>{message}</div>
          ) : null}
        </div>
      ) : null}
      {activeOperation ? <OperationStatus operation={activeOperation} /> : null}
      {hasHeap ? (
        <>
          <Link to="/" style={{ ...buttonStyle(), textDecoration: "none", display: "inline-block" }}>
            Home
          </Link>
          <button
            type="button"
            style={buttonStyle(true)}
            disabled={busy}
            onClick={() => void handleOpenAnother()}
          >
            Open another
          </button>
          <button
            type="button"
            style={buttonStyle()}
            disabled={busy}
            onClick={() => void handleClose()}
          >
            Close
          </button>
        </>
      ) : null}
    </div>
  );
}

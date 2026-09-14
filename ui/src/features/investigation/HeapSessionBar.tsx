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
  const navigate = useNavigate();
  const [phase, setPhase] = useState<OpenHeapPhase>("idle");
  const [message, setMessage] = useState<string | undefined>();
  const busy = phase !== "idle";

  if (!artifactName || !artifact) {
    return null;
  }

  const remembered = getRememberedDesktopHeapSource();
  const label = remembered?.displayName ?? artifactName;

  async function handleOpenAnother() {
    setMessage(undefined);
    const result = await openDesktopHeapLean(setPhase);
    if (result.status === "cancelled") {
      setMessage("Heap dump selection cancelled.");
      return;
    }
    if (result.status === "unavailable" || result.status === "error") {
      setMessage(result.message);
      return;
    }
    applyOpenedHeap(result.displayName, result.artifact);
    setMessage(`Opened ${result.displayName}.`);
    navigate("/dashboard");
  }

  async function handleClose() {
    setMessage(undefined);
    setPhase("analyzing");
    try {
      await closeInvestigationWorkspace();
      navigate("/");
    } finally {
      setPhase("idle");
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
        {busy ? (
          <div style={{ color: "#67e8f9", fontSize: "0.75rem" }} aria-busy="true">
            {phase === "picking" ? "Opening…" : "Analyzing…"}
          </div>
        ) : null}
      </div>
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
      <button type="button" style={buttonStyle()} disabled={busy} onClick={() => void handleClose()}>
        Close
      </button>
    </div>
  );
}

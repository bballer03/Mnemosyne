import { useMemo, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";

import { getRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import {
  appendBoundedTurn,
  buildRulesModeAnswer,
  displayHeapBasename,
  isProviderChatAvailable,
  type AssistantChatTurn,
} from "./assistant-bridge-client";

const pageStyle = {
  display: "grid",
  gap: "1rem",
  padding: "1.5rem",
} as const;

const panelStyle = {
  border: "1px solid #1e293b",
  borderRadius: 16,
  background: "rgba(15, 23, 42, 0.88)",
  padding: "1rem 1.15rem",
  display: "grid",
  gap: "0.65rem",
} as const;

const aiPanelStyle = {
  ...panelStyle,
  border: "1px solid #7c3aed",
  background: "rgba(46, 16, 101, 0.35)",
} as const;

const factPanelStyle = {
  ...panelStyle,
  border: "1px solid #0e7490",
  background: "rgba(8, 47, 73, 0.45)",
} as const;

function pickDefaultLeakId(
  leaks: Array<{ id: string; suspectScore?: number }>,
): string | undefined {
  if (leaks.length === 0) {
    return undefined;
  }
  return [...leaks].sort((a, b) => (b.suspectScore ?? 0) - (a.suspectScore ?? 0))[0]?.id;
}

/**
 * M23.A investigation session workspace.
 *
 * Composes loaded artifact facts + optional desktop source identity + local
 * rules-mode history. Provider transport is probed only; unavailable states
 * stay explicit. Absolute heap paths never render — basename / opaque sourceId
 * only.
 */
export function InvestigationAssistantPage() {
  const artifact = useArtifactStore((state) => state.artifact);
  const remembered = getRememberedDesktopHeapSource();
  const leaks = artifact?.leaks ?? [];
  const defaultLeakId = pickDefaultLeakId(leaks);

  const [focusLeakId, setFocusLeakId] = useState<string | undefined>(defaultLeakId);
  const [question, setQuestion] = useState("");
  const [history, setHistory] = useState<AssistantChatTurn[]>([]);
  const [providerNotice, setProviderNotice] = useState<string | undefined>();
  const [workflowId] = useState<string | undefined>();
  const [workflowStep] = useState<string | undefined>();

  const focusedLeak = useMemo(
    () => leaks.find((leak) => leak.id === focusLeakId),
    [leaks, focusLeakId],
  );

  const heapDisplayName = displayHeapBasename(
    remembered?.displayName ?? artifact?.summary.heapPath ?? "no heap loaded",
  );

  function handleAsk(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const field = form.elements.namedItem("follow-up");
    const fieldValue =
      field && typeof field === "object" && "value" in field && typeof field.value === "string"
        ? field.value
        : undefined;
    const trimmed = (fieldValue ?? question).trim();
    if (!trimmed) {
      return;
    }

    const turn = buildRulesModeAnswer(trimmed, {
      heapDisplayName,
      sourceId: remembered?.sourceId,
      workflowId,
      workflowStep,
      focusLeakId: focusedLeak?.id,
      focusLeakClassName: focusedLeak?.className,
      focusLeakSeverity: focusedLeak?.severity,
      focusLeakDescription: focusedLeak?.description,
      totalObjects: artifact?.summary.totalObjects,
    });

    setHistory((prev) => appendBoundedTurn(prev, turn));
    setQuestion("");
  }

  function handleTryProvider() {
    if (!isProviderChatAvailable()) {
      setProviderNotice(
        "Provider chat is unavailable without a connected assistant host bridge. Rules mode remains available offline.",
      );
      return;
    }
    setProviderNotice(undefined);
  }

  return (
    <main style={pageStyle}>
      <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
        <Link to="/">Home</Link>
        <Link to="/assistant" aria-current="page">
          Assistant
        </Link>
      </div>

      <p
        style={{
          margin: 0,
          color: "#38bdf8",
          fontSize: "0.78rem",
          letterSpacing: "0.16em",
          textTransform: "uppercase",
        }}
      >
        Guided investigation
      </p>
      <h1 style={{ margin: 0, fontSize: "clamp(1.6rem, 3vw, 2.2rem)" }}>Investigation session</h1>
      <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
        Keep measured heap facts separate from advisory AI text. Rules mode is the offline default;
        deterministic workbench views stay one click away.
      </p>
      <p style={{ margin: 0, color: "#67e8f9" }}>Mode: rules</p>

      <section style={factPanelStyle} aria-label="Measured heap facts">
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Measured heap facts</h2>
        <div>Heap: {heapDisplayName}</div>
        {remembered?.sourceId ? <div>Source id: {remembered.sourceId}</div> : null}
        {typeof artifact?.summary.totalObjects === "number" ? (
          <div>Objects: {artifact.summary.totalObjects}</div>
        ) : (
          <div>Load an artifact or open a desktop heap to populate measured facts.</div>
        )}
        {workflowId ? (
          <div>
            Workflow: {workflowId}
            {workflowStep ? ` · step ${workflowStep}` : ""}
          </div>
        ) : (
          <div>Workflow: none active in this workspace yet</div>
        )}
        {focusedLeak ? (
          <div>
            Focus: {focusedLeak.id} · {focusedLeak.className} · {focusedLeak.severity}
            <div style={{ color: "#cbd5e1", marginTop: "0.25rem" }}>{focusedLeak.description}</div>
          </div>
        ) : (
          <div>Focus: none</div>
        )}
      </section>

      <label style={{ display: "grid", gap: "0.35rem", maxWidth: "28rem" }}>
        <span>Focus leak</span>
        <select
          aria-label="Focus leak"
          value={focusLeakId ?? ""}
          onChange={(event) => setFocusLeakId(event.target.value || undefined)}
          style={{
            background: "#020617",
            color: "#e2e8f0",
            border: "1px solid #334155",
            borderRadius: 10,
            padding: "0.45rem 0.6rem",
          }}
        >
          {leaks.length === 0 ? <option value="">No leaks loaded</option> : null}
          {leaks.map((leak) => (
            <option key={leak.id} value={leak.id}>
              {leak.id} ({leak.severity})
            </option>
          ))}
        </select>
      </label>

      <nav aria-label="Deterministic workbench links" style={{ display: "flex", flexWrap: "wrap", gap: "0.65rem" }}>
        <Link to="/dashboard">Dashboard</Link>
        <Link to="/heap-explorer/object-inspector">Object Inspector</Link>
        <Link to="/heap-explorer/dominators">Dominators</Link>
        <Link to="/heap-explorer/query-console">Query Console</Link>
        {focusLeakId ? (
          <>
            <Link to={`/leaks/${focusLeakId}/overview`}>Leak Workspace</Link>
            <Link to={`/leaks/${focusLeakId}/gc-path`}>GC Path</Link>
          </>
        ) : null}
      </nav>

      <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap", alignItems: "center" }}>
        <button type="button" onClick={handleTryProvider}>
          Try provider mode
        </button>
        {providerNotice ? <span style={{ color: "#facc15" }}>{providerNotice}</span> : null}
      </div>

      <section style={aiPanelStyle} aria-label="AI guidance">
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>AI guidance</h2>
        <p style={{ margin: 0, color: "#c4b5fd", fontSize: "0.9rem" }}>
          Advisory only — visually separated from measured facts above. Each turn carries provenance.
        </p>

        <form onSubmit={handleAsk} style={{ display: "grid", gap: "0.55rem" }}>
          <label style={{ display: "grid", gap: "0.35rem" }}>
            <span>Ask a follow-up</span>
            <input
              name="follow-up"
              aria-label="Ask a follow-up"
              value={question}
              onChange={(event) => setQuestion(event.target.value)}
              placeholder="e.g. What should I investigate first?"
              style={{
                background: "#020617",
                color: "#e2e8f0",
                border: "1px solid #4c1d95",
                borderRadius: 10,
                padding: "0.55rem 0.75rem",
              }}
            />
          </label>
          <button type="submit">Ask</button>
        </form>

        {history.length === 0 ? (
          <p style={{ margin: 0, color: "#a78bfa" }}>No turns yet. Ask a question to start a bounded rules-mode session.</p>
        ) : (
          <ol style={{ margin: 0, paddingLeft: "1.2rem", display: "grid", gap: "0.75rem" }}>
            {history.map((turn, index) => (
              <li key={`${turn.question}-${index}`} style={{ display: "grid", gap: "0.25rem" }}>
                <div style={{ color: "#e9d5ff" }}>Q: {turn.question}</div>
                <div>A: {turn.answerSummary}</div>
                <div style={{ fontSize: "0.8rem", color: "#c4b5fd" }}>
                  Provenance: {turn.provenance} · model {turn.model}
                </div>
              </li>
            ))}
          </ol>
        )}
      </section>
    </main>
  );
}

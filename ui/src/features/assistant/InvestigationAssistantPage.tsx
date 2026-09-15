import { useMemo, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";

import { getRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { useInvestigationStore } from "../investigation/investigation-store";
import {
  appendBoundedTurn,
  askWithProviderFallback,
  buildRulesModeAnswer,
  displayHeapBasename,
  isOpaqueSourceId,
  isProviderChatAvailable,
  OUTBOUND_METADATA_NOTICE,
  type AssistantChatTurn,
  type AssistantProvenance,
  type AssistantSessionContext,
} from "./assistant-bridge-client";
import { buildAssistantMeasuredContext } from "./assistant-context";
import { WORKFLOW_KIND_LABELS } from "../workflow-landing/workflow-types";

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

function modeLabel(provenance: AssistantProvenance | undefined): string {
  if (provenance === "provider") {
    return "provider";
  }
  if (provenance === "fallback") {
    return "fallback (rules)";
  }
  return "rules";
}

/**
 * M23 / M29.C investigation session workspace.
 *
 * Measured facts come only from the revision-stable store selection and
 * immutable findingFacts projection. Rules mode remains the offline default;
 * advisory AI text lives in a collapsible region and never mutates findings.
 */
export function InvestigationAssistantPage() {
  const artifact = useArtifactStore((state) => state.artifact);
  const remembered = getRememberedDesktopHeapSource();
  const leaks = artifact?.leaks ?? [];

  const objectId = useInvestigationStore((state) => state.objectId);
  const classKey = useInvestigationStore((state) => state.classKey);
  const selectedLeakId = useInvestigationStore((state) => state.leakId);
  const originPane = useInvestigationStore((state) => state.originPane);
  const findingFacts = useInvestigationStore((state) => state.findingFacts);
  const activeWorkflow = useInvestigationStore((state) =>
    state.workflowNeedsRecovery ? undefined : state.activeWorkflow,
  );

  const [focusLeakId, setFocusLeakId] = useState<string | undefined>(() =>
    leaks.some((leak) => leak.id === selectedLeakId) ? selectedLeakId : undefined,
  );
  const [question, setQuestion] = useState("");
  const [history, setHistory] = useState<AssistantChatTurn[]>([]);
  const [providerNotice, setProviderNotice] = useState<string | undefined>();
  const [outboundNotice, setOutboundNotice] = useState<string | undefined>();
  const [sessionId, setSessionId] = useState<string | undefined>();
  const [asking, setAsking] = useState(false);

  const selection = useMemo(
    () => ({
      revision: useInvestigationStore.getState().revision,
      objectId,
      classKey,
      leakId: selectedLeakId,
      originPane,
    }),
    [objectId, classKey, selectedLeakId, originPane],
  );

  const measuredFindings = useMemo(
    () => buildAssistantMeasuredContext(selection, findingFacts),
    [selection, findingFacts],
  );

  const heapDisplayName = displayHeapBasename(
    remembered?.displayName ?? artifact?.summary.heapPath ?? "no heap loaded",
  );

  const activeMode = modeLabel(history[history.length - 1]?.provenance);

  function buildSessionContext(): AssistantSessionContext {
    return {
      heapDisplayName,
      sourceId: remembered?.sourceId,
      workflowKind: activeWorkflow
        ? WORKFLOW_KIND_LABELS[activeWorkflow.kind]
        : undefined,
      workflowStep: activeWorkflow?.currentStep,
      selection: {
        objectId,
        classKey,
        leakId: selectedLeakId,
        originPane,
      },
      measuredFindings,
      totalObjects: artifact?.summary.totalObjects,
    };
  }

  async function handleAsk(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (asking) {
      return;
    }
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

    const context = buildSessionContext();
    const findingSnapshot = findingFacts;

    const request = useInvestigationStore
      .getState()
      .beginWorkspaceRequest("assistant");
    setAsking(true);
    try {
      const result = await askWithProviderFallback({
        question: trimmed,
        context,
        sessionId,
        sourceId: remembered?.sourceId,
      });
      if (
        !useInvestigationStore
          .getState()
          .acceptWorkspaceRequest("assistant", request)
      ) {
        return;
      }

      if (result.sessionId) {
        setSessionId(result.sessionId);
      }
      if (result.outboundNotice) {
        setOutboundNotice(result.outboundNotice);
      }
      if (result.notice) {
        setProviderNotice(result.notice);
      } else if (result.turn.provenance === "provider") {
        setProviderNotice(undefined);
      }

      setHistory((prev) => appendBoundedTurn(prev, result.turn));
      setQuestion("");
      // Advisory turns must never mutate measured findingFacts.
      void findingSnapshot;
    } catch (error) {
      if (
        !useInvestigationStore
          .getState()
          .acceptWorkspaceRequest("assistant", request)
      ) {
        return;
      }
      const fallback = {
        ...buildRulesModeAnswer(trimmed, context),
        provenance: "fallback" as const,
      };
      setHistory((prev) => appendBoundedTurn(prev, fallback));
      setProviderNotice(
        `recovery=rules_mode_available; error=${
          error instanceof Error ? error.message : "ask_failed"
        }`,
      );
      setQuestion("");
    } finally {
      useInvestigationStore
        .getState()
        .finishWorkspaceRequest("assistant", request);
      setAsking(false);
    }
  }

  function handleCheckProvider() {
    if (!isProviderChatAvailable()) {
      setProviderNotice(
        "Provider chat is unavailable without a connected assistant host bridge. Rules mode remains available offline.",
      );
      return;
    }
    setProviderNotice(
      "Assistant host bridge detected. Ask uses chatSession with provider provenance; rules mode remains the offline fallback on error.",
    );
    if (!outboundNotice) {
      setOutboundNotice(OUTBOUND_METADATA_NOTICE);
    }
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
      <p style={{ margin: 0, color: "#67e8f9" }}>Mode: {activeMode}</p>

      <section style={factPanelStyle} aria-label="Measured heap facts">
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Measured heap facts</h2>
        <div>Heap: {heapDisplayName}</div>
        {remembered?.sourceId && isOpaqueSourceId(remembered.sourceId) ? (
          <div>Source id: {remembered.sourceId.slice(0, 8)}…</div>
        ) : null}
        {typeof artifact?.summary.totalObjects === "number" ? (
          <div>Objects: {artifact.summary.totalObjects}</div>
        ) : (
          <div>Load an artifact or open a desktop heap to populate measured facts.</div>
        )}
        {activeWorkflow ? (
          <div>
            Workflow: {WORKFLOW_KIND_LABELS[activeWorkflow.kind]} · current step:{" "}
            {activeWorkflow.currentStep}
          </div>
        ) : (
          <div>Workflow: none active in this workspace yet</div>
        )}
        <div>
          Selection:
          {selectedLeakId ? ` leak ${selectedLeakId}` : ""}
          {objectId ? ` object ${objectId}` : ""}
          {classKey ? ` class ${classKey}` : ""}
          {!selectedLeakId && !objectId && !classKey ? " none" : ""}
          {originPane ? ` (from ${originPane})` : ""}
        </div>
        {measuredFindings.length === 0 ? (
          <div>Matched findings: none for the current selection.</div>
        ) : (
          <ul style={{ margin: 0, paddingLeft: "1.1rem", display: "grid", gap: "0.45rem" }}>
            {measuredFindings.map((finding) => (
              <li key={finding.id}>
                <div>
                  {finding.title} · {finding.severity}
                </div>
                <div style={{ color: "#cbd5e1" }}>{finding.description}</div>
                <div style={{ fontSize: "0.8rem", color: "#7dd3fc" }}>
                  Provenance:{" "}
                  {finding.provenance
                    .map((marker) =>
                      marker.detail ? `${marker.kind} (${marker.detail})` : marker.kind,
                    )
                    .join(", ")}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      <label style={{ display: "grid", gap: "0.35rem", maxWidth: "28rem" }}>
        <span>Focus leak</span>
        <select
          aria-label="Focus leak"
          value={focusLeakId ?? ""}
          onChange={(event) => {
            const nextLeakId = event.target.value || undefined;
            setFocusLeakId(nextLeakId);
            const investigation = useInvestigationStore.getState();
            investigation.clearSelection();
            if (nextLeakId) {
              investigation.setLeakId(nextLeakId, "leak");
              const nextLeak = leaks.find((leak) => leak.id === nextLeakId);
              if (nextLeak) {
                investigation.setClassKey(nextLeak.className, "leak");
              }
            }
          }}
          style={{
            background: "#020617",
            color: "#e2e8f0",
            border: "1px solid #334155",
            borderRadius: 10,
            padding: "0.45rem 0.6rem",
          }}
        >
          <option value="">No leak selected</option>
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
        {selectedLeakId ? (
          <>
            <Link to={`/leaks/${selectedLeakId}/overview`}>Leak Workspace</Link>
            <Link to={`/leaks/${selectedLeakId}/gc-path`}>GC Path</Link>
          </>
        ) : null}
      </nav>

      <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap", alignItems: "center" }}>
        <button type="button" onClick={handleCheckProvider}>
          Check provider availability
        </button>
        {providerNotice ? <span style={{ color: "#facc15" }}>{providerNotice}</span> : null}
      </div>

      {(outboundNotice || isProviderChatAvailable()) && (
        <p
          aria-label="Outbound metadata notice"
          style={{ margin: 0, color: "#94a3b8", fontSize: "0.85rem", maxWidth: "68ch" }}
        >
          {outboundNotice ?? OUTBOUND_METADATA_NOTICE}
        </p>
      )}

      <details open style={aiPanelStyle}>
        <summary style={{ cursor: "pointer", fontSize: "1.05rem", fontWeight: 600 }}>
          AI guidance
        </summary>
        <section aria-label="AI guidance" style={{ display: "grid", gap: "0.65rem", marginTop: "0.65rem" }}>
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
                disabled={asking}
                style={{
                  background: "#020617",
                  color: "#e2e8f0",
                  border: "1px solid #4c1d95",
                  borderRadius: 10,
                  padding: "0.55rem 0.75rem",
                }}
              />
            </label>
            <button type="submit" disabled={asking}>
              Ask
            </button>
          </form>

          {history.length === 0 ? (
            <p style={{ margin: 0, color: "#a78bfa" }}>
              No turns yet. Ask a question to start a bounded session (rules offline, provider when
              the host bridge is connected).
            </p>
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
      </details>
    </main>
  );
}

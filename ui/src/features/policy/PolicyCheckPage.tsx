import { useState } from "react";
import { Link } from "react-router-dom";

import {
  ensureDesktopHeapSource,
  isCiCheckAvailable,
  pickDesktopBaselineSource,
  runCiCheck,
} from "./policy-bridge-client";

const DEFAULT_POLICY = `[[rule]]
id = "leak-budget"
predicate = "leak_count"
op = "<="
value = 0
severity = "error"
mode_requirement = "deep_only"
`;

export function PolicyCheckPage() {
  const [policyToml, setPolicyToml] = useState(DEFAULT_POLICY);
  const [failOn, setFailOn] = useState<"info" | "warning" | "error" | "critical">("error");
  const [mode, setMode] = useState<"auto" | "deep" | "overview">("deep");
  const [status, setStatus] = useState<string>("Ready.");
  const [running, setRunning] = useState(false);
  const [pickingBaseline, setPickingBaseline] = useState(false);
  const [baseline, setBaseline] = useState<{ sourceId: string; displayName: string }>();
  const [response, setResponse] = useState<Awaited<ReturnType<typeof runCiCheck>> | undefined>();
  const requiresBaseline =
    /^\s*predicate\s*=\s*["']object_growth_threshold["']\s*(?:#.*)?$/m.test(policyToml);

  async function handleRun() {
    setRunning(true);
    setResponse(undefined);
    try {
      if (!isCiCheckAvailable()) {
        setStatus("Policy check requires the desktop host bridge.");
        setResponse({ status: "unavailable" });
        return;
      }

      if (requiresBaseline && !baseline) {
        setStatus("Select a baseline heap before evaluating object_growth_threshold rules.");
        return;
      }

      const source = await ensureDesktopHeapSource();
      if (source.status === "cancelled") {
        setStatus("Heap selection cancelled.");
        return;
      }
      if (source.status === "unavailable") {
        setStatus("Open a heap dump in the desktop app first (Home → Open heap dump).");
        return;
      }
      if (source.status === "error") {
        setStatus(source.error);
        return;
      }

      setStatus(`Evaluating policy against ${source.displayName}…`);
      const result = await runCiCheck({
        sourceId: source.sourceId,
        policyToml,
        failOn,
        mode,
        baselineSourceId: baseline?.sourceId,
      });
      setResponse(result);
      if (result.status === "ready") {
        setStatus(
          `Finished with exit classification ${result.data.exit_code} (fail_on=${result.data.fail_on}).`,
        );
      } else if (result.status === "error") {
        setStatus(result.error);
      } else {
        setStatus("Policy check unavailable.");
      }
    } finally {
      setRunning(false);
    }
  }

  async function handlePickBaseline() {
    setPickingBaseline(true);
    try {
      const result = await pickDesktopBaselineSource();
      if (result.status === "selected") {
        setBaseline({ sourceId: result.sourceId, displayName: result.displayName });
        setStatus(`Baseline selected: ${result.displayName}`);
      } else if (result.status === "cancelled") {
        setStatus("Baseline selection cancelled.");
      } else if (result.status === "unavailable") {
        setStatus("Baseline picker is unavailable outside the desktop host.");
      } else {
        setStatus(result.error);
      }
    } finally {
      setPickingBaseline(false);
    }
  }

  const ready = response?.status === "ready" ? response.data : undefined;

  return (
    <main style={{ display: "grid", gap: "1rem", padding: "1.5rem" }}>
      <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
        <Link to="/">Home</Link>
        <Link to="/workbench/policies" aria-current="page">
          Policies
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
        Workbench
      </p>
      <h1 style={{ margin: 0, fontSize: "clamp(1.6rem, 3vw, 2.2rem)" }}>Policy check</h1>
      <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
        Evaluate an inline TOML policy against the remembered desktop heap source via the shipped{" "}
        <code>ci_check</code> semantics. Skipped deep-only rules are listed separately from pass/fail.
      </p>

      <label style={{ display: "grid", gap: "0.35rem" }}>
        <span>Policy TOML</span>
        <textarea
          value={policyToml}
          onChange={(event) => setPolicyToml(event.target.value)}
          rows={12}
          style={{
            fontFamily: "ui-monospace, monospace",
            background: "#020617",
            color: "#e2e8f0",
            border: "1px solid #1e293b",
            borderRadius: 12,
            padding: "0.75rem",
          }}
        />
      </label>

      <section
        aria-label="Policy baseline"
        style={{
          border: "1px solid #1e293b",
          borderRadius: 12,
          padding: "0.85rem",
          display: "grid",
          gap: "0.5rem",
        }}
      >
        <strong>Baseline heap {requiresBaseline ? "(required by this policy)" : "(optional)"}</strong>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Object-growth rules compare the current heap with a separately selected baseline. Only the
          host&apos;s opaque source ID is submitted.
        </p>
        <div style={{ display: "flex", gap: "0.75rem", alignItems: "center", flexWrap: "wrap" }}>
          <button type="button" disabled={pickingBaseline} onClick={() => void handlePickBaseline()}>
            {pickingBaseline ? "Selecting baseline…" : "Select baseline heap"}
          </button>
          {baseline ? <span>Selected: {baseline.displayName}</span> : <span>No baseline selected.</span>}
        </div>
      </section>

      <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap" }}>
        <label>
          fail_on{" "}
          <select value={failOn} onChange={(event) => setFailOn(event.target.value as typeof failOn)}>
            <option value="info">info</option>
            <option value="warning">warning</option>
            <option value="error">error</option>
            <option value="critical">critical</option>
          </select>
        </label>
        <label>
          mode{" "}
          <select value={mode} onChange={(event) => setMode(event.target.value as typeof mode)}>
            <option value="deep">deep</option>
            <option value="overview">overview</option>
            <option value="auto">auto</option>
          </select>
        </label>
        <button type="button" onClick={() => void handleRun()} disabled={running}>
          {running ? "Running…" : "Run policy check"}
        </button>
      </div>

      <p role="status" style={{ margin: 0, color: "#cbd5e1" }}>
        {status}
      </p>

      {ready ? (
        <section style={{ display: "grid", gap: "1rem" }}>
          <div style={{ color: "#94a3b8" }}>
            mode_used={ready.result.mode_used} · exit_code={ready.exit_code} · violations=
            {ready.result.violations.length} · skipped={ready.result.skipped.length} ·{" "}
            {ready.evaluation_complete ? "complete" : "incomplete"}
          </div>
          {!ready.evaluation_complete ? (
            <p role="note" style={{ color: "#facc15", margin: 0 }}>
              Incomplete evaluation: skipped rules were not proven. Exit code 0 is not a green pass.
            </p>
          ) : null}
          {ready.result.skipped.length > 0 ? (
            <div>
              <strong>Skipped (not a green pass)</strong>
              <ul>
                {ready.result.skipped.map((rule) => (
                  <li key={rule.rule_id}>
                    {rule.rule_id}: {typeof rule.reason === "string" ? rule.reason : JSON.stringify(rule.reason)}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {ready.result.violations.length > 0 ? (
            <div>
              <strong>Violations</strong>
              <ul>
                {ready.result.violations.map((violation) => (
                  <li key={`${violation.rule_id}-${violation.message}`}>
                    [{violation.severity}] {violation.rule_id}: {violation.message}
                  </li>
                ))}
              </ul>
            </div>
          ) : ready.evaluation_complete ? (
            <p style={{ color: "#86efac" }}>No violations at or above the evaluated rules.</p>
          ) : (
            <p style={{ color: "#facc15" }}>
              No violations among rules that ran; skipped deep-only rules remain unproven.
            </p>
          )}
        </section>
      ) : null}
    </main>
  );
}

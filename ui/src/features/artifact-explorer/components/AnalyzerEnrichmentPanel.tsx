import { useMemo, useState } from "react";

import {
  runAnalyzerEnrichment,
  type AnalyzerEnrichmentResult,
  type AnalyzerSelection,
} from "../analyzer-enrichment-client";

type AnalyzerEnrichmentPanelProps = {
  runEnrichment?: (selection: AnalyzerSelection) => Promise<AnalyzerEnrichmentResult>;
};

const initialSelection: AnalyzerSelection = {
  strings: false,
  collections: false,
  duplicateArrays: false,
  threads: false,
  referrers: false,
  classloaders: false,
};

const options = [
  { key: "strings", label: "Strings", fieldData: true },
  { key: "collections", label: "Collections", fieldData: true },
  { key: "duplicateArrays", label: "Duplicate primitive arrays", fieldData: true },
  { key: "threads", label: "Threads", fieldData: true },
  { key: "referrers", label: "Referrers", fieldData: false },
  { key: "classloaders", label: "Classloaders", fieldData: false },
] as const satisfies ReadonlyArray<{
  key: keyof AnalyzerSelection;
  label: string;
  fieldData: boolean;
}>;

function ResultSummary({ result }: { result: AnalyzerEnrichmentResult }) {
  if (result.status === "stale") {
    return (
      <p role="status" style={{ margin: 0, color: "#facc15" }}>
        Enrichment result was ignored because the workspace changed while it was running.
      </p>
    );
  }
  if (result.status === "unavailable" || result.status === "error") {
    return (
      <p role={result.status === "error" ? "alert" : "status"} style={{ margin: 0, color: "#fca5a5" }}>
        {result.message}
      </p>
    );
  }

  return (
    <div role="status" style={{ display: "grid", gap: "0.45rem", color: "#cbd5e1" }}>
      <strong>Enrichment committed to this workspace revision.</strong>
      <ul style={{ margin: 0, paddingLeft: "1.2rem" }}>
        {result.requested.map((label) => (
          <li key={label}>
            {label}: {result.unavailable.includes(label) ? "UNAVAILABLE" : "AVAILABLE"}
          </li>
        ))}
      </ul>
      {result.provenance.length > 0 ? (
        <ul style={{ margin: 0, paddingLeft: "1.2rem", color: "#facc15" }}>
          {result.provenance.map((marker, index) => (
            <li key={`${marker.kind}-${index}`}>
              {marker.kind}: {marker.detail ?? "No additional detail supplied."}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

export function AnalyzerEnrichmentPanel({
  runEnrichment = runAnalyzerEnrichment,
}: AnalyzerEnrichmentPanelProps) {
  const [selection, setSelection] = useState<AnalyzerSelection>(initialSelection);
  const [confirmingFieldData, setConfirmingFieldData] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<AnalyzerEnrichmentResult>();
  const selectedOptions = useMemo(
    () => options.filter((option) => selection[option.key]),
    [selection],
  );
  const hasFieldDataSelection = selectedOptions.some((option) => option.fieldData);

  async function submit() {
    setRunning(true);
    setResult(undefined);
    try {
      setResult(await runEnrichment(selection));
    } finally {
      setRunning(false);
      setConfirmingFieldData(false);
    }
  }

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>On-demand analyzers</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Enrich the current workspace with one host analysis request. First open stays lean; nothing
          below runs until selected.
        </p>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "0.55rem" }}>
        {options.map((option) => (
          <label key={option.key} style={{ display: "flex", gap: "0.5rem", alignItems: "start", color: "#cbd5e1" }}>
            <input
              type="checkbox"
              checked={selection[option.key]}
              onChange={(event) => {
                setSelection((current) => ({ ...current, [option.key]: event.target.checked }));
                setConfirmingFieldData(false);
              }}
            />
            <span>
              {option.label}
              <small style={{ display: "block", color: "#64748b" }}>
                {option.fieldData ? "Retains field data" : "Deep graph only"}
              </small>
            </span>
          </label>
        ))}
      </div>

      {selectedOptions.length > 0 ? (
        <div
          style={{
            border: "1px solid #334155",
            borderRadius: 12,
            background: "rgba(2, 6, 23, 0.72)",
            padding: "0.75rem",
            display: "grid",
            gap: "0.4rem",
          }}
        >
          <strong>Cost preview</strong>
          <p style={{ margin: 0, color: "#cbd5e1", lineHeight: 1.55 }}>
            The host will reparse the current heap for this deep analyzer pass.
          </p>
          {selectedOptions.map((option) => (
            <p key={option.key} style={{ margin: 0, color: "#94a3b8" }}>
              {option.label} {option.fieldData ? "requires retained field data." : "uses the deep graph without retained field data."}
            </p>
          ))}
          {hasFieldDataSelection ? (
            <p role="note" style={{ margin: 0, color: "#facc15", lineHeight: 1.55 }}>
              Retaining per-object field and primitive-array bytes can materially increase peak memory and duration.
            </p>
          ) : null}
        </div>
      ) : null}

      <div style={{ display: "flex", gap: "0.65rem", flexWrap: "wrap" }}>
        <button
          type="button"
          disabled={running || selectedOptions.length === 0}
          onClick={() => {
            if (hasFieldDataSelection) {
              setConfirmingFieldData(true);
            } else {
              void submit();
            }
          }}
        >
          {running ? "Running enrichment…" : hasFieldDataSelection ? "Review enrichment cost" : "Run enrichment"}
        </button>
        {confirmingFieldData ? (
          <button type="button" disabled={running} onClick={() => void submit()}>
            Confirm and run enrichment
          </button>
        ) : null}
      </div>

      {result ? <ResultSummary result={result} /> : null}
    </div>
  );
}

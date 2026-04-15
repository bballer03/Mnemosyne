import { useState } from "react";

import { isHeapQueryAvailable, runHeapQuery } from "../heap-explorer-query-client";

type QueryConsolePanelProps = {
  heapPath: string;
};

export function QueryConsolePanel({ heapPath }: QueryConsolePanelProps) {
  const [queryText, setQueryText] = useState("SELECT object_id, class_name LIMIT 20");
  const [result, setResult] = useState<Awaited<ReturnType<typeof runHeapQuery>> | undefined>(() =>
    isHeapQueryAvailable() ? undefined : { status: "unavailable" },
  );
  const [isRunning, setIsRunning] = useState(false);

  async function handleRunQuery() {
    if (isRunning) {
      return;
    }

    setIsRunning(true);

    try {
      setResult(await runHeapQuery({ heapPath, query: queryText }));
    } finally {
      setIsRunning(false);
    }
  }

  return (
    <section style={{ display: "grid", gap: "0.9rem" }}>
      <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Query Console</h2>
      <textarea
        aria-label="Heap query"
        value={queryText}
        onChange={(event) => setQueryText(event.target.value)}
        rows={10}
        style={{
          width: "100%",
          minHeight: "14rem",
          borderRadius: 16,
          border: "1px solid #334155",
          background: "rgba(15, 23, 42, 0.7)",
          color: "#e2e8f0",
          padding: "0.85rem",
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace",
          fontSize: "0.95rem",
        }}
      />
      <div>
        <button type="button" onClick={handleRunQuery} disabled={isRunning}>
          {isRunning ? "Running Query..." : "Run Query"}
        </button>
      </div>
      {result?.status === "unavailable" ? <p>Query execution is unavailable in this browser session.</p> : null}
      {result?.status === "error" ? <p>{result.error}</p> : null}
      {result?.status === "ready" ? (
        <table>
          <thead>
            <tr>
              {result.data.columns.map((column) => (
                <th key={column}>{column}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {result.data.rows.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {row.map((cell, cellIndex) => (
                  <td key={`${rowIndex}-${cellIndex}`}>{String(cell)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </section>
  );
}

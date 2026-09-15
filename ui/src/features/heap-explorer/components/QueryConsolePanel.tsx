import { useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useInvestigationStore } from "../../investigation/investigation-store";
import { isHeapQueryAvailable, runHeapQuery } from "../heap-explorer-query-client";

type QueryConsolePanelProps = {
  heapPath: string;
};

const QUERY_HISTORY_LIMIT = 10;
const DEFAULT_QUERY = 'SELECT @objectId, @className FROM "*" LIMIT 20';

const VETTED_QUERY_EXAMPLES = [
  {
    label: "Largest retained objects",
    query:
      'SELECT @objectId, @className, @retainedSize FROM "*" WHERE @retainedSize > 1048576 LIMIT 50',
  },
  {
    label: "Strings with matching content",
    query:
      'SELECT @objectId, @toString FROM "java.lang.String" WHERE @toString LIKE \'cache%\' LIMIT 50',
  },
  {
    label: "Objects referenced by a field",
    query: 'SELECT OBJECTS n.parent FROM "com.example.Node" WHERE n.parent IS NOT NULL LIMIT 50',
  },
  {
    label: "Outbound references",
    query: "SELECT @objectId, @className FROM outbounds(1) LIMIT 50",
  },
] as const;

const SUPPORTED_SYNTAX = [
  "SELECT fields, *, OBJECTS paths, and DISTINCT OBJECTS",
  "exact/glob classes and multi-class FROM (up to 8 patterns)",
  "WHERE comparisons, LIKE, CONTAINS, =~, IS NULL, AND, and OR",
  "@objectId, @className, @shallowSize, @retainedSize, @toString, and @gcRootPath",
  "outbounds(id), inbounds(id), and dominators(id)",
  "one subquery level, one UNION, and OBJECTS paths up to 3 field hops",
] as const;

const NAMED_DEFERRALS = [
  "eval(...) expressions",
  "arbitrary-depth subquery nesting",
  "more than 8 class patterns in FROM",
  "multiple chained UNION clauses",
  "OBJECTS traversal beyond 3 field hops",
] as const;

function isObjectIdColumn(column: string): boolean {
  return column.replace(/^@/, "").replace(/_/g, "").toLowerCase() === "objectid";
}

function isObjectIdCell(value: unknown): value is string {
  return typeof value === "string" && /^(?:0x)?[0-9a-f]+$/i.test(value);
}

export function QueryConsolePanel({ heapPath }: QueryConsolePanelProps) {
  const navigate = useNavigate();
  const [queryText, setQueryText] = useState(DEFAULT_QUERY);
  const [history, setHistory] = useState<string[]>([]);
  const [result, setResult] = useState<Awaited<ReturnType<typeof runHeapQuery>> | undefined>(() =>
    isHeapQueryAvailable() ? undefined : { status: "unavailable" },
  );
  const [isRunning, setIsRunning] = useState(false);
  const requestSequence = useRef(0);

  async function handleRunQuery() {
    if (isRunning) {
      return;
    }

    const query = queryText.trim();
    if (!query) {
      return;
    }

    const sequence = requestSequence.current + 1;
    requestSequence.current = sequence;
    const requestWorkspace = useInvestigationStore.getState();
    const requestIdentity = {
      workspaceId: requestWorkspace.workspaceId,
      revision: requestWorkspace.revision,
    };
    setHistory((current) =>
      [query, ...current.filter((entry) => entry !== query)].slice(0, QUERY_HISTORY_LIMIT),
    );
    setIsRunning(true);

    try {
      const response = await runHeapQuery({ heapPath, query });
      const currentWorkspace = useInvestigationStore.getState();
      if (
        sequence === requestSequence.current &&
        currentWorkspace.workspaceId === requestIdentity.workspaceId &&
        currentWorkspace.revision === requestIdentity.revision
      ) {
        setResult(response);
      }
    } finally {
      if (sequence === requestSequence.current) {
        setIsRunning(false);
      }
    }
  }

  function openObjectInInspector(objectId: string) {
    useInvestigationStore.getState().setObjectId(objectId, "inspector");
    navigate(`/heap-explorer/object-inspector?objectId=${encodeURIComponent(objectId)}`);
  }

  return (
    <section style={{ display: "grid", gap: "0.9rem" }}>
      <h2 style={{ margin: 0, fontSize: "1.05rem" }}>OQL Workbench</h2>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(18rem, 1fr))",
          gap: "1rem",
          alignItems: "start",
        }}
      >
        <div style={{ display: "grid", gap: "0.75rem" }}>
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
              fontFamily:
                "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, monospace",
              fontSize: "0.95rem",
            }}
          />
          <div>
            <button type="button" onClick={handleRunQuery} disabled={isRunning || !queryText.trim()}>
              {isRunning ? "Running Query..." : "Run Query"}
            </button>
          </div>
          <section aria-label="Vetted query examples" style={{ display: "grid", gap: "0.45rem" }}>
            <h3 style={{ margin: 0, fontSize: "0.95rem" }}>Vetted examples</h3>
            <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
              {VETTED_QUERY_EXAMPLES.map((example) => (
                <button
                  key={example.label}
                  type="button"
                  onClick={() => setQueryText(example.query)}
                >
                  {example.label}
                </button>
              ))}
            </div>
          </section>
          <section aria-label="Query history" style={{ display: "grid", gap: "0.45rem" }}>
            <h3 style={{ margin: 0, fontSize: "0.95rem" }}>
              Recent queries <span style={{ color: "#94a3b8" }}>(last {QUERY_HISTORY_LIMIT})</span>
            </h3>
            {history.length === 0 ? (
              <p style={{ margin: 0, color: "#94a3b8" }}>No queries run in this workbench yet.</p>
            ) : (
              <div style={{ display: "grid", gap: "0.35rem" }}>
                {history.map((query) => (
                  <button
                    key={query}
                    type="button"
                    title={query}
                    onClick={() => setQueryText(query)}
                    style={{ textAlign: "left", overflowWrap: "anywhere" }}
                  >
                    {query}
                  </button>
                ))}
              </div>
            )}
          </section>
        </div>

        <aside style={{ display: "grid", gap: "0.9rem" }}>
          <section aria-label="Supported OQL syntax">
            <h3 style={{ margin: "0 0 0.45rem", fontSize: "0.95rem" }}>Supported syntax</h3>
            <ul style={{ margin: 0, paddingLeft: "1.25rem", color: "#cbd5e1" }}>
              {SUPPORTED_SYNTAX.map((entry) => (
                <li key={entry}>{entry}</li>
              ))}
            </ul>
          </section>
          <section aria-label="Named OQL deferrals">
            <h3 style={{ margin: "0 0 0.45rem", fontSize: "0.95rem" }}>Named deferrals</h3>
            <ul style={{ margin: 0, paddingLeft: "1.25rem", color: "#fbbf24" }}>
              {NAMED_DEFERRALS.map((entry) => (
                <li key={entry}>{entry}</li>
              ))}
            </ul>
          </section>
        </aside>
      </div>
      {result?.status === "unavailable" ? <p>Query execution is unavailable in this browser session.</p> : null}
      {result?.status === "error" ? (
        <div role="alert" style={{ color: "#fca5a5" }}>
          <p style={{ margin: 0 }}>{result.error}</p>
          {result.location ? (
            <p style={{ margin: "0.35rem 0 0" }}>
              Line {result.location.line}, column {result.location.column}
            </p>
          ) : null}
        </div>
      ) : null}
      {result?.status === "ready" ? (
        <div style={{ overflowX: "auto" }}>
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
                  {row.map((cell, cellIndex) => {
                    const column = result.data.columns[cellIndex] ?? "";
                    return (
                      <td key={`${rowIndex}-${cellIndex}`}>
                        {isObjectIdColumn(column) && isObjectIdCell(cell) ? (
                          <button
                            type="button"
                            onClick={() => openObjectInInspector(cell)}
                            aria-label={`Inspect object ${cell}`}
                          >
                            {cell}
                          </button>
                        ) : (
                          String(cell)
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  );
}

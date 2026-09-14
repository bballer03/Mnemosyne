import { useEffect, useState } from "react";
import { Link } from "react-router-dom";

import {
  isListSnapshotsAvailable,
  runListSnapshots,
  type SnapshotManifest,
} from "../workflow-landing/workflow-bridge-client";

export function SnapshotManagerPage() {
  const [snapshots, setSnapshots] = useState<SnapshotManifest[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "unavailable" | "error">("loading");
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    let cancelled = false;

    async function load() {
      if (!isListSnapshotsAvailable()) {
        if (!cancelled) {
          setStatus("unavailable");
        }
        return;
      }

      const result = await runListSnapshots();
      if (cancelled) {
        return;
      }

      if (result.status === "unavailable") {
        setStatus("unavailable");
        return;
      }

      if (result.status === "error") {
        setStatus("error");
        setError(result.error);
        return;
      }

      setSnapshots(result.data);
      setStatus("ready");
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main style={{ display: "grid", gap: "1rem", padding: "1.5rem" }}>
      <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
        <Link to="/">Home</Link>
        <Link to="/workbench/snapshots" aria-current="page">
          Snapshots
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
      <h1 style={{ margin: 0, fontSize: "clamp(1.6rem, 3vw, 2.2rem)" }}>Snapshots</h1>
      <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>
        Cached heap snapshot manifests from the desktop session store. Save/remove controls land in a
        follow-up slice; listing uses the shipped <code>list_snapshots</code> bridge.
      </p>

      {status === "loading" ? <p style={{ color: "#94a3b8" }}>Loading snapshots…</p> : null}
      {status === "unavailable" ? (
        <p role="status" style={{ color: "#facc15" }}>
          Snapshot listing is unavailable outside the desktop host bridge.
        </p>
      ) : null}
      {status === "error" ? (
        <p role="alert" style={{ color: "#fca5a5" }}>
          {error}
        </p>
      ) : null}
      {status === "ready" && snapshots.length === 0 ? (
        <p style={{ color: "#94a3b8" }}>No snapshots are cached in this session yet.</p>
      ) : null}
      {status === "ready" && snapshots.length > 0 ? (
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr style={{ textAlign: "left", color: "#94a3b8" }}>
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Heap</th>
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Created</th>
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Objects</th>
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>SHA-256</th>
              </tr>
            </thead>
            <tbody>
              {snapshots.map((snapshot) => (
                <tr key={`${snapshot.heapSha256}-${snapshot.createdAt}`}>
                  <td
                    style={{
                      padding: "0.55rem 0.6rem 0.55rem 0",
                      borderTop: "1px solid #1e293b",
                      overflowWrap: "anywhere",
                    }}
                  >
                    {snapshot.heapPath}
                  </td>
                  <td style={{ padding: "0.55rem 0.6rem 0.55rem 0", borderTop: "1px solid #1e293b" }}>
                    {snapshot.createdAt}
                  </td>
                  <td style={{ padding: "0.55rem 0.6rem 0.55rem 0", borderTop: "1px solid #1e293b" }}>
                    {snapshot.objectCount.toLocaleString()}
                  </td>
                  <td
                    style={{
                      padding: "0.55rem 0.6rem 0.55rem 0",
                      borderTop: "1px solid #1e293b",
                      fontFamily: "ui-monospace, monospace",
                      fontSize: "0.82rem",
                      color: "#94a3b8",
                    }}
                  >
                    {snapshot.heapSha256.slice(0, 12)}…
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </main>
  );
}

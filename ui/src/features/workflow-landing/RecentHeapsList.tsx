import { useEffect, useRef, useState } from "react";

import { applyOpenedSnapshotSession } from "../investigation/workspace-actions";
import {
  isListSnapshotsAvailable,
  isOpenSnapshotAvailable,
  runListSnapshots,
  runOpenSnapshot,
  type SnapshotManifest,
} from "./workflow-bridge-client";

type ListState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; entries: SnapshotManifest[] }
  | { status: "error"; message: string };

const sectionStyle = {
  border: "1px solid #1e293b",
  borderRadius: 20,
  background: "rgba(15, 23, 42, 0.88)",
  padding: "1.25rem",
} as const;

/**
 * Design doc §4 item 7: "Recent heaps" list backed by the M9 `list_snapshots`
 * MCP tool. Per the item's own scope note -- "when unavailable (browser-only,
 * no live MCP connection), the guided landing simply omits this section
 * rather than showing an empty/broken one" -- this component renders nothing
 * at all (`null`) when `isListSnapshotsAvailable()` is false, rather than an
 * empty-state placeholder. This mirrors every other optional-bridge-gated UI
 * piece in this codebase (e.g. `LeakGcPathPage`'s multi-path section).
 */
export function RecentHeapsList() {
  const available = isListSnapshotsAvailable();
  const openAvailable = isOpenSnapshotAvailable();
  const [state, setState] = useState<ListState>({ status: "idle" });
  const [actionStatus, setActionStatus] = useState<string>();
  const [busyKey, setBusyKey] = useState<string>();
  const requestedRef = useRef(false);

  useEffect(() => {
    if (!available || requestedRef.current) {
      return;
    }

    requestedRef.current = true;
    setState({ status: "loading" });

    void runListSnapshots().then((result) => {
      if (result.status === "unavailable") {
        setState({ status: "idle" });
        return;
      }

      if (result.status === "error") {
        setState({ status: "error", message: result.error });
        return;
      }

      setState({ status: "ready", entries: result.data });
    });
  }, [available]);

  if (!available) {
    return null;
  }

  async function handleOpen(entry: SnapshotManifest) {
    if (!openAvailable) {
      setActionStatus("Opening snapshots requires the desktop host bridge.");
      return;
    }

    setBusyKey(entry.heapSha256);
    setActionStatus(`Opening ${entry.heapPath}…`);
    try {
      const result = await runOpenSnapshot(entry.heapSha256);
      if (result.status === "unavailable") {
        setActionStatus("Opening snapshots requires the desktop host bridge.");
        return;
      }
      if (result.status === "error") {
        setActionStatus(result.error);
        return;
      }

      applyOpenedSnapshotSession(
        result.data.displayName,
        result.data.sourceId,
        result.data.objectCount,
      );
      setActionStatus(
        `Opened ${result.data.displayName} (${result.data.objectCount.toLocaleString()} objects). ` +
          "Snapshot graph is active; prior artifact views were cleared because this bridge does not return an analysis artifact.",
      );
    } finally {
      setBusyKey(undefined);
    }
  }

  return (
    <section style={sectionStyle} aria-label="Recent heaps">
      <h4 style={{ marginTop: 0 }}>Recent Heaps</h4>
      <p style={{ marginTop: 0, color: "#94a3b8" }}>Snapshots cached by the connected Mnemosyne host.</p>
      {actionStatus ? (
        <p role="status" style={{ color: "#cbd5e1" }}>
          {actionStatus}
        </p>
      ) : null}
      {!openAvailable ? (
        <p style={{ color: "#facc15" }}>Opening snapshots requires the desktop host bridge.</p>
      ) : null}

      {state.status === "loading" ? <p style={{ color: "#94a3b8" }}>Loading snapshots...</p> : null}
      {state.status === "error" ? (
        <p role="alert" style={{ color: "#fca5a5" }}>
          {state.message}
        </p>
      ) : null}
      {state.status === "ready" && state.entries.length === 0 ? (
        <p style={{ color: "#64748b" }}>No cached snapshots yet.</p>
      ) : null}

      {state.status === "ready" && state.entries.length > 0 ? (
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr style={{ textAlign: "left", color: "#94a3b8" }}>
                <th style={{ padding: "0 0 0.6rem" }}>Heap path</th>
                <th style={{ padding: "0 0 0.6rem" }}>Objects</th>
                <th style={{ padding: "0 0 0.6rem" }}>Field data</th>
                <th style={{ padding: "0 0 0.6rem" }}>Action</th>
              </tr>
            </thead>
            <tbody>
              {state.entries.map((entry) => (
                <tr key={entry.heapSha256}>
                  <td style={{ padding: "0.65rem 0", borderTop: "1px solid #1e293b" }}>{entry.heapPath}</td>
                  <td style={{ padding: "0.65rem 0", borderTop: "1px solid #1e293b" }}>
                    {entry.objectCount.toLocaleString()}
                  </td>
                  <td style={{ padding: "0.65rem 0", borderTop: "1px solid #1e293b" }}>
                    {entry.hasFieldData ? "Yes" : "No"}
                  </td>
                  <td style={{ padding: "0.65rem 0", borderTop: "1px solid #1e293b" }}>
                    <button
                      type="button"
                      aria-label={`Open ${entry.heapPath}`}
                      disabled={!openAvailable || busyKey !== undefined}
                      onClick={() => void handleOpen(entry)}
                    >
                      {busyKey === entry.heapSha256 ? "Opening…" : "Open"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  );
}

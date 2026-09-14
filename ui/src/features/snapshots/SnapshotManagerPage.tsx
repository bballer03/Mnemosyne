import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { pickHeapFile } from "../artifact-loader/desktop-heap-client";
import {
  getRememberedDesktopHeapSource,
  rememberDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import {
  isListSnapshotsAvailable,
  isRemoveSnapshotAvailable,
  isSaveSnapshotAvailable,
  runListSnapshots,
  runRemoveSnapshot,
  runSaveSnapshot,
  type SnapshotManifest,
} from "../workflow-landing/workflow-bridge-client";

function displayHeapName(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

export function SnapshotManagerPage() {
  const [snapshots, setSnapshots] = useState<SnapshotManifest[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "unavailable" | "error">("loading");
  const [error, setError] = useState<string | undefined>();
  const [actionStatus, setActionStatus] = useState("Ready.");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    if (!isListSnapshotsAvailable()) {
      setStatus("unavailable");
      return;
    }

    const result = await runListSnapshots();
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
    setError(undefined);
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      if (cancelled) {
        return;
      }
      await refresh();
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  async function handleSave() {
    if (!isSaveSnapshotAvailable()) {
      setActionStatus("Saving snapshots requires the desktop host bridge.");
      return;
    }

    setBusy(true);
    try {
      let source = getRememberedDesktopHeapSource();
      if (!source) {
        const picked = await pickHeapFile();
        if (picked.status !== "selected") {
          setActionStatus(
            picked.status === "cancelled"
              ? "Heap selection cancelled."
              : "Open a heap dump in the desktop app first.",
          );
          return;
        }
        rememberDesktopHeapSource(picked.sourceId, picked.displayName);
        source = picked;
      }

      setActionStatus(`Saving snapshot for ${source.displayName}…`);
      const result = await runSaveSnapshot(source.sourceId, false);
      if (result.status === "unavailable") {
        setActionStatus("Saving snapshots requires the desktop host bridge.");
        return;
      }
      if (result.status === "error") {
        setActionStatus(result.error);
        return;
      }

      setActionStatus(
        `Saved ${result.data.heapSha256.slice(0, 12)}… (${result.data.objectCount.toLocaleString()} objects).`,
      );
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function handleRemove(snapshot: SnapshotManifest) {
    if (!isRemoveSnapshotAvailable()) {
      setActionStatus("Removing snapshots requires the desktop host bridge.");
      return;
    }

    const short = snapshot.heapSha256.slice(0, 12);
    const confirmed = window.confirm(
      `Remove snapshot ${short}… from the session cache? This cannot be undone.`,
    );
    if (!confirmed) {
      setActionStatus("Removal cancelled.");
      return;
    }

    setBusy(true);
    try {
      setActionStatus(`Removing ${short}…`);
      const result = await runRemoveSnapshot(snapshot.heapSha256);
      if (result.status === "unavailable") {
        setActionStatus("Removing snapshots requires the desktop host bridge.");
        return;
      }
      if (result.status === "error") {
        setActionStatus(result.error);
        return;
      }

      setActionStatus(`Removed ${result.data.key.slice(0, 12)}….`);
      await refresh();
    } finally {
      setBusy(false);
    }
  }

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
        Cache, list, and remove heap snapshot manifests from the desktop session store. Removal is
        limited to store keys (SHA-256), never arbitrary filesystem paths.
      </p>

      <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap", alignItems: "center" }}>
        <button type="button" onClick={() => void handleSave()} disabled={busy}>
          {busy ? "Working…" : "Save from remembered heap"}
        </button>
        <button type="button" onClick={() => void refresh()} disabled={busy || status === "unavailable"}>
          Refresh
        </button>
      </div>

      <p role="status" style={{ margin: 0, color: "#cbd5e1" }}>
        {actionStatus}
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
                <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Actions</th>
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
                    {displayHeapName(snapshot.heapPath)}
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
                  <td style={{ padding: "0.55rem 0.6rem 0.55rem 0", borderTop: "1px solid #1e293b" }}>
                    <button
                      type="button"
                      onClick={() => void handleRemove(snapshot)}
                      disabled={busy}
                    >
                      Remove
                    </button>
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

import { useEffect, useState } from "react";

import type { AnalysisArtifact, FrameLocalRootKind } from "../../../lib/analysis-types";

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

function formatObjectId(objectId: number) {
  return `0x${objectId.toString(16)}`;
}

const rootKindLabel: Record<FrameLocalRootKind, string> = {
  JavaFrame: "Java frame local",
  JniLocal: "JNI local",
};

const cardStyle = {
  display: "grid",
  gap: "0.5rem",
  borderRadius: 16,
  border: "1px solid #1e293b",
  background: "rgba(2, 6, 23, 0.75)",
  padding: "0.9rem",
} as const;

export function ThreadExplorerPanel({ artifact }: { artifact: AnalysisArtifact }) {
  const report = artifact.threadReport;
  const [selectedIndex, setSelectedIndex] = useState<number | undefined>(
    report && report.threads.length > 0 ? 0 : undefined,
  );

  useEffect(() => {
    setSelectedIndex(report && report.threads.length > 0 ? 0 : undefined);
  }, [report]);

  if (!report) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Thread Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Thread analysis is absent from this artifact. Re-run <code>mnemosyne analyze --threads</code> to include it.
        </p>
      </div>
    );
  }

  if (report.threads.length === 0) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Thread Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Thread analysis is present but this artifact contains no threads.
        </p>
      </div>
    );
  }

  const selectedThread = selectedIndex !== undefined ? report.threads[selectedIndex] : undefined;

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Thread Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          {report.totalThreadCount.toLocaleString()} threads retaining {formatBytes(report.totalThreadRetained)} in total.
        </p>
      </div>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(0, 260px) minmax(0, 1fr)",
          gap: "1rem",
          alignItems: "start",
        }}
      >
        <nav aria-label="Threads" style={{ display: "grid", gap: "0.5rem" }}>
          {report.threads.map((thread, index) => {
            const isSelected = index === selectedIndex;
            return (
              <button
                key={thread.objectId}
                type="button"
                aria-pressed={isSelected}
                aria-label={`Select ${thread.name}`}
                onClick={() => setSelectedIndex(index)}
                style={{
                  display: "grid",
                  gap: "0.3rem",
                  textAlign: "left",
                  borderRadius: 14,
                  border: isSelected ? "1px solid #38bdf8" : "1px solid #1e293b",
                  background: isSelected ? "rgba(14, 116, 144, 0.18)" : "rgba(2, 6, 23, 0.75)",
                  padding: "0.7rem 0.85rem",
                  color: "#e2e8f0",
                  cursor: "pointer",
                }}
              >
                <strong style={{ overflowWrap: "anywhere" }}>{thread.name}</strong>
                <span style={{ color: "#94a3b8", fontSize: "0.82rem" }}>
                  {thread.daemon ? "daemon" : "non-daemon"} - {formatBytes(thread.retainedBytes)} retained
                </span>
              </button>
            );
          })}
        </nav>

        <section aria-label="Selected thread detail" style={cardStyle}>
          {selectedThread ? (
            <div style={{ display: "grid", gap: "0.75rem" }}>
              <div style={{ display: "grid", gap: "0.2rem" }}>
                <strong style={{ overflowWrap: "anywhere" }}>{selectedThread.name}</strong>
                <span style={{ color: "#94a3b8", fontSize: "0.85rem", overflowWrap: "anywhere" }}>
                  {formatObjectId(selectedThread.objectId)} - {selectedThread.daemon ? "daemon" : "non-daemon"}
                </span>
                <span style={{ color: "#94a3b8", fontSize: "0.85rem" }}>
                  {formatBytes(selectedThread.retainedBytes)} retained across {selectedThread.threadLocalCount.toLocaleString()}{" "}
                  thread-local objects ({formatBytes(selectedThread.threadLocalBytes)})
                </span>
              </div>

              {!selectedThread.stackTrace || selectedThread.stackTrace.length === 0 ? (
                <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
                  No stack trace was captured for this thread in the heap dump.
                </p>
              ) : (
                <div style={{ display: "grid", gap: "0.6rem" }}>
                  {selectedThread.stackTrace.map((frame, frameIndex) => (
                    <div
                      key={`${frame.methodName}-${frameIndex}`}
                      style={{
                        display: "grid",
                        gap: "0.4rem",
                        borderRadius: 12,
                        border: "1px solid #1e293b",
                        padding: "0.65rem 0.75rem",
                      }}
                    >
                      <div style={{ overflowWrap: "anywhere" }}>
                        <span style={{ fontWeight: 600 }}>
                          {frame.className}.{frame.methodName}
                        </span>
                        <span style={{ color: "#64748b" }}>
                          {" "}
                          ({frame.sourceFile ?? "unknown source"}:{frame.lineNumber})
                        </span>
                      </div>

                      {frame.locals.length === 0 ? (
                        <span style={{ color: "#64748b", fontSize: "0.85rem" }}>No locals rooted at this frame.</span>
                      ) : (
                        <table style={{ width: "100%", borderCollapse: "collapse" }}>
                          <thead>
                            <tr style={{ textAlign: "left", color: "#94a3b8", fontSize: "0.82rem" }}>
                              <th style={{ padding: "0 0.5rem 0.3rem 0" }}>Slot</th>
                              <th style={{ padding: "0 0.5rem 0.3rem 0" }}>Object</th>
                              <th style={{ padding: "0 0.5rem 0.3rem 0" }}>Class</th>
                              <th style={{ padding: "0 0.5rem 0.3rem 0" }}>Root kind</th>
                            </tr>
                          </thead>
                          <tbody>
                            {frame.locals.map((local) => (
                              <tr key={`${local.variableSlot}-${local.objectId}`}>
                                <td style={{ padding: "0.3rem 0.5rem 0.3rem 0" }}>{local.variableSlot}</td>
                                <td style={{ padding: "0.3rem 0.5rem 0.3rem 0", overflowWrap: "anywhere" }}>
                                  {local.objectId}
                                </td>
                                <td style={{ padding: "0.3rem 0.5rem 0.3rem 0", overflowWrap: "anywhere" }}>
                                  {local.className}
                                </td>
                                <td style={{ padding: "0.3rem 0.5rem 0.3rem 0" }}>{rootKindLabel[local.rootKind]}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>Select a thread to inspect its stack trace.</p>
          )}
        </section>
      </div>
    </div>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  isListClassInstancesAvailable,
  listClassInstances,
  type ClassInstancesPage,
  type HistogramGroupByMode,
} from "../../heap-explorer/heap-explorer-query-client";
import { useInvestigationStore } from "../../investigation/investigation-store";

const INSTANCE_PAGE_SIZE = 100;

type ClassInstancesPanelProps = {
  groupBy: HistogramGroupByMode;
  onRegroupToClass?: () => void | Promise<void>;
};

type PanelState =
  | { status: "idle" | "loading" | "unavailable" }
  | { status: "ready"; data: ClassInstancesPage }
  | { status: "error"; error: string };

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

export function ClassInstancesPanel({
  groupBy,
  onRegroupToClass,
}: ClassInstancesPanelProps) {
  const navigate = useNavigate();
  const classKey = useInvestigationStore((state) => state.classKey);
  const setObjectId = useInvestigationStore((state) => state.setObjectId);
  const [state, setState] = useState<PanelState>({ status: "idle" });
  const requestId = useRef(0);

  const requestPage = useCallback(
    async (offset: number) => {
      if (groupBy !== "class" || !classKey) {
        setState({ status: "idle" });
        return;
      }
      if (!isListClassInstancesAvailable()) {
        setState({ status: "unavailable" });
        return;
      }

      const currentRequest = ++requestId.current;
      setState({ status: "loading" });
      const result = await listClassInstances(classKey, offset, INSTANCE_PAGE_SIZE);
      if (currentRequest !== requestId.current) {
        return;
      }

      if (result.status === "unavailable") {
        setState({ status: "unavailable" });
      } else if (result.status === "error") {
        setState({ status: "error", error: result.error });
      } else {
        setState({ status: "ready", data: result.data });
      }
    },
    [classKey, groupBy],
  );

  useEffect(() => {
    void requestPage(0);
    return () => {
      requestId.current += 1;
    };
  }, [requestPage]);

  if (groupBy !== "class") {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Class Instances</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Instances are available only for Class grouping, not aggregate {groupBy} buckets.
        </p>
        {onRegroupToClass ? (
          <button type="button" onClick={() => void onRegroupToClass()}>
            Regroup histogram to Class
          </button>
        ) : null}
      </div>
    );
  }

  if (!classKey) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Class Instances</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Select a Class bucket to list its instances.
        </p>
      </div>
    );
  }

  if (state.status === "unavailable") {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Class Instances</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Class instances are unavailable in this host. Connect a desktop or MCP heap session
          exposing <code>listClassInstances</code>.
        </p>
      </div>
    );
  }

  if (state.status === "error") {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Class Instances</h2>
        <p role="alert" style={{ margin: 0, color: "#fca5a5", lineHeight: 1.6 }}>
          {state.error}
        </p>
        <button type="button" onClick={() => void requestPage(0)}>
          Retry class instances
        </button>
      </div>
    );
  }

  if (state.status !== "ready") {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Class Instances</h2>
        <p aria-live="polite" style={{ margin: 0, color: "#94a3b8" }}>
          Loading bounded instances for {classKey}…
        </p>
      </div>
    );
  }

  const { data } = state;
  const firstShown = data.returned === 0 ? 0 : data.offset + 1;
  const lastShown = data.offset + data.returned;

  return (
    <div style={{ display: "grid", gap: "0.85rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Class Instances</h2>
        <div style={{ color: "#cbd5e1", overflowWrap: "anywhere" }}>{data.classKey}</div>
        <div aria-live="polite" style={{ color: "#94a3b8", fontSize: "0.9rem" }}>
          {data.returned === 0
            ? `Showing 0 of ${data.total.toLocaleString()}`
            : `Showing ${firstShown.toLocaleString()}–${lastShown.toLocaleString()} of ${data.total.toLocaleString()}`}
        </div>
        {data.truncated ? (
          <div style={{ color: "#fbbf24", fontSize: "0.85rem" }}>
            More instances remain on later bounded pages.
          </div>
        ) : null}
      </div>

      {data.instances.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8" }}>No live instances matched this class.</p>
      ) : (
        <div style={{ display: "grid", gap: "0.6rem" }}>
          {data.instances.map((instance) => (
            <button
              key={instance.objectId}
              type="button"
              aria-label={`Open object ${instance.objectId}`}
              onClick={() => {
                setObjectId(instance.objectId, "histogram");
                navigate(
                  `/heap-explorer/object-inspector?objectId=${encodeURIComponent(instance.objectId)}`,
                );
              }}
              style={{
                display: "grid",
                gap: "0.35rem",
                textAlign: "left",
                borderRadius: 12,
                border: "1px solid #1e293b",
                background: "rgba(2, 6, 23, 0.75)",
                color: "#e2e8f0",
                padding: "0.75rem",
              }}
            >
              <strong style={{ overflowWrap: "anywhere" }}>{instance.objectId}</strong>
              <span style={{ color: "#94a3b8", overflowWrap: "anywhere" }}>
                {instance.className}
              </span>
              <span style={{ color: "#cbd5e1", fontSize: "0.85rem" }}>
                Retained {formatBytes(instance.retainedSize)} · Shallow{" "}
                {formatBytes(instance.shallowSize)}
              </span>
            </button>
          ))}
        </div>
      )}

      <nav
        aria-label="Class instance pagination"
        style={{ display: "flex", justifyContent: "space-between", gap: "0.75rem" }}
      >
        <button
          type="button"
          aria-label="Previous instance page"
          disabled={data.offset === 0}
          onClick={() => void requestPage(Math.max(0, data.offset - data.limit))}
        >
          Previous
        </button>
        <button
          type="button"
          aria-label="Next instance page"
          disabled={!data.truncated}
          onClick={() => void requestPage(data.offset + data.limit)}
        >
          Next
        </button>
      </nav>
    </div>
  );
}

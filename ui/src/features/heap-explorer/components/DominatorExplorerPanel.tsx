import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";
import {
  getDominatorChildren,
  isGetDominatorChildrenAvailable,
  type DominatorChildEntry,
  type DominatorChildrenPage,
} from "../heap-explorer-query-client";

const searchInputStyle = {
  width: "100%",
  borderRadius: 12,
  border: "1px solid #334155",
  background: "rgba(2, 6, 23, 0.82)",
  color: "#e2e8f0",
  padding: "0.65rem 0.8rem",
} as const;

const DOMINATOR_PAGE_SIZE = 50;
const ARTIFACT_PREVIEW_LIMIT = 100;

type DominatorExplorerPanelProps = {
  rows: AnalysisArtifact["graph"]["dominators"];
  totalSizeBytes?: number;
  selectedObjectId?: string;
  onSelectObjectId?: (objectId: string) => void;
  selectedRowIndex?: number;
  onSelectRowIndex: (rowIndex: number) => void;
};

type RootState =
  | { status: "loading"; page?: DominatorChildrenPage }
  | { status: "ready"; page: DominatorChildrenPage }
  | { status: "error"; error: string; page?: DominatorChildrenPage };

type BranchState =
  | { status: "collapsed" }
  | { status: "loading"; page?: DominatorChildrenPage }
  | { status: "ready"; page: DominatorChildrenPage }
  | { status: "error"; error: string; page?: DominatorChildrenPage; retryOffset: number };

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

function mergePages(
  current: DominatorChildrenPage | undefined,
  next: DominatorChildrenPage,
): DominatorChildrenPage {
  if (!current || next.offset === 0) {
    return next;
  }

  const children = [...current.children, ...next.children];
  return {
    ...next,
    offset: 0,
    returned: children.length,
    children,
  };
}

export function DominatorExplorerPanel({
  rows,
  totalSizeBytes = 0,
  selectedObjectId,
  onSelectObjectId,
  selectedRowIndex,
  onSelectRowIndex,
}: DominatorExplorerPanelProps) {
  const [searchText, setSearchText] = useState("");
  const [retainedPercent, setRetainedPercent] = useState(0);
  const [rootRetry, setRootRetry] = useState(0);
  const [rootState, setRootState] = useState<RootState>({ status: "loading" });
  const [branchStates, setBranchStates] = useState<Record<string, BranchState>>({});
  const branchVersionRef = useRef(0);
  const liveTreeAvailable = isGetDominatorChildrenAvailable();
  const normalizedSearch = searchText.trim().toLowerCase();
  const minRetainedBytes = Math.floor((totalSizeBytes * retainedPercent) / 100);

  const filteredRows = useMemo(() => {
    return rows.flatMap((row, index) => {
      if (normalizedSearch === "") {
        return [{ row, index }];
      }

      return [row.className, row.objectId, row.name].some((value) => value.toLowerCase().includes(normalizedSearch))
        ? [{ row, index }]
        : [];
    });
  }, [rows, normalizedSearch]);
  const previewRows = filteredRows.slice(0, ARTIFACT_PREVIEW_LIMIT);

  const maxRetainedSize = previewRows.length > 0 ? Math.max(...previewRows.map(({ row }) => row.retainedSize)) : 0;
  const maxDominates = previewRows.length > 0 ? Math.max(...previewRows.map(({ row }) => row.dominates)) : 0;

  useEffect(() => {
    if (!liveTreeAvailable) {
      return undefined;
    }

    let cancelled = false;
    branchVersionRef.current += 1;
    setBranchStates({});
    setRootState({ status: "loading" });

    void getDominatorChildren(undefined, 0, DOMINATOR_PAGE_SIZE, minRetainedBytes).then((result) => {
      if (cancelled) {
        return;
      }
      if (result.status === "ready") {
        setRootState({ status: "ready", page: result.data });
      } else {
        setRootState({
          status: "error",
          error:
            result.status === "error"
              ? result.error
              : "Live dominator children are unavailable.",
        });
      }
    });

    return () => {
      cancelled = true;
    };
  }, [liveTreeAvailable, minRetainedBytes, rootRetry]);

  const loadBranchPage = useCallback(
    async (node: DominatorChildEntry, offset: number, currentPage?: DominatorChildrenPage) => {
      const requestVersion = branchVersionRef.current;
      setBranchStates((current) => ({
        ...current,
        [node.objectId]: { status: "loading", page: currentPage },
      }));

      const result = await getDominatorChildren(
        node.objectId,
        offset,
        DOMINATOR_PAGE_SIZE,
        minRetainedBytes,
      );
      if (requestVersion !== branchVersionRef.current) {
        return;
      }

      setBranchStates((current) => {
        if (current[node.objectId]?.status === "collapsed") {
          return current;
        }
        if (result.status === "ready") {
          return {
            ...current,
            [node.objectId]: {
              status: "ready",
              page: mergePages(currentPage, result.data),
            },
          };
        }
        return {
          ...current,
          [node.objectId]: {
            status: "error",
            error:
              result.status === "error"
                ? result.error
                : "Live dominator children are unavailable.",
            page: currentPage,
            retryOffset: offset,
          },
        };
      });
    },
    [minRetainedBytes],
  );

  function toggleBranch(node: DominatorChildEntry) {
    const state = branchStates[node.objectId];
    if (state && state.status !== "collapsed") {
      setBranchStates((current) => ({
        ...current,
        [node.objectId]: { status: "collapsed" },
      }));
      return;
    }

    void loadBranchPage(node, 0);
  }

  async function loadNextRoots() {
    const currentPage = rootState.page;
    if (!currentPage) {
      return;
    }
    const offset = currentPage.children.length;
    setRootState({ status: "loading", page: currentPage });
    const result = await getDominatorChildren(
      undefined,
      offset,
      DOMINATOR_PAGE_SIZE,
      minRetainedBytes,
    );
    if (result.status === "ready") {
      setRootState({ status: "ready", page: mergePages(currentPage, result.data) });
    } else {
      setRootState({
        status: "error",
        error:
          result.status === "error"
            ? result.error
            : "Live dominator children are unavailable.",
        page: currentPage,
      });
    }
  }

  function renderLiveNodes(nodes: DominatorChildEntry[], depth = 0) {
    return (
      <ul
        aria-label={depth === 0 ? "Dominator roots" : "Dominator children"}
        style={{ display: "grid", gap: "0.55rem", listStyle: "none", margin: 0, padding: 0 }}
      >
        {nodes.map((node) => {
          const branchState = branchStates[node.objectId] ?? { status: "collapsed" as const };
          const isExpanded = branchState.status !== "collapsed";
          const page = "page" in branchState ? branchState.page : undefined;

          return (
            <li key={node.objectId} style={{ display: "grid", gap: "0.45rem", marginLeft: depth > 0 ? "1rem" : 0 }}>
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "auto minmax(0, 1fr)",
                  gap: "0.5rem",
                  alignItems: "stretch",
                }}
              >
                {node.hasChildren ? (
                  <button
                    type="button"
                    aria-expanded={isExpanded}
                    aria-label={`${isExpanded ? "Collapse" : "Expand"} ${node.className} ${node.objectId}`}
                    onClick={() => toggleBranch(node)}
                    style={{
                      borderRadius: 10,
                      border: "1px solid #334155",
                      background: "rgba(15, 23, 42, 0.9)",
                      color: "#e2e8f0",
                      cursor: "pointer",
                      minWidth: 42,
                    }}
                  >
                    {isExpanded ? "−" : "+"}
                  </button>
                ) : (
                  <span aria-hidden="true" style={{ width: 42 }} />
                )}
                <button
                  type="button"
                  aria-pressed={selectedObjectId === node.objectId}
                  aria-label={`Select ${node.className} ${node.objectId}`}
                  onClick={() => onSelectObjectId?.(node.objectId)}
                  style={{
                    display: "grid",
                    gap: "0.35rem",
                    textAlign: "left",
                    borderRadius: 14,
                    border: selectedObjectId === node.objectId ? "1px solid #38bdf8" : "1px solid #1e293b",
                    background:
                      selectedObjectId === node.objectId
                        ? "rgba(14, 116, 144, 0.18)"
                        : "rgba(2, 6, 23, 0.75)",
                    padding: "0.75rem",
                    color: "#e2e8f0",
                    cursor: "pointer",
                  }}
                >
                  <span style={{ display: "flex", justifyContent: "space-between", gap: "1rem" }}>
                    <strong style={{ overflowWrap: "anywhere" }}>{node.className}</strong>
                    <span style={{ color: "#94a3b8", whiteSpace: "nowrap" }}>{node.objectId}</span>
                  </span>
                  <span style={{ color: "#94a3b8" }}>
                    Retained {formatBytes(node.retainedSize)} · Shallow {formatBytes(node.shallowSize)} · Dominates{" "}
                    {node.dominatedCount.toLocaleString()}
                  </span>
                </button>
              </div>

              {isExpanded ? (
                <div style={{ display: "grid", gap: "0.45rem", marginLeft: 42 }}>
                  {page && page.children.length > 0 ? renderLiveNodes(page.children, depth + 1) : null}
                  {branchState.status === "loading" ? (
                    <p role="status" style={{ margin: 0, color: "#94a3b8" }}>
                      Loading children…
                    </p>
                  ) : null}
                  {branchState.status === "error" ? (
                    <div role="alert" style={{ display: "grid", gap: "0.35rem", color: "#fca5a5" }}>
                      <span>{branchState.error}</span>
                      <button
                        type="button"
                        aria-label={`Retry children for ${node.className} ${node.objectId}`}
                        onClick={() =>
                          void loadBranchPage(
                            node,
                            branchState.retryOffset,
                            branchState.page,
                          )
                        }
                      >
                        Retry
                      </button>
                    </div>
                  ) : null}
                  {branchState.status === "ready" && branchState.page.children.length === 0 ? (
                    <p style={{ margin: 0, color: "#94a3b8" }}>No children match the retained-size floor.</p>
                  ) : null}
                  {page?.truncated && branchState.status !== "loading" ? (
                    <button
                      type="button"
                      aria-label={`Load next children for ${node.className} ${node.objectId}`}
                      onClick={() => void loadBranchPage(node, page.children.length, page)}
                    >
                      Load next children
                    </button>
                  ) : null}
                </div>
              ) : null}
            </li>
          );
        })}
      </ul>
    );
  }

  const liveRootPage = rootState.page;
  const liveRoots =
    liveRootPage?.children.filter((node) =>
      normalizedSearch === ""
        ? true
        : [node.className, node.objectId].some((value) =>
            value.toLowerCase().includes(normalizedSearch),
          ),
    ) ?? [];

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.4rem)", lineHeight: 1.08 }}>Dominator Explorer</h1>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7 }}>
          Search retained heap roots by class, object id, or dominator label.
        </p>
      </div>

      <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
        <span>Search dominators</span>
        <input
          aria-label="Search dominators"
          type="text"
          value={searchText}
          onChange={(event) => setSearchText(event.target.value)}
          placeholder="class, id, or label"
          style={searchInputStyle}
        />
      </label>

      {liveTreeAvailable ? (
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ color: "#38bdf8", fontSize: "0.82rem", letterSpacing: "0.08em", textTransform: "uppercase" }}>
            Live lazy dominator tree
          </div>
          <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1", maxWidth: 360 }}>
            <span>Minimum retained percent</span>
            <input
              aria-label="Minimum retained percent"
              type="number"
              min={0}
              max={100}
              step={1}
              value={retainedPercent}
              onChange={(event) => {
                const next = Number(event.target.value);
                setRetainedPercent(Number.isFinite(next) ? Math.min(100, Math.max(0, next)) : 0);
              }}
              style={searchInputStyle}
            />
            <span style={{ color: "#94a3b8", fontSize: "0.85rem" }}>
              Floor: {formatBytes(minRetainedBytes)} of {formatBytes(totalSizeBytes)}
            </span>
          </label>
          {rootState.status === "loading" && !liveRootPage ? (
            <p role="status" style={{ margin: 0, color: "#94a3b8" }}>
              Loading dominator roots…
            </p>
          ) : null}
          {rootState.status === "error" ? (
            <div role="alert" style={{ display: "grid", gap: "0.4rem", color: "#fca5a5" }}>
              <span>{rootState.error}</span>
              <button type="button" onClick={() => setRootRetry((value) => value + 1)}>
                Retry roots
              </button>
            </div>
          ) : null}
          {liveRootPage && liveRoots.length > 0 ? renderLiveNodes(liveRoots) : null}
          {liveRootPage && liveRoots.length === 0 && rootState.status !== "loading" ? (
            <p style={{ margin: 0, color: "#94a3b8" }}>
              {normalizedSearch === ""
                ? "No dominator roots match the retained-size floor."
                : "No loaded dominator roots match the current search."}
            </p>
          ) : null}
          {liveRootPage?.truncated && rootState.status !== "loading" ? (
            <button type="button" aria-label="Load next children for dominator roots" onClick={() => void loadNextRoots()}>
              Load next children
            </button>
          ) : null}
        </div>
      ) : (
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ color: "#fbbf24", fontSize: "0.82rem", letterSpacing: "0.08em", textTransform: "uppercase" }}>
            Artifact-only bounded flat preview
          </div>
          {filteredRows.length > ARTIFACT_PREVIEW_LIMIT ? (
            <p style={{ margin: 0, color: "#94a3b8" }}>
              Showing first {ARTIFACT_PREVIEW_LIMIT} of {filteredRows.length} artifact rows.
            </p>
          ) : null}
      {previewRows.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          {normalizedSearch === ""
            ? "No dominator rows are available in this artifact."
            : "No dominator rows match the current search."}
        </p>
      ) : (
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ color: "#94a3b8", fontSize: "0.9rem" }}>Retained vs dominates</div>
          {previewRows.map(({ row, index }) => {
            const retainedWidth =
              maxRetainedSize > 0 && row.retainedSize > 0 ? Math.max((row.retainedSize / maxRetainedSize) * 100, 8) : 0;
            const dominatesWidth =
              maxDominates > 0 && row.dominates > 0 ? Math.max((row.dominates / maxDominates) * 100, 8) : 0;
            const isSelected = index === selectedRowIndex;
            const objectIdLabel = row.objectId || "artifact-only row";
            const selectionLabel = row.objectId
              ? `Select ${row.className} ${row.objectId}`
              : `Select ${row.className} artifact-only row ${row.name} row ${index + 1}`;

            return (
              <button
                key={index}
                type="button"
                aria-pressed={isSelected}
                aria-label={selectionLabel}
                onClick={() => onSelectRowIndex(index)}
                style={{
                  display: "grid",
                  gap: "0.65rem",
                  textAlign: "left",
                  borderRadius: 16,
                  border: isSelected ? "1px solid #38bdf8" : "1px solid #1e293b",
                  background: isSelected ? "rgba(14, 116, 144, 0.18)" : "rgba(2, 6, 23, 0.75)",
                  padding: "0.9rem",
                  color: "#e2e8f0",
                  cursor: "pointer",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "start" }}>
                  <div style={{ display: "grid", gap: "0.2rem" }}>
                    <strong style={{ overflowWrap: "anywhere" }}>{row.className}</strong>
                    <span style={{ color: "#94a3b8", overflowWrap: "anywhere" }}>{row.name}</span>
                  </div>
                  <span style={{ color: "#94a3b8", whiteSpace: "nowrap" }}>{objectIdLabel}</span>
                </div>

                <div style={{ display: "grid", gap: "0.35rem" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
                    <span>Retained</span>
                    <span>{formatBytes(row.retainedSize)}</span>
                  </div>
                  <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
                    <div
                      data-testid="retained-bar"
                      style={{
                        width: `${retainedWidth}%`,
                        height: "100%",
                        borderRadius: 999,
                        background: "linear-gradient(90deg, #38bdf8, #67e8f9)",
                      }}
                    />
                  </div>
                </div>

                <div style={{ display: "grid", gap: "0.35rem" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
                    <span>Dominates</span>
                    <span>{row.dominates.toLocaleString()} rows</span>
                  </div>
                  <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
                    <div
                      data-testid="dominates-bar"
                      style={{
                        width: `${dominatesWidth}%`,
                        height: "100%",
                        borderRadius: 999,
                        background: "linear-gradient(90deg, #22c55e, #86efac)",
                      }}
                    />
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}
        </div>
      )}
    </div>
  );
}

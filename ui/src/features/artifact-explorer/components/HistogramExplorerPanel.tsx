import { useEffect, useMemo, useState, type ReactNode } from "react";

import type { AnalysisArtifact } from "../../../lib/analysis-types";
import {
  HISTOGRAM_GROUP_BY_OPTIONS,
  isRegroupHistogramAvailable,
  normalizeHistogramGroupBy,
  regroupHistogram,
  type HistogramGroupByMode,
  type HistogramResultView,
} from "../../heap-explorer/heap-explorer-query-client";
import type {
  HistogramSortKey,
  HistogramViewState,
} from "../../investigation/investigation-store";
import {
  buildHierarchyForest,
  supportsDeterministicParentRelation,
  type HierarchyNode,
} from "./histogram-hierarchy";

const HISTOGRAM_PAGE_SIZE = 100;

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

const searchInputStyle = {
  width: "100%",
  borderRadius: 12,
  border: "1px solid #334155",
  background: "rgba(2, 6, 23, 0.82)",
  color: "#e2e8f0",
  padding: "0.65rem 0.8rem",
} as const;

type HistogramExplorerPanelProps = {
  artifact: AnalysisArtifact;
  selectedKey?: string;
  onSelectKey: (key: string | undefined) => void;
  histogramView: HistogramViewState;
  setHistogramView: (patch: Partial<HistogramViewState>) => void;
  /** When live regroup replaces the artifact histogram, parent owns the view. */
  liveHistogram?: HistogramResultView;
  onLiveHistogramChange?: (histogram: HistogramResultView | undefined, source: "artifact" | "live") => void;
  histogramSource?: "artifact" | "live";
};

type HistogramEntryView = NonNullable<HistogramResultView["entries"]>[number];

export function HistogramExplorerPanel({
  artifact,
  selectedKey,
  onSelectKey,
  histogramView,
  setHistogramView,
  liveHistogram,
  onLiveHistogramChange,
  histogramSource = "artifact",
}: HistogramExplorerPanelProps) {
  const [regroupError, setRegroupError] = useState<string | undefined>();
  const [regroupBusy, setRegroupBusy] = useState(false);
  const [collapsedKeys, setCollapsedKeys] = useState<Set<string>>(() => new Set());
  const regroupAvailable = isRegroupHistogramAvailable();

  const activeHistogram = liveHistogram ?? artifact.histogram;
  const artifactGroupBy = normalizeHistogramGroupBy(artifact.histogram?.groupBy) ?? "class";
  const selectedGroupBy = histogramView.groupBy;

  const filteredEntries = useMemo(() => {
    if (!activeHistogram) {
      return [];
    }

    const normalizedSearch = histogramView.searchText.trim().toLowerCase();
    const direction = histogramView.sortDirection === "asc" ? 1 : -1;

    return activeHistogram.entries
      .filter((entry) => entry.key.toLowerCase().includes(normalizedSearch))
      .slice()
      .sort((left, right) => {
        let comparison: number;

        switch (histogramView.sortKey) {
          case "class":
            comparison = left.key.localeCompare(right.key);
            break;
          case "instances":
            comparison = left.instanceCount - right.instanceCount;
            break;
          case "shallow":
            comparison = left.shallowSize - right.shallowSize;
            break;
          case "retained":
            comparison = left.retainedSize - right.retainedSize;
            break;
        }

        return comparison * direction || left.key.localeCompare(right.key);
      });
  }, [
    activeHistogram,
    histogramView.searchText,
    histogramView.sortDirection,
    histogramView.sortKey,
  ]);

  const hierarchySupported = supportsDeterministicParentRelation(
    activeHistogram?.groupBy,
    filteredEntries,
  );

  const hierarchyForest = useMemo(() => {
    if (!hierarchySupported) {
      return [];
    }

    return buildHierarchyForest(filteredEntries);
  }, [filteredEntries, hierarchySupported]);

  const pagedItemCount = hierarchySupported ? hierarchyForest.length : filteredEntries.length;
  const lastPageOffset =
    pagedItemCount === 0
      ? 0
      : Math.floor((pagedItemCount - 1) / HISTOGRAM_PAGE_SIZE) * HISTOGRAM_PAGE_SIZE;
  const pageOffset = Math.min(
    Math.max(0, histogramView.pageOffset),
    lastPageOffset,
  );
  const pagedEntries = filteredEntries.slice(
    pageOffset,
    pageOffset + HISTOGRAM_PAGE_SIZE,
  );
  const pagedHierarchyRoots = hierarchyForest.slice(
    pageOffset,
    pageOffset + HISTOGRAM_PAGE_SIZE,
  );

  useEffect(() => {
    if (pageOffset !== histogramView.pageOffset) {
      setHistogramView({ pageOffset });
    }
  }, [histogramView.pageOffset, pageOffset, setHistogramView]);

  useEffect(() => {
    if (!activeHistogram) {
      return;
    }

    if (filteredEntries.length === 0) {
      return;
    }

    if (!selectedKey || !filteredEntries.some((entry) => entry.key === selectedKey)) {
      onSelectKey(filteredEntries[0]?.key);
    }
  }, [activeHistogram, filteredEntries, onSelectKey, selectedKey]);

  async function handleGroupByChange(next: HistogramGroupByMode) {
    setRegroupError(undefined);

    if (next === artifactGroupBy && histogramSource === "live") {
      setHistogramView({ groupBy: next });
      onLiveHistogramChange?.(undefined, "artifact");
      return;
    }

    if (next === artifactGroupBy && histogramSource === "artifact") {
      setHistogramView({ groupBy: next });
      return;
    }

    if (!regroupAvailable) {
      setRegroupError(
        "Live regroup requires a desktop or MCP host bridge (regroupHistogram). Superclass and other modes are available when a heap session is loaded.",
      );
      return;
    }

    setRegroupBusy(true);
    const result = await regroupHistogram(next);
    setRegroupBusy(false);

    if (result.status === "unavailable") {
      setRegroupError("Live regroup is unavailable in this host.");
      return;
    }

    if (result.status === "error") {
      setRegroupError(result.error);
      return;
    }

    setHistogramView({ groupBy: next });
    onLiveHistogramChange?.(result.data, "live");
  }

  function toggleCollapsed(key: string) {
    setCollapsedKeys((previous) => {
      const next = new Set(previous);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  if (!activeHistogram) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Histogram Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Histogram data is absent from this artifact.
        </p>
      </div>
    );
  }

  if (activeHistogram.entries.length === 0) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Histogram Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          This artifact includes histogram metadata, but no grouped entries are available.
        </p>
      </div>
    );
  }

  const maxRetainedSize = filteredEntries.reduce(
    (maximum, entry) => Math.max(maximum, entry.retainedSize),
    0,
  );
  const sourceLabel =
    histogramSource === "live"
      ? "Live regroup (session heap)"
      : "Precomputed artifact grouping";
  const isSuperclass = selectedGroupBy === "superclass";
  const presentationLabel = hierarchySupported
    ? "Deterministic parent hierarchy (expand/collapse)"
    : isSuperclass
      ? "Flat superclass list — no parent relation in returned data"
      : `Flat retained and shallow comparison across ${activeHistogram.groupBy} buckets`;

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", flexWrap: "wrap" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Histogram Explorer</h2>
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            {presentationLabel}. Live regroup does not invent superclass ancestry from flat keys.
          </p>
        </div>
        <div style={{ display: "grid", gap: "0.25rem", textAlign: "right" }}>
          <div
            style={{
              color: "#38bdf8",
              fontSize: "0.78rem",
              letterSpacing: "0.08em",
              textTransform: "uppercase",
            }}
          >
            {activeHistogram.groupBy}
          </div>
          <div style={{ color: "#94a3b8", fontSize: "0.78rem" }}>{sourceLabel}</div>
        </div>
      </div>

      <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
        <span>Group by</span>
        <select
          aria-label="Histogram group by"
          value={selectedGroupBy}
          disabled={regroupBusy}
          onChange={(event) => {
            void handleGroupByChange(event.target.value as HistogramGroupByMode);
          }}
          style={searchInputStyle}
        >
          {HISTOGRAM_GROUP_BY_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
              {option.value === "superclass" && !hierarchySupported ? " (flat list)" : ""}
            </option>
          ))}
        </select>
      </label>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)",
          gap: "0.75rem",
        }}
      >
        <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
          <span>Sort by</span>
          <select
            aria-label="Histogram sort by"
            value={histogramView.sortKey}
            onChange={(event) => {
              setHistogramView({ sortKey: event.target.value as HistogramSortKey });
            }}
            style={searchInputStyle}
          >
            <option value="retained">Retained size</option>
            <option value="shallow">Shallow size</option>
            <option value="instances">Instance count</option>
            <option value="class">Class or bucket key</option>
          </select>
        </label>
        <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
          <span>Direction</span>
          <select
            aria-label="Histogram sort direction"
            value={histogramView.sortDirection}
            onChange={(event) => {
              setHistogramView({
                sortDirection: event.target.value as HistogramViewState["sortDirection"],
              });
            }}
            style={searchInputStyle}
          >
            <option value="desc">Descending</option>
            <option value="asc">Ascending</option>
          </select>
        </label>
      </div>

      {!regroupAvailable ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6, fontSize: "0.9rem" }}>
          Showing the precomputed artifact grouping. Live regroup (including superclass) needs the
          host <code>regroupHistogram</code> bridge.
        </p>
      ) : null}

      {regroupError ? (
        <p role="alert" style={{ margin: 0, color: "#fca5a5", lineHeight: 1.6 }}>
          {regroupError}
        </p>
      ) : null}

      <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
        <span>Search histogram</span>
        <input
          aria-label="Search histogram"
          type="text"
          value={histogramView.searchText}
          onChange={(event) => setHistogramView({ searchText: event.target.value })}
          placeholder="class, package, loader, or superclass"
          style={searchInputStyle}
        />
      </label>

      {filteredEntries.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          No histogram buckets match the current search.
        </p>
      ) : hierarchySupported ? (
        <div style={{ display: "grid", gap: "0.75rem" }} aria-label="Superclass hierarchy">
          <div style={{ color: "#94a3b8", fontSize: "0.9rem" }}>
            Retained vs shallow (parent-linked hierarchy)
          </div>
          {pagedHierarchyRoots.map((node) => (
            <HierarchyEntryButton
              key={node.entry.key}
              node={node}
              depth={0}
              maxRetainedSize={maxRetainedSize}
              selectedKey={selectedKey}
              collapsedKeys={collapsedKeys}
              onToggleCollapsed={toggleCollapsed}
              onSelectKey={onSelectKey}
            />
          ))}
        </div>
      ) : (
        <div style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ color: "#94a3b8", fontSize: "0.9rem" }}>Retained vs shallow</div>
          {pagedEntries.map((entry) => (
            <HistogramEntryButton
              key={entry.key}
              entry={entry}
              maxRetainedSize={maxRetainedSize}
              isSelected={entry.key === selectedKey}
              onSelectKey={onSelectKey}
            />
          ))}
        </div>
      )}

      {filteredEntries.length > 0 ? (
        <nav
          aria-label="Histogram pagination"
          style={{
            display: "flex",
            justifyContent: "space-between",
            gap: "0.75rem",
            alignItems: "center",
            flexWrap: "wrap",
          }}
        >
          <button
            type="button"
            aria-label="Previous histogram page"
            disabled={pageOffset === 0}
            onClick={() => {
              setHistogramView({
                pageOffset: Math.max(0, pageOffset - HISTOGRAM_PAGE_SIZE),
              });
            }}
          >
            Previous
          </button>
          <span aria-live="polite" style={{ color: "#94a3b8" }}>
            Showing {pageOffset + 1}–{Math.min(pageOffset + HISTOGRAM_PAGE_SIZE, pagedItemCount)} of{" "}
            {pagedItemCount}
          </span>
          <button
            type="button"
            aria-label="Next histogram page"
            disabled={pageOffset + HISTOGRAM_PAGE_SIZE >= pagedItemCount}
            onClick={() => {
              setHistogramView({ pageOffset: pageOffset + HISTOGRAM_PAGE_SIZE });
            }}
          >
            Next
          </button>
        </nav>
      ) : null}
    </div>
  );
}

function HistogramEntryButton({
  entry,
  maxRetainedSize,
  isSelected,
  onSelectKey,
  depth = 0,
  expandControl,
}: {
  entry: HistogramEntryView;
  maxRetainedSize: number;
  isSelected: boolean;
  onSelectKey: (key: string | undefined) => void;
  depth?: number;
  expandControl?: ReactNode;
}) {
  const retainedWidth = maxRetainedSize > 0 ? Math.max((entry.retainedSize / maxRetainedSize) * 100, 8) : 0;
  const shallowWidth = maxRetainedSize > 0 ? Math.max((entry.shallowSize / maxRetainedSize) * 100, 4) : 0;

  return (
    <button
      type="button"
      aria-pressed={isSelected}
      aria-label={`Select ${entry.key}`}
      onClick={() => onSelectKey(entry.key)}
      style={{
        display: "grid",
        gap: "0.65rem",
        textAlign: "left",
        borderRadius: 16,
        border: isSelected ? "1px solid #38bdf8" : "1px solid #1e293b",
        background: isSelected ? "rgba(14, 116, 144, 0.18)" : "rgba(2, 6, 23, 0.75)",
        padding: "0.9rem",
        paddingLeft: `${0.9 + depth * 0.85}rem`,
        color: "#e2e8f0",
        cursor: "pointer",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "start" }}>
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "start", minWidth: 0 }}>
          {expandControl}
          <strong style={{ overflowWrap: "anywhere" }}>{entry.key}</strong>
        </div>
        <span style={{ color: "#94a3b8", whiteSpace: "nowrap" }}>
          {entry.instanceCount.toLocaleString()} instances
        </span>
      </div>

      <div style={{ display: "grid", gap: "0.35rem" }}>
        <div style={{ display: "grid", gap: "0.25rem" }}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
            <span>Retained</span>
            <span>{formatBytes(entry.retainedSize)}</span>
          </div>
          <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
            <div
              style={{
                width: `${retainedWidth}%`,
                height: "100%",
                borderRadius: 999,
                background: "linear-gradient(90deg, #38bdf8, #67e8f9)",
              }}
            />
          </div>
        </div>

        <div style={{ display: "grid", gap: "0.25rem" }}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", fontSize: "0.9rem" }}>
            <span>Shallow</span>
            <span>{formatBytes(entry.shallowSize)}</span>
          </div>
          <div style={{ height: 10, borderRadius: 999, background: "rgba(30, 41, 59, 0.9)", overflow: "hidden" }}>
            <div
              style={{
                width: `${shallowWidth}%`,
                height: "100%",
                borderRadius: 999,
                background: "linear-gradient(90deg, #22c55e, #86efac)",
              }}
            />
          </div>
        </div>
      </div>
    </button>
  );
}

function HierarchyEntryButton({
  node,
  depth,
  maxRetainedSize,
  selectedKey,
  collapsedKeys,
  onToggleCollapsed,
  onSelectKey,
}: {
  node: HierarchyNode<HistogramEntryView>;
  depth: number;
  maxRetainedSize: number;
  selectedKey?: string;
  collapsedKeys: Set<string>;
  onToggleCollapsed: (key: string) => void;
  onSelectKey: (key: string | undefined) => void;
}) {
  const hasChildren = node.children.length > 0;
  const isCollapsed = collapsedKeys.has(node.entry.key);

  const expandControl = hasChildren ? (
    <span
      role="button"
      tabIndex={0}
      aria-expanded={!isCollapsed}
      aria-label={isCollapsed ? `Expand ${node.entry.key}` : `Collapse ${node.entry.key}`}
      onClick={(event) => {
        event.stopPropagation();
        onToggleCollapsed(node.entry.key);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          event.stopPropagation();
          onToggleCollapsed(node.entry.key);
        }
      }}
      style={{
        color: "#38bdf8",
        fontSize: "0.85rem",
        lineHeight: 1.2,
        userSelect: "none",
        flexShrink: 0,
      }}
    >
      {isCollapsed ? "▸" : "▾"}
    </span>
  ) : (
    <span aria-hidden="true" style={{ width: "0.85rem", flexShrink: 0 }} />
  );

  return (
    <>
      <HistogramEntryButton
        entry={node.entry}
        maxRetainedSize={maxRetainedSize}
        isSelected={node.entry.key === selectedKey}
        onSelectKey={onSelectKey}
        depth={depth}
        expandControl={expandControl}
      />
      {hasChildren && !isCollapsed
        ? node.children.map((child) => (
            <HierarchyEntryButton
              key={child.entry.key}
              node={child}
              depth={depth + 1}
              maxRetainedSize={maxRetainedSize}
              selectedKey={selectedKey}
              collapsedKeys={collapsedKeys}
              onToggleCollapsed={onToggleCollapsed}
              onSelectKey={onSelectKey}
            />
          ))
        : null}
    </>
  );
}

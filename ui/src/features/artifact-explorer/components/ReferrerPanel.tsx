import { useMemo } from "react";
import { createColumnHelper, flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table";

import type { AnalysisArtifact, ReferrerEntry } from "../../../lib/analysis-types";

function formatBytes(bytes: number | undefined) {
  if (bytes === undefined) {
    return "-";
  }

  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

const columnHelper = createColumnHelper<ReferrerEntry>();

const columns = [
  columnHelper.accessor("className", {
    header: "Class",
    cell: (info) => (
      <div>
        <div style={{ fontWeight: 600, overflowWrap: "anywhere" }}>{info.getValue()}</div>
        <div style={{ color: "#64748b", fontSize: "0.82rem", marginTop: "0.2rem", overflowWrap: "anywhere" }}>
          {info.row.original.objectId}
        </div>
      </div>
    ),
  }),
  columnHelper.accessor("referrerCount", {
    header: "Referrers",
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("retainedSize", {
    header: "Retained size",
    cell: (info) => formatBytes(info.getValue()),
  }),
  columnHelper.accessor("topReferrerClasses", {
    header: "Top referrer classes",
    cell: (info) => {
      const classes = info.getValue();

      if (classes.length === 0) {
        return <span style={{ color: "#64748b" }}>-</span>;
      }

      return (
        <div style={{ display: "grid", gap: "0.2rem" }}>
          {classes.map(([className, count]) => (
            <div key={className} style={{ overflowWrap: "anywhere" }}>
              {className} <span style={{ color: "#64748b" }}>x{count}</span>
            </div>
          ))}
        </div>
      );
    },
  }),
];

export function ReferrerPanel({ artifact }: { artifact: AnalysisArtifact }) {
  const report = artifact.referrerReport;
  const entries = useMemo(() => report?.entries ?? [], [report]);

  const table = useReactTable({
    data: entries,
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  if (!report) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Top Referenced Objects</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Referrer analysis is absent from this artifact. Re-run <code>mnemosyne analyze --by-referrer</code> to include it.
        </p>
      </div>
    );
  }

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", flexWrap: "wrap" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Top Referenced Objects</h2>
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            Ranked by incoming reference count across {report.totalObjectsConsidered.toLocaleString()} objects considered.
          </p>
        </div>
      </div>

      {entries.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Referrer analysis is present but reports no ranked entries.
        </p>
      ) : (
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              {table.getHeaderGroups().map((headerGroup) => (
                <tr key={headerGroup.id} style={{ textAlign: "left", color: "#94a3b8" }}>
                  {headerGroup.headers.map((header) => (
                    <th key={header.id} style={{ padding: "0 0.6rem 0.6rem 0" }}>
                      {flexRender(header.column.columnDef.header, header.getContext())}
                    </th>
                  ))}
                </tr>
              ))}
            </thead>
            <tbody>
              {table.getRowModel().rows.map((row) => (
                <tr key={row.id}>
                  {row.getVisibleCells().map((cell) => (
                    <td
                      key={cell.id}
                      style={{ padding: "0.7rem 0.6rem 0.7rem 0", borderTop: "1px solid #1e293b", verticalAlign: "top" }}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

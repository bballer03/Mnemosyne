import { useMemo } from "react";
import { createColumnHelper, flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

type DuplicateArrayRow = NonNullable<AnalysisArtifact["arrayReport"]>["duplicateGroups"][number];

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

const columnHelper = createColumnHelper<DuplicateArrayRow>();

const columns = [
  columnHelper.accessor("elementType", {
    header: "Element type",
    cell: (info) => <span style={{ fontWeight: 600 }}>{info.getValue()}</span>,
  }),
  columnHelper.accessor("length", {
    header: "Length",
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("count", {
    header: "Duplicates",
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("totalWastedBytes", {
    header: "Wasted",
    cell: (info) => formatBytes(info.getValue()),
  }),
  columnHelper.accessor("contentHash", {
    header: "Content hash",
    cell: (info) => (
      <span style={{ color: "#94a3b8", fontFamily: "ui-monospace, monospace", fontSize: "0.82rem" }}>
        {info.getValue().toString(16)}
      </span>
    ),
  }),
];

export function DuplicateArrayPanel({ artifact }: { artifact: AnalysisArtifact }) {
  const report = artifact.arrayReport;
  const groups = useMemo(() => report?.duplicateGroups ?? [], [report]);

  const table = useReactTable({
    data: groups,
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  if (!report) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Duplicate Primitive Arrays</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Duplicate-array analysis is absent from this artifact. Re-analyze with{" "}
          <code>--duplicate-arrays</code> (or enable_duplicate_arrays) to populate{" "}
          <code>array_report</code>.
        </p>
      </div>
    );
  }

  if (groups.length === 0) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Duplicate Primitive Arrays</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Array analysis is present but reports no duplicate groups ({report.totalArrays.toLocaleString()}{" "}
          arrays scanned, {report.uniqueContents.toLocaleString()} unique contents).
        </p>
      </div>
    );
  }

  return (
    <div style={{ display: "grid", gap: "0.75rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Duplicate Primitive Arrays</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          {groups.length.toLocaleString()} duplicate group
          {groups.length === 1 ? "" : "s"} · {formatBytes(report.totalDuplicateWaste)} wasted ·{" "}
          {report.totalArrays.toLocaleString()} arrays scanned
        </p>
      </div>

      <div style={{ overflowX: "auto" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.9rem" }}>
          <thead>
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <th
                    key={header.id}
                    style={{
                      textAlign: "left",
                      padding: "0.45rem 0.55rem",
                      borderBottom: "1px solid #1e293b",
                      color: "#94a3b8",
                      fontWeight: 500,
                    }}
                  >
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
                    style={{
                      padding: "0.55rem",
                      borderBottom: "1px solid #0f172a",
                      verticalAlign: "top",
                    }}
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

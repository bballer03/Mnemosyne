import { useMemo } from "react";
import { createColumnHelper, flexRender, tableFeatures, useTable } from "@tanstack/react-table";

import type { ObjectDelta, ObjectDeltaKind } from "../../lib/diff-types";

const features = tableFeatures({});

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${bytes} B`;
}

function severityTone(severity: string) {
  if (severity === "CRITICAL") {
    return { text: "#fecaca", border: "#7f1d1d", background: "rgba(127, 29, 29, 0.22)" };
  }

  if (severity === "HIGH") {
    return { text: "#fdba74", border: "#7c2d12", background: "rgba(124, 45, 18, 0.22)" };
  }

  return { text: "#cbd5e1", border: "#334155", background: "rgba(30, 41, 59, 0.7)" };
}

const kindLabel: Record<ObjectDeltaKind, string> = {
  Added: "Added",
  Removed: "Removed",
  RetainedChanged: "Retained changed",
};

const kindEmptyCopy: Record<ObjectDeltaKind, string> = {
  Added: "No object classes were added between these two heaps.",
  Removed: "No object classes were removed between these two heaps.",
  RetainedChanged: "No object classes changed retained size beyond the configured threshold.",
};

const columnHelper = createColumnHelper<typeof features, ObjectDelta>();

export function ObjectDeltaTable({ kind, deltas }: { kind: ObjectDeltaKind; deltas: ObjectDelta[] }) {
  const columns = useMemo(
    () =>
      columnHelper.columns([
        columnHelper.accessor("className", {
          header: "Class",
          cell: (info) => (
            <div>
              <div style={{ fontWeight: 600, overflowWrap: "anywhere" }}>{info.getValue()}</div>
              <div style={{ color: "#64748b", fontSize: "0.82rem", marginTop: "0.2rem" }}>
                example object id: {info.row.original.exampleObjectId}
              </div>
            </div>
          ),
        }),
        columnHelper.display({
          id: "count",
          header: "Count (before -> after)",
          cell: (info) =>
            `${info.row.original.beforeCount.toLocaleString()} -> ${info.row.original.afterCount.toLocaleString()}`,
        }),
        columnHelper.display({
          id: "retained",
          header: "Retained (before -> after)",
          cell: (info) =>
            `${formatBytes(info.row.original.beforeRetainedBytes)} -> ${formatBytes(info.row.original.afterRetainedBytes)}`,
        }),
        columnHelper.accessor("dominatorChain", {
          header: "Dominator chain",
          cell: (info) => {
            const chain = info.getValue();
            return chain.length > 0 ? chain.join(" -> ") : "-";
          },
        }),
        columnHelper.accessor("leakSeverity", {
          header: "Leak severity",
          cell: (info) => {
            const severity = info.getValue();

            if (!severity) {
              return <span style={{ color: "#64748b" }}>-</span>;
            }

            const tone = severityTone(severity);

            return (
              <span
                style={{
                  display: "inline-flex",
                  borderRadius: 999,
                  border: `1px solid ${tone.border}`,
                  color: tone.text,
                  background: tone.background,
                  padding: "0.2rem 0.5rem",
                  fontSize: "0.78rem",
                  letterSpacing: "0.06em",
                }}
              >
                {severity}
              </span>
            );
          },
        }),
      ]),
    [],
  );

  const table = useTable({
    features,
    data: deltas,
    columns,
  });

  return (
    <section
      aria-label={`${kindLabel[kind]} objects`}
      style={{
        border: "1px solid #1e293b",
        borderRadius: 18,
        background: "rgba(2, 6, 23, 0.78)",
        padding: "1rem",
        display: "grid",
        gap: "0.75rem",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "baseline" }}>
        <h3 style={{ margin: 0, fontSize: "1rem" }}>{kindLabel[kind]}</h3>
        <span style={{ color: "#64748b", fontSize: "0.85rem" }}>{deltas.length.toLocaleString()} classes</span>
      </div>

      {deltas.length === 0 ? (
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>{kindEmptyCopy[kind]}</p>
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
                  {row.getAllCells().map((cell) => (
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
    </section>
  );
}

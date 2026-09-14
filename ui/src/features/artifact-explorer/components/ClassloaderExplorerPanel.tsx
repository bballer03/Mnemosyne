import type { AnalysisArtifact } from "../../../lib/analysis-types";

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

const cardStyle = {
  display: "grid",
  gap: "0.5rem",
  borderRadius: 16,
  border: "1px solid #1e293b",
  background: "rgba(2, 6, 23, 0.75)",
  padding: "0.9rem",
} as const;

export function ClassloaderExplorerPanel({ artifact }: { artifact: AnalysisArtifact }) {
  const report = artifact.classloaderReport;

  if (!report) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Classloader Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Classloader analysis is absent from this artifact. Re-run <code>mnemosyne analyze --classloaders</code> to include it.
        </p>
      </div>
    );
  }

  const hasLoaders = report.loaders.length > 0;
  const hasDuplicates = report.duplicateClasses.length > 0;

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Classloader Explorer</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Loader-level aggregates and cross-loader duplicate classes from the loaded artifact.
        </p>
      </div>

      <section aria-label="Duplicate classes across loaders" style={cardStyle}>
        <strong>Duplicate classes across loaders ({report.duplicateClasses.length})</strong>
        {hasDuplicates ? (
          <div style={{ display: "grid", gap: "0.6rem" }}>
            {report.duplicateClasses.map((group) => (
              <div key={group.className} style={{ display: "grid", gap: "0.15rem" }}>
                <span style={{ overflowWrap: "anywhere" }}>{group.className}</span>
                <span style={{ color: "#94a3b8", fontSize: "0.88rem" }}>
                  loaded by {group.loaderCount} loaders: {group.loaderObjectIds.map((id) => `0x${id.toString(16)}`).join(", ")}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            No class name was loaded by more than one distinct loader in this artifact.
          </p>
        )}
      </section>

      <section aria-label="Loaders" style={cardStyle}>
        <strong>Loaders ({report.loaders.length})</strong>
        {hasLoaders ? (
          <div style={{ overflowX: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead>
                <tr style={{ textAlign: "left", color: "#94a3b8" }}>
                  <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Loader</th>
                  <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Loaded classes</th>
                  <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Unique classes</th>
                  <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Retained</th>
                  <th style={{ padding: "0 0.6rem 0.5rem 0" }}>Ancestor chain</th>
                </tr>
              </thead>
              <tbody>
                {report.loaders.map((loader) => (
                  <tr key={loader.objectId}>
                    <td style={{ padding: "0.6rem 0.6rem 0.6rem 0", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      <div style={{ fontWeight: 600, overflowWrap: "anywhere" }}>{loader.className}</div>
                      <div style={{ color: "#64748b", fontSize: "0.82rem" }}>{`0x${loader.objectId.toString(16)}`}</div>
                    </td>
                    <td style={{ padding: "0.6rem 0.6rem 0.6rem 0", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      {loader.loadedClassCount.toLocaleString()}
                    </td>
                    <td style={{ padding: "0.6rem 0.6rem 0.6rem 0", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      {loader.uniqueClassCount.toLocaleString()}
                    </td>
                    <td style={{ padding: "0.6rem 0.6rem 0.6rem 0", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      {formatBytes(loader.retainedBytes)}
                    </td>
                    <td style={{ padding: "0.6rem 0.6rem 0.6rem 0", borderTop: "1px solid #1e293b", verticalAlign: "top" }}>
                      {loader.ancestorChain.length > 0 ? (
                        loader.ancestorChain.map((id) => `0x${id.toString(16)}`).join(" -> ")
                      ) : (
                        <span style={{ color: "#64748b" }}>-</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
            Classloader analysis is present but reports no loaders.
          </p>
        )}
      </section>
    </div>
  );
}

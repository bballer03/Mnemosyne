import type { AnalysisArtifact } from "../../../lib/analysis-types";

type PluginFindingsPanelProps = {
  artifact: AnalysisArtifact;
};

export function PluginFindingsPanel({ artifact }: PluginFindingsPanelProps) {
  const pluginResults = artifact.pluginResults;

  if (pluginResults === undefined) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Static Plugin Findings</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          No <code>plugin_results</code> were serialized into this artifact. Standard builds ship an
          empty static plugin registry, so this absence is expected unless custom analyzers were
          registered at process startup.
        </p>
      </div>
    );
  }

  if (pluginResults.length === 0) {
    return (
      <div style={{ display: "grid", gap: "0.75rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Static Plugin Findings</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          <code>plugin_results</code> is present but empty.
        </p>
      </div>
    );
  }

  const findingCount = pluginResults.reduce((total, plugin) => total + plugin.findings.length, 0);

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0, fontSize: "1.05rem" }}>Static Plugin Findings</h2>
        <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
          Provenance-bearing findings from statically registered analyzers ({pluginResults.length}{" "}
          plugin{pluginResults.length === 1 ? "" : "s"}, {findingCount} finding
          {findingCount === 1 ? "" : "s"}). Plugin name and finding text are untrusted display
          strings.
        </p>
      </div>

      <div style={{ display: "grid", gap: "0.85rem" }}>
        {pluginResults.map((plugin, pluginIndex) => (
          <section
            key={`${plugin.name}-${pluginIndex}`}
            style={{
              display: "grid",
              gap: "0.65rem",
              borderRadius: 16,
              border: "1px solid #1e293b",
              background: "rgba(2, 6, 23, 0.75)",
              padding: "0.9rem",
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", gap: "0.75rem" }}>
              <strong style={{ overflowWrap: "anywhere" }}>{plugin.name}</strong>
              <span
                style={{
                  color: "#94a3b8",
                  fontSize: "0.72rem",
                  letterSpacing: "0.08em",
                  textTransform: "uppercase",
                  whiteSpace: "nowrap",
                }}
              >
                Plugin
              </span>
            </div>

            {plugin.findings.length === 0 ? (
              <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.6 }}>
                This plugin returned no findings.
              </p>
            ) : (
              plugin.findings.map((finding, findingIndex) => (
                <article
                  key={`${plugin.name}-finding-${findingIndex}`}
                  style={{
                    display: "grid",
                    gap: "0.35rem",
                    borderRadius: 12,
                    border: "1px solid #334155",
                    background: "rgba(15, 23, 42, 0.7)",
                    padding: "0.75rem",
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", gap: "0.75rem" }}>
                    <span style={{ overflowWrap: "anywhere" }}>{finding.summary}</span>
                    <span style={{ color: "#facc15", fontSize: "0.85rem", whiteSpace: "nowrap" }}>
                      {finding.severity}
                    </span>
                  </div>
                  {finding.detail ? (
                    <pre
                      style={{
                        margin: 0,
                        whiteSpace: "pre-wrap",
                        overflowWrap: "anywhere",
                        color: "#94a3b8",
                        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                        fontSize: "0.82rem",
                        lineHeight: 1.5,
                      }}
                    >
                      {finding.detail}
                    </pre>
                  ) : null}
                </article>
              ))
            )}
          </section>
        ))}
      </div>
    </div>
  );
}

import type { MatchQuality, Risk } from "../../lib/diff-types";

function riskTone(risk: Risk) {
  if (risk === "High") {
    return { text: "#fecaca", border: "#7f1d1d", background: "rgba(127, 29, 29, 0.22)" };
  }

  if (risk === "Medium") {
    return { text: "#fdba74", border: "#7c2d12", background: "rgba(124, 45, 18, 0.22)" };
  }

  return { text: "#bbf7d0", border: "#14532d", background: "rgba(20, 83, 45, 0.22)" };
}

function formatPercent(rate: number) {
  return `${(rate * 100).toFixed(2)}%`;
}

function RiskPill({ label, risk }: { label: string; risk: Risk }) {
  const tone = riskTone(risk);

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "0.35rem",
        borderRadius: 999,
        border: `1px solid ${tone.border}`,
        color: tone.text,
        background: tone.background,
        padding: "0.2rem 0.6rem",
        fontSize: "0.78rem",
      }}
    >
      {label}: {risk}
    </span>
  );
}

export function MatchQualityBadge({ matchQuality }: { matchQuality: MatchQuality }) {
  return (
    <section
      aria-label="Match quality"
      style={{
        border: "1px solid #1e293b",
        borderRadius: 18,
        background: "rgba(2, 6, 23, 0.78)",
        padding: "0.9rem 1rem",
        display: "grid",
        gap: "0.6rem",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", flexWrap: "wrap", alignItems: "center" }}>
        <div>
          <div style={{ color: "#38bdf8", fontSize: "0.78rem", letterSpacing: "0.1em", textTransform: "uppercase" }}>
            Match quality
          </div>
          <div style={{ color: "#e2e8f0", fontWeight: 600, marginTop: "0.2rem" }}>{matchQuality.strategy}</div>
        </div>
        <div style={{ color: "#cbd5e1" }}>
          Collision rate: <strong>{formatPercent(matchQuality.collisionRate)}</strong>
        </div>
      </div>

      <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
        <RiskPill label="False-match risk" risk={matchQuality.estimatedFalseMatchRisk} />
        <RiskPill label="False-split risk" risk={matchQuality.estimatedFalseSplitRisk} />
      </div>

      {matchQuality.notes.length > 0 ? (
        <ul style={{ margin: 0, paddingLeft: "1.2rem", color: "#94a3b8", lineHeight: 1.6 }}>
          {matchQuality.notes.map((note, index) => (
            <li key={`${index}-${note.slice(0, 24)}`}>{note}</li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

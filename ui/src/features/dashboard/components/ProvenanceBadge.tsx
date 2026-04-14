function colorForKind(kind: string) {
  switch (kind.toUpperCase()) {
    case "FALLBACK":
      return { border: "#7c2d12", text: "#fdba74", background: "rgba(124, 45, 18, 0.2)" };
    case "PARTIAL":
      return { border: "#1d4ed8", text: "#93c5fd", background: "rgba(29, 78, 216, 0.2)" };
    case "SYNTHETIC":
      return { border: "#4c1d95", text: "#c4b5fd", background: "rgba(76, 29, 149, 0.22)" };
    default:
      return { border: "#334155", text: "#cbd5e1", background: "rgba(30, 41, 59, 0.75)" };
  }
}

export function ProvenanceBadge({ kind }: { kind: string }) {
  const color = colorForKind(kind);

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        borderRadius: 999,
        border: `1px solid ${color.border}`,
        background: color.background,
        color: color.text,
        padding: "0.2rem 0.55rem",
        fontSize: "0.72rem",
        letterSpacing: "0.08em",
        textTransform: "uppercase",
      }}
    >
      {kind}
    </span>
  );
}

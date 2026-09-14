export function WorkbenchPlaceholderPage({
  title,
  summary,
}: {
  title: string;
  summary: string;
}) {
  return (
    <main style={{ display: "grid", gap: "1rem", padding: "1.5rem" }}>
      <p
        style={{
          margin: 0,
          color: "#38bdf8",
          fontSize: "0.78rem",
          letterSpacing: "0.16em",
          textTransform: "uppercase",
        }}
      >
        Workbench
      </p>
      <h1 style={{ margin: 0, fontSize: "clamp(1.6rem, 3vw, 2.2rem)" }}>{title}</h1>
      <p style={{ margin: 0, color: "#94a3b8", lineHeight: 1.7, maxWidth: "68ch" }}>{summary}</p>
      <p style={{ margin: 0, color: "#64748b", lineHeight: 1.6 }}>
        This route is reserved for M20 workbench surfaces. Backend capabilities may already exist;
        the dedicated UI panel lands in a follow-up slice.
      </p>
    </main>
  );
}

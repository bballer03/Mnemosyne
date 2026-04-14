import { AppProviders } from "./providers";
import { AppRouter } from "./router";

export function App() {
  return (
    <AppProviders>
      <div
        style={{
          minHeight: "100vh",
          background:
            "radial-gradient(circle at top, rgba(14, 165, 233, 0.12), transparent 28%), #020617",
          color: "#e2e8f0",
        }}
      >
        <header
          style={{
            position: "sticky",
            top: 0,
            zIndex: 1,
            backdropFilter: "blur(14px)",
            background: "rgba(2, 6, 23, 0.82)",
            borderBottom: "1px solid #0f172a",
          }}
        >
          <div
            style={{
              maxWidth: "1200px",
              margin: "0 auto",
              padding: "1rem 1.5rem",
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              gap: "1rem",
              flexWrap: "wrap",
            }}
          >
            <div>
          <h1>Mnemosyne</h1>
              <p style={{ margin: "0.35rem 0 0", color: "#94a3b8" }}>Heap analysis console</p>
            </div>
            <div
              style={{
                border: "1px solid #1e293b",
                borderRadius: 999,
                padding: "0.45rem 0.8rem",
                color: "#67e8f9",
                fontSize: "0.82rem",
                letterSpacing: "0.08em",
                textTransform: "uppercase",
              }}
            >
              Browser-first local workflow
            </div>
          </div>
        </header>
        <div
          style={{
            maxWidth: "1200px",
            margin: "0 auto",
            padding: "2rem 1.5rem 3rem",
          }}
        >
          <AppRouter />
        </div>
      </div>
    </AppProviders>
  );
}

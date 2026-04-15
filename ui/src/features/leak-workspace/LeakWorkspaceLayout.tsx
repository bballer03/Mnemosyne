import { useEffect, useState } from "react";
import { Link, Navigate, Outlet, useLocation, useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { useLeakWorkspaceStore } from "./leak-workspace-store";

const shellStyle = {
  display: "grid",
  gap: "1rem",
} as const;

const panelStyle = {
  border: "1px solid #1e293b",
  borderRadius: 24,
  background: "linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(2, 6, 23, 0.96))",
  padding: "1.3rem",
} as const;

const subtleTextStyle = {
  margin: 0,
  color: "#94a3b8",
} as const;

const inputStyle = {
  borderRadius: 12,
  border: "1px solid #334155",
  background: "rgba(2, 6, 23, 0.82)",
  color: "#e2e8f0",
  padding: "0.65rem 0.8rem",
} as const;

const buttonStyle = {
  borderRadius: 999,
  border: "1px solid #334155",
  background: "rgba(15, 23, 42, 0.8)",
  color: "#cbd5e1",
  padding: "0.35rem 0.7rem",
  cursor: "pointer",
} as const;

export function LeakWorkspaceLayout() {
  const { artifact, artifactName } = useArtifactStore();
  const { leakId } = useParams();
  const location = useLocation();
  const setSelection = useLeakWorkspaceStore((state) => state.setSelection);
  const projectRoot = useLeakWorkspaceStore((state) => state.projectRoot);
  const objectId = useLeakWorkspaceStore((state) => state.objectId);
  const [projectRootDraft, setProjectRootDraft] = useState(projectRoot ?? "");
  const [objectIdDraft, setObjectIdDraft] = useState(objectId ?? "");
  const resolvedLeakId = leakId ?? "unknown";
  const encodedLeakId = encodeURIComponent(resolvedLeakId);
  const basePath = `/leaks/${encodedLeakId}`;
  const leak = artifact?.leaks.find((entry) => entry.id === leakId);
  const recentObjectTargets = useLeakWorkspaceStore(
    (state) => (leak?.id ? state.recentObjectTargetsByLeak[leak.id] ?? [] : []),
  );
  const tabs = [
    { to: `${basePath}/overview`, label: "Overview" },
    { to: `${basePath}/explain`, label: "Explain" },
    { to: `${basePath}/gc-path`, label: "GC Path" },
    { to: `${basePath}/source-map`, label: "Source Map" },
    { to: `${basePath}/fix`, label: "Fix Proposal" },
  ];

  useEffect(() => {
    if (!artifact || !leak) {
      return;
    }

    setSelection({
      leakId: leak.id,
      heapPath: artifact.summary.heapPath,
    });
  }, [artifact, leak, setSelection]);

  useEffect(() => {
    setProjectRootDraft(projectRoot ?? "");
  }, [projectRoot]);

  useEffect(() => {
    setObjectIdDraft(objectId ?? "");
  }, [objectId]);

  if (!artifact) {
    return <Navigate to="/" replace />;
  }

  return (
    <main style={shellStyle}>
      <section style={panelStyle}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <div>
            <Link to="/dashboard">Back to dashboard</Link>
          </div>
          <div style={{ color: "#38bdf8", fontSize: "0.78rem", letterSpacing: "0.16em", textTransform: "uppercase" }}>
            Leak Workspace
          </div>
          <h1 style={{ margin: 0, fontSize: "1.1rem", color: "#e2e8f0" }}>Leak Workspace</h1>
          <div style={{ fontSize: "1rem", color: "#cbd5e1", overflowWrap: "anywhere" }}>{artifact.summary.heapPath}</div>
          <h2 style={{ margin: 0, fontSize: "clamp(1.8rem, 4vw, 2.6rem)", lineHeight: 1.08 }}>
            {leak?.className ?? "Invalid leak selection"}
          </h2>
          <p style={subtleTextStyle}>Leak ID: {resolvedLeakId}</p>
          <p style={subtleTextStyle}>Artifact: {artifactName ?? "Unnamed artifact"}</p>
        </header>
      </section>

      <section style={panelStyle}>
        <section style={{ display: "grid", gap: "1rem" }}>
          <h3 style={{ margin: 0 }}>Local Context Activation</h3>
          <div style={{ display: "grid", gap: "0.5rem" }}>
            <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
              <span>Project root</span>
              <input
                aria-label="Project root"
                type="text"
                value={projectRootDraft}
                onChange={(event) => setProjectRootDraft(event.target.value)}
                placeholder="D:/repo"
                style={inputStyle}
              />
            </label>
            <div style={{ color: "#94a3b8", fontSize: "0.9rem" }}>
              Unlocks source map and fix follow-through for this workspace.
            </div>
            <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
              <button
                type="button"
                style={buttonStyle}
                onClick={() => setSelection({ projectRoot: projectRootDraft.trim() || undefined })}
              >
                Apply project root
              </button>
              <button
                type="button"
                style={buttonStyle}
                onClick={() => setSelection({ projectRoot: undefined })}
              >
                Clear project root
              </button>
            </div>
          </div>

          <div style={{ display: "grid", gap: "0.5rem" }}>
            <label style={{ display: "grid", gap: "0.35rem", color: "#cbd5e1" }}>
              <span>Object target ID</span>
              <input
                aria-label="Object target ID"
                type="text"
                value={objectIdDraft}
                onChange={(event) => setObjectIdDraft(event.target.value)}
                placeholder="0x1234"
                style={inputStyle}
              />
            </label>
            <div style={{ color: "#94a3b8", fontSize: "0.9rem" }}>
              Unlocks the GC path route when you have a real object target.
            </div>
            <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
              <button
                type="button"
                style={buttonStyle}
                onClick={() => setSelection({ objectId: objectIdDraft.trim() || undefined })}
              >
                Apply object target
              </button>
              <button
                type="button"
                style={buttonStyle}
                onClick={() => setSelection({ objectId: undefined })}
              >
                Clear object target
              </button>
            </div>
            {recentObjectTargets.length ? (
              <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
                {recentObjectTargets.map((target) => (
                  <button
                    key={target}
                    type="button"
                    style={buttonStyle}
                    onClick={() => {
                      setObjectIdDraft(target);
                      setSelection({ objectId: target });
                    }}
                  >
                    Reuse {target}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        </section>
      </section>

      <section style={panelStyle}>
        <nav aria-label="Leak workspace modes" style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap" }}>
          {tabs.map((tab) => {
            const active = location.pathname === tab.to;

            return (
              <Link key={tab.to} to={tab.to} aria-current={active ? "page" : undefined}>
                {tab.label}
              </Link>
            );
          })}
        </nav>
      </section>

      <section style={panelStyle}>
        {leak ? (
          <Outlet />
        ) : (
          <div style={{ display: "grid", gap: "0.75rem" }}>
            <p style={subtleTextStyle}>Selected leak was not found in the loaded artifact.</p>
            <div>
              <Link to="/dashboard">Back to dashboard</Link>
            </div>
          </div>
        )}
      </section>
    </main>
  );
}

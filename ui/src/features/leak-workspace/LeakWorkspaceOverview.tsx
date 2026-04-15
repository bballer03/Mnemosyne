import { useParams } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { getLeakWorkspaceBridgeStatus } from "./live-detail-client";
import type { LeakWorkspaceDependencyStatus } from "./types";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

const sectionStyle = {
  display: "grid",
  gap: "0.5rem",
} as const;

export function LeakWorkspaceOverview() {
  const { artifact } = useArtifactStore();
  const { leakId } = useParams();
  const projectRoot = useLeakWorkspaceStore((state) => state.projectRoot);
  const objectId = useLeakWorkspaceStore((state) => state.objectId);

  if (!artifact) {
    return null;
  }

  const leak = artifact.leaks.find((entry) => entry.id === leakId);

  if (!leak) {
    return null;
  }

  const bridgeStatus = getLeakWorkspaceBridgeStatus();

  const dependencyStatus: LeakWorkspaceDependencyStatus = {
    bridge: bridgeStatus.bridge,
    projectRoot: projectRoot ? "present" : "missing",
    objectTarget: objectId ? "present" : "missing",
    provider: bridgeStatus.provider,
  };

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section style={sectionStyle}>
        <h3 style={{ margin: 0 }}>Overview</h3>
        <div>{leak.description}</div>
      </section>

      <section aria-label="Dependency readiness" style={sectionStyle}>
        <h4 style={{ margin: 0 }}>Dependency readiness</h4>
        <div>Bridge: {dependencyStatus.bridge}</div>
        <div>Project root: {dependencyStatus.projectRoot}</div>
        <div>Object target: {dependencyStatus.objectTarget}</div>
        <div>Provider: {dependencyStatus.provider}</div>
      </section>

      <section style={sectionStyle}>
        <div>Explain preview</div>
        <div>{leak.className}</div>
      </section>

      <section style={sectionStyle}>
        <div>GC Path preview</div>
        <div>
          {dependencyStatus.objectTarget === "present"
            ? "GC path can be loaded from a concrete object target."
            : "GC path unavailable until an object target is present."}
        </div>
      </section>

      <section style={sectionStyle}>
        <div>Source Map preview</div>
        <div>Heap path: {artifact.summary.heapPath}</div>
      </section>

      <section style={sectionStyle}>
        <div>Fix Proposal preview</div>
        <div>Leak ID: {leak.id}</div>
      </section>
    </section>
  );
}

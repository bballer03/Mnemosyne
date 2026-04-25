import { useOutletContext } from "react-router-dom";

import type { HeapExplorerOutletContext } from "./HeapExplorerLayout";
import { ExplorerCrossNavActions } from "./components/ExplorerCrossNavActions";
import { QueryConsolePanel } from "./components/QueryConsolePanel";

export function HeapQueryConsolePage() {
  const { artifact, selectedObject, resolvedLeakId } = useOutletContext<HeapExplorerOutletContext>();

  return (
    <section style={{ display: "grid", gap: "0.9rem" }}>
      <ExplorerCrossNavActions objectId={selectedObject?.objectId || undefined} leakId={resolvedLeakId} />
      <QueryConsolePanel heapPath={artifact.summary.heapPath} />
    </section>
  );
}

import { useOutletContext } from "react-router-dom";

import type { HeapExplorerOutletContext } from "./HeapExplorerLayout";
import { ExplorerCrossNavActions } from "./components/ExplorerCrossNavActions";
import { ThreadExplorerPanel } from "./components/ThreadExplorerPanel";

export function HeapThreadsPage() {
  const { artifact, selectedObject, resolvedLeakId } = useOutletContext<HeapExplorerOutletContext>();

  return (
    <section style={{ display: "grid", gap: "0.9rem" }}>
      <ExplorerCrossNavActions objectId={selectedObject?.objectId || undefined} leakId={resolvedLeakId} />
      <ThreadExplorerPanel artifact={artifact} />
    </section>
  );
}

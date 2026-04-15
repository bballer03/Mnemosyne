import { useOutletContext } from "react-router-dom";

import type { HeapExplorerOutletContext } from "./HeapExplorerLayout";
import { DominatorExplorerPanel } from "./components/DominatorExplorerPanel";
import { ExplorerCrossNavActions } from "./components/ExplorerCrossNavActions";

export function HeapDominatorPage() {
  const { artifact, selectedObject, selectedRowIndex, setSelectedRowIndex } = useOutletContext<HeapExplorerOutletContext>();

  return (
    <section style={{ display: "grid", gap: "0.9rem" }}>
      <ExplorerCrossNavActions objectId={selectedObject?.objectId || undefined} />
      <DominatorExplorerPanel
        rows={artifact.graph.dominators}
        selectedRowIndex={selectedRowIndex}
        onSelectRowIndex={setSelectedRowIndex}
      />
    </section>
  );
}

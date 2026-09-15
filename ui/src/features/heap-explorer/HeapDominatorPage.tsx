import { useOutletContext } from "react-router-dom";

import { useInvestigationStore } from "../investigation/investigation-store";

import type { HeapExplorerOutletContext } from "./HeapExplorerLayout";
import { DominatorExplorerPanel } from "./components/DominatorExplorerPanel";
import { ExplorerCrossNavActions } from "./components/ExplorerCrossNavActions";

export function HeapDominatorPage() {
  const { artifact, selectedObject, resolvedLeakId, selectedRowIndex, setSelectedRowIndex } = useOutletContext<HeapExplorerOutletContext>();
  const selectedObjectId = useInvestigationStore((state) => state.objectId);
  const setObjectId = useInvestigationStore((state) => state.setObjectId);

  function handleLiveObjectSelection(objectId: string) {
    setSelectedRowIndex(undefined);
    setObjectId(objectId, "dominators");
  }

  return (
    <section style={{ display: "grid", gap: "0.9rem" }}>
      <ExplorerCrossNavActions objectId={selectedObject?.objectId || undefined} leakId={resolvedLeakId} />
      <DominatorExplorerPanel
        rows={artifact.graph.dominators}
        totalSizeBytes={artifact.summary.totalSizeBytes}
        selectedObjectId={selectedObjectId}
        onSelectObjectId={handleLiveObjectSelection}
        selectedRowIndex={selectedRowIndex}
        onSelectRowIndex={setSelectedRowIndex}
      />
    </section>
  );
}

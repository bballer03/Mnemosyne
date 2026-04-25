import { useOutletContext } from "react-router-dom";

import type { HeapExplorerOutletContext } from "./HeapExplorerLayout";
import { ExplorerCrossNavActions } from "./components/ExplorerCrossNavActions";
import { ObjectInspectorPanel } from "./components/ObjectInspectorPanel";

export function HeapObjectInspectorPage() {
  const { artifact, selectedObject, resolvedLeakId, selectedRowIndex } = useOutletContext<HeapExplorerOutletContext>();

  return (
    <section style={{ display: "grid", gap: "0.9rem" }}>
      <ExplorerCrossNavActions objectId={selectedObject?.objectId || undefined} leakId={resolvedLeakId} />
      <ObjectInspectorPanel artifact={artifact} selectedRowIndex={selectedRowIndex} />
    </section>
  );
}

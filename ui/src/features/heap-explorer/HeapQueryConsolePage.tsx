import { useOutletContext } from "react-router-dom";

import type { HeapExplorerOutletContext } from "./HeapExplorerLayout";
import { QueryConsolePanel } from "./components/QueryConsolePanel";

export function HeapQueryConsolePage() {
  const { artifact } = useOutletContext<HeapExplorerOutletContext>();

  return <QueryConsolePanel heapPath={artifact.summary.heapPath} />;
}

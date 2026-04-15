import { Link } from "react-router-dom";

type ExplorerCrossNavActionsProps = {
  objectId?: string;
  leakId?: string;
};

export function ExplorerCrossNavActions({ objectId, leakId }: ExplorerCrossNavActionsProps) {
  const encodedObjectId = objectId ? encodeURIComponent(objectId) : undefined;

  return (
    <div style={{ display: "flex", gap: "0.65rem", flexWrap: "wrap" }}>
      <Link to={encodedObjectId ? `/heap-explorer/object-inspector?objectId=${encodedObjectId}` : "/heap-explorer/object-inspector"}>
        Open Object Inspector
      </Link>
      <Link to={encodedObjectId ? `/heap-explorer/query-console?objectId=${encodedObjectId}` : "/heap-explorer/query-console"}>
        Open Query Console
      </Link>
      {leakId ? <Link to={`/leaks/${encodeURIComponent(leakId)}/overview`}>Open Leak Workspace</Link> : null}
    </div>
  );
}

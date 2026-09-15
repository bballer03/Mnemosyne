export type PerspectiveId =
  | "leak-hunt"
  | "dominator-browse"
  | "compare"
  | "oql-lab";

export type WorkbenchPerspective = Readonly<{
  id: PerspectiveId;
  name: string;
  description: string;
  route: string;
  panes: readonly string[];
}>;

export const DEFAULT_PERSPECTIVE_ID: PerspectiveId = "leak-hunt";

export const WORKBENCH_PERSPECTIVES: readonly WorkbenchPerspective[] = [
  {
    id: "leak-hunt",
    name: "Leak Hunt",
    description: "Suspects, histogram, and GC-path follow-through",
    route: "/dashboard",
    panes: ["Leak suspects", "Histogram", "GC paths"],
  },
  {
    id: "dominator-browse",
    name: "Dominator Browse",
    description: "Retained-size tree with object inspection",
    route: "/heap-explorer/dominators",
    panes: ["Dominator tree", "Object inspector", "References"],
  },
  {
    id: "compare",
    name: "Compare",
    description: "Baseline selection and ranked object deltas",
    route: "/compare",
    panes: ["Snapshot pair", "Match quality", "Object deltas"],
  },
  {
    id: "oql-lab",
    name: "OQL Lab",
    description: "Query editor, history, and result navigation",
    route: "/heap-explorer/query-console",
    panes: ["OQL editor", "Query history", "Results"],
  },
];

const perspectiveIds = WORKBENCH_PERSPECTIVES.map((perspective) => perspective.id);

export function isPerspectiveId(value: unknown): value is PerspectiveId {
  return (
    typeof value === "string" &&
    perspectiveIds.includes(value as PerspectiveId)
  );
}

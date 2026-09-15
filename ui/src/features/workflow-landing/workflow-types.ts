export const WORKFLOW_KIND_IDS = [
  "triage_memory_leak",
  "tune_gc",
  "traverse_object_graph",
  "compare_snapshots",
  "classloader_leak",
] as const;

export type WorkflowKindId = (typeof WORKFLOW_KIND_IDS)[number];

export type WorkspaceRequestSlot = "workflow" | "assistant";

export type WorkspaceRequestContext = Readonly<{
  workspaceId: string;
  revision: number;
  operationId: string;
}>;

export type WorkspaceWorkflowBinding = Readonly<{
  workspaceId: string;
  revision: number;
  workflowId: string;
  kind: WorkflowKindId;
  currentStep: string;
}>;

export type PersistedWorkflowBinding = Readonly<
  Omit<WorkspaceWorkflowBinding, "workspaceId">
>;

export const WORKFLOW_KIND_LABELS: Record<WorkflowKindId, string> = {
  triage_memory_leak: "Triage memory leak",
  tune_gc: "Tune GC",
  traverse_object_graph: "Traverse object graph",
  compare_snapshots: "Compare snapshots",
  classloader_leak: "Classloader leak",
};

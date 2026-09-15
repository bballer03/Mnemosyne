export const OPERATION_KINDS = [
  "open",
  "analyze",
  "enrich",
  "query",
  "diff",
  "snapshot",
  "flamegraph",
  "gc-path",
  "inspect",
] as const;

export type OperationKind = (typeof OPERATION_KINDS)[number];

export const OPERATION_PHASES = [
  "accepted",
  "opening",
  "parsing",
  "building-graph",
  "computing-dominators",
  "analyzing",
  "rendering",
  "committing",
  "cancelling",
  "cancelled",
  "complete",
  "failed",
] as const;

export type OperationPhase = (typeof OPERATION_PHASES)[number];

export type OperationContext = {
  workspaceId: string;
  revision: number;
  operationId: string;
};

export type OperationEnvelope<T> = OperationContext & {
  data: T;
};

let fallbackIdSequence = 0;

function createOpaqueId(prefix: "workspace" | "operation"): string {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (typeof randomUUID === "function") {
    return randomUUID.call(globalThis.crypto);
  }

  fallbackIdSequence += 1;
  return `${prefix}-${fallbackIdSequence}`;
}

export function createWorkspaceId(): string {
  return createOpaqueId("workspace");
}

export function createOperationId(): string {
  return createOpaqueId("operation");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function isOperationContext(value: unknown): value is OperationContext {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value.workspaceId === "string" &&
    value.workspaceId.trim().length > 0 &&
    typeof value.revision === "number" &&
    Number.isSafeInteger(value.revision) &&
    value.revision >= 0 &&
    typeof value.operationId === "string" &&
    value.operationId.trim().length > 0
  );
}

export function isOperationEnvelope<T = unknown>(value: unknown): value is OperationEnvelope<T> {
  return isOperationContext(value) && "data" in value;
}

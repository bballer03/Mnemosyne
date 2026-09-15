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

export type OperationProgress = {
  context: OperationContext;
  kind: OperationKind;
  phase: OperationPhase;
  completed?: number;
  total?: number;
  unit?: string;
  indeterminate: boolean;
  elapsedMs: number;
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

function readOptionalCount(value: unknown): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : Number.NaN;
}

/** Validate and normalize an untrusted desktop operation-progress event. */
export function parseOperationProgress(value: unknown): OperationProgress | undefined {
  if (!isRecord(value) || !isOperationContext(value.context)) {
    return undefined;
  }

  if (
    typeof value.kind !== "string" ||
    !OPERATION_KINDS.includes(value.kind as OperationKind) ||
    typeof value.phase !== "string" ||
    !OPERATION_PHASES.includes(value.phase as OperationPhase) ||
    typeof value.indeterminate !== "boolean" ||
    typeof value.elapsedMs !== "number" ||
    !Number.isSafeInteger(value.elapsedMs) ||
    value.elapsedMs < 0
  ) {
    return undefined;
  }

  const completed = readOptionalCount(value.completed);
  const total = readOptionalCount(value.total);
  if (Number.isNaN(completed) || Number.isNaN(total)) {
    return undefined;
  }

  const unit =
    value.unit === undefined || value.unit === null
      ? undefined
      : typeof value.unit === "string" && value.unit.trim().length > 0
        ? value.unit.trim()
        : null;
  if (unit === null) {
    return undefined;
  }

  if (
    !value.indeterminate &&
    (completed === undefined || total === undefined || total <= 0 || completed > total)
  ) {
    return undefined;
  }

  return {
    context: value.context,
    kind: value.kind as OperationKind,
    phase: value.phase as OperationPhase,
    completed,
    total,
    unit,
    indeterminate: value.indeterminate,
    elapsedMs: value.elapsedMs,
  };
}

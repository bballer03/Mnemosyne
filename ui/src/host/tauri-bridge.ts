/**
 * Inject Mnemosyne host bridges when running inside the Tauri webview.
 *
 * Source of truth for desktop IPC wiring (was previously dead code under
 * `tauri/src/bridge.ts`, which never entered the Vite `ui/dist` bundle).
 * No-ops in browser / Vitest so browser-first flows stay honest.
 */
import { formatHostError } from "./format-host-error";
import type { PickHeapFileResult } from "../features/artifact-loader/desktop-heap-client";
import { useInvestigationStore } from "../features/investigation/investigation-store";
import {
  isOperationEnvelope,
  isOperationCancelledError,
  parseCancelOperationResult,
  parseOperationProgress,
  type CancelOperationResult,
  type OperationContext,
  type OperationKind,
} from "./operation-protocol";

const OPERATION_PROGRESS_EVENT = "mnemosyne://operation-progress";
let operationProgressSubscription: Promise<unknown> | undefined;

export function isTauriRuntime(): boolean {
  return typeof globalThis !== "undefined" && "__TAURI_INTERNALS__" in globalThis;
}

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

export type OperationCancellationHost = {
  cancelOperation: (operationId: string) => Promise<CancelOperationResult>;
};

async function invokeOrThrow<T>(
  invoke: InvokeFn,
  cmd: string,
  args?: Record<string, unknown>,
  options?: { logRawError?: boolean },
): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    const message = formatHostError(error, `${cmd} failed`);
    if (options?.logRawError === false) {
      console.error(`[mnemosyne] ${cmd} failed`);
    } else {
      console.error(`[mnemosyne] ${cmd} failed`, error);
    }
    throw new Error(message);
  }
}

async function subscribeToOperationProgress(): Promise<void> {
  if (!operationProgressSubscription) {
    operationProgressSubscription = import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<unknown>(OPERATION_PROGRESS_EVENT, ({ payload }) => {
          const progress = parseOperationProgress(payload);
          if (!progress) {
            return;
          }
          useInvestigationStore.getState().updateOperationProgress(progress);
        }),
      )
      .catch((error) => {
        operationProgressSubscription = undefined;
        throw error;
      });
  }

  await operationProgressSubscription;
}

async function invokeOperation<T>(
  kind: OperationKind,
  invoke: (context: OperationContext) => Promise<unknown>,
): Promise<T> {
  const store = useInvestigationStore.getState();
  const context = store.beginOperation(kind);
  let result: unknown;
  try {
    result = await invoke(context);
  } catch (error) {
    useInvestigationStore
      .getState()
      .finishOperation(context, isOperationCancelledError(error) ? "cancelled" : "failed");
    throw error;
  }
  if (!isOperationEnvelope<T>(result)) {
    useInvestigationStore.getState().finishOperation(context, "failed");
    throw new Error("Operation host returned a response without an operation envelope.");
  }
  if (
    result.workspaceId !== context.workspaceId ||
    result.revision !== context.revision ||
    result.operationId !== context.operationId
  ) {
    useInvestigationStore.getState().finishOperation(context, "failed");
    throw new Error("Operation response identity does not match the active request.");
  }

  const currentStore = useInvestigationStore.getState();
  if (!currentStore.acceptOperationResult(result)) {
    throw new Error("Operation result was ignored because the request is no longer active.");
  }
  currentStore.finishOperation(result, "complete");
  return result.data;
}

async function cancelOperationWithInvoke(
  invoke: InvokeFn,
  operationId: string,
): Promise<CancelOperationResult> {
  const raw = await invokeOrThrow<unknown>(invoke, "cancel_operation", { operationId });
  const result = parseCancelOperationResult(raw);
  if (!result || result.operationId !== operationId) {
    throw new Error("cancel_operation returned an unexpected result.");
  }
  return result;
}

export async function cancelOperation(operationId: string): Promise<CancelOperationResult> {
  if (!isTauriRuntime()) {
    throw new Error("Operation cancellation is unavailable in this environment.");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return cancelOperationWithInvoke(invoke as InvokeFn, operationId);
}

export function getOperationCancellationHost(): OperationCancellationHost | undefined {
  return isTauriRuntime() ? { cancelOperation } : undefined;
}

/** Accept camelCase (current) or snake_case (v0.4.1 regression) pick payloads. */
export function normalizePickHeapFileResult(raw: unknown): PickHeapFileResult {
  if (!raw || typeof raw !== "object") {
    throw new Error("Heap picker returned an unexpected result.");
  }
  const record = raw as Record<string, unknown>;
  if (record.status === "cancelled") {
    return { status: "cancelled" };
  }
  if (record.status === "unavailable") {
    return { status: "unavailable" };
  }
  if (record.status !== "selected") {
    throw new Error(`Heap picker returned an unknown status: ${String(record.status)}`);
  }
  const sourceId =
    (typeof record.sourceId === "string" && record.sourceId) ||
    (typeof record.source_id === "string" && record.source_id) ||
    undefined;
  const displayName =
    (typeof record.displayName === "string" && record.displayName) ||
    (typeof record.display_name === "string" && record.display_name) ||
    undefined;
  if (!sourceId || !displayName) {
    throw new Error(
      "Heap picker returned an incomplete selection (missing source id or file name).",
    );
  }
  return { status: "selected", sourceId, displayName };
}

export async function injectHostBridges(): Promise<boolean> {
  const hostWindow = globalThis.window;
  if (!hostWindow || !isTauriRuntime()) {
    return false;
  }

  const { invoke } = await import("@tauri-apps/api/core");
  const call = <T>(cmd: string, args?: Record<string, unknown>) =>
    invokeOrThrow<T>(invoke as InvokeFn, cmd, args);
  const callWithoutRawError = <T>(cmd: string, args?: Record<string, unknown>) =>
    invokeOrThrow<T>(invoke as InvokeFn, cmd, args, { logRawError: false });
  await subscribeToOperationProgress();

  hostWindow.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
    pickHeapFile: async () => normalizePickHeapFileResult(await call("pick_heap_file")),
    loadHeapFromSource: (sourceId) =>
      invokeOperation("open", (context) => call("load_heap_from_source", { sourceId, context })),
    runDesktopAnalysis: (input) =>
      invokeOperation("analyze", (context) =>
        call("run_desktop_analysis", { input: { ...input, context } }),
      ),
    unloadHeap: () => call("unload_heap"),
    getDesktopLogPath: () => call("get_desktop_log_path"),
    runCiCheck: (input) => call("run_ci_check", { input }),
    generateFlamegraph: (input) =>
      invokeOperation("flamegraph", (context) =>
        call("generate_desktop_flamegraph", { input: { ...input, context } }),
      ),
  };

  hostWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
    queryHeap: (input) =>
      invokeOperation("query", (context) =>
        callWithoutRawError("query_heap", { input: { ...input, context } }),
      ),
    getReferences: (objectId) => call("get_references", { objectId }),
    getReferrers: (objectId) => call("get_referrers", { objectId }),
    inspectObject: (objectId, retainFieldData) =>
      invokeOperation("inspect", (context) =>
        call("inspect_object", { objectId, retainFieldData, context }),
      ),
    regroupHistogram: (groupBy) => call("regroup_histogram", { groupBy }),
    listClassInstances: (classKey, offset, limit) =>
      call("list_class_instances", { classKey, offset, limit }),
    getDominatorChildren: (parentObjectId, offset, limit, minRetainedBytes) =>
      call("get_dominator_children", {
        parentObjectId,
        offset,
        limit,
        minRetainedBytes,
      }),
  };

  hostWindow.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    capabilities: {
      provider: "ready" as const,
    },
    explainLeak: (input) => call("explain_leak", input as Record<string, unknown>),
    findGcPath: (input) =>
      invokeOperation("gc-path", (context) =>
        call("find_gc_path", { ...(input as Record<string, unknown>), context }),
      ),
    findAllGcPaths: (objectId, maxPaths) =>
      invokeOperation("gc-path", (context) =>
        call("find_all_gc_paths", { objectId, maxPaths, context }),
      ),
    mapToCode: (input) => call("map_to_code", input as Record<string, unknown>),
    proposeFix: (input) => call("propose_fix", input as Record<string, unknown>),
  };

  hostWindow.__MNEMOSYNE_COMPARISON_BRIDGE__ = {
    diffObjects: (input) =>
      invokeOperation("diff", (context) =>
        call("diff_objects", { input: { ...input, context } }),
      ),
  };

  hostWindow.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
    describeWorkflow: (kind) => call("describe_workflow", { kind }),
    startWorkflow: (kind, params) =>
      call("start_workflow", {
        kind,
        heapPath: params?.heapPath,
        objectId: params?.objectId,
        beforeHeapPath: params?.beforeHeapPath,
        afterHeapPath: params?.afterHeapPath,
        beforeSnapshotKey: params?.beforeSnapshotKey,
        afterSnapshotKey: params?.afterSnapshotKey,
      }),
    nextStep: (workflowId, input) => call("next_step", { workflowId, input }),
    getWorkflow: (workflowId) => call("get_workflow", { workflowId }),
    closeWorkflow: (workflowId) => call("close_workflow", { workflowId }),
    listSnapshots: () => call("list_snapshots"),
    saveSnapshot: (sourceId, retainFieldData) =>
      invokeOperation("snapshot", (context) =>
        call("save_snapshot", {
          input: { sourceId, retainFieldData, context },
        }),
      ),
    removeSnapshot: (key) => call("remove_snapshot", { key }),
    openSnapshot: (key) =>
      invokeOperation("snapshot", (context) => call("open_snapshot", { key, context })),
  };

  hostWindow.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
    createAiSession: (input) => call("create_ai_session", { sourceId: input?.sourceId }),
    resumeAiSession: (sessionId) => call("resume_ai_session", { sessionId }),
    getAiSession: (sessionId) => call("get_ai_session", { sessionId }),
    closeAiSession: (sessionId) => call("close_ai_session", { sessionId }),
    chatSession: (input) =>
      call("chat_session", {
        sessionId: input.sessionId,
        question: input.question,
        focusLeakId: input.focusLeakId,
      }),
  };

  return true;
}

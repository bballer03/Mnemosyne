/**
 * Inject Mnemosyne host bridges when running inside the Tauri webview.
 *
 * Source of truth for desktop IPC wiring (was previously dead code under
 * `tauri/src/bridge.ts`, which never entered the Vite `ui/dist` bundle).
 * No-ops in browser / Vitest so browser-first flows stay honest.
 */
import { formatHostError } from "./format-host-error";
import type { PickHeapFileResult } from "../features/artifact-loader/desktop-heap-client";

export function isTauriRuntime(): boolean {
  return typeof globalThis !== "undefined" && "__TAURI_INTERNALS__" in globalThis;
}

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

async function invokeOrThrow<T>(
  invoke: InvokeFn,
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    const message = formatHostError(error, `${cmd} failed`);
    console.error(`[mnemosyne] ${cmd} failed`, error);
    throw new Error(message);
  }
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

  hostWindow.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
    pickHeapFile: async () => normalizePickHeapFileResult(await call("pick_heap_file")),
    loadHeapFromSource: (sourceId) => call("load_heap_from_source", { sourceId }),
    runDesktopAnalysis: (input) => call("run_desktop_analysis", { input }),
    getDesktopLogPath: () => call("get_desktop_log_path"),
    runCiCheck: (input) => call("run_ci_check", { input }),
    generateFlamegraph: (input) => call("generate_desktop_flamegraph", { input }),
  };

  hostWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
    queryHeap: (input) => call("query_heap", { input }),
    getReferences: (objectId) => call("get_references", { objectId }),
    getReferrers: (objectId) => call("get_referrers", { objectId }),
    inspectObject: (objectId, retainFieldData) =>
      call("inspect_object", { objectId, retainFieldData }),
    regroupHistogram: (groupBy) => call("regroup_histogram", { groupBy }),
  };

  hostWindow.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    capabilities: {
      provider: "ready" as const,
    },
    explainLeak: (input) => call("explain_leak", input as Record<string, unknown>),
    findGcPath: (input) => call("find_gc_path", input as Record<string, unknown>),
    findAllGcPaths: (objectId, maxPaths) => call("find_all_gc_paths", { objectId, maxPaths }),
    mapToCode: (input) => call("map_to_code", input as Record<string, unknown>),
    proposeFix: (input) => call("propose_fix", input as Record<string, unknown>),
  };

  hostWindow.__MNEMOSYNE_COMPARISON_BRIDGE__ = {
    diffObjects: (input) => call("diff_objects", { input }),
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
      call("save_snapshot", {
        input: { sourceId, retainFieldData },
      }),
    removeSnapshot: (key) => call("remove_snapshot", { key }),
    openSnapshot: (key) => call("open_snapshot", { key }),
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

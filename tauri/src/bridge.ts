// Bridge injection for Tauri - wires window.__MNEMOSYNE_*_BRIDGE__ to Tauri IPC
const hostWindow = globalThis.window;
const isTauri = hostWindow !== undefined && "__TAURI_INTERNALS__" in globalThis;

if (isTauri) {
  const { invoke } = await import("@tauri-apps/api/core");

  hostWindow.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__ = {
    pickHeapFile: () => invoke("pick_heap_file"),
    loadHeapFromSource: (sourceId) => invoke("load_heap_from_source", { sourceId }),
    runDesktopAnalysis: (input) => invoke("run_desktop_analysis", { input }),
    runCiCheck: (input) => invoke("run_ci_check", { input }),
    generateFlamegraph: (input) => invoke("generate_desktop_flamegraph", { input }),
  };

  hostWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
    queryHeap: (input) => invoke("query_heap", { input }),
    getReferences: (objectId) => invoke("get_references", { objectId }),
    getReferrers: (objectId) => invoke("get_referrers", { objectId }),
    inspectObject: (objectId, retainFieldData) =>
      invoke("inspect_object", { objectId, retainFieldData }),
    regroupHistogram: (groupBy) => invoke("regroup_histogram", { groupBy }),
  };

  hostWindow.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    capabilities: {
      provider: "ready" as const,
    },
    explainLeak: (input) => invoke("explain_leak", input),
    findGcPath: (input) => invoke("find_gc_path", input),
    findAllGcPaths: (objectId, maxPaths) =>
      invoke("find_all_gc_paths", { objectId, maxPaths }),
    mapToCode: (input) => invoke("map_to_code", input),
    proposeFix: (input) => invoke("propose_fix", input),
  };

  hostWindow.__MNEMOSYNE_COMPARISON_BRIDGE__ = {
    diffObjects: (input) => invoke("diff_objects", { input }),
  };

  hostWindow.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
    describeWorkflow: (kind) => invoke("describe_workflow", { kind }),
    startWorkflow: (kind, params) =>
      invoke("start_workflow", {
        kind,
        heapPath: params?.heapPath,
        objectId: params?.objectId,
        beforeHeapPath: params?.beforeHeapPath,
        afterHeapPath: params?.afterHeapPath,
        beforeSnapshotKey: params?.beforeSnapshotKey,
        afterSnapshotKey: params?.afterSnapshotKey,
      }),
    nextStep: (workflowId, input) => invoke("next_step", { workflowId, input }),
    getWorkflow: (workflowId) => invoke("get_workflow", { workflowId }),
    closeWorkflow: (workflowId) => invoke("close_workflow", { workflowId }),
    listSnapshots: () => invoke("list_snapshots"),
    saveSnapshot: (sourceId, retainFieldData) =>
      invoke("save_snapshot", {
        input: { sourceId, retainFieldData },
      }),
    removeSnapshot: (key) => invoke("remove_snapshot", { key }),
    openSnapshot: (key) => invoke("open_snapshot", { key }),
  };

  // M23.C — thin adapters over shipped MCP AI session / chat_session behavior.
  hostWindow.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
    createAiSession: (input) =>
      invoke("create_ai_session", { sourceId: input?.sourceId }),
    resumeAiSession: (sessionId) => invoke("resume_ai_session", { sessionId }),
    getAiSession: (sessionId) => invoke("get_ai_session", { sessionId }),
    closeAiSession: (sessionId) => invoke("close_ai_session", { sessionId }),
    chatSession: (input) =>
      invoke("chat_session", {
        sessionId: input.sessionId,
        question: input.question,
        focusLeakId: input.focusLeakId,
      }),
  };
}
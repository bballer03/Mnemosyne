// Bridge injection for Tauri - wires window.__MNEMOSYNE_*_BRIDGE__ to Tauri IPC
const hostWindow = globalThis.window;
const isTauri = hostWindow !== undefined && "__TAURI_INTERNALS__" in globalThis;

if (isTauri) {
  const { invoke } = await import("@tauri-apps/api/core");

  hostWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
    queryHeap: (input) => invoke("query_heap", { input }),
    getReferences: (objectId) => invoke("get_references", { objectId }),
    getReferrers: (objectId) => invoke("get_referrers", { objectId }),
  };

  hostWindow.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__ = {
    capabilities: {
      provider: "ready" as const,
    },
    explainLeak: (input) => invoke("explain_leak", input),
    findGcPath: (input) => invoke("find_gc_path", input),
    mapToCode: (input) => invoke("map_to_code", input),
    proposeFix: (input) => invoke("propose_fix", input),
  };
}
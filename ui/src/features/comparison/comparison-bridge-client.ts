// Bridge client for the M10 object-level diff capability, following the
// exact optional-capability-probe pattern already established by
// `heap-explorer-query-client.ts` (`isReferencesAvailable`/`getObjectReferences`)
// and `live-detail-client.ts` (`explainLeak`, etc.): an `isXAvailable()`
// guard, an explicit "unavailable" status when the bridge/method is absent,
// and never synthetic/fake data.
//
// Bridge-placement decision (design doc §6.1, resolved here): `diffObjects`
// gets its own new bridge -- `__MNEMOSYNE_COMPARISON_BRIDGE__` -- rather
// than joining `HeapExplorerHostBridge` or `LeakWorkspaceHostBridge`. See
// this slice's commit body for the full reasoning; in short, both existing
// bridges are scoped to a single already-loaded heap (`HeapExplorer`:
// `queryHeap`/`getReferences`/`getReferrers` all operate against one loaded
// heap's live object graph; `LeakWorkspace`: `explainLeak`/`findGcPath`/
// `mapToCode`/`proposeFix` all operate against one specific leak within one
// heap's analysis). `diffObjects` inherently needs *two* heap snapshots
// (before/after) and doesn't fit either "one loaded heap" or "one leak"
// shape -- it's a distinct capability class, matching the design doc's own
// architecture diagram (§5), which groups `diffObjects` with
// `listSnapshots`/`startWorkflow`/`nextStep` under "NEW third bridge, or
// extend an existing one". This bridge is intentionally minimal for Slice
// 14.A (only `diffObjects`); whether `listSnapshots`/`startWorkflow`/
// `nextStep` (14.D scope) join this same bridge or get their own is left
// for that slice to decide.
//
// Tauri wiring (M17 Slice 17.B; injection moved to UI host): `ui/src/host/tauri-bridge.ts` injects
// `__MNEMOSYNE_COMPARISON_BRIDGE__.diffObjects` → native `diff_objects`
// command over `DiffMode::Object`. The file-load path remains the primary
// browser fallback; this client-side probe activates live diff when the
// packaged desktop host is connected.

import { parseObjectDiffReport, type IdentityStrategy, type ObjectDiffReport } from "../../lib/diff-types";

export type DiffObjectsInput = {
  beforeKey: string;
  afterKey: string;
  strategy: IdentityStrategy;
  topN: number;
  crossReferenceLeaks: boolean;
};

export type ComparisonHostBridge = {
  diffObjects?: (input: DiffObjectsInput) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_COMPARISON_BRIDGE__?: ComparisonHostBridge;
  }
}

export type DiffObjectsResult =
  | { status: "unavailable" }
  | { status: "ready"; data: ObjectDiffReport }
  | { status: "error"; error: string };

function getComparisonBridge(): ComparisonHostBridge | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.__MNEMOSYNE_COMPARISON_BRIDGE__;
}

export function isDiffObjectsAvailable(): boolean {
  return Boolean(getComparisonBridge()?.diffObjects);
}

export async function runDiffObjects(input: DiffObjectsInput): Promise<DiffObjectsResult> {
  const bridge = getComparisonBridge();

  if (!bridge?.diffObjects) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.diffObjects(input);

    return {
      status: "ready",
      data: parseObjectDiffReport(raw),
    };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown diffObjects bridge failure.",
    };
  }
}

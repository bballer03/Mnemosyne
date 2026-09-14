export type PickHeapFileResult =
  | { status: "selected"; sourceId: string; displayName: string }
  | { status: "cancelled" }
  | { status: "unavailable" };

export type HeapLoadSummary = {
  displayName: string;
  sourceId?: string;
  objectCount: number;
  classCount: number;
  gcRootCount: number;
};

export type DesktopAnalysisInput = {
  sourceId: string;
  mode?: "incident" | "custom" | "overview";
  enableClassloaders?: boolean;
  enableThreads?: boolean;
  enableStrings?: boolean;
  enableCollections?: boolean;
  enableTopInstances?: boolean;
  enableByReferrer?: boolean;
  enableDuplicateArrays?: boolean;
  topN?: number;
  minCollectionCapacity?: number;
};

type DesktopHeapBridge = {
  pickHeapFile: () => Promise<PickHeapFileResult>;
  loadHeapFromSource: (sourceId: string) => Promise<HeapLoadSummary>;
  runDesktopAnalysis: (input: DesktopAnalysisInput) => Promise<unknown>;
  runCiCheck?: (input: {
    sourceId: string;
    policyToml: string;
    failOn?: string;
    mode?: string;
    baselineSourceId?: string;
  }) => Promise<unknown>;
  generateFlamegraph?: (input: {
    sourceId: string;
    root?: string;
    format?: string;
  }) => Promise<{ format: string; content: string; byteLength: number }>;
};

declare global {
  interface Window {
    __MNEMOSYNE_DESKTOP_HEAP_BRIDGE__?: DesktopHeapBridge;
  }
}

export function getDesktopHeapBridge(): DesktopHeapBridge | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.__MNEMOSYNE_DESKTOP_HEAP_BRIDGE__;
}

export async function pickHeapFile(): Promise<PickHeapFileResult> {
  const bridge = getDesktopHeapBridge();
  if (!bridge) {
    return { status: "unavailable" };
  }

  return bridge.pickHeapFile();
}

export async function loadHeapFromSource(sourceId: string): Promise<HeapLoadSummary> {
  const bridge = getDesktopHeapBridge();
  if (!bridge) {
    throw new Error("Desktop heap loading is unavailable in this environment.");
  }

  return bridge.loadHeapFromSource(sourceId);
}

export async function runDesktopAnalysis(input: DesktopAnalysisInput): Promise<unknown> {
  const bridge = getDesktopHeapBridge();
  if (!bridge) {
    throw new Error("Desktop analysis is unavailable in this environment.");
  }

  return bridge.runDesktopAnalysis(input);
}

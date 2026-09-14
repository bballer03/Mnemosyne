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

type DesktopHeapBridge = {
  pickHeapFile: () => Promise<PickHeapFileResult>;
  loadHeapFromSource: (sourceId: string) => Promise<HeapLoadSummary>;
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

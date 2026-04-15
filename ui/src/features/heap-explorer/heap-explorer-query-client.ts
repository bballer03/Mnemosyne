export type HeapQueryInput = {
  heapPath: string;
  query: string;
};

export type HeapQueryCell = string | number | boolean | null;

export type HeapQueryResult = {
  columns: string[];
  rows: HeapQueryCell[][];
};

export type HeapExplorerHostBridge = {
  queryHeap?: (input: HeapQueryInput) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?: HeapExplorerHostBridge;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readStringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid heap explorer bridge payload: expected ${field} to be an array.`);
  }

  return value.map((entry, index) => {
    if (typeof entry !== "string") {
      throw new Error(`Invalid heap explorer bridge payload: expected ${field}[${index}] to be a string.`);
    }

    return entry;
  });
}

function readRows(value: unknown, field: string): HeapQueryCell[][] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid heap explorer bridge payload: expected ${field} to be an array.`);
  }

  return value.map((row, rowIndex) => {
    if (!Array.isArray(row)) {
      throw new Error(`Invalid heap explorer bridge payload: expected ${field}[${rowIndex}] to be an array.`);
    }

    return row.map((cell, cellIndex) => {
      if (cell === null || typeof cell === "string" || typeof cell === "number" || typeof cell === "boolean") {
        return cell;
      }

      throw new Error(
        `Invalid heap explorer bridge payload: expected ${field}[${rowIndex}][${cellIndex}] to be a scalar value.`,
      );
    });
  });
}

function parseHeapQueryResult(value: unknown): HeapQueryResult {
  if (!isRecord(value)) {
    throw new Error("Invalid heap explorer bridge payload: query result must be an object.");
  }

  return {
    columns: readStringArray(value.columns, "query.columns"),
    rows: readRows(value.rows, "query.rows"),
  };
}

function getHeapExplorerBridge(): HeapExplorerHostBridge | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
}

export function isHeapQueryAvailable() {
  return Boolean(getHeapExplorerBridge()?.queryHeap);
}

export async function runHeapQuery(input: HeapQueryInput) {
  const bridge = getHeapExplorerBridge();

  if (!bridge?.queryHeap) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.queryHeap(input);

    return {
      status: "ready" as const,
      data: parseHeapQueryResult(raw),
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown heap query failure.",
    };
  }
}

export type HeapQueryInput = {
  heapPath: string;
  query: string;
};

export type HeapQueryCell = string | number | boolean | null;

export type HeapQueryResult = {
  columns: string[];
  rows: HeapQueryCell[][];
};

export type ObjectReferenceEntry = {
  objectId: string;
  className: string;
  shallowSize: number;
  displayName?: string;
};

export type ObjectReferencesResult = {
  objectId: string;
  references: ObjectReferenceEntry[];
};

export type ObjectReferrersResult = {
  objectId: string;
  referrers: ObjectReferenceEntry[];
};

export type HeapExplorerHostBridge = {
  queryHeap?: (input: HeapQueryInput) => Promise<unknown>;
  getReferences?: (objectId: string) => Promise<unknown>;
  getReferrers?: (objectId: string) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?: HeapExplorerHostBridge;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${field} to be a string.`);
  }

  return value;
}

function readNumber(value: unknown, field: string): number {
  if (typeof value !== "number") {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${field} to be a number.`);
  }

  return value;
}

function readOptionalString(value: unknown, field: string): string | undefined {
  if (value === undefined) {
    return undefined;
  }

  if (typeof value !== "string") {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${field} to be a string when present.`);
  }

  return value;
}

function readStringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${field} to be an array.`);
  }

  return value.map((entry, index) => {
    if (typeof entry !== "string") {
      throw new TypeError(`Invalid heap explorer bridge payload: expected ${field}[${index}] to be a string.`);
    }

    return entry;
  });
}

function readRows(value: unknown, field: string): HeapQueryCell[][] {
  if (!Array.isArray(value)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${field} to be an array.`);
  }

  return value.map((row, rowIndex) => {
    if (!Array.isArray(row)) {
      throw new TypeError(`Invalid heap explorer bridge payload: expected ${field}[${rowIndex}] to be an array.`);
    }

    return row.map((cell, cellIndex) => {
      if (cell === null || typeof cell === "string" || typeof cell === "number" || typeof cell === "boolean") {
        return cell;
      }

      throw new TypeError(
        `Invalid heap explorer bridge payload: expected ${field}[${rowIndex}][${cellIndex}] to be a scalar value.`,
      );
    });
  });
}

function parseHeapQueryResult(value: unknown): HeapQueryResult {
  if (!isRecord(value)) {
    throw new TypeError("Invalid heap explorer bridge payload: query result must be an object.");
  }

  return {
    columns: readStringArray(value.columns, "query.columns"),
    rows: readRows(value.rows, "query.rows"),
  };
}

function parseObjectReferenceEntry(value: unknown, path: string): ObjectReferenceEntry {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${path} to be an object.`);
  }

  return {
    objectId: readString(value.objectId, `${path}.objectId`),
    className: readString(value.className, `${path}.className`),
    shallowSize: readNumber(value.shallowSize, `${path}.shallowSize`),
    displayName: readOptionalString(value.displayName, `${path}.displayName`),
  };
}

function parseObjectReferencesResult(value: unknown): ObjectReferencesResult {
  if (!isRecord(value)) {
    throw new TypeError("Invalid heap explorer bridge payload: references result must be an object.");
  }

  if (!Array.isArray(value.references)) {
    throw new TypeError("Invalid heap explorer bridge payload: expected references.references to be an array.");
  }

  return {
    objectId: readString(value.objectId, "references.objectId"),
    references: value.references.map((entry, index) => parseObjectReferenceEntry(entry, `references.references[${index}]`)),
  };
}

function parseObjectReferrersResult(value: unknown): ObjectReferrersResult {
  if (!isRecord(value)) {
    throw new TypeError("Invalid heap explorer bridge payload: referrers result must be an object.");
  }

  if (!Array.isArray(value.referrers)) {
    throw new TypeError("Invalid heap explorer bridge payload: expected referrers.referrers to be an array.");
  }

  return {
    objectId: readString(value.objectId, "referrers.objectId"),
    referrers: value.referrers.map((entry, index) => parseObjectReferenceEntry(entry, `referrers.referrers[${index}]`)),
  };
}

function getHeapExplorerBridge(): HeapExplorerHostBridge | undefined {
  if (globalThis.window === undefined) {
    return undefined;
  }

  return globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
}

export function isHeapQueryAvailable() {
  return Boolean(getHeapExplorerBridge()?.queryHeap);
}

export function isReferencesAvailable(): boolean {
  return Boolean(getHeapExplorerBridge()?.getReferences);
}

export function isReferrersAvailable(): boolean {
  return Boolean(getHeapExplorerBridge()?.getReferrers);
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

export async function getObjectReferences(objectId: string) {
  const bridge = getHeapExplorerBridge();

  if (!bridge?.getReferences) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.getReferences(objectId);

    return {
      status: "ready" as const,
      data: parseObjectReferencesResult(raw),
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown references lookup failure.",
    };
  }
}

export async function getObjectReferrers(objectId: string) {
  const bridge = getHeapExplorerBridge();

  if (!bridge?.getReferrers) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.getReferrers(objectId);

    return {
      status: "ready" as const,
      data: parseObjectReferrersResult(raw),
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown referrers lookup failure.",
    };
  }
}

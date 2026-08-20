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

/// A class-name-resolved object reference, mirroring core's structured
/// `analysis::inspector::ObjectRef` (`{ object_id, class_name }`) -- kept
/// separate from `ObjectReferenceEntry` above (which carries `shallowSize`/
/// `displayName` from `getReferences`/`getReferrers`) since `inspectObject`
/// returns the leaner MCP-shaped `ObjectRef`, not the query-client's own
/// reference-entry shape.
export type ObjectInspectionRef = {
  objectId: string;
  className: string;
};

export type FieldValueEntry = {
  name: string;
  typeName: string;
  value: string;
};

export type ObjectInspection = {
  objectId: string;
  className: string;
  shallowSize: number;
  retainedSize?: number;
  fields?: FieldValueEntry[];
  referencesOut: ObjectInspectionRef[];
  referrersIn: ObjectInspectionRef[];
  dominatorParent?: ObjectInspectionRef;
  dominatorChildren: ObjectInspectionRef[];
};

export type HeapExplorerHostBridge = {
  queryHeap?: (input: HeapQueryInput) => Promise<unknown>;
  getReferences?: (objectId: string) => Promise<unknown>;
  getReferrers?: (objectId: string) => Promise<unknown>;
  inspectObject?: (objectId: string, retainFieldData?: boolean) => Promise<unknown>;
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

function readOptionalNumber(value: unknown, field: string): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }

  return readNumber(value, field);
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

function parseObjectInspectionRef(value: unknown, path: string): ObjectInspectionRef {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${path} to be an object.`);
  }

  return {
    objectId: readString(value.object_id, `${path}.object_id`),
    className: readString(value.class_name, `${path}.class_name`),
  };
}

function parseFieldValueEntry(value: unknown, path: string): FieldValueEntry {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${path} to be an object.`);
  }

  return {
    name: readString(value.name, `${path}.name`),
    typeName: readString(value.type_name, `${path}.type_name`),
    value: readString(value.value, `${path}.value`),
  };
}

function parseObjectInspection(value: unknown): ObjectInspection {
  if (!isRecord(value)) {
    throw new TypeError("Invalid heap explorer bridge payload: inspection result must be an object.");
  }

  if (!Array.isArray(value.references_out)) {
    throw new TypeError("Invalid heap explorer bridge payload: expected inspection.references_out to be an array.");
  }

  if (!Array.isArray(value.referrers_in)) {
    throw new TypeError("Invalid heap explorer bridge payload: expected inspection.referrers_in to be an array.");
  }

  if (!Array.isArray(value.dominator_children)) {
    throw new TypeError("Invalid heap explorer bridge payload: expected inspection.dominator_children to be an array.");
  }

  const fields = value.fields === undefined
    ? undefined
    : (() => {
        if (!Array.isArray(value.fields)) {
          throw new TypeError("Invalid heap explorer bridge payload: expected inspection.fields to be an array.");
        }

        return value.fields.map((entry, index) => parseFieldValueEntry(entry, `inspection.fields[${index}]`));
      })();

  return {
    objectId: readString(value.object_id, "inspection.object_id"),
    className: readString(value.class_name, "inspection.class_name"),
    shallowSize: readNumber(value.shallow_size, "inspection.shallow_size"),
    retainedSize: readOptionalNumber(value.retained_size, "inspection.retained_size"),
    fields,
    referencesOut: value.references_out.map((entry, index) =>
      parseObjectInspectionRef(entry, `inspection.references_out[${index}]`),
    ),
    referrersIn: value.referrers_in.map((entry, index) =>
      parseObjectInspectionRef(entry, `inspection.referrers_in[${index}]`),
    ),
    dominatorParent:
      value.dominator_parent === null || value.dominator_parent === undefined
        ? undefined
        : parseObjectInspectionRef(value.dominator_parent, "inspection.dominator_parent"),
    dominatorChildren: value.dominator_children.map((entry, index) =>
      parseObjectInspectionRef(entry, `inspection.dominator_children[${index}]`),
    ),
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

export function isInspectObjectAvailable(): boolean {
  return Boolean(getHeapExplorerBridge()?.inspectObject);
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

export async function inspectObject(objectId: string, retainFieldData?: boolean) {
  const bridge = getHeapExplorerBridge();

  if (!bridge?.inspectObject) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.inspectObject(objectId, retainFieldData);

    return {
      status: "ready" as const,
      data: parseObjectInspection(raw),
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown object inspection failure.",
    };
  }
}

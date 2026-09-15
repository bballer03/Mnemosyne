export type HeapQueryInput = {
  heapPath: string;
  query: string;
};

export type HeapQueryCell = string | number | boolean | null;

export type HeapQueryResult = {
  columns: string[];
  rows: HeapQueryCell[][];
};

export type HeapQueryErrorLocation = {
  byteOffset: number;
  line: number;
  column: number;
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

export type ClassInstanceEntry = {
  objectId: string;
  className: string;
  shallowSize: number;
  retainedSize: number;
};

export type ClassInstancesPage = {
  classKey: string;
  total: number;
  returned: number;
  offset: number;
  limit: number;
  truncated: boolean;
  instances: ClassInstanceEntry[];
};

export type DominatorChildEntry = {
  objectId: string;
  className: string;
  shallowSize: number;
  retainedSize: number;
  dominatedCount: number;
  hasChildren: boolean;
};

export type DominatorChildrenPage = {
  total: number;
  returned: number;
  offset: number;
  limit: number;
  truncated: boolean;
  children: DominatorChildEntry[];
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
  /** M19.B — live flat regroup via session graph / MCP analyze_heap.histogram_group_by. */
  regroupHistogram?: (groupBy: string) => Promise<unknown>;
  listClassInstances?: (classKey: string, offset?: number, limit?: number) => Promise<unknown>;
  getDominatorChildren?: (
    parentObjectId?: string,
    offset?: number,
    limit?: number,
    minRetainedBytes?: number,
  ) => Promise<unknown>;
};

export type HistogramGroupByMode = "class" | "package" | "class_loader" | "superclass";

export type HistogramResultView = {
  groupBy: string;
  entries: Array<{
    key: string;
    instanceCount: number;
    shallowSize: number;
    retainedSize: number;
    /**
     * Optional explicit parent bucket key. Present only when a host payload
     * supplies a deterministic parent relation — never inferred from flat keys.
     */
    parentKey?: string;
  }>;
  totalInstances: number;
  totalShallowSize: number;
};

export const HISTOGRAM_GROUP_BY_OPTIONS: Array<{ value: HistogramGroupByMode; label: string }> = [
  { value: "class", label: "Class" },
  { value: "package", label: "Package" },
  { value: "class_loader", label: "Class loader" },
  { value: "superclass", label: "Superclass" },
];

export function normalizeHistogramGroupBy(raw: string | undefined): HistogramGroupByMode | undefined {
  if (!raw) {
    return undefined;
  }

  const normalized = raw.trim().toLowerCase();
  if (normalized === "classloader") {
    return "class_loader";
  }

  if (
    normalized === "class" ||
    normalized === "package" ||
    normalized === "class_loader" ||
    normalized === "superclass"
  ) {
    return normalized;
  }

  return undefined;
}

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

function readCount(value: unknown, field: string): number {
  const count = readNumber(value, field);
  if (!Number.isSafeInteger(count) || count < 0) {
    throw new TypeError(
      `Invalid heap explorer bridge payload: expected ${field} to be a non-negative safe integer.`,
    );
  }

  return count;
}

function readBoolean(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${field} to be a boolean.`);
  }

  return value;
}

function readObjectId(value: unknown, field: string): string {
  const objectId = readString(value, field);
  if (!/^(?:0x)?[0-9a-f]+$/i.test(objectId)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${field} to be an object id.`);
  }

  return objectId;
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

function parseHeapQueryErrorLocation(
  query: string,
  message: string,
): HeapQueryErrorLocation | undefined {
  const match = /\bat byte (\d+)\b/.exec(message);
  if (!match) {
    return undefined;
  }

  const byteOffset = Number(match[1]);
  const encodedQuery = new TextEncoder().encode(query);
  if (!Number.isSafeInteger(byteOffset) || byteOffset < 0 || byteOffset > encodedQuery.length) {
    return undefined;
  }

  const prefix = new TextDecoder().decode(encodedQuery.slice(0, byteOffset));
  const lines = prefix.split(/\r\n|\r|\n/);
  const currentLine = lines[lines.length - 1] ?? "";

  return {
    byteOffset,
    line: lines.length,
    column: Array.from(currentLine).length + 1,
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

function parseClassInstanceEntry(value: unknown, path: string): ClassInstanceEntry {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${path} to be an object.`);
  }

  return {
    objectId: readObjectId(value.object_id, `${path}.object_id`),
    className: readString(value.class_name, `${path}.class_name`),
    shallowSize: readCount(value.shallow_size, `${path}.shallow_size`),
    retainedSize: readCount(value.retained_size, `${path}.retained_size`),
  };
}

function parseClassInstancesPage(value: unknown): ClassInstancesPage {
  if (!isRecord(value)) {
    throw new TypeError("Invalid heap explorer bridge payload: class instances result must be an object.");
  }
  if (!Array.isArray(value.instances)) {
    throw new TypeError(
      "Invalid heap explorer bridge payload: expected class_instances.instances to be an array.",
    );
  }

  const instances = value.instances.map((entry, index) =>
    parseClassInstanceEntry(entry, `class_instances.instances[${index}]`),
  );
  const returned = readCount(value.returned, "class_instances.returned");
  if (returned !== instances.length) {
    throw new TypeError(
      "Invalid heap explorer bridge payload: class_instances.returned must match instances.length.",
    );
  }

  return {
    classKey: readString(value.class_key, "class_instances.class_key"),
    total: readCount(value.total, "class_instances.total"),
    returned,
    offset: readCount(value.offset, "class_instances.offset"),
    limit: readCount(value.limit, "class_instances.limit"),
    truncated: readBoolean(value.truncated, "class_instances.truncated"),
    instances,
  };
}

function parseDominatorChildEntry(value: unknown, path: string): DominatorChildEntry {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid heap explorer bridge payload: expected ${path} to be an object.`);
  }

  return {
    objectId: readObjectId(value.object_id, `${path}.object_id`),
    className: readString(value.class_name, `${path}.class_name`),
    shallowSize: readCount(value.shallow_size, `${path}.shallow_size`),
    retainedSize: readCount(value.retained_size, `${path}.retained_size`),
    dominatedCount: readCount(value.dominated_count, `${path}.dominated_count`),
    hasChildren: readBoolean(value.has_children, `${path}.has_children`),
  };
}

function parseDominatorChildrenPage(value: unknown): DominatorChildrenPage {
  if (!isRecord(value)) {
    throw new TypeError("Invalid heap explorer bridge payload: dominator children result must be an object.");
  }
  if (!Array.isArray(value.children)) {
    throw new TypeError(
      "Invalid heap explorer bridge payload: expected dominator_children.children to be an array.",
    );
  }

  const children = value.children.map((entry, index) =>
    parseDominatorChildEntry(entry, `dominator_children.children[${index}]`),
  );
  const returned = readCount(value.returned, "dominator_children.returned");
  if (returned !== children.length) {
    throw new TypeError(
      "Invalid heap explorer bridge payload: dominator_children.returned must match children.length.",
    );
  }

  return {
    total: readCount(value.total, "dominator_children.total"),
    returned,
    offset: readCount(value.offset, "dominator_children.offset"),
    limit: readCount(value.limit, "dominator_children.limit"),
    truncated: readBoolean(value.truncated, "dominator_children.truncated"),
    children,
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

export function isRegroupHistogramAvailable(): boolean {
  return Boolean(getHeapExplorerBridge()?.regroupHistogram);
}

export function isListClassInstancesAvailable(): boolean {
  return Boolean(getHeapExplorerBridge()?.listClassInstances);
}

export function isGetDominatorChildrenAvailable(): boolean {
  return Boolean(getHeapExplorerBridge()?.getDominatorChildren);
}

export function parseHistogramResult(raw: unknown): HistogramResultView {
  if (!isRecord(raw)) {
    throw new TypeError("Invalid histogram regroup payload: expected an object.");
  }

  const entries = raw.entries;
  if (!Array.isArray(entries)) {
    throw new TypeError("Invalid histogram regroup payload: expected entries to be an array.");
  }

  return {
    groupBy: readString(raw.group_by, "group_by"),
    totalInstances: readNumber(raw.total_instances, "total_instances"),
    totalShallowSize: readNumber(raw.total_shallow_size, "total_shallow_size"),
    entries: entries.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new TypeError(`Invalid histogram regroup payload: expected entries[${index}] to be an object.`);
      }

      const parentRaw = entry.parent_key ?? entry.parentKey;
      const parentKey =
        typeof parentRaw === "string" && parentRaw.trim().length > 0
          ? parentRaw.trim()
          : undefined;

      return {
        key: readString(entry.key, `entries[${index}].key`),
        instanceCount: readNumber(entry.instance_count, `entries[${index}].instance_count`),
        shallowSize: readNumber(entry.shallow_size, `entries[${index}].shallow_size`),
        retainedSize: readNumber(entry.retained_size, `entries[${index}].retained_size`),
        ...(parentKey ? { parentKey } : {}),
      };
    }),
  };
}

export async function regroupHistogram(groupBy: string) {
  const bridge = getHeapExplorerBridge();

  if (!bridge?.regroupHistogram) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.regroupHistogram(groupBy);

    return {
      status: "ready" as const,
      data: parseHistogramResult(raw),
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown histogram regroup failure.",
    };
  }
}

export async function listClassInstances(classKey: string, offset?: number, limit?: number) {
  const bridge = getHeapExplorerBridge();

  if (!bridge?.listClassInstances) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.listClassInstances(classKey, offset, limit);

    return {
      status: "ready" as const,
      data: parseClassInstancesPage(raw),
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown class instances lookup failure.",
    };
  }
}

export async function getDominatorChildren(
  parentObjectId?: string,
  offset?: number,
  limit?: number,
  minRetainedBytes?: number,
) {
  const bridge = getHeapExplorerBridge();

  if (!bridge?.getDominatorChildren) {
    return { status: "unavailable" as const };
  }

  try {
    const raw = await bridge.getDominatorChildren(
      parentObjectId,
      offset,
      limit,
      minRetainedBytes,
    );

    return {
      status: "ready" as const,
      data: parseDominatorChildrenPage(raw),
    };
  } catch (error) {
    return {
      status: "error" as const,
      error: error instanceof Error ? error.message : "Unknown dominator children lookup failure.",
    };
  }
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
    const message = error instanceof Error ? error.message : "Unknown heap query failure.";
    const location = parseHeapQueryErrorLocation(input.query, message);

    return {
      status: "error" as const,
      error: message,
      ...(location ? { location } : {}),
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

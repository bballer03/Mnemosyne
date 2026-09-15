import type {
  HistogramSortDirection,
  HistogramSortKey,
  HistogramViewState,
  InvestigationOriginPane,
} from "./investigation-store";
import {
  WORKFLOW_KIND_IDS,
  type PersistedWorkflowBinding,
  type WorkflowKindId,
} from "../workflow-landing/workflow-types";
import { isPerspectiveId, type PerspectiveId } from "./perspectives";

export const WORKSPACE_PERSISTENCE_SCHEMA_VERSION = 1 as const;

const ID_MAX_LENGTH = 512;
const ENTRY_ID_MAX_LENGTH = 128;
const NOTE_TEXT_MAX_LENGTH = 2_000;
const BOOKMARK_LABEL_MAX_LENGTH = 200;
const COLLECTION_MAX_LENGTH = 100;

const originPanes: readonly InvestigationOriginPane[] = [
  "histogram",
  "dominators",
  "inspector",
  "gc-path",
  "leak",
  "findings",
];
const histogramGroups = ["class", "package", "class_loader", "superclass"] as const;
const histogramSortKeys: readonly HistogramSortKey[] = [
  "retained",
  "shallow",
  "instances",
  "class",
];
const histogramSortDirections: readonly HistogramSortDirection[] = ["asc", "desc"];
const identityKinds = ["workspace", "snapshot"] as const;
const noteTargetKinds = ["workspace", "object", "class", "leak"] as const;
const bookmarkTargetKinds = ["object", "class", "leak"] as const;

export type WorkspacePersistenceIdentity = {
  kind: (typeof identityKinds)[number];
  key: string;
};

export type WorkspaceTarget = {
  kind: (typeof noteTargetKinds)[number];
  id: string;
};

export type WorkspaceNote = {
  id: string;
  target: WorkspaceTarget;
  text: string;
};

export type WorkspaceBookmark = {
  id: string;
  target: {
    kind: (typeof bookmarkTargetKinds)[number];
    id: string;
  };
  label?: string;
};

export type PersistedWorkspaceV1 = {
  schemaVersion: typeof WORKSPACE_PERSISTENCE_SCHEMA_VERSION;
  identity: WorkspacePersistenceIdentity;
  revision: number;
  layout: {
    activePane?: InvestigationOriginPane;
    perspectiveId?: PerspectiveId;
  };
  filters: {
    histogram: HistogramViewState;
  };
  selection: {
    revision: number;
    objectId?: string;
    classKey?: string;
    leakId?: string;
  };
  notes: WorkspaceNote[];
  bookmarks: WorkspaceBookmark[];
  workflow?: PersistedWorkflowBinding;
};

export type WorkspaceParseResult =
  | { status: "ready"; record: PersistedWorkspaceV1 }
  | { status: "unsupported-schema"; schemaVersion?: number }
  | { status: "rejected"; reason: string };

export type WorkspaceCompatibility = {
  identity: WorkspacePersistenceIdentity;
  revision: number;
  objectIds: ReadonlySet<string>;
  classKeys: ReadonlySet<string>;
  leakIds: ReadonlySet<string>;
};

export type DroppedSelectionId = {
  kind: "object" | "class" | "leak";
  id: string;
};

export type WorkspaceRestoreResult = {
  revision: number;
  layout: PersistedWorkspaceV1["layout"];
  filters: PersistedWorkspaceV1["filters"];
  selection: PersistedWorkspaceV1["selection"];
  notes: WorkspaceNote[];
  bookmarks: WorkspaceBookmark[];
  workflow?: PersistedWorkflowBinding;
  droppedSelectionIds: DroppedSelectionId[];
};

export type WorkspacePersistenceResult =
  | WorkspaceParseResult
  | { status: "missing" }
  | { status: "saved" }
  | { status: "unavailable" };

export type WorkspacePersistence = {
  load(identity: WorkspacePersistenceIdentity): WorkspacePersistenceResult;
  save(record: PersistedWorkspaceV1): WorkspacePersistenceResult;
  remove(identity: WorkspacePersistenceIdentity): void;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(
  value: Record<string, unknown>,
  allowedKeys: readonly string[],
): string | undefined {
  return Object.keys(value).find((key) => !allowedKeys.includes(key));
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function containsAbsolutePath(value: string): boolean {
  return (
    /(?:^|\s)\/(?:[^\s/][^\s]*)/.test(value) ||
    /(?:^|\s)[A-Za-z]:[\\/]/.test(value) ||
    /(?:^|\s)\\\\[^\\\s]+\\/.test(value)
  );
}

function readDisplaySafeString(
  value: unknown,
  minLength: number,
  maxLength: number,
): string | undefined {
  if (typeof value !== "string" || value.length < minLength || value.length > maxLength) {
    return undefined;
  }
  return containsAbsolutePath(value) ? undefined : value;
}

function hasAbsolutePathDeep(value: unknown): boolean {
  if (typeof value === "string") {
    return containsAbsolutePath(value);
  }
  if (Array.isArray(value)) {
    return value.some(hasAbsolutePathDeep);
  }
  if (isRecord(value)) {
    return Object.values(value).some(hasAbsolutePathDeep);
  }
  return false;
}

function parseIdentity(value: unknown): WorkspacePersistenceIdentity | undefined {
  if (!isRecord(value) || hasExactKeys(value, ["kind", "key"])) {
    return undefined;
  }
  if (
    typeof value.kind !== "string" ||
    !identityKinds.includes(value.kind as WorkspacePersistenceIdentity["kind"])
  ) {
    return undefined;
  }
  const key = readDisplaySafeString(value.key, 1, ID_MAX_LENGTH);
  if (!key || key.includes("/") || key.includes("\\")) {
    return undefined;
  }
  return {
    kind: value.kind as WorkspacePersistenceIdentity["kind"],
    key,
  };
}

function parseHistogramView(value: unknown): HistogramViewState | undefined {
  if (
    !isRecord(value) ||
    hasExactKeys(value, ["searchText", "groupBy", "sortKey", "sortDirection", "pageOffset"])
  ) {
    return undefined;
  }
  const searchText = readDisplaySafeString(value.searchText, 0, 512);
  if (
    searchText === undefined ||
    typeof value.groupBy !== "string" ||
    !histogramGroups.includes(value.groupBy as HistogramViewState["groupBy"]) ||
    typeof value.sortKey !== "string" ||
    !histogramSortKeys.includes(value.sortKey as HistogramSortKey) ||
    typeof value.sortDirection !== "string" ||
    !histogramSortDirections.includes(value.sortDirection as HistogramSortDirection) ||
    !isNonNegativeSafeInteger(value.pageOffset)
  ) {
    return undefined;
  }
  return {
    searchText,
    groupBy: value.groupBy as HistogramViewState["groupBy"],
    sortKey: value.sortKey as HistogramSortKey,
    sortDirection: value.sortDirection as HistogramSortDirection,
    pageOffset: value.pageOffset,
  };
}

function parseOptionalId(value: unknown): string | undefined | null {
  if (value === undefined) {
    return undefined;
  }
  return readDisplaySafeString(value, 1, ID_MAX_LENGTH) ?? null;
}

function parseWorkflow(value: unknown): PersistedWorkflowBinding | undefined {
  if (
    !isRecord(value) ||
    hasExactKeys(value, ["workflowId", "kind", "currentStep", "revision"])
  ) {
    return undefined;
  }
  const workflowId = readDisplaySafeString(value.workflowId, 1, ID_MAX_LENGTH);
  const currentStep = readDisplaySafeString(value.currentStep, 1, ENTRY_ID_MAX_LENGTH);
  if (
    !workflowId ||
    workflowId.includes("/") ||
    workflowId.includes("\\") ||
    !currentStep ||
    currentStep.includes("/") ||
    currentStep.includes("\\") ||
    typeof value.kind !== "string" ||
    !WORKFLOW_KIND_IDS.includes(value.kind as WorkflowKindId) ||
    !isNonNegativeSafeInteger(value.revision)
  ) {
    return undefined;
  }
  return {
    workflowId,
    kind: value.kind as WorkflowKindId,
    currentStep,
    revision: value.revision,
  };
}

function parseNote(value: unknown): WorkspaceNote | undefined {
  if (!isRecord(value) || hasExactKeys(value, ["id", "target", "text"])) {
    return undefined;
  }
  const id = readDisplaySafeString(value.id, 1, ENTRY_ID_MAX_LENGTH);
  const text = readDisplaySafeString(value.text, 0, NOTE_TEXT_MAX_LENGTH);
  if (!id || text === undefined || !isRecord(value.target)) {
    return undefined;
  }
  if (hasExactKeys(value.target, ["kind", "id"])) {
    return undefined;
  }
  if (
    typeof value.target.kind !== "string" ||
    !noteTargetKinds.includes(value.target.kind as WorkspaceTarget["kind"])
  ) {
    return undefined;
  }
  const targetId = readDisplaySafeString(value.target.id, 1, ID_MAX_LENGTH);
  return targetId
    ? {
        id,
        target: {
          kind: value.target.kind as WorkspaceTarget["kind"],
          id: targetId,
        },
        text,
      }
    : undefined;
}

function parseBookmark(value: unknown): WorkspaceBookmark | undefined {
  if (!isRecord(value) || hasExactKeys(value, ["id", "target", "label"])) {
    return undefined;
  }
  const id = readDisplaySafeString(value.id, 1, ENTRY_ID_MAX_LENGTH);
  const label =
    value.label === undefined
      ? undefined
      : readDisplaySafeString(value.label, 0, BOOKMARK_LABEL_MAX_LENGTH);
  if (!id || label === null || !isRecord(value.target)) {
    return undefined;
  }
  if (value.label !== undefined && label === undefined) {
    return undefined;
  }
  if (hasExactKeys(value.target, ["kind", "id"])) {
    return undefined;
  }
  if (
    typeof value.target.kind !== "string" ||
    !bookmarkTargetKinds.includes(value.target.kind as WorkspaceBookmark["target"]["kind"])
  ) {
    return undefined;
  }
  const targetId = readDisplaySafeString(value.target.id, 1, ID_MAX_LENGTH);
  return targetId
    ? {
        id,
        target: {
          kind: value.target.kind as WorkspaceBookmark["target"]["kind"],
          id: targetId,
        },
        ...(label === undefined ? {} : { label }),
      }
    : undefined;
}

function hasUniqueIds(entries: Array<{ id: string }>): boolean {
  return new Set(entries.map((entry) => entry.id)).size === entries.length;
}

export function parsePersistedWorkspace(value: unknown): WorkspaceParseResult {
  if (!isRecord(value)) {
    return { status: "rejected", reason: "workspace record must be an object" };
  }
  if (value.schemaVersion !== WORKSPACE_PERSISTENCE_SCHEMA_VERSION) {
    return {
      status: "unsupported-schema",
      ...(typeof value.schemaVersion === "number"
        ? { schemaVersion: value.schemaVersion }
        : {}),
    };
  }

  const unexpectedField = hasExactKeys(value, [
    "schemaVersion",
    "identity",
    "revision",
    "layout",
    "filters",
    "selection",
    "notes",
    "bookmarks",
    "workflow",
  ]);
  if (unexpectedField) {
    return { status: "rejected", reason: `unexpected field ${unexpectedField}` };
  }
  if (hasAbsolutePathDeep(value)) {
    return { status: "rejected", reason: "absolute paths are not display-safe" };
  }

  const identity = parseIdentity(value.identity);
  if (!identity || !isNonNegativeSafeInteger(value.revision)) {
    return { status: "rejected", reason: "invalid workspace identity or revision" };
  }

  if (
    !isRecord(value.layout) ||
    hasExactKeys(value.layout, ["activePane", "perspectiveId"])
  ) {
    return { status: "rejected", reason: "invalid workspace layout" };
  }
  const activePane =
    value.layout.activePane === undefined
      ? undefined
      : typeof value.layout.activePane === "string" &&
          originPanes.includes(value.layout.activePane as InvestigationOriginPane)
        ? (value.layout.activePane as InvestigationOriginPane)
        : null;
  if (activePane === null) {
    return { status: "rejected", reason: "invalid active pane" };
  }
  const perspectiveId =
    value.layout.perspectiveId === undefined
      ? undefined
      : isPerspectiveId(value.layout.perspectiveId)
        ? value.layout.perspectiveId
        : null;
  if (perspectiveId === null) {
    return { status: "rejected", reason: "invalid perspective id" };
  }

  if (!isRecord(value.filters) || hasExactKeys(value.filters, ["histogram"])) {
    return { status: "rejected", reason: "invalid workspace filters" };
  }
  const histogram = parseHistogramView(value.filters.histogram);
  if (!histogram) {
    return { status: "rejected", reason: "invalid histogram filters" };
  }

  if (
    !isRecord(value.selection) ||
    hasExactKeys(value.selection, ["revision", "objectId", "classKey", "leakId"]) ||
    !isNonNegativeSafeInteger(value.selection.revision)
  ) {
    return { status: "rejected", reason: "invalid workspace selection" };
  }
  const objectId = parseOptionalId(value.selection.objectId);
  const classKey = parseOptionalId(value.selection.classKey);
  const leakId = parseOptionalId(value.selection.leakId);
  if (objectId === null || classKey === null || leakId === null) {
    return { status: "rejected", reason: "invalid selection id" };
  }

  if (
    !Array.isArray(value.notes) ||
    value.notes.length > COLLECTION_MAX_LENGTH ||
    !Array.isArray(value.bookmarks) ||
    value.bookmarks.length > COLLECTION_MAX_LENGTH
  ) {
    return { status: "rejected", reason: "invalid metadata collection" };
  }
  const notes = value.notes.map(parseNote);
  const bookmarks = value.bookmarks.map(parseBookmark);
  if (
    notes.some((entry) => entry === undefined) ||
    bookmarks.some((entry) => entry === undefined)
  ) {
    return { status: "rejected", reason: "invalid note or bookmark" };
  }
  const safeNotes = notes as WorkspaceNote[];
  const safeBookmarks = bookmarks as WorkspaceBookmark[];
  if (!hasUniqueIds(safeNotes) || !hasUniqueIds(safeBookmarks)) {
    return { status: "rejected", reason: "duplicate metadata id" };
  }
  const workflow =
    value.workflow === undefined ? undefined : parseWorkflow(value.workflow);
  if (value.workflow !== undefined && !workflow) {
    return { status: "rejected", reason: "invalid workflow binding" };
  }

  return {
    status: "ready",
    record: {
      schemaVersion: WORKSPACE_PERSISTENCE_SCHEMA_VERSION,
      identity,
      revision: value.revision,
      layout: {
        ...(activePane === undefined ? {} : { activePane }),
        ...(perspectiveId === undefined ? {} : { perspectiveId }),
      },
      filters: { histogram },
      selection: {
        revision: value.selection.revision,
        ...(objectId === undefined ? {} : { objectId }),
        ...(classKey === undefined ? {} : { classKey }),
        ...(leakId === undefined ? {} : { leakId }),
      },
      notes: safeNotes,
      bookmarks: safeBookmarks,
      ...(workflow ? { workflow } : {}),
    },
  };
}

function identitiesMatch(
  left: WorkspacePersistenceIdentity,
  right: WorkspacePersistenceIdentity,
): boolean {
  return left.kind === right.kind && left.key === right.key;
}

function isActivePaneCompatible(
  activePane: InvestigationOriginPane | undefined,
  selection: PersistedWorkspaceV1["selection"],
): boolean {
  switch (activePane) {
    case "histogram":
      return selection.classKey !== undefined;
    case "dominators":
    case "inspector":
    case "gc-path":
      return selection.objectId !== undefined;
    case "leak":
      return selection.leakId !== undefined;
    case "findings":
      return true;
    default:
      return false;
  }
}

export function restoreCompatibleWorkspace(
  record: PersistedWorkspaceV1,
  compatibility: WorkspaceCompatibility,
): WorkspaceRestoreResult {
  if (!identitiesMatch(record.identity, compatibility.identity)) {
    return {
      revision: compatibility.revision,
      layout: {},
      filters: record.filters,
      selection: { revision: compatibility.revision },
      notes: [],
      bookmarks: [],
      droppedSelectionIds: [],
    };
  }

  const selection: PersistedWorkspaceV1["selection"] = {
    revision: compatibility.revision,
  };
  const droppedSelectionIds: DroppedSelectionId[] = [];
  const candidates = [
    {
      kind: "object" as const,
      property: "objectId" as const,
      id: record.selection.objectId,
      accepted: compatibility.objectIds,
    },
    {
      kind: "class" as const,
      property: "classKey" as const,
      id: record.selection.classKey,
      accepted: compatibility.classKeys,
    },
    {
      kind: "leak" as const,
      property: "leakId" as const,
      id: record.selection.leakId,
      accepted: compatibility.leakIds,
    },
  ];
  for (const candidate of candidates) {
    if (!candidate.id) {
      continue;
    }
    if (candidate.accepted.has(candidate.id)) {
      selection[candidate.property] = candidate.id;
    } else {
      droppedSelectionIds.push({ kind: candidate.kind, id: candidate.id });
    }
  }

  const layout: PersistedWorkspaceV1["layout"] = {
    ...(record.layout.perspectiveId
      ? { perspectiveId: record.layout.perspectiveId }
      : {}),
    ...(isActivePaneCompatible(record.layout.activePane, selection)
      ? { activePane: record.layout.activePane }
      : {}),
  };

  return {
    revision: compatibility.revision,
    layout,
    filters: record.filters,
    selection,
    notes: record.notes,
    bookmarks: record.bookmarks,
    ...(record.workflow?.revision === record.revision
      ? { workflow: { ...record.workflow, revision: compatibility.revision } }
      : {}),
    droppedSelectionIds,
  };
}

function storageKey(identity: WorkspacePersistenceIdentity): string {
  return [
    "mnemosyne",
    "workspace",
    `v${WORKSPACE_PERSISTENCE_SCHEMA_VERSION}`,
    identity.kind,
    encodeURIComponent(identity.key),
  ].join(".");
}

function resolveSessionStorage(): Storage | undefined {
  try {
    return globalThis.sessionStorage ?? globalThis.window?.sessionStorage;
  } catch {
    return undefined;
  }
}

export function createWorkspacePersistence(
  requestedStorage?: Storage,
): WorkspacePersistence {
  const storage = requestedStorage ?? resolveSessionStorage();
  return {
    load(identity) {
      if (!storage) {
        return { status: "unavailable" };
      }
      let serialized: string | null;
      try {
        serialized = storage.getItem(storageKey(identity));
      } catch {
        return { status: "unavailable" };
      }
      if (serialized === null) {
        return { status: "missing" };
      }
      let parsedValue: unknown;
      try {
        parsedValue = JSON.parse(serialized);
      } catch {
        return { status: "rejected", reason: "stored value is not valid JSON" };
      }
      const parsed = parsePersistedWorkspace(parsedValue);
      if (
        parsed.status === "ready" &&
        !identitiesMatch(parsed.record.identity, identity)
      ) {
        return {
          status: "rejected",
          reason: "stored identity does not match requested identity",
        };
      }
      return parsed;
    },
    save(record) {
      const parsed = parsePersistedWorkspace(record);
      if (parsed.status !== "ready") {
        return parsed;
      }
      if (!storage) {
        return { status: "unavailable" };
      }
      try {
        storage.setItem(storageKey(parsed.record.identity), JSON.stringify(parsed.record));
        return { status: "saved" };
      } catch {
        return { status: "unavailable" };
      }
    },
    remove(identity) {
      try {
        storage?.removeItem(storageKey(identity));
      } catch {
        // Persistence is optional; storage denial must not break investigation.
      }
    },
  };
}

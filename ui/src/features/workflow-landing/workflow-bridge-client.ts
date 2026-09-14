// Bridge client for the M11 workflow suite + M9 snapshot listing, following
// the exact optional-capability-probe pattern established by
// `heap-explorer-query-client.ts` and, most directly, `comparison-bridge-client.ts`:
// an `isXAvailable()` guard per method, an explicit "unavailable" status when
// the bridge/method is absent, and never synthetic/fake data.
//
// Bridge-placement decision (design doc §6.1 / §5's "NEW third bridge, or
// extend an existing one" note, resolved here for Slice 14.D): a fourth,
// dedicated bridge -- `__MNEMOSYNE_WORKFLOW_BRIDGE__` -- rather than joining
// `HeapExplorerHostBridge`, `LeakWorkspaceHostBridge`, or
// `ComparisonHostBridge`. Reasoning, mirroring `comparison-bridge-client.ts`'s
// own documented call: `HeapExplorer` is scoped to one loaded heap's live
// object graph, `LeakWorkspace` to one specific leak within one heap, and
// `Comparison` to a before/after pair of heap snapshots. `describeWorkflow`/
// `startWorkflow`/`nextStep`/`listSnapshots` don't fit any of those shapes --
// they're cross-cutting orchestration (a workflow can *itself* wrap a
// heap-path load, a leak lookup, or a snapshot pair, depending on `kind`) plus
// snapshot-cache introspection that isn't scoped to any single already-loaded
// heap at all. A fifth shape, not a fourth instance of an existing one.
//
// Wire shapes verified directly against `core::mcp::server::handle_request`
// (`describe_workflow`/`start_workflow`/`next_step` match arms) and
// `core::workflow::mod` (`WorkflowState`, `WorkflowDescription`,
// `StepDescription`, `ParamDescription`) plus `core::snapshot::mod`
// (`SnapshotManifest`) -- not the design doc's illustrative §6 sketch, which
// predates this slice's implementation pass:
//   - `start_workflow`/`next_step` both return
//     `{ workflow_id, current_step, step_result, next_expected_input }`
//     (see `workflow_step_response` in `core/src/mcp/server.rs`).
//   - `describe_workflow` returns `{ kind, steps: [{ name, description,
//     expected_input: [{ name, type, required, description }],
//     underlying_primitives: [...] }] }`.
//   - `list_snapshots` returns an array of `SnapshotManifest`, whose Rust
//     fields (`schema_version`, `heap_sha256`, `heap_path`, `created_at`,
//     `mnemosyne_version`, `object_count`, `has_field_data`) are already
//     snake_case with no `#[serde(rename_all)]` override, so the wire JSON
//     keys equal the Rust field names verbatim.
//
// Tauri wiring: `ui/src/host/tauri-bridge.ts` injects (see also deprecated stub `tauri/src/bridge.ts`)
// `__MNEMOSYNE_WORKFLOW_BRIDGE__` yet (no `describe_workflow`/`start_workflow`/
// `next_step`/`list_snapshots` Tauri commands exist in `tauri/src/commands.rs`).
// Per the design doc's scope cap (§4 "Out"), this slice does not add Tauri
// commands -- the guided landing degrades gracefully (every workflow card and
// the "Recent heaps" section render an explicit unavailable/omitted state)
// when no host has wired this bridge up, exactly like every other optional
// bridge capability in this codebase.

export type WorkflowKindId =
  | "triage_memory_leak"
  | "tune_gc"
  | "traverse_object_graph"
  | "compare_snapshots"
  | "classloader_leak";

export type StartWorkflowParams = {
  heapPath?: string;
  objectId?: string;
  beforeHeapPath?: string;
  afterHeapPath?: string;
  beforeSnapshotKey?: string;
  afterSnapshotKey?: string;
};

export type WorkflowHostBridge = {
  describeWorkflow?: (kind: WorkflowKindId) => Promise<unknown>;
  startWorkflow?: (kind: WorkflowKindId, params?: StartWorkflowParams) => Promise<unknown>;
  nextStep?: (workflowId: string, input?: unknown) => Promise<unknown>;
  getWorkflow?: (workflowId: string) => Promise<unknown>;
  closeWorkflow?: (workflowId: string) => Promise<unknown>;
  listSnapshots?: () => Promise<unknown>;
  saveSnapshot?: (sourceId: string, retainFieldData?: boolean) => Promise<unknown>;
  removeSnapshot?: (key: string) => Promise<unknown>;
  openSnapshot?: (key: string) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_WORKFLOW_BRIDGE__?: WorkflowHostBridge;
  }
}

export type ParamDescription = {
  name: string;
  type: string;
  required: boolean;
  description: string;
};

export type StepDescription = {
  name: string;
  description: string;
  expectedInput: ParamDescription[];
  underlyingPrimitives: string[];
};

export type WorkflowDescriptionResult = {
  steps: StepDescription[];
};

export type WorkflowStepResult = {
  workflowId: string;
  currentStep: string;
  stepResult: unknown;
  nextExpectedInput: unknown;
};

/** Display-safe projection of MCP/desktop `get_workflow` WorkflowState. */
export type WorkflowStateView = WorkflowStepResult & {
  kind: WorkflowKindId;
  heapDisplayName: string;
  stepHistory: Array<{ stepName: string; outputSummary: unknown }>;
};

export type CloseWorkflowResult = {
  workflowId: string;
  closed: boolean;
};

export type SnapshotManifest = {
  schemaVersion: number;
  heapSha256: string;
  heapPath: string;
  createdAt: string;
  mnemosyneVersion: string;
  objectCount: number;
  hasFieldData: boolean;
};

/** Display-safe summary from desktop `open_snapshot` (no absolute paths). */
export type OpenSnapshotSummary = {
  displayName: string;
  sourceId: string;
  objectCount: number;
  classCount: number;
  gcRootCount: number;
};

export type WorkflowBridgeResult<T> =
  | { status: "unavailable" }
  | { status: "ready"; data: T }
  | { status: "error"; error: string };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new TypeError(`Invalid workflow bridge payload: expected ${field} to be a string.`);
  }

  return value;
}

function readBoolean(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") {
    throw new TypeError(`Invalid workflow bridge payload: expected ${field} to be a boolean.`);
  }

  return value;
}

function readNumber(value: unknown, field: string): number {
  if (typeof value !== "number") {
    throw new TypeError(`Invalid workflow bridge payload: expected ${field} to be a number.`);
  }

  return value;
}

function readStringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) {
    throw new TypeError(`Invalid workflow bridge payload: expected ${field} to be an array.`);
  }

  return value.map((entry, index) => readString(entry, `${field}[${index}]`));
}

function parseParamDescription(value: unknown, path: string): ParamDescription {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid workflow bridge payload: expected ${path} to be an object.`);
  }

  return {
    name: readString(value.name, `${path}.name`),
    type: readString(value.type, `${path}.type`),
    required: readBoolean(value.required, `${path}.required`),
    description: readString(value.description, `${path}.description`),
  };
}

function parseStepDescription(value: unknown, path: string): StepDescription {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid workflow bridge payload: expected ${path} to be an object.`);
  }

  if (!Array.isArray(value.expected_input)) {
    throw new TypeError(`Invalid workflow bridge payload: expected ${path}.expected_input to be an array.`);
  }

  return {
    name: readString(value.name, `${path}.name`),
    description: readString(value.description, `${path}.description`),
    expectedInput: value.expected_input.map((entry, index) =>
      parseParamDescription(entry, `${path}.expected_input[${index}]`),
    ),
    underlyingPrimitives: readStringArray(value.underlying_primitives, `${path}.underlying_primitives`),
  };
}

function parseWorkflowDescription(value: unknown): WorkflowDescriptionResult {
  if (!isRecord(value)) {
    throw new TypeError("Invalid workflow bridge payload: describeWorkflow result must be an object.");
  }

  if (!Array.isArray(value.steps)) {
    throw new TypeError("Invalid workflow bridge payload: expected steps to be an array.");
  }

  return {
    steps: value.steps.map((entry, index) => parseStepDescription(entry, `steps[${index}]`)),
  };
}

function parseWorkflowStepResult(value: unknown): WorkflowStepResult {
  if (!isRecord(value)) {
    throw new TypeError("Invalid workflow bridge payload: workflow step result must be an object.");
  }

  return {
    workflowId: readString(value.workflow_id, "workflow_id"),
    currentStep: readString(value.current_step, "current_step"),
    stepResult: value.step_result ?? null,
    nextExpectedInput: value.next_expected_input ?? [],
  };
}

const KIND_WIRE_TO_ID: Record<string, WorkflowKindId> = {
  TRIAGE_MEMORY_LEAK: "triage_memory_leak",
  triage_memory_leak: "triage_memory_leak",
  TUNE_GC: "tune_gc",
  tune_gc: "tune_gc",
  TRAVERSE_OBJECT_GRAPH: "traverse_object_graph",
  traverse_object_graph: "traverse_object_graph",
  COMPARE_SNAPSHOTS: "compare_snapshots",
  compare_snapshots: "compare_snapshots",
  CLASSLOADER_LEAK: "classloader_leak",
  classloader_leak: "classloader_leak",
};

function parseWorkflowKindId(value: unknown, field: string): WorkflowKindId {
  const raw = readString(value, field);
  const kind = KIND_WIRE_TO_ID[raw];
  if (!kind) {
    throw new TypeError(`Invalid workflow bridge payload: unsupported ${field} '${raw}'.`);
  }
  return kind;
}

function parseWorkflowStateView(value: unknown): WorkflowStateView {
  if (!isRecord(value)) {
    throw new TypeError("Invalid workflow bridge payload: getWorkflow result must be an object.");
  }

  if (!Array.isArray(value.step_history)) {
    throw new TypeError("Invalid workflow bridge payload: expected step_history to be an array.");
  }

  const stepHistory = value.step_history.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new TypeError(
        `Invalid workflow bridge payload: expected step_history[${index}] to be an object.`,
      );
    }
    return {
      stepName: readString(entry.step_name, `step_history[${index}].step_name`),
      outputSummary: entry.output_summary ?? null,
    };
  });

  const lastSummary =
    stepHistory.length > 0 ? stepHistory[stepHistory.length - 1]?.outputSummary ?? null : null;

  return {
    workflowId: readString(value.workflow_id, "workflow_id"),
    kind: parseWorkflowKindId(value.kind, "kind"),
    currentStep: readString(value.current_step, "current_step"),
    heapDisplayName: displayHeapName(readString(value.heap_path, "heap_path")),
    stepHistory,
    stepResult: lastSummary,
    nextExpectedInput: [],
  };
}

function parseCloseWorkflowResult(value: unknown): CloseWorkflowResult {
  if (!isRecord(value)) {
    throw new TypeError("Invalid workflow bridge payload: closeWorkflow result must be an object.");
  }

  return {
    workflowId: readString(value.workflow_id, "workflow_id"),
    closed: readBoolean(value.closed, "closed"),
  };
}

function displayHeapName(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function parseSnapshotManifest(value: unknown, path: string): SnapshotManifest {
  if (!isRecord(value)) {
    throw new TypeError(`Invalid workflow bridge payload: expected ${path} to be an object.`);
  }

  return {
    schemaVersion: readNumber(value.schema_version, `${path}.schema_version`),
    heapSha256: readString(value.heap_sha256, `${path}.heap_sha256`),
    // Terra M20.G: never keep absolute store paths in React state.
    heapPath: displayHeapName(readString(value.heap_path, `${path}.heap_path`)),
    createdAt: readString(value.created_at, `${path}.created_at`),
    mnemosyneVersion: readString(value.mnemosyne_version, `${path}.mnemosyne_version`),
    objectCount: readNumber(value.object_count, `${path}.object_count`),
    hasFieldData: readBoolean(value.has_field_data, `${path}.has_field_data`),
  };
}

function parseSnapshotManifests(value: unknown): SnapshotManifest[] {
  if (!Array.isArray(value)) {
    throw new TypeError("Invalid workflow bridge payload: listSnapshots result must be an array.");
  }

  return value.map((entry, index) => parseSnapshotManifest(entry, `[${index}]`));
}

function getWorkflowBridge(): WorkflowHostBridge | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
}

export function isDescribeWorkflowAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.describeWorkflow);
}

export function isStartWorkflowAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.startWorkflow);
}

export function isNextStepAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.nextStep);
}

export function isGetWorkflowAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.getWorkflow);
}

export function isCloseWorkflowAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.closeWorkflow);
}

export function isListSnapshotsAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.listSnapshots);
}

export function isSaveSnapshotAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.saveSnapshot);
}

export function isRemoveSnapshotAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.removeSnapshot);
}

export function isOpenSnapshotAvailable(): boolean {
  return Boolean(getWorkflowBridge()?.openSnapshot);
}

export async function runDescribeWorkflow(
  kind: WorkflowKindId,
): Promise<WorkflowBridgeResult<WorkflowDescriptionResult>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.describeWorkflow) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.describeWorkflow(kind);
    return { status: "ready", data: parseWorkflowDescription(raw) };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown describeWorkflow bridge failure.",
    };
  }
}

export async function runStartWorkflow(
  kind: WorkflowKindId,
  params?: StartWorkflowParams,
): Promise<WorkflowBridgeResult<WorkflowStepResult>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.startWorkflow) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.startWorkflow(kind, params);
    return { status: "ready", data: parseWorkflowStepResult(raw) };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown startWorkflow bridge failure.",
    };
  }
}

export async function runNextStep(
  workflowId: string,
  input?: unknown,
): Promise<WorkflowBridgeResult<WorkflowStepResult>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.nextStep) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.nextStep(workflowId, input);
    return { status: "ready", data: parseWorkflowStepResult(raw) };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown nextStep bridge failure.",
    };
  }
}

export async function runGetWorkflow(
  workflowId: string,
): Promise<WorkflowBridgeResult<WorkflowStateView>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.getWorkflow) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.getWorkflow(workflowId);
    return { status: "ready", data: parseWorkflowStateView(raw) };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown getWorkflow bridge failure.",
    };
  }
}

export async function runCloseWorkflow(
  workflowId: string,
): Promise<WorkflowBridgeResult<CloseWorkflowResult>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.closeWorkflow) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.closeWorkflow(workflowId);
    return { status: "ready", data: parseCloseWorkflowResult(raw) };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown closeWorkflow bridge failure.",
    };
  }
}

export async function runListSnapshots(): Promise<WorkflowBridgeResult<SnapshotManifest[]>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.listSnapshots) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.listSnapshots();
    return { status: "ready", data: parseSnapshotManifests(raw) };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown listSnapshots bridge failure.",
    };
  }
}

export async function runSaveSnapshot(
  sourceId: string,
  retainFieldData = false,
): Promise<WorkflowBridgeResult<SnapshotManifest>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.saveSnapshot) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.saveSnapshot(sourceId, retainFieldData);
    return { status: "ready", data: parseSnapshotManifest(raw, "saveSnapshot") };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown saveSnapshot bridge failure.",
    };
  }
}

export async function runRemoveSnapshot(
  key: string,
): Promise<WorkflowBridgeResult<{ removed: boolean; key: string }>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.removeSnapshot) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.removeSnapshot(key);
    if (!isRecord(raw)) {
      throw new TypeError("Invalid workflow bridge payload: removeSnapshot result must be an object.");
    }
    return {
      status: "ready",
      data: {
        removed: readBoolean(raw.removed, "removed"),
        key: readString(raw.key, "key"),
      },
    };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown removeSnapshot bridge failure.",
    };
  }
}

function parseOpenSnapshotSummary(value: unknown): OpenSnapshotSummary {
  if (!isRecord(value)) {
    throw new TypeError("Invalid workflow bridge payload: openSnapshot result must be an object.");
  }

  const displayName = displayHeapName(readString(value.displayName, "displayName"));
  const sourceId = readString(value.sourceId, "sourceId");
  if (!sourceId.trim()) {
    throw new TypeError("Invalid workflow bridge payload: expected sourceId to be a non-empty string.");
  }

  return {
    displayName,
    sourceId,
    objectCount: readNumber(value.objectCount, "objectCount"),
    classCount: readNumber(value.classCount, "classCount"),
    gcRootCount: readNumber(value.gcRootCount, "gcRootCount"),
  };
}

export async function runOpenSnapshot(
  key: string,
): Promise<WorkflowBridgeResult<OpenSnapshotSummary>> {
  const bridge = getWorkflowBridge();

  if (!bridge?.openSnapshot) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.openSnapshot(key);
    return { status: "ready", data: parseOpenSnapshotSummary(raw) };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown openSnapshot bridge failure.",
    };
  }
}

export const WORKFLOW_COMPLETE_STEP = "complete";

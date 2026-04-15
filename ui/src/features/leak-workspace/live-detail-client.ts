import type { ArtifactProvenanceMarker } from "../../lib/analysis-types";

import type { LiveSubviewStatus } from "./types";

export type LiveDetailKey = "explain" | "gcPath" | "sourceMap" | "fix";

export type LiveDetailResult<T> = {
  status: LiveSubviewStatus;
  data?: T;
  error?: string;
};

export type ExplainResult = {
  leak_id: string;
  summary: string;
  provenance?: ArtifactProvenanceMarker[];
};

export type GcPathNode = {
  object_id: string;
  class_name: string;
  via?: string;
  is_root?: boolean;
};

export type GcPathResult = {
  leak_id: string;
  object_id: string;
  path: GcPathNode[];
  path_length: number;
  provenance?: ArtifactProvenanceMarker[];
};

export type SourceMapLocation = {
  file: string;
  line: number;
  symbol: string;
  code_snippet: string;
  git: unknown;
};

export type SourceMapResult = {
  leak_id: string;
  locations: SourceMapLocation[];
  provenance?: ArtifactProvenanceMarker[];
};

export type FixSuggestion = {
  leak_id?: string;
  class_name?: string;
  target_file?: string;
  description?: string;
  diff?: string;
  confidence?: number;
  style?: string;
};

export type FixResult = {
  suggestions: FixSuggestion[];
  provenance?: ArtifactProvenanceMarker[];
};

export type ExplainLeakInput = {
  leakId: string;
  heapPath: string;
};

export type FindLeakGcPathInput = {
  leakId: string;
  heapPath: string;
  objectId?: string;
};

export type ResolveLeakSourceMapInput = {
  leakId: string;
  className: string;
  projectRoot: string;
};

export type ProposeLeakFixInput = {
  leakId: string;
  heapPath: string;
  projectRoot?: string;
};

export type LeakWorkspaceBridgeStatus = {
  bridge: "ready" | "unavailable";
  provider: "ready" | "unknown" | "unavailable";
};

export type LeakWorkspaceHostBridge = {
  capabilities?: {
    provider?: "ready" | "unknown" | "unavailable";
  };
  explainLeak?: (input: ExplainLeakInput) => Promise<unknown>;
  findGcPath?: (input: FindLeakGcPathInput) => Promise<unknown>;
  mapToCode?: (input: ResolveLeakSourceMapInput) => Promise<unknown>;
  proposeFix?: (input: ProposeLeakFixInput) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__?: LeakWorkspaceHostBridge;
  }
}

function hasFallbackProvenance(provenance?: ArtifactProvenanceMarker[]) {
  return (provenance ?? []).some((marker) => marker.kind === "FALLBACK" || marker.kind === "SYNTHETIC");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown, field: string) {
  if (typeof value !== "string") {
    throw new Error(`Invalid leak workspace bridge payload: expected ${field} to be a string.`);
  }

  return value;
}

function readOptionalString(value: unknown, field: string) {
  if (value === undefined || value === null) {
    return undefined;
  }

  return readString(value, field);
}

function readNumber(value: unknown, field: string) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`Invalid leak workspace bridge payload: expected ${field} to be a number.`);
  }

  return value;
}

function readOptionalNumber(value: unknown, field: string) {
  if (value === undefined || value === null) {
    return undefined;
  }

  return readNumber(value, field);
}

function readOptionalBoolean(value: unknown, field: string) {
  if (value === undefined || value === null) {
    return undefined;
  }

  if (typeof value !== "boolean") {
    throw new Error(`Invalid leak workspace bridge payload: expected ${field} to be a boolean.`);
  }

  return value;
}

function readProvenance(value: unknown, field: string) {
  if (value === undefined || value === null) {
    return undefined;
  }

  if (!Array.isArray(value)) {
    throw new Error(`Invalid leak workspace bridge payload: expected ${field} to be an array.`);
  }

  return value.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`Invalid leak workspace bridge payload: expected ${field}[${index}] to be an object.`);
    }

    return {
      kind: readString(entry.kind, `${field}[${index}].kind`),
      detail: readOptionalString(entry.detail, `${field}[${index}].detail`),
    } satisfies ArtifactProvenanceMarker;
  });
}

function parseExplainResult(value: unknown): ExplainResult {
  if (!isRecord(value)) {
    throw new Error("Invalid leak workspace bridge payload: explain result must be an object.");
  }

  return {
    leak_id: readOptionalString(value.leak_id, "explain.leak_id") ?? "",
    summary: readString(value.summary, "explain.summary"),
    provenance: readProvenance(value.provenance, "explain.provenance"),
  };
}

function parseGcPathResult(value: unknown, leakId: string): GcPathResult {
  if (!isRecord(value)) {
    throw new Error("Invalid leak workspace bridge payload: gc-path result must be an object.");
  }

  if (!Array.isArray(value.path)) {
    throw new Error("Invalid leak workspace bridge payload: gc-path path must be an array.");
  }

  return {
    leak_id: leakId,
    object_id: readString(value.object_id, "gcPath.object_id"),
    path: value.path.map((node, index) => {
      if (!isRecord(node)) {
        throw new Error(`Invalid leak workspace bridge payload: gcPath.path[${index}] must be an object.`);
      }

      return {
        object_id: readString(node.object_id, `gcPath.path[${index}].object_id`),
        class_name: readString(node.class_name, `gcPath.path[${index}].class_name`),
        via: readOptionalString(node.field, `gcPath.path[${index}].field`),
        is_root: readOptionalBoolean(node.is_root, `gcPath.path[${index}].is_root`),
      } satisfies GcPathNode;
    }),
    path_length: readNumber(value.path_length, "gcPath.path_length"),
    provenance: readProvenance(value.provenance, "gcPath.provenance"),
  };
}

function parseSourceMapResult(value: unknown): SourceMapResult {
  if (!isRecord(value)) {
    throw new Error("Invalid leak workspace bridge payload: source-map result must be an object.");
  }

  if (!Array.isArray(value.locations)) {
    throw new Error("Invalid leak workspace bridge payload: source-map locations must be an array.");
  }

  return {
    leak_id: readOptionalString(value.leak_id, "sourceMap.leak_id") ?? "",
    locations: value.locations.map((location, index) => {
      if (!isRecord(location)) {
        throw new Error(`Invalid leak workspace bridge payload: sourceMap.locations[${index}] must be an object.`);
      }

      return {
        file: readString(location.file, `sourceMap.locations[${index}].file`),
        line: readNumber(location.line, `sourceMap.locations[${index}].line`),
        symbol: readString(location.symbol, `sourceMap.locations[${index}].symbol`),
        code_snippet: readOptionalString(location.code_snippet, `sourceMap.locations[${index}].code_snippet`) ?? "",
        git: location.git,
      } satisfies SourceMapLocation;
    }),
    provenance: readProvenance(value.provenance, "sourceMap.provenance"),
  };
}

function parseFixResult(value: unknown): FixResult {
  if (!isRecord(value)) {
    throw new Error("Invalid leak workspace bridge payload: fix result must be an object.");
  }

  if (!Array.isArray(value.suggestions)) {
    throw new Error("Invalid leak workspace bridge payload: fix suggestions must be an array.");
  }

  return {
    suggestions: value.suggestions.map((suggestion, index) => {
      if (!isRecord(suggestion)) {
        throw new Error(`Invalid leak workspace bridge payload: fix.suggestions[${index}] must be an object.`);
      }

      return {
        leak_id: readOptionalString(suggestion.leak_id, `fix.suggestions[${index}].leak_id`),
        class_name: readOptionalString(suggestion.class_name, `fix.suggestions[${index}].class_name`),
        target_file: readOptionalString(suggestion.target_file, `fix.suggestions[${index}].target_file`),
        description: readOptionalString(suggestion.description, `fix.suggestions[${index}].description`),
        diff: readOptionalString(suggestion.diff, `fix.suggestions[${index}].diff`),
        confidence: readOptionalNumber(suggestion.confidence, `fix.suggestions[${index}].confidence`),
        style: readOptionalString(suggestion.style, `fix.suggestions[${index}].style`),
      } satisfies FixSuggestion;
    }),
    provenance: readProvenance(value.provenance, "fix.provenance"),
  };
}

function getLeakWorkspaceHostBridge(): LeakWorkspaceHostBridge | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.__MNEMOSYNE_LEAK_WORKSPACE_BRIDGE__;
}

export function getLeakWorkspaceBridgeStatus(): LeakWorkspaceBridgeStatus {
  const bridge = getLeakWorkspaceHostBridge();
  const hasBridgeMethods = Boolean(bridge?.explainLeak || bridge?.findGcPath || bridge?.mapToCode || bridge?.proposeFix);

  if (!bridge || !hasBridgeMethods) {
    return {
      bridge: "unavailable",
      provider: "unavailable",
    };
  }

  return {
    bridge: "ready",
    provider: bridge.capabilities?.provider ?? "unknown",
  };
}

export function normalizeExplainResult(input: ExplainResult): LiveDetailResult<ExplainResult> {
  return {
    status: hasFallbackProvenance(input.provenance) ? "fallback" : "ready",
    data: input,
  };
}

export function normalizeGcPathResult(input: GcPathResult): LiveDetailResult<GcPathResult> {
  return {
    status: hasFallbackProvenance(input.provenance) ? "fallback" : "ready",
    data: input,
  };
}

export function normalizeSourceMapResult(input: SourceMapResult): LiveDetailResult<SourceMapResult> {
  const fallback =
    hasFallbackProvenance(input.provenance)
    || input.locations.some((location) => location.file.includes(".mnemosyne/unmapped"));

  return {
    status: fallback ? "fallback" : "ready",
    data: input,
  };
}

export function normalizeFixResult(input: FixResult): LiveDetailResult<FixResult> {
  return {
    status: hasFallbackProvenance(input.provenance) ? "fallback" : "ready",
    data: input,
  };
}

export async function explainLeak(input: ExplainLeakInput): Promise<LiveDetailResult<ExplainResult>> {
  const bridge = getLeakWorkspaceHostBridge();

  if (!bridge?.explainLeak) {
    return {
      status: "unavailable",
      error: "Local explain bridge is unavailable.",
    };
  }

  try {
    const raw = await bridge.explainLeak(input);
    const parsed = parseExplainResult(raw);

    return normalizeExplainResult({
      ...parsed,
      leak_id: parsed.leak_id || input.leakId,
    });
  } catch (error: unknown) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Explain bridge request failed.",
    };
  }
}

export async function findLeakGcPath(
  input: FindLeakGcPathInput,
): Promise<LiveDetailResult<GcPathResult>> {
  if (!input.objectId) {
    return {
      status: "unavailable",
      data: {
        leak_id: input.leakId,
        object_id: "",
        path: [],
        path_length: 0,
      },
    };
  }

  const bridge = getLeakWorkspaceHostBridge();

  if (!bridge?.findGcPath) {
    return {
      status: "unavailable",
      error: "Local GC path bridge is unavailable.",
    };
  }

  try {
    const raw = await bridge.findGcPath(input);
    const parsed = parseGcPathResult(raw, input.leakId);

    return normalizeGcPathResult(parsed);
  } catch (error: unknown) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "GC path bridge request failed.",
    };
  }
}

export async function resolveLeakSourceMap(
  input: ResolveLeakSourceMapInput,
): Promise<LiveDetailResult<SourceMapResult>> {
  const bridge = getLeakWorkspaceHostBridge();

  if (!bridge?.mapToCode) {
    return {
      status: "unavailable",
      error: "Local source map bridge is unavailable.",
    };
  }

  try {
    const raw = await bridge.mapToCode(input);
    const parsed = parseSourceMapResult(raw);

    return normalizeSourceMapResult({
      ...parsed,
      leak_id: parsed.leak_id || input.leakId,
    });
  } catch (error: unknown) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Source map bridge request failed.",
    };
  }
}

export async function proposeLeakFix(
  input: ProposeLeakFixInput,
): Promise<LiveDetailResult<FixResult>> {
  if (!input.projectRoot) {
    return {
      status: "unavailable",
      data: {
        suggestions: [],
      },
      error: "Required local context is missing.",
    };
  }

  const bridge = getLeakWorkspaceHostBridge();

  if (!bridge?.proposeFix) {
    return {
      status: "unavailable",
      error: "Local fix bridge is unavailable.",
    };
  }

  try {
    const raw = await bridge.proposeFix(input);

    return normalizeFixResult(parseFixResult(raw));
  } catch (error: unknown) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Fix bridge request failed.",
    };
  }
}

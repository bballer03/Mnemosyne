export type ArtifactProvenanceMarker = {
  kind: string;
  detail?: string;
};

export type AnalysisArtifact = {
  summary: {
    heapPath: string;
    totalObjects: number;
    totalSizeBytes: number;
    generatedAt?: string;
    totalRecords: number;
  };
  leaks: Array<{
    id: string;
    className: string;
    leakKind: string;
    severity: string;
    retainedSizeBytes: number;
    shallowSizeBytes?: number;
    suspectScore?: number;
    instances: number;
    description: string;
    provenance: ArtifactProvenanceMarker[];
  }>;
  recommendations: string[];
  elapsedSeconds: number;
  graph: {
    nodeCount: number;
    edgeCount: number;
    dominatorCount: number;
  };
  histogram?: {
    groupBy: string;
    entries: Array<{
      key: string;
      instanceCount: number;
      shallowSize: number;
      retainedSize: number;
    }>;
    totalInstances: number;
    totalShallowSize: number;
  };
  provenance: ArtifactProvenanceMarker[];
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be a string`);
  }

  return value;
}

function readNumber(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be a number`);
  }

  return value;
}

function readOptionalString(value: unknown, field: string): string | undefined {
  if (value === undefined) {
    return undefined;
  }

  return readString(value, field);
}

function readOptionalGeneratedAt(value: unknown, field: string): string | undefined {
  if (value === undefined) {
    return undefined;
  }

  if (typeof value === "string") {
    return value;
  }

  if (!isRecord(value)) {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be a string or object`);
  }

  const secs = readNumber(value.secs_since_epoch, `${field}.secs_since_epoch`);
  const nanos = readNumber(value.nanos_since_epoch, `${field}.nanos_since_epoch`);
  const millis = secs * 1_000 + nanos / 1_000_000;

  return new Date(millis).toISOString();
}

function readOptionalNumber(value: unknown, field: string): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }

  return readNumber(value, field);
}

function readStringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be an array`);
  }

  return value.map((entry, index) => readString(entry, `${field}[${index}]`));
}

function readProvenanceMarkers(value: unknown, field: string): ArtifactProvenanceMarker[] {
  if (value === undefined) {
    return [];
  }

  if (!Array.isArray(value)) {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be an array`);
  }

  return value.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(
        `Invalid Mnemosyne analysis artifact: expected ${field}[${index}] to be an object`,
      );
    }

    return {
      kind: readString(entry.kind, `${field}[${index}].kind`),
      detail: readOptionalString(entry.detail, `${field}[${index}].detail`),
    };
  });
}

function readArray(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be an array`);
  }

  return value;
}

export function parseAnalysisArtifact(input: unknown): AnalysisArtifact {
  if (!isRecord(input)) {
    throw new Error("Invalid Mnemosyne analysis artifact: expected a JSON object");
  }

  if (!isRecord(input.summary)) {
    throw new Error("Invalid Mnemosyne analysis artifact: missing summary");
  }

  if (!Array.isArray(input.leaks)) {
    throw new Error("Invalid Mnemosyne analysis artifact: missing leaks");
  }

  if (!Array.isArray(input.recommendations)) {
    throw new Error("Invalid Mnemosyne analysis artifact: missing recommendations");
  }

  if (!isRecord(input.elapsed)) {
    throw new Error("Invalid Mnemosyne analysis artifact: missing elapsed");
  }

  if (!isRecord(input.graph)) {
    throw new Error("Invalid Mnemosyne analysis artifact: missing graph");
  }

  const dominators = readArray(input.graph.dominators, "graph.dominators");

  const histogram = input.histogram === undefined
    ? undefined
    : (() => {
        if (!isRecord(input.histogram)) {
          throw new Error("Invalid Mnemosyne analysis artifact: expected histogram to be an object");
        }

        const entries = readArray(input.histogram.entries, "histogram.entries");

        return {
          groupBy: readString(input.histogram.group_by, "histogram.group_by"),
          entries: entries.map((entry, index) => {
            if (!isRecord(entry)) {
              throw new Error(
                `Invalid Mnemosyne analysis artifact: expected histogram.entries[${index}] to be an object`,
              );
            }

            return {
              key: readString(entry.key, `histogram.entries[${index}].key`),
              instanceCount: readNumber(
                entry.instance_count,
                `histogram.entries[${index}].instance_count`,
              ),
              shallowSize: readNumber(
                entry.shallow_size,
                `histogram.entries[${index}].shallow_size`,
              ),
              retainedSize: readNumber(
                entry.retained_size,
                `histogram.entries[${index}].retained_size`,
              ),
            };
          }),
          totalInstances: readNumber(input.histogram.total_instances, "histogram.total_instances"),
          totalShallowSize: readNumber(
            input.histogram.total_shallow_size,
            "histogram.total_shallow_size",
          ),
        };
      })();

  const elapsedSeconds =
    readNumber(input.elapsed.secs, "elapsed.secs") +
    readNumber(input.elapsed.nanos, "elapsed.nanos") / 1_000_000_000;

  return {
    summary: {
      heapPath: readString(input.summary.heap_path, "summary.heap_path"),
      totalObjects: readNumber(input.summary.total_objects, "summary.total_objects"),
      totalSizeBytes: readNumber(input.summary.total_size_bytes, "summary.total_size_bytes"),
      generatedAt: readOptionalGeneratedAt(input.summary.generated_at, "summary.generated_at"),
      totalRecords: readNumber(input.summary.total_records, "summary.total_records"),
    },
    leaks: input.leaks.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(`Invalid Mnemosyne analysis artifact: expected leaks[${index}] to be an object`);
      }

      return {
        id: readString(entry.id, `leaks[${index}].id`),
        className: readString(entry.class_name, `leaks[${index}].class_name`),
        leakKind: readString(entry.leak_kind, `leaks[${index}].leak_kind`),
        severity: readString(entry.severity, `leaks[${index}].severity`),
        retainedSizeBytes: readNumber(
          entry.retained_size_bytes,
          `leaks[${index}].retained_size_bytes`,
        ),
        shallowSizeBytes: readOptionalNumber(
          entry.shallow_size_bytes,
          `leaks[${index}].shallow_size_bytes`,
        ),
        suspectScore: readOptionalNumber(entry.suspect_score, `leaks[${index}].suspect_score`),
        instances: readNumber(entry.instances, `leaks[${index}].instances`),
        description: readString(entry.description, `leaks[${index}].description`),
        provenance: readProvenanceMarkers(entry.provenance, `leaks[${index}].provenance`),
      };
    }),
    recommendations: readStringArray(input.recommendations, "recommendations"),
    elapsedSeconds,
    graph: {
      nodeCount: readNumber(input.graph.node_count, "graph.node_count"),
      edgeCount: readNumber(input.graph.edge_count, "graph.edge_count"),
      dominatorCount: dominators.length,
    },
    histogram,
    provenance: readProvenanceMarkers(input.provenance, "provenance"),
  };
}

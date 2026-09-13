export type ArtifactProvenanceMarker = {
  kind: string;
  detail?: string;
};

export type ReferrerEntry = {
  objectId: string;
  className: string;
  retainedSize?: number;
  referrerCount: number;
  topReferrerClasses: Array<[string, number]>;
};

export type ReferrerReport = {
  entries: ReferrerEntry[];
  totalObjectsConsidered: number;
};

export type DuplicateClassGroup = {
  className: string;
  loaderObjectIds: number[];
  loaderCount: number;
};

// Which raw HPROF GC-root sub-record a frame local was resolved from
// (core::analysis::thread::FrameLocalRootKind). Serialized as the bare
// Rust enum variant name -- no `#[serde(rename_all)]` on this enum, so
// the wire values are exactly "JavaFrame" / "JniLocal".
export type FrameLocalRootKind = "JavaFrame" | "JniLocal";

export type FrameLocal = {
  variableSlot: number;
  objectId: string;
  className: string;
  rootKind: FrameLocalRootKind;
};

export type StackFrameInfo = {
  methodName: string;
  className: string;
  sourceFile?: string;
  lineNumber: number;
  locals: FrameLocal[];
};

export type ThreadInfo = {
  objectId: number;
  name: string;
  daemon: boolean;
  stackTrace?: StackFrameInfo[];
  retainedBytes: number;
  threadLocalCount: number;
  threadLocalBytes: number;
};

export type ThreadReport = {
  threads: ThreadInfo[];
  totalThreadCount: number;
  totalThreadRetained: number;
  topRetainers: ThreadInfo[];
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
    dominators: Array<{
      name: string;
      className: string;
      objectId: string;
      dominates: number;
      immediateDominator?: string;
      retainedSize: number;
      shallowSize: number;
    }>;
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
  unreachable?: {
    totalCount: number;
    totalShallowSize: number;
    byClass: Array<{
      className: string;
      count: number;
      shallowSize: number;
    }>;
  };
  stringReport?: {
    totalStrings: number;
    totalStringBytes: number;
    uniqueStrings: number;
    duplicateGroups: Array<{
      value: string;
      count: number;
      totalWastedBytes: number;
    }>;
    totalDuplicateWaste: number;
    topStringsBySize: Array<{
      objectId: number;
      value: string;
      byteLength: number;
      retainedBytes?: number;
    }>;
  };
  collectionReport?: {
    totalCollections: number;
    totalWasteBytes: number;
    emptyCollections: number;
    oversizedCollections: Array<{
      objectId: number;
      collectionType: string;
      size: number;
      capacity?: number;
      fillRatio?: number;
      shallowBytes: number;
      retainedBytes?: number;
      wasteBytes: number;
    }>;
    summaryByType: Record<string, {
      count: number;
      totalShallow: number;
      totalRetained: number;
      totalWaste: number;
      avgFillRatio: number;
    }>;
  };
  topInstances?: {
    totalCount: number;
    instances: Array<{
      objectId: number;
      className: string;
      shallowSize: number;
      retainedSize?: number;
    }>;
  };
  classloaderReport?: {
    loaders: Array<{
      objectId: number;
      className: string;
      loadedClassCount: number;
      instanceCount: number;
      totalShallowBytes: number;
      retainedBytes?: number;
      parentLoader?: number;
      uniqueClassCount: number;
      ancestorChain: number[];
    }>;
    potentialLeaks: Array<{
      objectId: number;
      className: string;
      retainedBytes: number;
      loadedClassCount: number;
      reason: string;
    }>;
    duplicateClasses: DuplicateClassGroup[];
  };
  referrerReport?: ReferrerReport;
  threadReport?: ThreadReport;
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

function readOptionalNonEmptyString(value: unknown, field: string): string | undefined {
  const parsed = readOptionalString(value, field);
  return parsed === "" ? undefined : parsed;
}

function parseGraphDominatorRows(
  value: unknown,
): NonNullable<AnalysisArtifact["graph"]>["dominators"] {
  const dominators = readArray(value, "graph.dominators");

  return dominators.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(
        `Invalid Mnemosyne analysis artifact: expected graph.dominators[${index}] to be an object`,
      );
    }

    const name = readString(entry.name, `graph.dominators[${index}].name`);

    return {
      name,
      className: readOptionalNonEmptyString(
        entry.class_name,
        `graph.dominators[${index}].class_name`,
      ) ?? name,
      objectId: readOptionalString(entry.object_id, `graph.dominators[${index}].object_id`) ?? "",
      dominates: readOptionalNumber(entry.dominates, `graph.dominators[${index}].dominates`) ?? 0,
      immediateDominator:
        entry.immediate_dominator === null
          ? undefined
          : readOptionalString(
              entry.immediate_dominator,
              `graph.dominators[${index}].immediate_dominator`,
            ),
      retainedSize: readOptionalNumber(
        entry.retained_size,
        `graph.dominators[${index}].retained_size`,
      ) ?? 0,
      shallowSize: readOptionalNumber(
        entry.shallow_size,
        `graph.dominators[${index}].shallow_size`,
      ) ?? 0,
    };
  });
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

function readOptionalSection<T>(
  value: unknown,
  field: string,
  parse: (section: Record<string, unknown>) => T,
): T | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }

  if (!isRecord(value)) {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be an object`);
  }

  return parse(value);
}

function parseUnreachableSection(
  section: Record<string, unknown>,
): NonNullable<AnalysisArtifact["unreachable"]> {
  const byClass = readArray(section.by_class, "unreachable.by_class");

  return {
    totalCount: readNumber(section.total_count, "unreachable.total_count"),
    totalShallowSize: readNumber(section.total_shallow_size, "unreachable.total_shallow_size"),
    byClass: byClass.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected unreachable.by_class[${index}] to be an object`,
        );
      }

      return {
        className: readString(entry.class_name, `unreachable.by_class[${index}].class_name`),
        count: readNumber(entry.count, `unreachable.by_class[${index}].count`),
        shallowSize: readNumber(
          entry.shallow_size,
          `unreachable.by_class[${index}].shallow_size`,
        ),
      };
    }),
  };
}

function parseStringReportSection(
  section: Record<string, unknown>,
): NonNullable<AnalysisArtifact["stringReport"]> {
  const duplicateGroups = readArray(section.duplicate_groups, "string_report.duplicate_groups");
  const topStringsBySize = readArray(
    section.top_strings_by_size,
    "string_report.top_strings_by_size",
  );

  return {
    totalStrings: readNumber(section.total_strings, "string_report.total_strings"),
    totalStringBytes: readNumber(section.total_string_bytes, "string_report.total_string_bytes"),
    uniqueStrings: readNumber(section.unique_strings, "string_report.unique_strings"),
    duplicateGroups: duplicateGroups.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected string_report.duplicate_groups[${index}] to be an object`,
        );
      }

      return {
        value: readString(entry.value, `string_report.duplicate_groups[${index}].value`),
        count: readNumber(entry.count, `string_report.duplicate_groups[${index}].count`),
        totalWastedBytes: readNumber(
          entry.total_wasted_bytes,
          `string_report.duplicate_groups[${index}].total_wasted_bytes`,
        ),
      };
    }),
    totalDuplicateWaste: readNumber(
      section.total_duplicate_waste,
      "string_report.total_duplicate_waste",
    ),
    topStringsBySize: topStringsBySize.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected string_report.top_strings_by_size[${index}] to be an object`,
        );
      }

      return {
        objectId: readNumber(entry.object_id, `string_report.top_strings_by_size[${index}].object_id`),
        value: readString(entry.value, `string_report.top_strings_by_size[${index}].value`),
        byteLength: readNumber(
          entry.byte_length,
          `string_report.top_strings_by_size[${index}].byte_length`,
        ),
        retainedBytes: readOptionalNumber(
          entry.retained_bytes,
          `string_report.top_strings_by_size[${index}].retained_bytes`,
        ),
      };
    }),
  };
}

function parseCollectionReportSection(
  section: Record<string, unknown>,
): NonNullable<AnalysisArtifact["collectionReport"]> {
  const oversizedCollections = readArray(
    section.oversized_collections,
    "collection_report.oversized_collections",
  );

  if (!isRecord(section.summary_by_type)) {
    throw new Error(
      "Invalid Mnemosyne analysis artifact: expected collection_report.summary_by_type to be an object",
    );
  }

  return {
    totalCollections: readNumber(section.total_collections, "collection_report.total_collections"),
    totalWasteBytes: readNumber(section.total_waste_bytes, "collection_report.total_waste_bytes"),
    emptyCollections: readNumber(section.empty_collections, "collection_report.empty_collections"),
    oversizedCollections: oversizedCollections.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected collection_report.oversized_collections[${index}] to be an object`,
        );
      }

      return {
        objectId: readNumber(
          entry.object_id,
          `collection_report.oversized_collections[${index}].object_id`,
        ),
        collectionType: readString(
          entry.collection_type,
          `collection_report.oversized_collections[${index}].collection_type`,
        ),
        size: readNumber(entry.size, `collection_report.oversized_collections[${index}].size`),
        capacity: readOptionalNumber(
          entry.capacity,
          `collection_report.oversized_collections[${index}].capacity`,
        ),
        fillRatio: readOptionalNumber(
          entry.fill_ratio,
          `collection_report.oversized_collections[${index}].fill_ratio`,
        ),
        shallowBytes: readNumber(
          entry.shallow_bytes,
          `collection_report.oversized_collections[${index}].shallow_bytes`,
        ),
        retainedBytes: readOptionalNumber(
          entry.retained_bytes,
          `collection_report.oversized_collections[${index}].retained_bytes`,
        ),
        wasteBytes: readNumber(
          entry.waste_bytes,
          `collection_report.oversized_collections[${index}].waste_bytes`,
        ),
      };
    }),
    summaryByType: Object.fromEntries(
      Object.entries(section.summary_by_type).map(([collectionType, entry]) => {
        if (!isRecord(entry)) {
          throw new Error(
            `Invalid Mnemosyne analysis artifact: expected collection_report.summary_by_type[${JSON.stringify(collectionType)}] to be an object`,
          );
        }

        return [
          collectionType,
          {
            count: readNumber(
              entry.count,
              `collection_report.summary_by_type[${JSON.stringify(collectionType)}].count`,
            ),
            totalShallow: readNumber(
              entry.total_shallow,
              `collection_report.summary_by_type[${JSON.stringify(collectionType)}].total_shallow`,
            ),
            totalRetained: readNumber(
              entry.total_retained,
              `collection_report.summary_by_type[${JSON.stringify(collectionType)}].total_retained`,
            ),
            totalWaste: readNumber(
              entry.total_waste,
              `collection_report.summary_by_type[${JSON.stringify(collectionType)}].total_waste`,
            ),
            avgFillRatio: readNumber(
              entry.avg_fill_ratio,
              `collection_report.summary_by_type[${JSON.stringify(collectionType)}].avg_fill_ratio`,
            ),
          },
        ];
      }),
    ),
  };
}

function parseTopInstancesSection(
  section: Record<string, unknown>,
): NonNullable<AnalysisArtifact["topInstances"]> {
  const instances = readArray(section.instances, "top_instances.instances");

  return {
    totalCount: readNumber(section.total_count, "top_instances.total_count"),
    instances: instances.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected top_instances.instances[${index}] to be an object`,
        );
      }

      return {
        objectId: readNumber(entry.object_id, `top_instances.instances[${index}].object_id`),
        className: readString(entry.class_name, `top_instances.instances[${index}].class_name`),
        shallowSize: readNumber(
          entry.shallow_size,
          `top_instances.instances[${index}].shallow_size`,
        ),
        retainedSize: readOptionalNumber(
          entry.retained_size,
          `top_instances.instances[${index}].retained_size`,
        ),
      };
    }),
  };
}

function readOptionalNumberArray(value: unknown, field: string): number[] {
  if (value === undefined || value === null) {
    return [];
  }

  if (!Array.isArray(value)) {
    throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be an array`);
  }

  return value.map((entry, index) => readNumber(entry, `${field}[${index}]`));
}

function parseDuplicateClassGroups(value: unknown, field: string): DuplicateClassGroup[] {
  if (value === undefined || value === null) {
    return [];
  }

  const groups = readArray(value, field);

  return groups.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field}[${index}] to be an object`);
    }

    return {
      className: readString(entry.class_name, `${field}[${index}].class_name`),
      loaderObjectIds: readOptionalNumberArray(
        entry.loader_object_ids,
        `${field}[${index}].loader_object_ids`,
      ),
      loaderCount: readNumber(entry.loader_count, `${field}[${index}].loader_count`),
    };
  });
}

function parseClassloaderReportSection(
  section: Record<string, unknown>,
): NonNullable<AnalysisArtifact["classloaderReport"]> {
  const loaders = readArray(section.loaders, "classloader_report.loaders");
  const potentialLeaks = readArray(section.potential_leaks, "classloader_report.potential_leaks");

  return {
    loaders: loaders.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected classloader_report.loaders[${index}] to be an object`,
        );
      }

      return {
        objectId: readNumber(entry.object_id, `classloader_report.loaders[${index}].object_id`),
        className: readString(entry.class_name, `classloader_report.loaders[${index}].class_name`),
        loadedClassCount: readNumber(
          entry.loaded_class_count,
          `classloader_report.loaders[${index}].loaded_class_count`,
        ),
        instanceCount: readNumber(
          entry.instance_count,
          `classloader_report.loaders[${index}].instance_count`,
        ),
        totalShallowBytes: readNumber(
          entry.total_shallow_bytes,
          `classloader_report.loaders[${index}].total_shallow_bytes`,
        ),
        retainedBytes: readOptionalNumber(
          entry.retained_bytes,
          `classloader_report.loaders[${index}].retained_bytes`,
        ),
        parentLoader: readOptionalNumber(
          entry.parent_loader,
          `classloader_report.loaders[${index}].parent_loader`,
        ),
        // Additive M13 fields (`unique_class_count`/`ancestor_chain`): the
        // current backend always populates them once `classloader_report`
        // is present, but an older, previously-saved artifact JSON file
        // (pre-M13) may lack them entirely -- default gracefully rather
        // than throw, same discipline as every other optional field here.
        uniqueClassCount:
          readOptionalNumber(
            entry.unique_class_count,
            `classloader_report.loaders[${index}].unique_class_count`,
          ) ?? 0,
        ancestorChain: readOptionalNumberArray(
          entry.ancestor_chain,
          `classloader_report.loaders[${index}].ancestor_chain`,
        ),
      };
    }),
    potentialLeaks: potentialLeaks.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected classloader_report.potential_leaks[${index}] to be an object`,
        );
      }

      return {
        objectId: readNumber(
          entry.object_id,
          `classloader_report.potential_leaks[${index}].object_id`,
        ),
        className: readString(
          entry.class_name,
          `classloader_report.potential_leaks[${index}].class_name`,
        ),
        retainedBytes: readNumber(
          entry.retained_bytes,
          `classloader_report.potential_leaks[${index}].retained_bytes`,
        ),
        loadedClassCount: readNumber(
          entry.loaded_class_count,
          `classloader_report.potential_leaks[${index}].loaded_class_count`,
        ),
        reason: readString(entry.reason, `classloader_report.potential_leaks[${index}].reason`),
      };
    }),
    // Additive M13 field: absent entirely on a pre-M13 artifact JSON.
    duplicateClasses: parseDuplicateClassGroups(
      section.duplicate_classes,
      "classloader_report.duplicate_classes",
    ),
  };
}

function parseReferrerReportSection(
  section: Record<string, unknown>,
): NonNullable<AnalysisArtifact["referrerReport"]> {
  const entries = readArray(section.entries, "referrer_report.entries");

  return {
    entries: entries.map((entry, index) => {
      if (!isRecord(entry)) {
        throw new Error(
          `Invalid Mnemosyne analysis artifact: expected referrer_report.entries[${index}] to be an object`,
        );
      }

      const topReferrerClasses = readArray(
        entry.top_referrer_classes,
        `referrer_report.entries[${index}].top_referrer_classes`,
      );

      return {
        objectId: readString(entry.object_id, `referrer_report.entries[${index}].object_id`),
        className: readString(entry.class_name, `referrer_report.entries[${index}].class_name`),
        retainedSize: readOptionalNumber(
          entry.retained_size,
          `referrer_report.entries[${index}].retained_size`,
        ),
        referrerCount: readNumber(
          entry.referrer_count,
          `referrer_report.entries[${index}].referrer_count`,
        ),
        topReferrerClasses: topReferrerClasses.map((pair, pairIndex) => {
          if (!Array.isArray(pair) || pair.length !== 2) {
            throw new Error(
              `Invalid Mnemosyne analysis artifact: expected referrer_report.entries[${index}].top_referrer_classes[${pairIndex}] to be a [string, number] pair`,
            );
          }

          return [
            readString(
              pair[0],
              `referrer_report.entries[${index}].top_referrer_classes[${pairIndex}][0]`,
            ),
            readNumber(
              pair[1],
              `referrer_report.entries[${index}].top_referrer_classes[${pairIndex}][1]`,
            ),
          ] as [string, number];
        }),
      };
    }),
    totalObjectsConsidered: readNumber(
      section.total_objects_considered,
      "referrer_report.total_objects_considered",
    ),
  };
}

function parseFrameLocalRootKind(value: unknown, field: string): FrameLocalRootKind {
  if (value === "JavaFrame" || value === "JniLocal") {
    return value;
  }

  throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field} to be "JavaFrame" or "JniLocal"`);
}

function parseStackFrameInfoList(value: unknown, field: string): StackFrameInfo[] {
  const frames = readArray(value, field);

  return frames.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field}[${index}] to be an object`);
    }

    const locals = readArray(entry.locals, `${field}[${index}].locals`);

    return {
      methodName: readString(entry.method_name, `${field}[${index}].method_name`),
      className: readString(entry.class_name, `${field}[${index}].class_name`),
      sourceFile: readOptionalString(entry.source_file, `${field}[${index}].source_file`),
      lineNumber: readNumber(entry.line_number, `${field}[${index}].line_number`),
      locals: locals.map((local, localIndex) => {
        if (!isRecord(local)) {
          throw new Error(
            `Invalid Mnemosyne analysis artifact: expected ${field}[${index}].locals[${localIndex}] to be an object`,
          );
        }

        return {
          variableSlot: readNumber(
            local.variable_slot,
            `${field}[${index}].locals[${localIndex}].variable_slot`,
          ),
          objectId: readString(local.object_id, `${field}[${index}].locals[${localIndex}].object_id`),
          className: readString(local.class_name, `${field}[${index}].locals[${localIndex}].class_name`),
          rootKind: parseFrameLocalRootKind(
            local.root_kind,
            `${field}[${index}].locals[${localIndex}].root_kind`,
          ),
        };
      }),
    };
  });
}

function parseThreadInfoList(value: unknown, field: string): ThreadInfo[] {
  const threads = readArray(value, field);

  return threads.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`Invalid Mnemosyne analysis artifact: expected ${field}[${index}] to be an object`);
    }

    const stackTrace =
      entry.stack_trace === undefined || entry.stack_trace === null
        ? undefined
        : parseStackFrameInfoList(entry.stack_trace, `${field}[${index}].stack_trace`);

    return {
      objectId: readNumber(entry.object_id, `${field}[${index}].object_id`),
      name: readString(entry.name, `${field}[${index}].name`),
      daemon: entry.daemon === true,
      stackTrace,
      retainedBytes: readNumber(entry.retained_bytes, `${field}[${index}].retained_bytes`),
      threadLocalCount: readNumber(
        entry.thread_local_count,
        `${field}[${index}].thread_local_count`,
      ),
      threadLocalBytes: readNumber(
        entry.thread_local_bytes,
        `${field}[${index}].thread_local_bytes`,
      ),
    };
  });
}

function parseThreadReportSection(
  section: Record<string, unknown>,
): NonNullable<AnalysisArtifact["threadReport"]> {
  return {
    threads: parseThreadInfoList(section.threads, "thread_report.threads"),
    totalThreadCount: readNumber(section.total_thread_count, "thread_report.total_thread_count"),
    totalThreadRetained: readNumber(
      section.total_thread_retained,
      "thread_report.total_thread_retained",
    ),
    topRetainers: parseThreadInfoList(section.top_retainers, "thread_report.top_retainers"),
  };
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

  const dominators = parseGraphDominatorRows(input.graph.dominators);

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

  const unreachable = readOptionalSection(input.unreachable, "unreachable", parseUnreachableSection);
  const stringReport = readOptionalSection(
    input.string_report,
    "string_report",
    parseStringReportSection,
  );
  const collectionReport = readOptionalSection(
    input.collection_report,
    "collection_report",
    parseCollectionReportSection,
  );
  const topInstances = readOptionalSection(
    input.top_instances,
    "top_instances",
    parseTopInstancesSection,
  );
  const classloaderReport = readOptionalSection(
    input.classloader_report,
    "classloader_report",
    parseClassloaderReportSection,
  );
  const referrerReport = readOptionalSection(
    input.referrer_report,
    "referrer_report",
    parseReferrerReportSection,
  );
  const threadReport = readOptionalSection(
    input.thread_report,
    "thread_report",
    parseThreadReportSection,
  );

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
      dominators,
    },
    histogram,
    unreachable,
    stringReport,
    collectionReport,
    topInstances,
    classloaderReport,
    referrerReport,
    threadReport,
    provenance: readProvenanceMarkers(input.provenance, "provenance"),
  };
}

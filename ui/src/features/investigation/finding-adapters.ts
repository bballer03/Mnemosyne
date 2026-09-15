import type { AnalysisArtifact } from "../../lib/analysis-types";
import type {
  FindingFact,
  FindingTarget,
} from "./investigation-store";

export type PolicyViolationFindingInput = Readonly<{
  rule_id: string;
  predicate: string;
  severity: string;
  message: string;
  remediation_hint?: string | null;
}>;

const severityRanks: Record<string, number> = {
  CRITICAL: 5,
  ERROR: 4,
  HIGH: 4,
  WARNING: 3,
  MEDIUM: 3,
  LOW: 2,
  INFO: 1,
};

function hexObjectId(objectId: number): string {
  return `0x${objectId.toString(16)}`;
}

function stableTextHash(value: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function measuredPriority(fact: FindingFact): number {
  for (const key of ["retainedBytes", "wasteBytes", "totalWastedBytes", "loaderCount"]) {
    const value = fact.metrics[key];
    if (typeof value === "number") {
      return value;
    }
  }
  return 0;
}

function sortFacts(facts: FindingFact[]): FindingFact[] {
  return facts.sort(
    (left, right) =>
      (severityRanks[right.severity.toUpperCase()] ?? 0) -
        (severityRanks[left.severity.toUpperCase()] ?? 0) ||
      measuredPriority(right) - measuredPriority(left) ||
      left.id.localeCompare(right.id),
  );
}

function artifactProvenance(artifact: AnalysisArtifact) {
  return artifact.provenance.map((marker) => ({ ...marker }));
}

function buildLeakFacts(artifact: AnalysisArtifact): FindingFact[] {
  return artifact.leaks.map((leak) => {
    const objectId = artifact.graph.dominators.find(
      (entry) => entry.className === leak.className && entry.objectId.length > 0,
    )?.objectId;
    return {
      id: `leak:${leak.id}`,
      source: "artifact",
      kind: "leak",
      severity: leak.severity,
      title: `Leak suspect: ${leak.className}`,
      description: leak.description,
      target: {
        kind: "leak",
        leakId: leak.id,
        classKey: leak.className,
        ...(objectId ? { objectId } : {}),
      },
      provenance: leak.provenance.map((marker) => ({ ...marker })),
      metrics: {
        retainedBytes: leak.retainedSizeBytes,
        instances: leak.instances,
      },
    };
  });
}

function buildClassloaderFacts(artifact: AnalysisArtifact): FindingFact[] {
  const report = artifact.classloaderReport;
  if (!report) {
    return [];
  }
  const provenance = artifactProvenance(artifact);
  return [
    ...report.potentialLeaks.map((leak): FindingFact => {
      const objectId = hexObjectId(leak.objectId);
      return {
        id: `classloader:loader:${objectId}`,
        source: "artifact",
        kind: "classloader",
        severity: "HIGH",
        title: `Classloader retention: ${leak.className}`,
        description: leak.reason,
        target: {
          kind: "object",
          objectId,
          classKey: leak.className,
        },
        provenance: provenance.map((marker) => ({ ...marker })),
        metrics: {
          retainedBytes: leak.retainedBytes,
          loadedClassCount: leak.loadedClassCount,
        },
      };
    }),
    ...report.duplicateClasses.map((group): FindingFact => ({
      id: `classloader:duplicate:${group.className}`,
      source: "artifact",
      kind: "classloader",
      severity: "WARNING",
      title: `Duplicate class: ${group.className}`,
      description: `${group.className} is loaded by ${group.loaderCount} distinct classloaders.`,
      target: { kind: "class", classKey: group.className },
      provenance: provenance.map((marker) => ({ ...marker })),
      metrics: { loaderCount: group.loaderCount },
    })),
  ];
}

function buildCollectionFacts(artifact: AnalysisArtifact): FindingFact[] {
  const report = artifact.collectionReport;
  if (!report) {
    return [];
  }
  const provenance = artifactProvenance(artifact);
  return report.oversizedCollections.map((collection) => {
    const objectId = hexObjectId(collection.objectId);
    return {
      id: `collection:${objectId}`,
      source: "artifact",
      kind: "collection-waste",
      severity: "WARNING",
      title: `Collection waste: ${collection.collectionType}`,
      description: `${collection.collectionType} uses ${collection.capacity ?? "unknown"} slots for ${collection.size} entries.`,
      target: {
        kind: "object",
        objectId,
        classKey: collection.collectionType,
      },
      provenance: provenance.map((marker) => ({ ...marker })),
      metrics: {
        wasteBytes: collection.wasteBytes,
        size: collection.size,
        ...(collection.capacity === undefined ? {} : { capacity: collection.capacity }),
      },
    } satisfies FindingFact;
  });
}

function buildStringFacts(artifact: AnalysisArtifact): FindingFact[] {
  const report = artifact.stringReport;
  if (!report) {
    return [];
  }
  const provenance = artifactProvenance(artifact);
  return report.duplicateGroups.map((group) => ({
    id: `string:${stableTextHash(group.value)}:${group.count}:${group.totalWastedBytes}`,
    source: "artifact",
    kind: "string-waste",
    severity: "WARNING",
    title: "Duplicate string content",
    description: `${group.count} strings share duplicate content and waste ${group.totalWastedBytes} bytes.`,
    target: { kind: "class", classKey: "java.lang.String" },
    provenance: provenance.map((marker) => ({ ...marker })),
    metrics: {
      count: group.count,
      wasteBytes: group.totalWastedBytes,
    },
  }));
}

function buildArrayFacts(artifact: AnalysisArtifact): FindingFact[] {
  const report = artifact.arrayReport;
  if (!report) {
    return [];
  }
  const provenance = artifactProvenance(artifact);
  return report.duplicateGroups.map((group) => ({
    id: `array:${group.elementType}:${group.contentHash}`,
    source: "artifact",
    kind: "array-waste",
    severity: "WARNING",
    title: `Duplicate ${group.elementType}[] content`,
    description: `${group.count} ${group.elementType}[] arrays of length ${group.length} waste ${group.totalWastedBytes} bytes.`,
    target: { kind: "class", classKey: `${group.elementType}[]` },
    provenance: provenance.map((marker) => ({ ...marker })),
    metrics: {
      count: group.count,
      length: group.length,
      wasteBytes: group.totalWastedBytes,
      contentHash: group.contentHash,
    },
  }));
}

export function buildArtifactFindingFacts(
  artifact: AnalysisArtifact,
): readonly FindingFact[] {
  return sortFacts([
    ...buildLeakFacts(artifact),
    ...buildClassloaderFacts(artifact),
    ...buildCollectionFacts(artifact),
    ...buildStringFacts(artifact),
    ...buildArrayFacts(artifact),
  ]);
}

function firstTarget(
  facts: readonly FindingFact[],
  predicate: (fact: FindingFact) => boolean,
): FindingTarget | undefined {
  return facts.find(predicate)?.target;
}

function policyTarget(
  predicate: string,
  measuredFacts: readonly FindingFact[],
): FindingTarget | undefined {
  if (predicate === "leak_count" || predicate === "retained_size") {
    return firstTarget(measuredFacts, (fact) => fact.kind === "leak");
  }
  if (predicate === "classloader_leak_count") {
    return firstTarget(measuredFacts, (fact) => fact.kind === "classloader");
  }
  if (predicate === "object_growth_threshold") {
    return (
      firstTarget(measuredFacts, (fact) => fact.target.kind === "object") ??
      firstTarget(measuredFacts, (fact) => fact.target.kind === "class")
    );
  }
  return undefined;
}

export function buildPolicyFindingFacts(
  violations: readonly PolicyViolationFindingInput[],
  measuredFacts: readonly FindingFact[],
): readonly FindingFact[] {
  const facts = violations.flatMap((violation): FindingFact[] => {
    const target = policyTarget(violation.predicate, measuredFacts);
    if (!target) {
      return [];
    }
    return [
      {
        id: `policy:${violation.rule_id}:${violation.predicate}`,
        source: "policy",
        kind: "policy",
        severity: violation.severity,
        title: `Policy violation: ${violation.rule_id}`,
        description: violation.message,
        target: { ...target },
        provenance: [{ kind: "RULES", detail: "deterministic ci_check result" }],
        metrics: {
          ruleId: violation.rule_id,
          predicate: violation.predicate,
          remediationHint: violation.remediation_hint ?? "",
        },
      },
    ];
  });
  return sortFacts(facts);
}

export function findingHref(target: FindingTarget): string {
  switch (target.kind) {
    case "leak":
      return `/leaks/${encodeURIComponent(target.leakId)}/overview`;
    case "object":
      return `/heap-explorer/object-inspector?objectId=${encodeURIComponent(target.objectId)}`;
    case "class":
      return `/artifacts/explorer?classKey=${encodeURIComponent(target.classKey)}`;
  }
}

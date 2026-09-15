import type {
  FindingFact,
  FindingKind,
  FindingTarget,
  InvestigationSelection,
} from "../investigation/investigation-store";

export const MAX_ASSISTANT_CONTEXT_FINDINGS = 5;

export type AssistantMeasuredFinding = Readonly<{
  id: string;
  kind: FindingKind;
  severity: string;
  title: string;
  description: string;
  target: FindingTarget;
  provenance: readonly Readonly<{ kind: string; detail?: string }>[];
  metrics: Readonly<Record<string, string | number>>;
}>;

function matchesSelection(
  selection: InvestigationSelection,
  target: FindingTarget,
): boolean {
  return (
    (target.kind === "leak" &&
      selection.leakId !== undefined &&
      target.leakId === selection.leakId) ||
    ("objectId" in target &&
      selection.objectId !== undefined &&
      target.objectId === selection.objectId) ||
    ("classKey" in target &&
      selection.classKey !== undefined &&
      target.classKey === selection.classKey)
  );
}

function freezeTarget(target: FindingTarget): FindingTarget {
  return Object.freeze({ ...target });
}

function projectFinding(fact: FindingFact): AssistantMeasuredFinding {
  return Object.freeze({
    id: fact.id,
    kind: fact.kind,
    severity: fact.severity,
    title: fact.title,
    description: fact.description,
    target: freezeTarget(fact.target),
    provenance: Object.freeze(
      fact.provenance.map((marker) => Object.freeze({ ...marker })),
    ),
    metrics: Object.freeze({ ...fact.metrics }),
  });
}

export function buildAssistantMeasuredContext(
  selection: InvestigationSelection,
  findingFacts: readonly FindingFact[],
): readonly AssistantMeasuredFinding[] {
  const hasStableSelection =
    selection.objectId !== undefined ||
    selection.classKey !== undefined ||
    selection.leakId !== undefined;
  if (!hasStableSelection) {
    return Object.freeze([]);
  }

  return Object.freeze(
    findingFacts
      .filter((fact) => matchesSelection(selection, fact.target))
      .slice(0, MAX_ASSISTANT_CONTEXT_FINDINGS)
      .map(projectFinding),
  );
}

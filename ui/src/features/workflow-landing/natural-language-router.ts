// Disambiguation heuristic for the guided landing's natural-language input
// bar (design doc §4 item 9 / §9 "define the disambiguation heuristic during
// implementation, document it"). Deliberately simple, per that same note --
// this is a fast, cheap client-side routing decision, not an NLU model:
//
//   1. If the trimmed input looks OQL-shaped (starts with a `SELECT`
//      keyword, or contains an operator OQL uses that plain English
//      essentially never does -- `=~`, `==`, `!=`, `<=`, `>=`), route it to
//      the existing OQL execution path (`runHeapQuery`, the same function
//      `QueryConsolePanel`/`HeapQueryConsolePage` already call).
//   2. Otherwise, treat it as free-text intent for a workflow. A small
//      keyword table maps common phrasings to one of the four
//      `WorkflowKind`s; anything that matches none of them defaults to
//      `triage_memory_leak` (the same workflow the triage summary card
//      already runs by default), since "what's wrong with this heap" is the
//      single most common way to phrase an open-ended request.
//
// This is intentionally not sophisticated -- a future slice replacing it
// with a real intent classifier would only need to change this one module.

import type { WorkflowKindId } from "./workflow-bridge-client";

const OQL_OPERATOR_PATTERN = /=~|==|!=|<=|>=/;
const OQL_KEYWORD_PATTERN = /^\s*select\b/i;

export function looksLikeOqlQuery(input: string): boolean {
  const trimmed = input.trim();

  if (trimmed.length === 0) {
    return false;
  }

  return OQL_KEYWORD_PATTERN.test(trimmed) || OQL_OPERATOR_PATTERN.test(trimmed);
}

const WORKFLOW_KEYWORDS: Array<{ pattern: RegExp; kind: WorkflowKindId }> = [
  { pattern: /\bcompare\b|\bdiff\b|\bbefore.*after\b/i, kind: "compare_snapshots" },
  { pattern: /\bgc\b|garbage collect|\btune\b/i, kind: "tune_gc" },
  { pattern: /\btraverse\b|\bgraph\b|reference path|walk (the )?(object|graph)/i, kind: "traverse_object_graph" },
  {
    pattern: /\bclass\s*loader\b|\bclassloader\b|\bduplicate class(es)?\b|\bredeploy\b/i,
    kind: "classloader_leak",
  },
];

export function routeFreeTextToWorkflow(input: string): WorkflowKindId {
  const trimmed = input.trim();

  for (const { pattern, kind } of WORKFLOW_KEYWORDS) {
    if (pattern.test(trimmed)) {
      return kind;
    }
  }

  return "triage_memory_leak";
}

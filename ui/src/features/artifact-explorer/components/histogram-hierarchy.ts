/**
 * M22.D — UI-only superclass hierarchy projection.
 *
 * Expand/collapse is allowed only when returned entries carry an explicit
 * `parentKey` that resolves to another entry key. Flat superclass regroup keys
 * alone never invent ancestry (MAT-like tree remains open until a parent
 * relation is present in the payload).
 */

export type HierarchyHistogramEntry = {
  key: string;
  parentKey?: string;
};

export type HierarchyNode<T extends HierarchyHistogramEntry> = {
  entry: T;
  children: Array<HierarchyNode<T>>;
};

/**
 * True only when group-by is superclass and every declared parentKey resolves
 * to another entry key. Flat key-only payloads return false.
 */
export function supportsDeterministicParentRelation(
  groupBy: string | undefined,
  entries: HierarchyHistogramEntry[],
): boolean {
  const normalized = (groupBy ?? "").trim().toLowerCase();
  if (normalized !== "superclass") {
    return false;
  }

  if (entries.length === 0) {
    return false;
  }

  const keys = new Set(entries.map((entry) => entry.key));
  if (keys.size !== entries.length) {
    // Duplicate keys make parent resolution ambiguous — stay flat.
    return false;
  }

  let declaredParents = 0;
  const parentOf = new Map<string, string>();

  for (const entry of entries) {
    const parent = entry.parentKey?.trim();
    if (!parent) {
      continue;
    }

    declaredParents += 1;
    if (parent === entry.key || !keys.has(parent)) {
      return false;
    }
    parentOf.set(entry.key, parent);
  }

  if (declaredParents === 0) {
    return false;
  }

  // Reject cycles: walking parent links from any node must terminate.
  for (const start of parentOf.keys()) {
    const seen = new Set<string>();
    let current: string | undefined = start;
    while (current && parentOf.has(current)) {
      if (seen.has(current)) {
        return false;
      }
      seen.add(current);
      current = parentOf.get(current);
    }
  }

  return true;
}

/**
 * Build a forest from entries that already passed
 * `supportsDeterministicParentRelation`. Roots are entries without a resolvable
 * parentKey. Orphans with broken parents are not invented here — callers must
 * gate on the support check first.
 */
export function buildHierarchyForest<T extends HierarchyHistogramEntry>(
  entries: T[],
): Array<HierarchyNode<T>> {
  const nodes = new Map<string, HierarchyNode<T>>();
  for (const entry of entries) {
    nodes.set(entry.key, { entry, children: [] });
  }

  const roots: Array<HierarchyNode<T>> = [];
  for (const entry of entries) {
    const node = nodes.get(entry.key);
    if (!node) {
      continue;
    }

    const parent = entry.parentKey?.trim();
    if (parent && nodes.has(parent) && parent !== entry.key) {
      nodes.get(parent)!.children.push(node);
      continue;
    }

    roots.push(node);
  }

  return roots;
}

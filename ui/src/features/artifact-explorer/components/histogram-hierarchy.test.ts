import { describe, expect, it } from "bun:test";

import {
  buildHierarchyForest,
  supportsDeterministicParentRelation,
} from "./histogram-hierarchy";

describe("histogram-hierarchy", () => {
  it("rejects flat superclass keys without parentKey", () => {
    expect(
      supportsDeterministicParentRelation("superclass", [
        { key: "java.lang.Object" },
        { key: "java.util.AbstractList" },
      ]),
    ).toBe(false);
  });

  it("rejects non-superclass group-by even with parentKey", () => {
    expect(
      supportsDeterministicParentRelation("package", [
        { key: "com.example.Child", parentKey: "com.example.Parent" },
        { key: "com.example.Parent" },
      ]),
    ).toBe(false);
  });

  it("accepts superclass entries when every parentKey resolves to a sibling key", () => {
    expect(
      supportsDeterministicParentRelation("superclass", [
        { key: "java.lang.Object" },
        { key: "java.util.AbstractList", parentKey: "java.lang.Object" },
        { key: "java.util.ArrayList", parentKey: "java.util.AbstractList" },
      ]),
    ).toBe(true);
  });

  it("rejects dangling parentKey that invents missing ancestry", () => {
    expect(
      supportsDeterministicParentRelation("superclass", [
        { key: "java.util.ArrayList", parentKey: "java.util.AbstractList" },
      ]),
    ).toBe(false);
  });

  it("builds a forest only from declared parent links", () => {
    const forest = buildHierarchyForest([
      { key: "java.lang.Object", retainedSize: 1 },
      { key: "java.util.AbstractList", parentKey: "java.lang.Object", retainedSize: 2 },
      { key: "java.util.ArrayList", parentKey: "java.util.AbstractList", retainedSize: 3 },
    ]);

    expect(forest).toHaveLength(1);
    expect(forest[0]?.entry.key).toBe("java.lang.Object");
    expect(forest[0]?.children).toHaveLength(1);
    expect(forest[0]?.children[0]?.entry.key).toBe("java.util.AbstractList");
    expect(forest[0]?.children[0]?.children[0]?.entry.key).toBe("java.util.ArrayList");
  });
});

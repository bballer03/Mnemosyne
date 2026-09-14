import { describe, expect, it } from "bun:test";

import { looksLikeOqlQuery, routeFreeTextToWorkflow } from "./natural-language-router";

describe("looksLikeOqlQuery", () => {
  it("recognizes a SELECT-prefixed query", () => {
    expect(looksLikeOqlQuery("SELECT * FROM objects")).toBe(true);
    expect(looksLikeOqlQuery("  select o.class_name from objects o")).toBe(true);
  });

  it("recognizes an OQL-shaped comparison expression", () => {
    expect(looksLikeOqlQuery("o.class_name =~ \"java.lang.*\"")).toBe(true);
    expect(looksLikeOqlQuery("retained_size >= 1024")).toBe(true);
  });

  it("does not treat plain English as OQL", () => {
    expect(looksLikeOqlQuery("why is my heap growing")).toBe(false);
    expect(looksLikeOqlQuery("tune the gc for me")).toBe(false);
  });

  it("treats empty input as not OQL", () => {
    expect(looksLikeOqlQuery("   ")).toBe(false);
  });
});

describe("routeFreeTextToWorkflow", () => {
  it("routes gc-flavored phrasing to tune_gc", () => {
    expect(routeFreeTextToWorkflow("can you tune the gc settings")).toBe("tune_gc");
  });

  it("routes graph/traversal phrasing to traverse_object_graph", () => {
    expect(routeFreeTextToWorkflow("walk the object graph from this root")).toBe("traverse_object_graph");
  });

  it("routes comparison phrasing to compare_snapshots", () => {
    expect(routeFreeTextToWorkflow("compare this heap to yesterday's")).toBe("compare_snapshots");
  });

  it("routes classloader phrasing to classloader_leak", () => {
    expect(routeFreeTextToWorkflow("find classloader leaks from redeploy")).toBe("classloader_leak");
    expect(routeFreeTextToWorkflow("show duplicate classes across loaders")).toBe("classloader_leak");
  });

  it("defaults to triage_memory_leak for unmatched free text", () => {
    expect(routeFreeTextToWorkflow("what's wrong with this heap?")).toBe("triage_memory_leak");
  });
});

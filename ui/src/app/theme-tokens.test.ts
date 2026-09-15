import "../test/setup";

import { describe, expect, it } from "bun:test";

import {
  COMPACT_LAYOUT_MAX_WIDTH,
  compactGridColumns,
  workbenchPanelStyle,
} from "./theme-tokens";

describe("theme-tokens", () => {
  it("exports workbench panel styles that reference CSS custom properties", () => {
    expect(workbenchPanelStyle.border).toContain("var(--mn-border-subtle)");
    expect(workbenchPanelStyle.background).toContain("var(--mn-surface-raised)");
    expect(COMPACT_LAYOUT_MAX_WIDTH).toBe(980);
  });

  it("stacks compact grids to a single column", () => {
    expect(compactGridColumns(true, "minmax(0, 1.4fr) minmax(280px, 0.8fr)")).toBe(
      "minmax(0, 1fr)",
    );
    expect(compactGridColumns(false, "1fr 1fr")).toBe("1fr 1fr");
  });
});

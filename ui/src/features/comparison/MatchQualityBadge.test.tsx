import "../../test/setup";

import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import type { MatchQuality } from "../../lib/diff-types";

import { MatchQualityBadge } from "./MatchQualityBadge";

const baseMatchQuality: MatchQuality = {
  strategy: "ClassDominator",
  collisionRate: 0.0234,
  estimatedFalseMatchRisk: "Low",
  estimatedFalseSplitRisk: "Medium",
  notes: ["class+dominator adds four hops of dominator context to reduce sibling collisions"],
};

describe("MatchQualityBadge", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders the strategy and the collision rate as a percentage matching the fixture's actual collision rate", () => {
    const view = render(<MatchQualityBadge matchQuality={baseMatchQuality} />);

    expect(view.getByText("ClassDominator")).toBeInTheDocument();
    expect(view.getByText(/2\.34%/)).toBeInTheDocument();
  });

  it("renders both risk pills with their reported levels", () => {
    const view = render(<MatchQualityBadge matchQuality={baseMatchQuality} />);

    // Scoped to `span` since the pill's own text also appears (via
    // `textContent`) on every ancestor element, which would otherwise
    // match this same query and trigger an ambiguous multi-match error.
    expect(view.getByText(/False-match risk: Low/, { selector: "span" })).toBeInTheDocument();
    expect(view.getByText(/False-split risk: Medium/, { selector: "span" })).toBeInTheDocument();
  });

  it("renders match-quality notes when present", () => {
    const view = render(<MatchQualityBadge matchQuality={baseMatchQuality} />);

    expect(
      view.getByText(/class\+dominator adds four hops of dominator context to reduce sibling collisions/, {
        selector: "li",
      }),
    ).toBeInTheDocument();
  });

  it("renders no notes list when the fixture has no notes", () => {
    const view = render(<MatchQualityBadge matchQuality={{ ...baseMatchQuality, notes: [] }} />);

    expect(view.container.querySelector("ul")).not.toBeInTheDocument();
  });

  it("reflects a zero collision rate for a perfectly clean match", () => {
    const view = render(<MatchQualityBadge matchQuality={{ ...baseMatchQuality, collisionRate: 0 }} />);

    expect(view.getByText(/0\.00%/)).toBeInTheDocument();
  });
});

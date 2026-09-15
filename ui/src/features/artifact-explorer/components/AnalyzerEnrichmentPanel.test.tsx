import "../../../test/setup";

import { cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";

import type {
  AnalyzerEnrichmentResult,
  AnalyzerSelection,
} from "../analyzer-enrichment-client";
import { AnalyzerEnrichmentPanel } from "./AnalyzerEnrichmentPanel";

describe("AnalyzerEnrichmentPanel", () => {
  afterEach(() => {
    cleanup();
  });

  it("keeps analyzers opt-in and confirms field-data cost before one request", async () => {
    const calls: AnalyzerSelection[] = [];
    const user = userEvent.setup();
    const view = render(
      <AnalyzerEnrichmentPanel
        runEnrichment={async (selection) => {
          calls.push(selection);
          return {
            status: "ready",
            requested: ["Strings", "Referrers"],
            unavailable: [],
            provenance: [],
          };
        }}
      />,
    );
    const page = within(view.container);

    const choices = page.getAllByRole("checkbox");
    expect(choices).toHaveLength(6);
    for (const choice of choices) {
      expect(choice).not.toBeChecked();
    }
    expect(page.getByRole("button", { name: /run enrichment/i })).toBeDisabled();

    await user.click(page.getByRole("checkbox", { name: /^strings/i }));
    await user.click(page.getByRole("checkbox", { name: /^referrers/i }));

    expect(page.getByText(/strings requires retained field data/i)).toBeInTheDocument();
    expect(page.getByText(/referrers uses the deep graph without retained field data/i)).toBeInTheDocument();
    expect(page.getByText(/reparse the current heap/i)).toBeInTheDocument();
    expect(page.getByText(/materially increase peak memory and duration/i)).toBeInTheDocument();

    await user.click(page.getByRole("button", { name: /review enrichment cost/i }));
    expect(calls).toHaveLength(0);
    await user.click(page.getByRole("button", { name: /confirm and run enrichment/i }));

    await waitFor(() => expect(calls).toHaveLength(1));
    expect(calls[0]).toEqual({
      strings: true,
      collections: false,
      duplicateArrays: false,
      threads: false,
      referrers: true,
      classloaders: false,
    });
  });

  it("labels unavailable requested sections and response provenance", async () => {
    const user = userEvent.setup();
    const view = render(
      <AnalyzerEnrichmentPanel
        runEnrichment={async () => ({
          status: "ready",
          requested: ["Referrers", "Classloaders"],
          unavailable: ["Referrers"],
          provenance: [
            { kind: "Partial", detail: "bounded result" },
            { kind: "Fallback", detail: "heuristic result" },
          ],
        })}
      />,
    );
    const page = within(view.container);

    await user.click(page.getByRole("checkbox", { name: /^referrers/i }));
    await user.click(page.getByRole("checkbox", { name: /^classloaders/i }));
    await user.click(page.getByRole("button", { name: /run enrichment/i }));

    expect(await page.findByText(/referrers: unavailable/i)).toBeInTheDocument();
    expect(page.getByText(/classloaders: available/i)).toBeInTheDocument();
    expect(page.getByText(/partial: bounded result/i)).toBeInTheDocument();
    expect(page.getByText(/fallback: heuristic result/i)).toBeInTheDocument();
  });

  for (const [result, expected] of [
    [{ status: "stale" }, /ignored because the workspace changed/i],
    [{ status: "unavailable", message: "No desktop bridge." }, /no desktop bridge/i],
    [{ status: "error", message: "Host analysis failed." }, /host analysis failed/i],
  ] as Array<[AnalyzerEnrichmentResult, RegExp]>) {
    it(`renders the ${result.status} outcome honestly`, async () => {
      const user = userEvent.setup();
      const view = render(
        <AnalyzerEnrichmentPanel runEnrichment={async () => result} />,
      );
      const page = within(view.container);

      await user.click(page.getByRole("checkbox", { name: /^referrers/i }));
      await user.click(page.getByRole("button", { name: /run enrichment/i }));

      expect(await page.findByText(expected)).toBeInTheDocument();
    });
  }
});

import "../../test/setup";

import { render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";

import { TriageSummaryCard } from "./TriageSummaryCard";

afterEach(() => {
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
});

describe("TriageSummaryCard", () => {
  it("renders the AI triage summary heading and a graceful unavailable state without a bridge", () => {
    const view = render(<TriageSummaryCard heapPath="fixture.hprof" />);
    const page = within(view.container);

    expect(page.getByText(/ai triage summary/i)).toBeInTheDocument();
    expect(page.getByText(/workflow bridge unavailable/i)).toBeInTheDocument();
  });

  it("starts triage_memory_leak against the loaded artifact's heap path", async () => {
    const user = userEvent.setup();

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async (kind, params) => {
        expect(kind).toBe("triage_memory_leak");
        expect(params).toEqual({ heapPath: "fixture.hprof", objectId: undefined });
        return {
          workflow_id: "wf-1",
          current_step: "investigate_suspect",
          step_result: { leaks: [{ id: "leak-1", class_name: "com.example.Cache" }] },
          next_expected_input: [],
        };
      },
    };

    const view = render(<TriageSummaryCard heapPath="fixture.hprof" />);
    const page = within(view.container);

    await user.click(page.getByRole("button", { name: /start triage memory leak/i }));

    await waitFor(() => {
      expect(page.getByText(/current step: investigate_suspect/i)).toBeInTheDocument();
    });
    expect(page.getByText(/com\.example\.cache/i)).toBeInTheDocument();
  });
});

import "../../test/setup";

import { render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";

import { NaturalLanguageInputBar } from "./NaturalLanguageInputBar";

afterEach(() => {
  delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
});

describe("NaturalLanguageInputBar", () => {
  it("routes OQL-shaped input to the existing heap-query execution path", async () => {
    const user = userEvent.setup();
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async (input) => {
        expect(input).toEqual({ heapPath: "fixture.hprof", query: "SELECT class_name FROM objects" });
        return { columns: ["class_name"], rows: [["com.example.Cache"]] };
      },
    };

    const view = render(<NaturalLanguageInputBar heapPath="fixture.hprof" />);
    const page = within(view.container);

    await user.type(page.getByLabelText(/natural language or oql input/i), "SELECT class_name FROM objects");
    await user.click(page.getByRole("button", { name: /ask/i }));

    await waitFor(() => {
      expect(page.getByText(/com\.example\.cache/i)).toBeInTheDocument();
    });
  });

  it("routes free text to start_workflow with a best-guess kind", async () => {
    const user = userEvent.setup();
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async (kind, params) => {
        expect(kind).toBe("tune_gc");
        expect(params).toEqual({ heapPath: "fixture.hprof" });
        return {
          workflow_id: "wf-5",
          current_step: "thread_local_review",
          step_result: { gc_algorithm: "G1" },
          next_expected_input: [],
        };
      },
    };

    const view = render(<NaturalLanguageInputBar heapPath="fixture.hprof" />);
    const page = within(view.container);

    await user.type(page.getByLabelText(/natural language or oql input/i), "please tune the gc");
    await user.click(page.getByRole("button", { name: /ask/i }));

    await waitFor(() => {
      expect(page.getByText(/started tune gc/i)).toBeInTheDocument();
    });
    expect(page.getByText(/g1/i)).toBeInTheDocument();
  });

  it("points comparison-shaped free text at the /compare card instead of starting a workflow", async () => {
    const user = userEvent.setup();
    const view = render(<NaturalLanguageInputBar heapPath="fixture.hprof" />);
    const page = within(view.container);

    await user.type(page.getByLabelText(/natural language or oql input/i), "compare this heap to yesterday");
    await user.click(page.getByRole("button", { name: /ask/i }));

    await waitFor(() => {
      expect(page.getByText(/compare snapshots.*card/i)).toBeInTheDocument();
    });
  });

  it("shows an explicit unavailable message when no artifact is loaded", async () => {
    const user = userEvent.setup();
    const view = render(<NaturalLanguageInputBar />);
    const page = within(view.container);

    await user.type(page.getByLabelText(/natural language or oql input/i), "what's wrong with this heap");
    await user.click(page.getByRole("button", { name: /ask/i }));

    await waitFor(() => {
      expect(page.getByText(/load an artifact before starting a workflow/i)).toBeInTheDocument();
    });
  });
});

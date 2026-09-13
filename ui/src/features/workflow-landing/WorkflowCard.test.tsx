import "../../test/setup";

import { render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";

import { WorkflowCard } from "./WorkflowCard";

afterEach(() => {
  delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
});

describe("WorkflowCard", () => {
  it("renders a graceful unavailable state when no bridge is installed", () => {
    const view = render(
      <WorkflowCard kind="tune_gc" title="Tune GC" description="Review GC settings." heapPath="fixture.hprof" />,
    );
    const page = within(view.container);

    expect(page.getByText(/workflow bridge unavailable/i)).toBeInTheDocument();
    expect(page.queryByRole("button", { name: /start tune gc/i })).not.toBeInTheDocument();
  });

  it("disables start until an artifact heap path is present", () => {
    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async () => ({ workflow_id: "wf-1", current_step: "complete", step_result: {}, next_expected_input: [] }),
    };

    const view = render(<WorkflowCard kind="tune_gc" title="Tune GC" description="Review GC settings." />);
    const page = within(view.container);

    expect(page.getByRole("button", { name: /start tune gc/i })).toBeDisabled();
    expect(page.getByText(/load an artifact first/i)).toBeInTheDocument();
  });

  it("drives a full start_workflow -> next_step round trip and renders each step's result", async () => {
    const user = userEvent.setup();
    let calls = 0;

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async (kind, params) => {
        calls += 1;
        expect(kind).toBe("tune_gc");
        expect(params).toEqual({ heapPath: "fixture.hprof", objectId: undefined });
        return {
          workflow_id: "wf-42",
          current_step: "thread_local_review",
          step_result: { gc_algorithm: "G1" },
          next_expected_input: [],
        };
      },
      nextStep: async (workflowId) => {
        calls += 1;
        expect(workflowId).toBe("wf-42");
        return {
          workflow_id: "wf-42",
          current_step: "complete",
          step_result: { recommendation: "increase heap" },
          next_expected_input: [],
        };
      },
    };

    const view = render(
      <WorkflowCard kind="tune_gc" title="Tune GC" description="Review GC settings." heapPath="fixture.hprof" />,
    );
    const page = within(view.container);

    await user.click(page.getByRole("button", { name: /start tune gc/i }));

    await waitFor(() => {
      expect(page.getByText(/current step: thread_local_review/i)).toBeInTheDocument();
    });
    expect(page.getByText(/g1/i)).toBeInTheDocument();

    await user.click(page.getByRole("button", { name: /continue/i }));

    await waitFor(() => {
      expect(page.getByText(/current step: complete/i)).toBeInTheDocument();
    });
    expect(page.getByText(/increase heap/i)).toBeInTheDocument();
    expect(page.getByText(/workflow complete/i)).toBeInTheDocument();
    expect(page.queryByRole("button", { name: /continue/i })).not.toBeInTheDocument();
    expect(calls).toBe(2);
  });

  it("renders a bridge error without crashing", async () => {
    const user = userEvent.setup();

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async () => {
        throw new Error("workflow start failed");
      },
    };

    const view = render(
      <WorkflowCard kind="tune_gc" title="Tune GC" description="Review GC settings." heapPath="fixture.hprof" />,
    );
    const page = within(view.container);

    await user.click(page.getByRole("button", { name: /start tune gc/i }));

    await waitFor(() => {
      expect(page.getByRole("alert")).toHaveTextContent(/workflow start failed/i);
    });
  });

  it("passes a typed object id for traverse_object_graph when the field is shown", async () => {
    const user = userEvent.setup();

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async (kind, params) => {
        expect(kind).toBe("traverse_object_graph");
        expect(params).toEqual({ heapPath: "fixture.hprof", objectId: "obj-7" });
        return { workflow_id: "wf-9", current_step: "complete", step_result: {}, next_expected_input: [] };
      },
    };

    const view = render(
      <WorkflowCard
        kind="traverse_object_graph"
        title="Traverse Object Graph"
        description="Walk the object graph from a starting object."
        heapPath="fixture.hprof"
        showObjectIdInput
      />,
    );
    const page = within(view.container);

    const input = page.getByLabelText(/starting object id/i);
    await user.type(input, "obj-7");
    await user.click(page.getByRole("button", { name: /start traverse object graph/i }));

    await waitFor(() => {
      expect(page.getByText(/workflow complete/i)).toBeInTheDocument();
    });
  });
});

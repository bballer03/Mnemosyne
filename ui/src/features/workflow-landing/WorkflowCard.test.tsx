import "../../test/setup";

import { render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

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

  it("resumes an in-progress workflow by id and links classloader steps", async () => {
    const user = userEvent.setup();

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      startWorkflow: async () => ({
        workflow_id: "wf-ignored",
        current_step: "select",
        step_result: {},
        next_expected_input: [],
      }),
      getWorkflow: async (workflowId) => {
        expect(workflowId).toBe("wf-resume");
        return {
          schema_version: 1,
          workflow_id: "wf-resume",
          kind: "CLASSLOADER_LEAK",
          created_at: "1",
          updated_at: "2",
          heap_path: "/tmp/fixture.hprof",
          current_step: "select",
          step_history: [
            {
              step_name: "detect",
              input: null,
              output_summary: { duplicate_class_names: ["com/example/Dup"] },
              timestamp: "1",
            },
          ],
          context: {},
        };
      },
      nextStep: async (workflowId, input) => {
        expect(workflowId).toBe("wf-resume");
        expect(input).toEqual({ class_name: "com/example/Dup" });
        return {
          workflow_id: "wf-resume",
          current_step: "inspect_retention",
          step_result: { loader_object_id: "0x1000" },
          next_expected_input: [],
        };
      },
      closeWorkflow: async () => ({ workflow_id: "wf-resume", closed: true }),
    };

    const view = render(
      <MemoryRouter>
        <WorkflowCard
          kind="classloader_leak"
          title="Classloader Leak"
          description="Find duplicate classes across loaders."
          heapPath="fixture.hprof"
        />
      </MemoryRouter>,
    );
    const page = within(view.container);

    await user.type(page.getByLabelText(/resume workflow id/i), "wf-resume");
    await user.click(page.getByRole("button", { name: /resume/i }));

    await waitFor(() => {
      expect(page.getByText(/current step: select/i)).toBeInTheDocument();
    });
    expect(page.getByText(/wf-resume/i)).toBeInTheDocument();
    expect(page.getByRole("link", { name: /open classloader explorer/i })).toHaveAttribute(
      "href",
      "/artifacts/explorer",
    );

    await user.click(page.getByRole("button", { name: /continue/i }));

    await waitFor(() => {
      expect(page.getByText(/current step: inspect_retention/i)).toBeInTheDocument();
    });
    expect(page.getByRole("link", { name: /open object inspector/i })).toBeInTheDocument();
    expect(page.getByRole("link", { name: /open gc paths/i })).toBeInTheDocument();
  });

  it("closes a resumed workflow and returns to idle", async () => {
    const user = userEvent.setup();
    let closed = false;

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      getWorkflow: async () => ({
        schema_version: 1,
        workflow_id: "wf-to-close",
        kind: "TUNE_GC",
        created_at: "1",
        updated_at: "2",
        heap_path: "fixture.hprof",
        current_step: "thread_local_review",
        step_history: [
          {
            step_name: "root_kind_breakdown",
            input: null,
            output_summary: { roots: 3 },
            timestamp: "1",
          },
        ],
        context: {},
      }),
      closeWorkflow: async (workflowId) => {
        expect(workflowId).toBe("wf-to-close");
        closed = true;
        return { workflow_id: "wf-to-close", closed: true };
      },
    };

    const view = render(
      <WorkflowCard kind="tune_gc" title="Tune GC" description="Review GC settings." heapPath="fixture.hprof" />,
    );
    const page = within(view.container);

    await user.type(page.getByLabelText(/resume workflow id/i), "wf-to-close");
    await user.click(page.getByRole("button", { name: /resume/i }));

    await waitFor(() => {
      expect(page.getByText(/current step: thread_local_review/i)).toBeInTheDocument();
    });

    await user.click(page.getByRole("button", { name: /close workflow/i }));

    await waitFor(() => {
      expect(page.getByRole("button", { name: /^resume$/i })).toBeInTheDocument();
    });
    expect(closed).toBe(true);
    expect(page.queryByText(/current step:/i)).not.toBeInTheDocument();
    expect(page.queryByRole("button", { name: /close workflow/i })).not.toBeInTheDocument();
  });

  it("shows complete state when resuming an already-complete workflow", async () => {
    const user = userEvent.setup();

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      getWorkflow: async () => ({
        schema_version: 1,
        workflow_id: "wf-done",
        kind: "TUNE_GC",
        created_at: "1",
        updated_at: "2",
        heap_path: "fixture.hprof",
        current_step: "complete",
        step_history: [
          {
            step_name: "top_retainers",
            input: null,
            output_summary: { done: true },
            timestamp: "1",
          },
        ],
        context: {},
      }),
      closeWorkflow: async () => ({ workflow_id: "wf-done", closed: true }),
    };

    const view = render(
      <WorkflowCard kind="tune_gc" title="Tune GC" description="Review GC settings." heapPath="fixture.hprof" />,
    );
    const page = within(view.container);

    await user.type(page.getByLabelText(/resume workflow id/i), "wf-done");
    await user.click(page.getByRole("button", { name: /resume/i }));

    await waitFor(() => {
      expect(page.getByText(/workflow complete/i)).toBeInTheDocument();
    });
    expect(page.queryByRole("button", { name: /continue/i })).not.toBeInTheDocument();
    expect(page.getByRole("button", { name: /close workflow/i })).toBeInTheDocument();
  });

  it("surfaces corrupt resume errors without crashing", async () => {
    const user = userEvent.setup();

    window.__MNEMOSYNE_WORKFLOW_BRIDGE__ = {
      getWorkflow: async () => {
        throw new Error("workflow_corrupt: failed to load workflow 'wf-bad'");
      },
    };

    const view = render(
      <WorkflowCard kind="tune_gc" title="Tune GC" description="Review GC settings." heapPath="fixture.hprof" />,
    );
    const page = within(view.container);

    await user.type(page.getByLabelText(/resume workflow id/i), "wf-bad");
    await user.click(page.getByRole("button", { name: /resume/i }));

    await waitFor(() => {
      expect(page.getByRole("alert")).toHaveTextContent(/workflow_corrupt/i);
    });
  });
});

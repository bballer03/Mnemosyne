import "../../../test/setup";

import userEvent from "@testing-library/user-event";
import { act, cleanup, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter, useLocation } from "react-router-dom";

import { useInvestigationStore } from "../../investigation/investigation-store";
import { QueryConsolePanel } from "./QueryConsolePanel";

function LocationProbe() {
  const location = useLocation();
  return <output aria-label="Current location">{`${location.pathname}${location.search}`}</output>;
}

function renderPanel() {
  return render(
    <MemoryRouter initialEntries={["/heap-explorer/query-console"]}>
      <QueryConsolePanel heapPath="fixture.hprof" />
      <LocationProbe />
    </MemoryRouter>,
  );
}

describe("QueryConsolePanel", () => {
  beforeEach(() => {
    useInvestigationStore.setState({
      workspaceId: "workspace-query",
      revision: 3,
      activeOperation: undefined,
      objectId: undefined,
      originPane: undefined,
    });
  });

  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  });

  it("renders the unavailable state on first render when no query bridge exists", () => {
    const view = renderPanel();
    const panel = within(view.container);

    expect(panel.getByText(/query execution is unavailable in this browser session/i)).toBeInTheDocument();
  });

  it("submits the current heap query text and renders returned rows", async () => {
    const user = userEvent.setup();
    const calls: Array<{ heapPath: string; query: string }> = [];

    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async (input) => {
        calls.push(input);

        return {
          columns: ["object_id"],
          rows: [["0x2a"]],
        };
      },
    };

    const view = renderPanel();
    const panel = within(view.container);

    await user.clear(panel.getByRole("textbox", { name: /heap query/i }));
    await user.type(panel.getByRole("textbox", { name: /heap query/i }), "SELECT class_name LIMIT 5");
    await user.click(panel.getByRole("button", { name: /run query/i }));

    expect(calls).toEqual([{ heapPath: "fixture.hprof", query: "SELECT class_name LIMIT 5" }]);
    expect(await panel.findByText(/0x2a/i)).toBeInTheDocument();
  });

  it("renders bridge errors after a failed query run", async () => {
    const user = userEvent.setup();

    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => {
        throw new Error("Query failed.");
      },
    };

    const view = renderPanel();
    const panel = within(view.container);

    await user.click(panel.getByRole("button", { name: /run query/i }));

    expect(await panel.findByText(/query failed\./i)).toBeInTheDocument();
  });

  it("prevents overlapping query submissions while a run is in flight", async () => {
    const user = userEvent.setup();
    let resolveQuery: ((value: { columns: string[]; rows: string[][] }) => void) | undefined;
    const calls: Array<{ heapPath: string; query: string }> = [];

    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: (input) => {
        calls.push(input);

        return new Promise((resolve) => {
          resolveQuery = resolve;
        });
      },
    };

    const view = renderPanel();
    const panel = within(view.container);
    const runButton = panel.getByRole("button", { name: /run query/i });

    await user.click(runButton);
    await user.click(runButton);

    expect(calls).toEqual([
      {
        heapPath: "fixture.hprof",
        query: 'SELECT @objectId, @className FROM "*" LIMIT 20',
      },
    ]);

    resolveQuery?.({
      columns: ["object_id"],
      rows: [["0x2a"]],
    });

    expect(await panel.findByText(/0x2a/i)).toBeInTheDocument();
  });

  it("keeps the ten newest unique queries and restores history into the editor", async () => {
    const user = userEvent.setup();
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => ({ columns: [], rows: [] }),
    };
    const view = renderPanel();
    const panel = within(view.container);
    const editor = panel.getByRole("textbox", { name: /heap query/i });

    for (let index = 0; index < 11; index += 1) {
      await user.clear(editor);
      await user.type(editor, `SELECT * FROM "Class${index}"`);
      await user.click(panel.getByRole("button", { name: /run query/i }));
    }

    const history = within(panel.getByLabelText(/query history/i));
    expect(history.getAllByRole("button")).toHaveLength(10);
    expect(history.getAllByRole("button")[0]).toHaveTextContent('SELECT * FROM "Class10"');
    expect(history.queryByRole("button", { name: 'SELECT * FROM "Class0"' })).not.toBeInTheDocument();

    await user.clear(editor);
    await user.type(editor, 'SELECT * FROM "Class5"');
    await user.click(panel.getByRole("button", { name: /run query/i }));

    expect(history.getAllByRole("button")).toHaveLength(10);
    expect(history.getAllByRole("button")[0]).toHaveTextContent('SELECT * FROM "Class5"');
    await user.click(history.getByRole("button", { name: 'SELECT * FROM "Class8"' }));
    expect(editor).toHaveValue('SELECT * FROM "Class8"');
  });

  it("offers vetted examples plus supported syntax and named deferrals", async () => {
    const user = userEvent.setup();
    const view = renderPanel();
    const panel = within(view.container);

    await user.click(
      within(panel.getByLabelText(/vetted query examples/i)).getByRole("button", {
        name: /largest retained objects/i,
      }),
    );

    expect(panel.getByRole("textbox", { name: /heap query/i })).toHaveValue(
      'SELECT @objectId, @className, @retainedSize FROM "*" WHERE @retainedSize > 1048576 LIMIT 50',
    );
    expect(panel.getByLabelText(/supported oql syntax/i)).toHaveTextContent("UNION");
    expect(panel.getByLabelText(/supported oql syntax/i)).toHaveTextContent(
      "multi-class FROM (up to 8 patterns)",
    );
    const deferrals = panel.getByLabelText(/named oql deferrals/i);
    expect(deferrals).toHaveTextContent("eval(...)");
    expect(deferrals).toHaveTextContent("arbitrary-depth subquery nesting");
    expect(deferrals).toHaveTextContent("more than 8 class patterns in FROM");
    expect(deferrals).toHaveTextContent("OBJECTS traversal beyond 3 field hops");
  });

  it("renders parser coordinates separately from the error message", async () => {
    const user = userEvent.setup();
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => {
        throw new Error("expected FROM at byte 7");
      },
    };
    const view = renderPanel();
    const panel = within(view.container);

    await user.click(panel.getByRole("button", { name: /run query/i }));

    const alert = await panel.findByRole("alert");
    expect(alert).toHaveTextContent("expected FROM at byte 7");
    expect(alert).toHaveTextContent("Line 1, column 8");
  });

  it("opens object-id result cells in Inspector through the investigation store", async () => {
    const user = userEvent.setup();
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => ({
        columns: ["object_id", "class_name"],
        rows: [["0x2a", "com.example.Cache"]],
      }),
    };
    const view = renderPanel();
    const panel = within(view.container);

    await user.click(panel.getByRole("button", { name: /run query/i }));
    await user.click(await panel.findByRole("button", { name: /inspect object 0x2a/i }));

    expect(useInvestigationStore.getState()).toMatchObject({
      objectId: "0x2a",
      originPane: "inspector",
    });
    expect(panel.getByLabelText(/current location/i)).toHaveTextContent(
      "/heap-explorer/object-inspector?objectId=0x2a",
    );
    expect(panel.getByRole("cell", { name: "com.example.Cache" }).querySelector("button")).toBeNull();
  });

  it("does not commit a ready response after the workspace revision changes", async () => {
    const user = userEvent.setup();
    let callCount = 0;
    let resolveStale!: (value: { columns: string[]; rows: string[][] }) => void;
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => {
        callCount += 1;
        if (callCount === 1) {
          return { columns: ["object_id"], rows: [["0x1"]] };
        }
        return new Promise((resolve) => {
          resolveStale = resolve;
        });
      },
    };
    const view = renderPanel();
    const panel = within(view.container);

    await user.click(panel.getByRole("button", { name: /run query/i }));
    expect(await panel.findByText("0x1")).toBeInTheDocument();
    await user.click(panel.getByRole("button", { name: /run query/i }));
    act(() => {
      useInvestigationStore.setState({ revision: 4 });
      resolveStale({ columns: ["object_id"], rows: [["0x2"]] });
    });

    expect(await panel.findByText("0x1")).toBeInTheDocument();
    expect(panel.queryByText("0x2")).not.toBeInTheDocument();
  });

  it("does not commit an error response after the workspace revision changes", async () => {
    const user = userEvent.setup();
    let rejectStale!: (reason: Error) => void;
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () =>
        new Promise((_resolve, reject) => {
          rejectStale = reject;
        }),
    };
    const view = renderPanel();
    const panel = within(view.container);

    await user.click(panel.getByRole("button", { name: /run query/i }));
    await act(async () => {
      useInvestigationStore.setState({ revision: 4 });
      rejectStale(new Error("stale query failure at byte 3"));
      await Promise.resolve();
    });

    expect(panel.queryByRole("alert")).not.toBeInTheDocument();
  });
});

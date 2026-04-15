import "../../../test/setup";

import userEvent from "@testing-library/user-event";
import { cleanup, render, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "bun:test";

import { QueryConsolePanel } from "./QueryConsolePanel";

describe("QueryConsolePanel", () => {
  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  });

  it("renders the unavailable state on first render when no query bridge exists", () => {
    const view = render(<QueryConsolePanel heapPath="fixture.hprof" />);
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

    const view = render(<QueryConsolePanel heapPath="fixture.hprof" />);
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

    const view = render(<QueryConsolePanel heapPath="fixture.hprof" />);
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

    const view = render(<QueryConsolePanel heapPath="fixture.hprof" />);
    const panel = within(view.container);
    const runButton = panel.getByRole("button", { name: /run query/i });

    await user.click(runButton);
    await user.click(runButton);

    expect(calls).toEqual([{ heapPath: "fixture.hprof", query: "SELECT object_id, class_name LIMIT 20" }]);

    resolveQuery?.({
      columns: ["object_id"],
      rows: [["0x2a"]],
    });

    expect(await panel.findByText(/0x2a/i)).toBeInTheDocument();
  });
});

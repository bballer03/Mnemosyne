import "../../../test/setup";

import userEvent from "@testing-library/user-event";
import { cleanup, render, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import type { AnalysisArtifact } from "../../../lib/analysis-types";

import { ObjectInspectorPanel } from "./ObjectInspectorPanel";

const artifact: AnalysisArtifact = {
  summary: {
    heapPath: "fixture.hprof",
    totalObjects: 42,
    totalSizeBytes: 2048,
    generatedAt: "2026-04-14T00:00:00Z",
    totalRecords: 2,
  },
  leaks: [],
  recommendations: [],
  elapsedSeconds: 1,
  graph: {
    nodeCount: 200,
    edgeCount: 400,
    dominatorCount: 2,
    dominators: [
      {
        name: "LruCache#root",
        className: "com.example.cache.LruCache",
        objectId: "0xdeadbeef",
        dominates: 12,
        immediateDominator: "GC Root <system class>",
        retainedSize: 1024,
        shallowSize: 64,
      },
      {
        name: "WorkerQueue#17",
        className: "com.example.jobs.WorkerQueue",
        objectId: "0xcafebabe",
        dominates: 5,
        immediateDominator: "com.example.cache.LruCache@0xdeadbeef",
        retainedSize: 768,
        shallowSize: 48,
      },
    ],
  },
  histogram: {
    groupBy: "class",
    totalInstances: 42,
    totalShallowSize: 2048,
    entries: [],
  },
  provenance: [],
};

type HeapExplorerGlobal = typeof globalThis & {
  __MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?: Window["__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__"];
};

function getHeapExplorerGlobal() {
  return globalThis as HeapExplorerGlobal;
}

function clearHeapExplorerBridge() {
  delete getHeapExplorerGlobal().__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  delete globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
}

function setHeapExplorerBridge(bridge: NonNullable<Window["__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__"]>) {
  getHeapExplorerGlobal().__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = bridge;
  globalThis.window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = bridge;
}

function renderPanel(selectedRowIndex?: number, objectId?: string) {
  return render(
    <MemoryRouter initialEntries={["/heap-explorer/object-inspector"]}>
      <ObjectInspectorPanel artifact={artifact} selectedRowIndex={selectedRowIndex} objectId={objectId} />
    </MemoryRouter>,
  );
}

describe("ObjectInspectorPanel", () => {
  beforeEach(() => {
    clearHeapExplorerBridge();
  });

  afterEach(() => {
    cleanup();
    clearHeapExplorerBridge();
  });

  it("renders selected dominator row details from the artifact", () => {
    const view = renderPanel(1);
    const panel = within(view.container);

    expect(panel.getByRole("heading", { name: /object inspector/i })).toBeInTheDocument();
    expect(panel.getByText(/com\.example\.jobs\.workerqueue/i)).toBeInTheDocument();
    expect(panel.getByText(/0xcafebabe/i)).toBeInTheDocument();
    expect(panel.getByText(/48 b/i)).toBeInTheDocument();
    expect(panel.getByText(/768 b/i)).toBeInTheDocument();
    expect(panel.getByText(/5 objects/i)).toBeInTheDocument();
    expect(panel.getByText(/com\.example\.cache\.lrucache@0xdeadbeef/i)).toBeInTheDocument();
    expect(panel.getByText(/live references and referrers require a host bridge connection\./i)).toBeInTheDocument();
  });

  it("renders an honest unselected state when no row is selected", () => {
    const view = renderPanel();
    const panel = within(view.container);

    expect(panel.getByRole("heading", { name: /object inspector/i })).toBeInTheDocument();
    expect(panel.getByText(/select a dominator row to inspect its artifact-backed details/i)).toBeInTheDocument();
    expect(panel.getByText(/live references and referrers require a host bridge connection\./i)).toBeInTheDocument();
    expect(panel.queryByText(/0xdeadbeef/i)).toBeNull();
  });

  it("shows the host bridge disclaimer when no bridge is connected", () => {
    const view = renderPanel(0);

    expect(view.getByText(/live references and referrers require a host bridge connection\./i)).toBeInTheDocument();
  });

  it("shows per-section unavailable messages when the bridge lacks reference methods", () => {
    setHeapExplorerBridge({
      queryHeap: async () => ({
        columns: [],
        rows: [],
      }),
    });

    const view = renderPanel(0);

    expect(view.getByText(/live references are not available.+no host bridge connected\./i)).toBeInTheDocument();
    expect(view.getByText(/live referrers are not available.+no host bridge connected\./i)).toBeInTheDocument();
  });

  it("shows the references and referrers section headings", () => {
    const view = renderPanel(0);

    expect(view.getByRole("heading", { name: /references \(outgoing\)/i })).toBeInTheDocument();
    expect(view.getByRole("heading", { name: /referrers \(incoming\)/i })).toBeInTheDocument();
  });

  it("renders reference entries as navigable links when the bridge returns data", async () => {
    setHeapExplorerBridge({
      getReferences: async () => ({
        objectId: "0xcafebabe",
        references: [
          {
            objectId: "worker queue/0x1",
            className: "java.lang.String",
            shallowSize: 32,
            displayName: "MyField",
          },
        ],
      }),
      getReferrers: async () => ({
        objectId: "0xcafebabe",
        referrers: [
          {
            objectId: "owner/0x2",
            className: "com.example.CacheOwner",
            shallowSize: 128,
          },
        ],
      }),
    });

    const view = renderPanel(1);

    const outgoingLink = await view.findByRole("link", { name: /java\.lang\.string/i });
    const incomingLink = await view.findByRole("link", { name: /com\.example\.cacheowner/i });

    expect(await view.findByText(/myfield/i)).toBeInTheDocument();
    expect(await view.findByText(/worker queue\/0x1/i)).toBeInTheDocument();
    expect(await view.findByText(/owner\/0x2/i)).toBeInTheDocument();
    expect(outgoingLink).toHaveAttribute(
      "href",
      "/heap-explorer/object-inspector?objectId=worker%20queue%2F0x1",
    );
    expect(incomingLink).toHaveAttribute("href", "/heap-explorer/object-inspector?objectId=owner%2F0x2");
  });

  it("shows empty messages when the bridge returns empty relations", async () => {
    setHeapExplorerBridge({
      getReferences: async () => ({
        objectId: "0xdeadbeef",
        references: [],
      }),
      getReferrers: async () => ({
        objectId: "0xdeadbeef",
        referrers: [],
      }),
    });

    const view = renderPanel(0);

    await waitFor(() => {
      expect(view.getByText(/no outgoing references\./i)).toBeInTheDocument();
      expect(view.getByText(/no incoming referrers\./i)).toBeInTheDocument();
    });
  });

  it("shows an error message when the bridge rejects the references lookup", async () => {
    setHeapExplorerBridge({
      getReferences: async () => {
        throw new Error("bridge down");
      },
      getReferrers: async () => ({
        objectId: "0xcafebabe",
        referrers: [],
      }),
    });

    const view = renderPanel(1);
    const errorMessage = await view.findByText(/bridge down/i);

    expect(errorMessage.tagName).toBe("P");
    expect(errorMessage).toHaveStyle("color: #fda4af");
  });

  it("renders today's view unchanged when the bridge lacks inspectObject (regression gate)", () => {
    setHeapExplorerBridge({
      getReferences: async () => ({ objectId: "0xcafebabe", references: [] }),
      getReferrers: async () => ({ objectId: "0xcafebabe", referrers: [] }),
    });

    const view = renderPanel(1);
    const panel = within(view.container);

    expect(panel.getByText(/com\.example\.cache\.lrucache@0xdeadbeef/i)).toBeInTheDocument();
    expect(panel.queryByRole("heading", { name: /dominator context/i })).toBeNull();
    expect(panel.queryByText(/no dominator parent/i)).toBeNull();
    expect(panel.queryByText(/no dominator children/i)).toBeNull();
  });

  it("renders dominator parent and children as navigable chips when inspectObject is available", async () => {
    setHeapExplorerBridge({
      inspectObject: async () => ({
        object_id: "0xcafebabe",
        class_name: "com.example.jobs.WorkerQueue",
        shallow_size: 48,
        retained_size: 768,
        references_out: [],
        referrers_in: [],
        dominator_parent: { object_id: "0xdeadbeef", class_name: "com.example.cache.LruCache" },
        dominator_children: [{ object_id: "0xfeedface", class_name: "com.example.jobs.Job" }],
      }),
    });

    const view = renderPanel(1);

    const parentLink = await view.findByRole("link", { name: /com\.example\.cache\.lrucache/i });
    const childLink = await view.findByRole("link", { name: /com\.example\.jobs\.job/i });

    expect(parentLink).toHaveAttribute("href", "/heap-explorer/object-inspector?objectId=0xdeadbeef");
    expect(childLink).toHaveAttribute("href", "/heap-explorer/object-inspector?objectId=0xfeedface");
  });

  it("renders honest empty dominator-context messages when inspectObject returns none", async () => {
    setHeapExplorerBridge({
      inspectObject: async () => ({
        object_id: "0xcafebabe",
        class_name: "com.example.jobs.WorkerQueue",
        shallow_size: 48,
        retained_size: 768,
        references_out: [],
        referrers_in: [],
        dominator_parent: null,
        dominator_children: [],
      }),
    });

    const view = renderPanel(1);

    expect(await view.findByText(/no dominator parent/i)).toBeInTheDocument();
    expect(view.getByText(/no dominator children/i)).toBeInTheDocument();
  });

  it("inspects an object id absent from the artifact without requesting field data automatically", async () => {
    const inspectionCalls: Array<[string, boolean | undefined]> = [];
    setHeapExplorerBridge({
      inspectObject: async (objectId, retainFieldData) => {
        inspectionCalls.push([objectId, retainFieldData]);
        return {
          object_id: objectId,
          class_name: "com.example.DetachedObject",
          shallow_size: 24,
          retained_size: 96,
          references_out: [],
          referrers_in: [],
          dominator_parent: null,
          dominator_children: [],
        };
      },
    });

    const view = renderPanel(undefined, "0xfeedface");

    await waitFor(() => {
      expect(inspectionCalls).toEqual([["0xfeedface", false]]);
    });
    expect(view.getByText("com.example.DetachedObject")).toBeInTheDocument();
    expect(view.getByText("0xfeedface")).toBeInTheDocument();
    expect(
      view.getByText(/may reparse the heap and retain field bytes; memory use can increase\./i),
    ).toBeInTheDocument();
    expect(view.getByRole("button", { name: /request field data/i })).toBeInTheDocument();
  });

  it("requests fields only after the disclosed opt-in and renders primitive and object-reference values", async () => {
    const user = userEvent.setup();
    const inspectionCalls: Array<[string, boolean | undefined]> = [];
    setHeapExplorerBridge({
      inspectObject: async (objectId, retainFieldData) => {
        inspectionCalls.push([objectId, retainFieldData]);
        return {
          object_id: objectId,
          class_name: "com.example.jobs.WorkerQueue",
          shallow_size: 48,
          retained_size: 768,
          fields: retainFieldData
            ? [
                { name: "size", type_name: "int", value: "42" },
                { name: "owner", type_name: "object reference", value: "0xdeadbeef" },
              ]
            : undefined,
          references_out: [],
          referrers_in: [],
          dominator_parent: null,
          dominator_children: [],
        };
      },
    });

    const view = renderPanel(1, "0xcafebabe");

    await waitFor(() => {
      expect(inspectionCalls).toEqual([["0xcafebabe", false]]);
    });
    expect(
      view.getByText(/may reparse the heap and retain field bytes; memory use can increase\./i),
    ).toBeInTheDocument();

    await user.click(view.getByRole("button", { name: /request field data/i }));

    await waitFor(() => {
      expect(inspectionCalls).toEqual([
        ["0xcafebabe", false],
        ["0xcafebabe", true],
      ]);
    });
    expect(view.getByText("size")).toBeInTheDocument();
    expect(view.getByText("int")).toBeInTheDocument();
    expect(view.getByText("42")).toBeInTheDocument();
    expect(view.getByText("owner")).toBeInTheDocument();
    expect(view.getByText("object reference")).toBeInTheDocument();
    expect(view.getByText("0xdeadbeef")).toBeInTheDocument();
  });

  it("distinguishes unavailable field bytes from an object with no decoded fields", async () => {
    const user = userEvent.setup();
    let fieldResponse: undefined | [] = undefined;
    setHeapExplorerBridge({
      inspectObject: async (objectId, retainFieldData) => ({
        object_id: objectId,
        class_name: "com.example.jobs.WorkerQueue",
        shallow_size: 48,
        retained_size: 768,
        fields: retainFieldData ? fieldResponse : undefined,
        references_out: [],
        referrers_in: [],
        dominator_parent: null,
        dominator_children: [],
      }),
    });

    const unavailableView = renderPanel(1, "0xcafebabe");
    await user.click(await unavailableView.findByRole("button", { name: /request field data/i }));
    expect(await unavailableView.findByText(/field bytes were unavailable/i)).toBeInTheDocument();

    cleanup();
    fieldResponse = [];
    const emptyView = renderPanel(1, "0xcafebabe");
    await user.click(await emptyView.findByRole("button", { name: /request field data/i }));
    expect(await emptyView.findByText(/no decoded fields/i)).toBeInTheDocument();
  });

  it("shows an error message when the bridge rejects the inspectObject lookup", async () => {
    setHeapExplorerBridge({
      inspectObject: async () => {
        throw new Error("inspection bridge down");
      },
    });

    const view = renderPanel(1);

    expect(await view.findByText(/inspection bridge down/i)).toBeInTheDocument();
  });
});

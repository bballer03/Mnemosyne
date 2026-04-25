import "../../../test/setup";

import { render, waitFor, within } from "@testing-library/react";
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

const globalWindow = globalThis as typeof globalThis & {
  __MNEMOSYNE_HEAP_EXPLORER_BRIDGE__?: Window["__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__"];
};

function clearHeapExplorerBridge() {
  delete globalWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
}

function renderPanel(selectedRowIndex?: number) {
  return render(
    <MemoryRouter initialEntries={["/heap-explorer/object-inspector"]}>
      <ObjectInspectorPanel artifact={artifact} selectedRowIndex={selectedRowIndex} />
    </MemoryRouter>,
  );
}

describe("ObjectInspectorPanel", () => {
  beforeEach(() => {
    clearHeapExplorerBridge();
  });

  afterEach(() => {
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
    globalWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      queryHeap: async () => ({
        columns: [],
        rows: [],
      }),
    };

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
    globalWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
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
    };

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
    globalWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      getReferences: async () => ({
        objectId: "0xdeadbeef",
        references: [],
      }),
      getReferrers: async () => ({
        objectId: "0xdeadbeef",
        referrers: [],
      }),
    };

    const view = renderPanel(0);

    await waitFor(() => {
      expect(view.getByText(/no outgoing references\./i)).toBeInTheDocument();
      expect(view.getByText(/no incoming referrers\./i)).toBeInTheDocument();
    });
  });

  it("shows an error message when the bridge rejects the references lookup", async () => {
    globalWindow.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      getReferences: async () => {
        throw new Error("bridge down");
      },
      getReferrers: async () => ({
        objectId: "0xcafebabe",
        referrers: [],
      }),
    };

    const view = renderPanel(1);
    const errorMessage = await view.findByText(/bridge down/i);

    expect(errorMessage.tagName).toBe("P");
    expect(errorMessage).toHaveStyle("color: #fda4af");
  });
});

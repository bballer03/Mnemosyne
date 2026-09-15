import "../../../test/setup";

import { act, cleanup, render, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter, useLocation } from "react-router-dom";

import { useInvestigationStore } from "../../investigation/investigation-store";

import { ClassInstancesPanel } from "./ClassInstancesPanel";

function LocationProbe() {
  const location = useLocation();
  return <output data-testid="location">{`${location.pathname}${location.search}`}</output>;
}

function renderPanel(options?: {
  groupBy?: "class" | "package" | "class_loader" | "superclass";
  onRegroupToClass?: () => void;
}) {
  return render(
    <MemoryRouter initialEntries={["/artifacts/explorer"]}>
      <ClassInstancesPanel
        groupBy={options?.groupBy ?? "class"}
        onRegroupToClass={options?.onRegroupToClass}
      />
      <LocationProbe />
    </MemoryRouter>,
  );
}

describe("ClassInstancesPanel", () => {
  beforeEach(() => {
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
    act(() => {
      useInvestigationStore.setState({
        classKey: undefined,
        objectId: undefined,
        originPane: undefined,
      });
    });
  });

  afterEach(() => {
    cleanup();
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  });

  it("does not request instances until a class is selected", async () => {
    const requests: Array<[string, number | undefined, number | undefined]> = [];
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      listClassInstances: async (classKey, offset, limit) => {
        requests.push([classKey, offset, limit]);
        return {
          class_key: classKey,
          total: 0,
          returned: 0,
          offset: offset ?? 0,
          limit: limit ?? 100,
          truncated: false,
          instances: [],
        };
      },
    };

    const view = renderPanel();

    expect(view.getByText(/select a class bucket to list its instances/i)).toBeInTheDocument();
    await waitFor(() => expect(requests).toHaveLength(0));
  });

  it("shows an explicit unavailable state without the class-instance bridge", () => {
    act(() => {
      useInvestigationStore.setState({ classKey: "com.example.Cache" });
    });

    const view = renderPanel();

    expect(view.getByText(/class instances are unavailable in this host/i)).toBeInTheDocument();
  });

  it("renders only the bounded page and reports total truncation honestly", async () => {
    const requests: Array<[string, number | undefined, number | undefined]> = [];
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      listClassInstances: async (classKey, offset, limit) => {
        requests.push([classKey, offset, limit]);
        const pageOffset = offset ?? 0;
        return {
          class_key: classKey,
          total: 250,
          returned: 2,
          offset: pageOffset,
          limit: limit ?? 100,
          truncated: pageOffset < 200,
          instances: [
            {
              object_id: pageOffset === 0 ? "0x10" : "0x30",
              class_name: classKey,
              shallow_size: 64,
              retained_size: 2048,
            },
            {
              object_id: pageOffset === 0 ? "0x20" : "0x40",
              class_name: classKey,
              shallow_size: 32,
              retained_size: 1024,
            },
          ],
        };
      },
    };
    act(() => {
      useInvestigationStore.setState({ classKey: "com.example.Cache" });
    });
    const user = userEvent.setup();
    const view = renderPanel();

    expect(await view.findByText("Showing 1–2 of 250")).toBeInTheDocument();
    expect(view.getAllByRole("button", { name: /open object/i })).toHaveLength(2);
    expect(view.getByText(/more instances remain on later bounded pages/i)).toBeInTheDocument();
    expect(requests).toEqual([["com.example.Cache", 0, 100]]);

    await user.click(view.getByRole("button", { name: /next instance page/i }));

    expect(await view.findByText("Showing 101–102 of 250")).toBeInTheDocument();
    expect(requests).toEqual([
      ["com.example.Cache", 0, 100],
      ["com.example.Cache", 100, 100],
    ]);
  });

  it("stores the selected object and navigates to its encoded Inspector URL", async () => {
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      listClassInstances: async (classKey, offset, limit) => ({
        class_key: classKey,
        total: 1,
        returned: 1,
        offset: offset ?? 0,
        limit: limit ?? 100,
        truncated: false,
        instances: [
          {
            object_id: "0x00af",
            class_name: classKey,
            shallow_size: 64,
            retained_size: 2048,
          },
        ],
      }),
    };
    act(() => {
      useInvestigationStore.setState({ classKey: "com.example.Cache" });
    });
    const user = userEvent.setup();
    const view = renderPanel();

    await user.click(await view.findByRole("button", { name: "Open object 0x00af" }));

    expect(useInvestigationStore.getState().objectId).toBe("0x00af");
    expect(useInvestigationStore.getState().originPane).toBe("histogram");
    expect(view.getByTestId("location")).toHaveTextContent(
      "/heap-explorer/object-inspector?objectId=0x00af",
    );
  });

  it("does not request aggregate buckets and offers regrouping to Class", async () => {
    let requestCount = 0;
    let regroupCount = 0;
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      listClassInstances: async () => {
        requestCount += 1;
        return {};
      },
    };
    act(() => {
      useInvestigationStore.setState({ classKey: "com.example" });
    });
    const user = userEvent.setup();
    const view = renderPanel({
      groupBy: "package",
      onRegroupToClass: () => {
        regroupCount += 1;
      },
    });

    expect(view.getByText(/instances are available only for class grouping/i)).toBeInTheDocument();
    await user.click(view.getByRole("button", { name: /regroup histogram to class/i }));

    expect(requestCount).toBe(0);
    expect(regroupCount).toBe(1);
  });
});

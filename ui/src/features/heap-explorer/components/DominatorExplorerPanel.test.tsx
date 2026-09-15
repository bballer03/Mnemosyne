import "../../../test/setup";

import userEvent from "@testing-library/user-event";
import { render, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";

import { DominatorExplorerPanel } from "./DominatorExplorerPanel";

const rows = [
  {
    name: "LruCache#root",
    className: "com.example.cache.LruCache",
    objectId: "0xdeadbeef",
    dominates: 12,
    retainedSize: 1024,
    shallowSize: 64,
  },
  {
    name: "WorkerQueue#17",
    className: "com.example.jobs.WorkerQueue",
    objectId: "0xcafebabe",
    dominates: 5,
    retainedSize: 768,
    shallowSize: 48,
  },
];

describe("DominatorExplorerPanel", () => {
  beforeEach(() => {
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  });

  afterEach(() => {
    delete window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__;
  });

  it("renders the dominator heading, comparison label, and row content", () => {
    const view = render(
      <DominatorExplorerPanel rows={rows} selectedRowIndex={0} onSelectRowIndex={() => {}} />,
    );
    const panel = within(view.container);

    expect(panel.getByRole("heading", { name: /dominator explorer/i })).toBeInTheDocument();
    expect(panel.getByText(/retained vs dominates/i)).toBeInTheDocument();
    expect(panel.getByRole("button", { name: /select com\.example\.cache\.lrucache 0xdeadbeef/i })).toBeInTheDocument();
    expect(panel.getByRole("button", { name: /select com\.example\.jobs\.workerqueue 0xcafebabe/i })).toBeInTheDocument();
    expect(panel.getByText(/lrucache#root/i)).toBeInTheDocument();
    expect(panel.getByText(/workerqueue#17/i)).toBeInTheDocument();
  });

  it("filters rows by class name, object id, or display name", async () => {
    const user = userEvent.setup();
    const view = render(
      <DominatorExplorerPanel rows={rows} selectedRowIndex={0} onSelectRowIndex={() => {}} />,
    );
    const panel = within(view.container);

    await user.type(panel.getByRole("textbox", { name: /search dominators/i }), "cafe");

    expect(panel.queryByRole("button", { name: /select com\.example\.cache\.lrucache 0xdeadbeef/i })).toBeNull();
    expect(panel.getByRole("button", { name: /select com\.example\.jobs\.workerqueue 0xcafebabe/i })).toBeInTheDocument();
  });

  it("marks only the selected row as pressed and updates selection on click", async () => {
    const user = userEvent.setup();
    let selectedRowIndex = 0;

    const view = render(
      <DominatorExplorerPanel
        rows={rows}
        selectedRowIndex={selectedRowIndex}
        onSelectRowIndex={(nextRowIndex) => {
          selectedRowIndex = nextRowIndex;
          view.rerender(
            <DominatorExplorerPanel rows={rows} selectedRowIndex={selectedRowIndex} onSelectRowIndex={() => {}} />,
          );
        }}
      />,
    );
    const panel = within(view.container);

    const firstRow = panel.getByRole("button", { name: /select com\.example\.cache\.lrucache 0xdeadbeef/i });
    const secondRow = panel.getByRole("button", { name: /select com\.example\.jobs\.workerqueue 0xcafebabe/i });

    expect(firstRow.getAttribute("aria-pressed")).toBe("true");
    expect(secondRow.getAttribute("aria-pressed")).toBe("false");

    await user.click(secondRow);

    expect(panel.getByRole("button", { name: /select com\.example\.cache\.lrucache 0xdeadbeef/i }).getAttribute("aria-pressed")).toBe(
      "false",
    );
    expect(panel.getByRole("button", { name: /select com\.example\.jobs\.workerqueue 0xcafebabe/i }).getAttribute("aria-pressed")).toBe(
      "true",
    );
  });

  it("scales retained and dominates bars from the filtered maxima even when input order is unsorted", () => {
    const unsortedRows = [
      {
        name: "SmallerRow",
        className: "com.example.Small",
        objectId: "0x1",
        dominates: 2,
        retainedSize: 128,
        shallowSize: 16,
      },
      {
        name: "LargerRow",
        className: "com.example.Large",
        objectId: "0x2",
        dominates: 8,
        retainedSize: 512,
        shallowSize: 32,
      },
    ];

    const view = render(
      <DominatorExplorerPanel rows={unsortedRows} selectedRowIndex={0} onSelectRowIndex={() => {}} />,
    );
    const panel = within(view.container);
    const smallerRow = panel.getByRole("button", { name: /select com\.example\.small 0x1/i });
    const largerRow = panel.getByRole("button", { name: /select com\.example\.large 0x2/i });
    const smallerBars = within(smallerRow).getAllByTestId(/(retained|dominates)-bar/i);
    const largerBars = within(largerRow).getAllByTestId(/(retained|dominates)-bar/i);

    expect(smallerBars[0]?.getAttribute("style")).toContain("width: 25%");
    expect(smallerBars[1]?.getAttribute("style")).toContain("width: 25%");
    expect(largerBars[0]?.getAttribute("style")).toContain("width: 100%");
    expect(largerBars[1]?.getAttribute("style")).toContain("width: 100%");
  });

  it("renders zero-width bars for rows with zero retained size and zero dominates", () => {
    const zeroRows = [
      {
        name: "ZeroRow",
        className: "com.example.Zero",
        objectId: "0x0",
        dominates: 0,
        retainedSize: 0,
        shallowSize: 0,
      },
      {
        name: "NonZeroRow",
        className: "com.example.NonZero",
        objectId: "0x1",
        dominates: 4,
        retainedSize: 256,
        shallowSize: 16,
      },
    ];

    const view = render(
      <DominatorExplorerPanel rows={zeroRows} selectedRowIndex={0} onSelectRowIndex={() => {}} />,
    );
    const panel = within(view.container);
    const zeroRow = panel.getByRole("button", { name: /select com\.example\.zero 0x0/i });
    const retainedBar = zeroRow.querySelector('[data-testid="retained-bar"]');
    const dominatesBar = zeroRow.querySelector('[data-testid="dominates-bar"]');

    expect(retainedBar).toBeTruthy();
    expect(dominatesBar).toBeTruthy();
    expect(retainedBar?.getAttribute("style")).toContain("width: 0%");
    expect(dominatesBar?.getAttribute("style")).toContain("width: 0%");
  });

  it("selects duplicate-label rows independently by position", async () => {
    const user = userEvent.setup();
    const duplicateRows = [
      {
        name: "DuplicateRow",
        className: "com.example.Duplicate",
        objectId: "",
        dominates: 1,
        retainedSize: 64,
        shallowSize: 8,
      },
      {
        name: "DuplicateRow",
        className: "com.example.Duplicate",
        objectId: "",
        dominates: 2,
        retainedSize: 96,
        shallowSize: 12,
      },
    ];
    let selectedRowIndex = 0;

    const view = render(
      <DominatorExplorerPanel
        rows={duplicateRows}
        selectedRowIndex={selectedRowIndex}
        onSelectRowIndex={(nextRowIndex) => {
          selectedRowIndex = nextRowIndex;
          view.rerender(
            <DominatorExplorerPanel rows={duplicateRows} selectedRowIndex={selectedRowIndex} onSelectRowIndex={() => {}} />,
          );
        }}
      />,
    );
    const panel = within(view.container);
    const duplicateButtons = panel.getAllByRole("button", { name: /select com\.example\.duplicate/i });

    expect(duplicateButtons).toHaveLength(2);
    expect(duplicateButtons[0]?.getAttribute("aria-pressed")).toBe("true");
    expect(duplicateButtons[1]?.getAttribute("aria-pressed")).toBe("false");

    await user.click(duplicateButtons[1]!);

    const updatedButtons = panel.getAllByRole("button", { name: /select com\.example\.duplicate/i });
    expect(updatedButtons[0]?.getAttribute("aria-pressed")).toBe("false");
    expect(updatedButtons[1]?.getAttribute("aria-pressed")).toBe("true");
  });

  it("gives artifact-only rows distinct accessible names", () => {
    const artifactOnlyRows = [
      {
        name: "FirstArtifactOnly",
        className: "com.example.FirstArtifactOnly",
        objectId: "",
        dominates: 1,
        retainedSize: 64,
        shallowSize: 8,
      },
      {
        name: "SecondArtifactOnly",
        className: "com.example.SecondArtifactOnly",
        objectId: "",
        dominates: 2,
        retainedSize: 96,
        shallowSize: 12,
      },
    ];

    const view = render(
      <DominatorExplorerPanel rows={artifactOnlyRows} selectedRowIndex={0} onSelectRowIndex={() => {}} />,
    );
    const panel = within(view.container);

    expect(
      panel.getByRole("button", { name: /select com\.example\.firstartifactonly artifact-only row firstartifactonly/i }),
    ).toBeInTheDocument();
    expect(
      panel.getByRole("button", { name: /select com\.example\.secondartifactonly artifact-only row secondartifactonly/i }),
    ).toBeInTheDocument();
  });

  it("adds a stable row ordinal when duplicate artifact-only rows fully collide", () => {
    const duplicateArtifactOnlyRows = [
      {
        name: "DuplicateArtifactRow",
        className: "com.example.DuplicateArtifactRow",
        objectId: "",
        dominates: 3,
        retainedSize: 256,
        shallowSize: 16,
      },
      {
        name: "DuplicateArtifactRow",
        className: "com.example.DuplicateArtifactRow",
        objectId: "",
        dominates: 7,
        retainedSize: 512,
        shallowSize: 32,
      },
    ];

    const view = render(
      <DominatorExplorerPanel rows={duplicateArtifactOnlyRows} selectedRowIndex={0} onSelectRowIndex={() => {}} />,
    );
    const panel = within(view.container);

    expect(
      panel.getByRole("button", {
        name: /select com\.example\.duplicateartifactrow artifact-only row duplicateartifactrow row 1/i,
      }),
    ).toBeInTheDocument();
    expect(
      panel.getByRole("button", {
        name: /select com\.example\.duplicateartifactrow artifact-only row duplicateartifactrow row 2/i,
      }),
    ).toBeInTheDocument();
  });

  it("renders an artifact-empty state when no dominator rows exist and search is blank", () => {
    const view = render(<DominatorExplorerPanel rows={[]} selectedRowIndex={undefined} onSelectRowIndex={() => {}} />);
    const panel = within(view.container);

    expect(panel.getByText(/no dominator rows are available in this artifact\./i)).toBeInTheDocument();
    expect(panel.queryByText(/no dominator rows match the current search\./i)).toBeNull();
  });

  it("requests roots once and children only when expanded, then removes descendants on collapse", async () => {
    const user = userEvent.setup();
    const requests: Array<[string | undefined, number | undefined, number | undefined, number | undefined]> = [];
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      getDominatorChildren: async (parentObjectId, offset, limit, minRetainedBytes) => {
        requests.push([parentObjectId, offset, limit, minRetainedBytes]);
        const children =
          parentObjectId === undefined
            ? [
                {
                  object_id: "0x1",
                  class_name: "com.example.Root",
                  shallow_size: 64,
                  retained_size: 4096,
                  dominated_count: 2,
                  has_children: true,
                },
              ]
            : [
                {
                  object_id: "0x2",
                  class_name: "com.example.Child",
                  shallow_size: 32,
                  retained_size: 2048,
                  dominated_count: 0,
                  has_children: false,
                },
              ];
        return { total: children.length, returned: children.length, offset: 0, limit: 50, truncated: false, children };
      },
    };

    const view = render(
      <DominatorExplorerPanel
        rows={rows}
        totalSizeBytes={10_000}
        selectedObjectId={undefined}
        onSelectObjectId={() => {}}
        selectedRowIndex={0}
        onSelectRowIndex={() => {}}
      />,
    );
    const panel = within(view.container);

    await waitFor(() => expect(panel.getByRole("button", { name: /select com\.example\.root 0x1/i })).toBeInTheDocument());
    expect(requests).toEqual([[undefined, 0, 50, 0]]);
    expect(panel.queryByRole("button", { name: /select com\.example\.child 0x2/i })).toBeNull();

    await user.click(panel.getByRole("button", { name: /expand com\.example\.root 0x1/i }));

    await waitFor(() => expect(panel.getByRole("button", { name: /select com\.example\.child 0x2/i })).toBeInTheDocument());
    expect(requests).toEqual([
      [undefined, 0, 50, 0],
      ["0x1", 0, 50, 0],
    ]);

    await user.click(panel.getByRole("button", { name: /collapse com\.example\.root 0x1/i }));

    expect(panel.queryByRole("button", { name: /select com\.example\.child 0x2/i })).toBeNull();
  });

  it("converts retained percent to bytes when refreshing roots", async () => {
    const user = userEvent.setup();
    const requests: Array<[string | undefined, number | undefined, number | undefined, number | undefined]> = [];
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      getDominatorChildren: async (parentObjectId, offset, limit, minRetainedBytes) => {
        requests.push([parentObjectId, offset, limit, minRetainedBytes]);
        return { total: 0, returned: 0, offset: 0, limit: 50, truncated: false, children: [] };
      },
    };

    const view = render(
      <DominatorExplorerPanel
        rows={rows}
        totalSizeBytes={10_000}
        selectedObjectId={undefined}
        onSelectObjectId={() => {}}
        selectedRowIndex={0}
        onSelectRowIndex={() => {}}
      />,
    );
    const panel = within(view.container);

    await waitFor(() => expect(requests).toEqual([[undefined, 0, 50, 0]]));
    const retainedInput = panel.getByRole("spinbutton", { name: /minimum retained percent/i });
    await user.clear(retainedInput);
    await user.type(retainedInput, "1");

    await waitFor(() => expect(requests[requests.length - 1]).toEqual([undefined, 0, 50, 100]));
  });

  it("loads the next bounded child page and selects a live child by object id", async () => {
    const user = userEvent.setup();
    const requests: Array<[string | undefined, number | undefined]> = [];
    let selectedObjectId: string | undefined;
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      getDominatorChildren: async (parentObjectId, offset) => {
        requests.push([parentObjectId, offset]);
        if (parentObjectId === undefined) {
          return {
            total: 1,
            returned: 1,
            offset: 0,
            limit: 50,
            truncated: false,
            children: [
              {
                object_id: "0x1",
                class_name: "com.example.Root",
                shallow_size: 64,
                retained_size: 4096,
                dominated_count: 2,
                has_children: true,
              },
            ],
          };
        }

        const child =
          offset === 0
            ? { object_id: "0x2", class_name: "com.example.First", retained_size: 2048 }
            : { object_id: "0x3", class_name: "com.example.Second", retained_size: 1024 };
        return {
          total: 2,
          returned: 1,
          offset: offset ?? 0,
          limit: 50,
          truncated: offset === 0,
          children: [{ ...child, shallow_size: 32, dominated_count: 0, has_children: false }],
        };
      },
    };

    const view = render(
      <DominatorExplorerPanel
        rows={rows}
        totalSizeBytes={10_000}
        selectedObjectId={selectedObjectId}
        onSelectObjectId={(objectId) => {
          selectedObjectId = objectId;
        }}
        selectedRowIndex={0}
        onSelectRowIndex={() => {}}
      />,
    );
    const panel = within(view.container);

    await user.click(await panel.findByRole("button", { name: /expand com\.example\.root 0x1/i }));
    const firstChild = await panel.findByRole("button", { name: /select com\.example\.first 0x2/i });
    expect(panel.queryByRole("button", { name: /select com\.example\.second 0x3/i })).toBeNull();

    await user.click(firstChild);
    expect(selectedObjectId).toBe("0x2");

    await user.click(panel.getByRole("button", { name: /load next children for com\.example\.root 0x1/i }));

    await waitFor(() => expect(panel.getByRole("button", { name: /select com\.example\.second 0x3/i })).toBeInTheDocument());
    expect(requests).toContainEqual(["0x1", 1]);
  });

  it("keeps a parent visible and offers retry after a child request fails", async () => {
    const user = userEvent.setup();
    let childAttempts = 0;
    window.__MNEMOSYNE_HEAP_EXPLORER_BRIDGE__ = {
      getDominatorChildren: async (parentObjectId) => {
        if (parentObjectId === undefined) {
          return {
            total: 1,
            returned: 1,
            offset: 0,
            limit: 50,
            truncated: false,
            children: [
              {
                object_id: "0x1",
                class_name: "com.example.Root",
                shallow_size: 64,
                retained_size: 4096,
                dominated_count: 1,
                has_children: true,
              },
            ],
          };
        }
        childAttempts += 1;
        if (childAttempts === 1) {
          throw new Error("child lookup failed");
        }
        return {
          total: 1,
          returned: 1,
          offset: 0,
          limit: 50,
          truncated: false,
          children: [
            {
              object_id: "0x2",
              class_name: "com.example.Recovered",
              shallow_size: 32,
              retained_size: 1024,
              dominated_count: 0,
              has_children: false,
            },
          ],
        };
      },
    };

    const view = render(
      <DominatorExplorerPanel
        rows={rows}
        totalSizeBytes={10_000}
        selectedObjectId={undefined}
        onSelectObjectId={() => {}}
        selectedRowIndex={0}
        onSelectRowIndex={() => {}}
      />,
    );
    const panel = within(view.container);

    await user.click(await panel.findByRole("button", { name: /expand com\.example\.root 0x1/i }));
    expect(await panel.findByText(/child lookup failed/i)).toBeInTheDocument();
    expect(panel.getByRole("button", { name: /select com\.example\.root 0x1/i })).toBeInTheDocument();

    await user.click(panel.getByRole("button", { name: /retry children for com\.example\.root 0x1/i }));

    expect(await panel.findByRole("button", { name: /select com\.example\.recovered 0x2/i })).toBeInTheDocument();
    expect(childAttempts).toBe(2);
  });

  it("labels and bounds the artifact-only flat preview", () => {
    const manyRows = Array.from({ length: 125 }, (_, index) => ({
      name: `ArtifactRow${index}`,
      className: `com.example.Artifact${index}`,
      objectId: "",
      dominates: index,
      retainedSize: index,
      shallowSize: 1,
    }));

    const view = render(
      <DominatorExplorerPanel rows={manyRows} selectedRowIndex={0} onSelectRowIndex={() => {}} />,
    );
    const panel = within(view.container);

    expect(panel.getByText(/artifact-only bounded flat preview/i)).toBeInTheDocument();
    expect(panel.getAllByRole("button", { name: /select com\.example\.artifact/i })).toHaveLength(100);
    expect(panel.getByText(/showing first 100 of 125 artifact rows/i)).toBeInTheDocument();
  });
});

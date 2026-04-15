import "../../../test/setup";

import userEvent from "@testing-library/user-event";
import { render, within } from "@testing-library/react";
import { describe, expect, it } from "bun:test";

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
});

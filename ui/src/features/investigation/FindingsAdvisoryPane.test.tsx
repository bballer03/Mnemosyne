import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { FindingsAdvisoryPane } from "./FindingsAdvisoryPane";
import { useInvestigationStore } from "./investigation-store";

function seedArtifact() {
  useArtifactStore.getState().setArtifact("fixture.json", {
    summary: {
      heapPath: "/secret/path/fixture.hprof",
      totalObjects: 42,
      totalSizeBytes: 2048,
      totalRecords: 2,
    },
    leaks: [
      {
        id: "leak-cache",
        className: "com.example.Cache",
        leakKind: "CACHE",
        severity: "HIGH",
        retainedSizeBytes: 1024,
        suspectScore: 0.9,
        instances: 4,
        description: "Cache retains request objects.",
        provenance: [{ kind: "FALLBACK", detail: "heuristic retained-size candidate" }],
      },
    ],
    recommendations: [],
    elapsedSeconds: 0,
    graph: {
      nodeCount: 4,
      edgeCount: 3,
      dominatorCount: 1,
      dominators: [
        {
          name: "com.example.Cache",
          className: "com.example.Cache",
          objectId: "0x2a",
          dominates: 3,
          retainedSize: 1024,
          shallowSize: 64,
        },
      ],
    },
    classloaderReport: {
      loaders: [],
      potentialLeaks: [
        {
          objectId: 43,
          className: "org.example.WebAppClassLoader",
          retainedBytes: 768,
          loadedClassCount: 1,
          reason: "Loader retained after redeploy.",
        },
      ],
      duplicateClasses: [
        {
          className: "com.example.Handler",
          loaderObjectIds: [43, 44],
          loaderCount: 2,
        },
      ],
    },
    collectionReport: {
      totalCollections: 1,
      totalWasteBytes: 512,
      emptyCollections: 0,
      oversizedCollections: [
        {
          objectId: 45,
          collectionType: "java.util.HashMap",
          size: 1,
          capacity: 64,
          fillRatio: 1 / 64,
          shallowBytes: 64,
          retainedBytes: 640,
          wasteBytes: 512,
        },
      ],
      summaryByType: {},
    },
    stringReport: {
      totalStrings: 2,
      totalStringBytes: 128,
      uniqueStrings: 1,
      duplicateGroups: [
        {
          value: "duplicate value",
          count: 2,
          totalWastedBytes: 64,
        },
      ],
      totalDuplicateWaste: 64,
      topStringsBySize: [],
    },
    arrayReport: {
      totalArrays: 2,
      uniqueContents: 1,
      duplicateGroups: [
        {
          elementType: "byte",
          contentHash: 12345,
          length: 16,
          count: 2,
          totalWastedBytes: 16,
        },
      ],
      totalDuplicateWaste: 16,
    },
    provenance: [{ kind: "PARTIAL", detail: "fixture reports" }],
  });
}

function renderPane() {
  return render(
    <MemoryRouter>
      <FindingsAdvisoryPane />
    </MemoryRouter>,
  );
}

describe("FindingsAdvisoryPane", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.setState({
        revision: 0,
        objectId: undefined,
        classKey: undefined,
        leakId: undefined,
        originPane: undefined,
        findingFacts: [],
        findingStatuses: {},
      });
    });
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.getState().clearSelection();
    });
  });

  it("labels offline rules and fallback provenance without replacing artifact facts", () => {
    seedArtifact();
    const view = renderPane();

    const pane = view.getByRole("region", { name: /findings advisory/i });
    expect(within(pane).getByText(/advisory provenance:\s*rules · offline/i)).toBeInTheDocument();
    expect(within(pane).getByText("FALLBACK")).toBeInTheDocument();
    expect(within(pane).getByText(/loaded artifact remains authoritative/i)).toBeInTheDocument();
    expect(pane.textContent ?? "").not.toContain("/secret/path");
  });

  it("collapses and expands the findings queue", async () => {
    const user = userEvent.setup();
    seedArtifact();
    const view = renderPane();

    const toggle = view.getByRole("button", { name: /collapse findings advisory/i });
    await user.click(toggle);

    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(view.queryByText("Cache retains request objects.")).not.toBeInTheDocument();

    await user.click(view.getByRole("button", { name: /expand findings advisory/i }));
    expect(view.getByText("Cache retains request objects.")).toBeInTheDocument();
  });

  it("renders all finding kinds with stable identifiers and no row-index links", () => {
    seedArtifact();
    const view = renderPane();
    act(() => {
      const state = useInvestigationStore.getState();
      state.replaceFindings(
        { workspaceId: state.workspaceId, revision: state.revision },
        "policy",
        [
          {
            id: "policy:leak-budget:leak_count",
            source: "policy",
            kind: "policy",
            severity: "error",
            title: "Policy violation: leak-budget",
            description: "Expected no leak suspects.",
            target: { kind: "leak", leakId: "leak-cache" },
            provenance: [{ kind: "RULES" }],
            metrics: { ruleId: "leak-budget", predicate: "leak_count" },
          },
        ],
      );
    });

    const pane = view.getByRole("region", { name: /findings advisory/i });
    expect(within(pane).getByText("Leak suspect: com.example.Cache")).toBeInTheDocument();
    expect(
      within(pane).getByText("Classloader retention: org.example.WebAppClassLoader"),
    ).toBeInTheDocument();
    expect(within(pane).getByText("Duplicate class: com.example.Handler")).toBeInTheDocument();
    expect(
      within(pane).getByText("Collection waste: java.util.HashMap"),
    ).toBeInTheDocument();
    expect(within(pane).getByText("Duplicate string content")).toBeInTheDocument();
    expect(within(pane).getByText("Duplicate byte[] content")).toBeInTheDocument();
    expect(within(pane).getByText("Policy violation: leak-budget")).toBeInTheDocument();

    const hrefs = within(pane)
      .getAllByRole("link")
      .map((link) => link.getAttribute("href") ?? "");
    expect(hrefs).toContain("/leaks/leak-cache/overview");
    expect(hrefs).toContain("/heap-explorer/object-inspector?objectId=0x2b");
    expect(hrefs).toContain("/artifacts/explorer?classKey=com.example.Handler");
    expect(hrefs.some((href) => /row|index/i.test(href))).toBe(false);
  });

  it("deep-links a leak and synchronizes stable leak, class, and object selection", async () => {
    const user = userEvent.setup();
    seedArtifact();
    const view = renderPane();

    const pane = view.getByRole("region", { name: /findings advisory/i });
    const leakLink = within(pane).getByRole("link", {
      name: "Leak suspect: com.example.Cache",
    });
    expect(leakLink.getAttribute("href")).toBe("/leaks/leak-cache/overview");
    expect(
      within(pane).getByRole("link", { name: /open full assistant/i }).getAttribute("href"),
    ).toBe("/assistant");

    await user.click(leakLink);
    expect(useInvestigationStore.getState()).toMatchObject({
      leakId: "leak-cache",
      classKey: "com.example.Cache",
      objectId: "0x2a",
    });
  });

  it("changes user status without rewriting immutable measured facts", async () => {
    const user = userEvent.setup();
    seedArtifact();
    const view = renderPane();
    const before = useInvestigationStore
      .getState()
      .findingFacts.find((fact) => fact.id === "leak:leak-cache");

    const status = view.getByRole("combobox", {
      name: "Status for Leak suspect: com.example.Cache",
    });
    expect(status).toHaveValue("open");
    await user.selectOptions(status, "resolved");

    expect(useInvestigationStore.getState().findingStatuses["leak:leak-cache"]).toBe(
      "resolved",
    );
    expect(
      useInvestigationStore
        .getState()
        .findingFacts.find((fact) => fact.id === "leak:leak-cache"),
    ).toBe(before);
    expect(view.getByText("Cache retains request objects.")).toBeInTheDocument();

    act(() => seedArtifact());

    expect(
      view.getByRole("combobox", {
        name: "Status for Leak suspect: com.example.Cache",
      }),
    ).toHaveValue("resolved");
  });
});

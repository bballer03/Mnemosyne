import "../../test/setup";

import { act, cleanup, render, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { createMemoryRouter, RouterProvider } from "react-router-dom";

import { routes } from "../../app/router";
import { rememberDesktopHeapSource, clearRememberedDesktopHeapSource } from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import { DEFAULT_HISTORY_MAX_TURNS } from "./assistant-bridge-client";

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
        id: "leak-low",
        className: "com.example.Low",
        leakKind: "CACHE",
        severity: "LOW",
        retainedSizeBytes: 10,
        suspectScore: 0.2,
        instances: 1,
        description: "low severity cache",
        provenance: [],
      },
      {
        id: "leak-high",
        className: "com.example.High",
        leakKind: "CACHE",
        severity: "HIGH",
        retainedSizeBytes: 100,
        suspectScore: 0.9,
        instances: 2,
        description: "high severity cache",
        provenance: [],
      },
    ],
    recommendations: [],
    elapsedSeconds: 0,
    graph: { nodeCount: 0, edgeCount: 0, dominatorCount: 0, dominators: [] },
    unreachable: { totalUnreachableObjects: 0, totalUnreachableBytes: 0, entries: [] },
    provenance: [],
  } as never);
}

describe("InvestigationAssistantPage", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
    });
    clearRememberedDesktopHeapSource();
    delete window.__MNEMOSYNE_ASSISTANT_BRIDGE__;
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useArtifactStore.getState().reset();
    });
    clearRememberedDesktopHeapSource();
    delete window.__MNEMOSYNE_ASSISTANT_BRIDGE__;
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
  });

  it("defaults to rules mode offline and separates AI turns from measured facts", async () => {
    const user = userEvent.setup();
    seedArtifact();
    rememberDesktopHeapSource("src-opaque", "fixture.hprof");

    const router = createMemoryRouter(routes, { initialEntries: ["/assistant"] });
    const view = render(<RouterProvider router={router} />);

    expect(view.getByRole("heading", { name: /investigation session/i })).toBeInTheDocument();
    expect(view.getByText(/mode:\s*rules/i)).toBeInTheDocument();

    const facts = view.getByRole("region", { name: /measured heap facts/i });
    expect(within(facts).getByText(/fixture\.hprof/)).toBeInTheDocument();
    expect(facts.textContent ?? "").not.toMatch(/\/secret\/path/);
    expect(within(facts).getByText(/42/)).toBeInTheDocument();
    expect(within(facts).getByText(/leak-high/i)).toBeInTheDocument();

    await user.type(view.getByLabelText(/ask a follow-up/i), "What should I check first?");
    await user.click(view.getByRole("button", { name: /^ask$/i }));

    const ai = view.getByRole("region", { name: /ai guidance/i });
    expect(ai.textContent ?? "").toMatch(/provenance:\s*rules/i);
    expect(ai.textContent ?? "").toMatch(/What should I check first\?/i);
    expect(facts.textContent ?? "").not.toMatch(/What should I check first\?/i);
  });

  it("shows provider unavailable while keeping rules mode usable", async () => {
    const user = userEvent.setup();
    seedArtifact();

    const router = createMemoryRouter(routes, { initialEntries: ["/assistant"] });
    const view = render(<RouterProvider router={router} />);

    await user.click(view.getByRole("button", { name: /try provider mode/i }));
    expect(view.getByText(/provider chat is unavailable/i)).toBeInTheDocument();
    expect(view.getByText(/mode:\s*rules/i)).toBeInTheDocument();

    await user.type(view.getByLabelText(/ask a follow-up/i), "Still works offline?");
    await user.click(view.getByRole("button", { name: /^ask$/i }));
    const ai = view.getByRole("region", { name: /ai guidance/i });
    expect(ai.textContent ?? "").toMatch(/Still works offline\?/i);
  });

  it("updates focus when the selected leak changes", async () => {
    const user = userEvent.setup();
    seedArtifact();

    const router = createMemoryRouter(routes, { initialEntries: ["/assistant"] });
    const view = render(<RouterProvider router={router} />);

    const select = view.getByLabelText(/focus leak/i);
    expect((select as HTMLSelectElement).value).toBe("leak-high");

    await user.selectOptions(select, "leak-low");
    expect((select as HTMLSelectElement).value).toBe("leak-low");

    const facts = view.getByRole("region", { name: /measured heap facts/i });
    expect(within(facts).getByText(/leak-low/i)).toBeInTheDocument();
    expect(within(facts).getByText(/low severity cache/i)).toBeInTheDocument();
  });

  it("evicts history past twelve turns in the workspace", async () => {
    const user = userEvent.setup();
    seedArtifact();

    const router = createMemoryRouter(routes, { initialEntries: ["/assistant"] });
    const view = render(<RouterProvider router={router} />);

    for (let i = 0; i < DEFAULT_HISTORY_MAX_TURNS + 1; i++) {
      const input = view.getByLabelText(/ask a follow-up/i);
      await user.clear(input);
      await user.type(input, `turn-${i}`);
      await user.click(view.getByRole("button", { name: /^ask$/i }));
      const ai = view.getByRole("region", { name: /ai guidance/i });
      expect(ai.textContent ?? "").toContain(`Q: turn-${i}`);
    }

    const ai = view.getByRole("region", { name: /ai guidance/i });
    expect(ai.textContent ?? "").not.toContain("Q: turn-0");
    expect(ai.textContent ?? "").toContain("Q: turn-1");
    expect(ai.textContent ?? "").toContain(`Q: turn-${DEFAULT_HISTORY_MAX_TURNS}`);
  });

  it("exposes deterministic deep links to workbench power views", () => {
    seedArtifact();

    const router = createMemoryRouter(routes, { initialEntries: ["/assistant"] });
    const view = render(<RouterProvider router={router} />);
    const links = view.getByRole("navigation", { name: /deterministic workbench links/i });

    expect(within(links).getByRole("link", { name: /dashboard/i }).getAttribute("href")).toBe(
      "/dashboard",
    );
    expect(within(links).getByRole("link", { name: /object inspector/i }).getAttribute("href")).toBe(
      "/heap-explorer/object-inspector",
    );
    expect(within(links).getByRole("link", { name: /dominators/i }).getAttribute("href")).toBe(
      "/heap-explorer/dominators",
    );
    expect(within(links).getByRole("link", { name: /query console/i }).getAttribute("href")).toBe(
      "/heap-explorer/query-console",
    );
    expect(within(links).getByRole("link", { name: /leak workspace/i }).getAttribute("href")).toBe(
      "/leaks/leak-high/overview",
    );
    expect(within(links).getByRole("link", { name: /gc path/i }).getAttribute("href")).toBe(
      "/leaks/leak-high/gc-path",
    );
  });
});

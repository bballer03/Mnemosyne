import "../../test/setup";

import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter } from "react-router-dom";

import {
  rememberDesktopHeapSource,
  clearRememberedDesktopHeapSource,
} from "../artifact-loader/desktop-heap-session";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";
import {
  useInvestigationStore,
  type FindingFact,
} from "../investigation/investigation-store";
import { DEFAULT_HISTORY_MAX_TURNS } from "./assistant-bridge-client";
import { InvestigationAssistantPage } from "./InvestigationAssistantPage";

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

function seedMeasuredFinding(fact: FindingFact = {
  id: "collection:0x10",
  source: "artifact",
  kind: "collection-waste",
  severity: "WARNING",
  title: "Collection waste: java.util.HashMap",
  description: "HashMap uses 1024 slots for 3 entries.",
  target: { kind: "object", objectId: "0x10", classKey: "java.util.HashMap" },
  provenance: [{ kind: "Measured", detail: "artifact analysis" }],
  metrics: { wasteBytes: 4084 },
}) {
  const state = useInvestigationStore.getState();
  state.replaceFindings(
    { workspaceId: state.workspaceId, revision: state.revision },
    "artifact",
    [fact],
  );
  return fact;
}

function renderPage() {
  return render(
    <MemoryRouter>
      <InvestigationAssistantPage />
    </MemoryRouter>,
  );
}

describe("InvestigationAssistantPage", () => {
  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.setState({
        revision: 0,
        activeWorkflow: undefined,
        workflowNeedsRecovery: false,
        workspaceRequests: {},
        objectId: undefined,
        classKey: undefined,
        leakId: undefined,
        originPane: undefined,
        findingFacts: [],
        findingStatuses: {},
      });
    });
    clearRememberedDesktopHeapSource();
    delete window.__MNEMOSYNE_ASSISTANT_BRIDGE__;
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useArtifactStore.getState().reset();
      useInvestigationStore.getState().clearSelection();
      useInvestigationStore.getState().clearFindings();
    });
    clearRememberedDesktopHeapSource();
    delete window.__MNEMOSYNE_ASSISTANT_BRIDGE__;
    delete window.__MNEMOSYNE_WORKFLOW_BRIDGE__;
  });

  it("renders stable selection measured facts and keeps findings immutable after a rules turn", async () => {
    const user = userEvent.setup();
    seedArtifact();
    rememberDesktopHeapSource("src-opaque", "fixture.hprof");
    const fact = seedMeasuredFinding();
    useInvestigationStore.getState().setObjectId("0x10", "inspector");
    useInvestigationStore.getState().setClassKey("com.example.Cache", "inspector");
    const originalFact = useInvestigationStore.getState().findingFacts[0];

    const view = renderPage();

    expect(view.getByRole("heading", { name: /investigation session/i })).toBeInTheDocument();
    expect(view.getByText(/mode:\s*rules/i)).toBeInTheDocument();

    const facts = view.getByRole("region", { name: /measured heap facts/i });
    expect(within(facts).getByText(/fixture\.hprof/)).toBeInTheDocument();
    expect(facts.textContent ?? "").not.toMatch(/\/secret\/path/);
    expect(within(facts).getByText(/object 0x10/i)).toBeInTheDocument();
    expect(within(facts).getByText(/Collection waste: java\.util\.HashMap/i)).toBeInTheDocument();
    expect(within(facts).getByText(/HashMap uses 1024 slots for 3 entries/i)).toBeInTheDocument();
    expect(within(facts).getByText(/Provenance: Measured \(artifact analysis\)/i)).toBeInTheDocument();

    await user.type(view.getByLabelText(/ask a follow-up/i), "What should I check first?");
    await user.click(view.getByRole("button", { name: /^ask$/i }));

    const ai = view.getByRole("region", { name: /ai guidance/i });
    await waitFor(() => {
      expect(ai.textContent ?? "").toMatch(/provenance:\s*rules/i);
      expect(ai.textContent ?? "").toMatch(/What should I check first\?/i);
      expect(ai.textContent ?? "").toMatch(/Collection waste: java\.util\.HashMap/i);
    });
    expect(facts.textContent ?? "").not.toMatch(/What should I check first\?/i);
    expect(useInvestigationStore.getState().findingFacts[0]).toBe(originalFact);
    expect(useInvestigationStore.getState().findingFacts[0]).toEqual(fact);
  });

  it("keeps measured facts and workbench links visible when AI guidance is collapsed", async () => {
    const user = userEvent.setup();
    seedArtifact();
    seedMeasuredFinding();
    useInvestigationStore.getState().setObjectId("0x10", "inspector");

    const view = renderPage();
    const disclosure = view.getByText(/^AI guidance$/i).closest("details");
    expect(disclosure).not.toBeNull();
    await user.click(view.getByText(/^AI guidance$/i));
    expect((disclosure as HTMLDetailsElement).open).toBe(false);

    const facts = view.getByRole("region", { name: /measured heap facts/i });
    expect(within(facts).getByText(/Collection waste: java\.util\.HashMap/i)).toBeInTheDocument();
    const links = view.getByRole("navigation", { name: /deterministic workbench links/i });
    expect(within(links).getByRole("link", { name: /dashboard/i })).toBeInTheDocument();
    expect(within(links).getByRole("link", { name: /object inspector/i })).toBeInTheDocument();
    expect(within(links).getByRole("link", { name: /dominators/i })).toBeInTheDocument();
    expect(within(links).getByRole("link", { name: /query console/i })).toBeInTheDocument();
  });

  it("shows provider unavailable while keeping rules mode usable without inventing a focus", async () => {
    const user = userEvent.setup();
    seedArtifact();

    const view = renderPage();

    expect((view.getByLabelText(/focus leak/i) as HTMLSelectElement).value).toBe("");
    expect(view.getByText(/Selection: none/i)).toBeInTheDocument();

    await user.click(view.getByRole("button", { name: /check provider availability/i }));
    expect(view.getByText(/provider chat is unavailable/i)).toBeInTheDocument();
    expect(view.getByText(/mode:\s*rules/i)).toBeInTheDocument();

    await user.type(view.getByLabelText(/ask a follow-up/i), "Still works offline?");
    await user.click(view.getByRole("button", { name: /^ask$/i }));
    const ai = view.getByRole("region", { name: /ai guidance/i });
    await waitFor(() => {
      expect(ai.textContent ?? "").toMatch(/Still works offline\?/i);
      expect(ai.textContent ?? "").toMatch(/provenance:\s*rules/i);
    });
  });

  it("uses chatSession with provider provenance when the host bridge is wired", async () => {
    const user = userEvent.setup();
    seedArtifact();
    rememberDesktopHeapSource("src-opaque", "fixture.hprof");
    useInvestigationStore.getState().setLeakId("leak-high", "leak");

    let chatCalls = 0;
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      createAiSession: async () => ({
        session_id: "mcp-ui-1",
        display_name: "fixture.hprof",
        outbound_metadata: {
          sends: ["heap_summary_stats"],
          never_sends: ["api_keys", "absolute_heap_paths"],
        },
      }),
      chatSession: async (input) => {
        chatCalls += 1;
        expect(input.sessionId).toBe("mcp-ui-1");
        expect(input.focusLeakId).toBe("leak-high");
        return { summary: "Session guidance for leak-high.", model: "gpt-test" };
      },
    };

    const view = renderPage();

    await user.click(view.getByRole("button", { name: /check provider availability/i }));
    expect(view.getByText(/ask uses chatsession/i)).toBeInTheDocument();
    expect(view.getByLabelText(/outbound metadata notice/i).textContent ?? "").toMatch(
      /api keys|never/i,
    );
    expect(view.getByLabelText(/outbound metadata notice/i).textContent ?? "").not.toMatch(/sk-/i);
    expect(document.body.textContent ?? "").not.toMatch(/\/secret\/path/);

    await user.type(view.getByLabelText(/ask a follow-up/i), "Explain the focus");
    await user.click(view.getByRole("button", { name: /^ask$/i }));

    const ai = view.getByRole("region", { name: /ai guidance/i });
    await waitFor(() => {
      expect(ai.textContent ?? "").toMatch(/provenance:\s*provider/i);
      expect(ai.textContent ?? "").toMatch(/Session guidance for leak-high/i);
      expect(view.getByText(/mode:\s*provider/i)).toBeInTheDocument();
    });
    expect(chatCalls).toBe(1);
  });

  it("falls back to rules with recovery guidance on provider timeout", async () => {
    const user = userEvent.setup();
    seedArtifact();

    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      createAiSession: async () => ({ session_id: "mcp-ui-2" }),
      chatSession: async () => {
        throw new Error("provider_timeout");
      },
    };

    const view = renderPage();

    await user.type(view.getByLabelText(/ask a follow-up/i), "Still recoverable?");
    await user.click(view.getByRole("button", { name: /^ask$/i }));

    await waitFor(() => {
      expect(view.getByText(/recovery=rules_mode_available; error=provider_timeout/i)).toBeInTheDocument();
      const ai = view.getByRole("region", { name: /ai guidance/i });
      expect(ai.textContent ?? "").toMatch(/provenance:\s*fallback/i);
      expect(ai.textContent ?? "").toMatch(/Still recoverable\?/i);
    });
  });

  it("updates focus when the selected leak changes", async () => {
    const user = userEvent.setup();
    seedArtifact();
    useInvestigationStore.getState().setLeakId("leak-high", "leak");
    useInvestigationStore.getState().setObjectId("0xstale", "findings");

    const view = renderPage();

    const select = view.getByLabelText(/focus leak/i);
    expect((select as HTMLSelectElement).value).toBe("leak-high");

    await user.selectOptions(select, "leak-low");
    expect((select as HTMLSelectElement).value).toBe("leak-low");

    const facts = view.getByRole("region", { name: /measured heap facts/i });
    expect(within(facts).getByText(/leak leak-low/i)).toBeInTheDocument();
    expect(useInvestigationStore.getState()).toMatchObject({
      leakId: "leak-low",
      classKey: "com.example.Low",
      objectId: undefined,
    });
  });

  it("opens on the finding selected in the shared investigation store", () => {
    seedArtifact();
    useInvestigationStore.getState().setLeakId("leak-low", "findings");

    const view = renderPage();

    expect((view.getByLabelText(/focus leak/i) as HTMLSelectElement).value).toBe("leak-low");
    const facts = view.getByRole("region", { name: /measured heap facts/i });
    expect(within(facts).getByText(/leak leak-low/i)).toBeInTheDocument();
  });

  it("shows the bound workflow kind and current step without id or heap path", () => {
    seedArtifact();
    const request = useInvestigationStore.getState().beginWorkspaceRequest("workflow");
    useInvestigationStore.getState().bindWorkflow(request, "tune_gc", {
      workflowId: "wf-internal-secret",
      currentStep: "thread_local_review",
    });

    const view = renderPage();
    const facts = view.getByRole("region", { name: /measured heap facts/i });
    expect(facts.textContent ?? "").toMatch(/Tune GC.*current step.*thread_local_review/i);
    expect(facts.textContent ?? "").not.toContain("wf-internal-secret");
    expect(document.body.textContent ?? "").not.toContain("/secret/path");
  });

  it("ignores an Assistant turn resolved after the workspace revision changes", async () => {
    const user = userEvent.setup();
    seedArtifact();
    let resolveChat!: (value: unknown) => void;
    const chat = new Promise<unknown>((resolve) => {
      resolveChat = resolve;
    });
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      createAiSession: async () => ({ session_id: "session-stale" }),
      chatSession: async () => chat,
    };
    const view = renderPage();

    await user.type(view.getByLabelText(/ask a follow-up/i), "stale question");
    await user.click(view.getByRole("button", { name: /^ask$/i }));
    useInvestigationStore.getState().bumpRevisionOnArtifactChange();
    resolveChat({ summary: "stale answer", model: "provider" });

    await waitFor(() => expect(view.getByRole("button", { name: /^ask$/i })).not.toBeDisabled());
    expect(view.queryByText(/stale answer/i)).not.toBeInTheDocument();
    expect(view.queryByText(/Q: stale question/i)).not.toBeInTheDocument();
  });

  it("evicts history past twelve turns in the workspace", async () => {
    const user = userEvent.setup();
    seedArtifact();

    const view = renderPage();

    for (let i = 0; i < DEFAULT_HISTORY_MAX_TURNS + 1; i++) {
      const input = view.getByLabelText(/ask a follow-up/i);
      await user.clear(input);
      await user.type(input, `turn-${i}`);
      await user.click(view.getByRole("button", { name: /^ask$/i }));
      const ai = view.getByRole("region", { name: /ai guidance/i });
      await waitFor(() => {
        expect(ai.textContent ?? "").toContain(`Q: turn-${i}`);
      });
    }

    const ai = view.getByRole("region", { name: /ai guidance/i });
    expect(ai.textContent ?? "").not.toContain("Q: turn-0");
    expect(ai.textContent ?? "").toContain("Q: turn-1");
    expect(ai.textContent ?? "").toContain(`Q: turn-${DEFAULT_HISTORY_MAX_TURNS}`);
  });

  it("exposes deterministic deep links to workbench power views", () => {
    seedArtifact();
    useInvestigationStore.getState().setLeakId("leak-high", "leak");

    const view = renderPage();
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

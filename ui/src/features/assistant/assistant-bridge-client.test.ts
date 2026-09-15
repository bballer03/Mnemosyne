import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import {
  DEFAULT_HISTORY_MAX_TURNS,
  HARD_MAX_HISTORY_TURNS,
  OUTBOUND_METADATA_NOTICE,
  appendBoundedTurn,
  askWithProviderFallback,
  buildRulesModeAnswer,
  createAiSession,
  displayHeapBasename,
  isProviderChatAvailable,
  providerRecoveryGuidance,
  runProviderChat,
  type AssistantChatTurn,
  type AssistantSessionContext,
} from "./assistant-bridge-client";

afterEach(() => {
  delete window.__MNEMOSYNE_ASSISTANT_BRIDGE__;
});

describe("assistant-bridge-client path opacity", () => {
  it("exposes only basename from absolute-looking heap paths", () => {
    expect(displayHeapBasename("/var/heaps/prod/app.hprof")).toBe("app.hprof");
    expect(displayHeapBasename("C:\\\\Users\\\\me\\\\dumps\\\\leak.hprof")).toBe("leak.hprof");
    expect(displayHeapBasename("fixture.hprof")).toBe("fixture.hprof");
  });
});

describe("assistant-bridge-client rules mode", () => {
  it("builds a rules-mode answer from stable selection and measured findings", () => {
    const context: AssistantSessionContext = {
      heapDisplayName: "fixture.hprof",
      selection: {
        objectId: "0x10",
        classKey: "com.example.Cache",
        originPane: "inspector",
      },
      measuredFindings: [
        {
          id: "collection:0x10",
          kind: "collection-waste",
          severity: "WARNING",
          title: "Collection waste: java.util.HashMap",
          description: "HashMap uses 1024 slots for 3 entries.",
          target: {
            kind: "object",
            objectId: "0x10",
            classKey: "java.util.HashMap",
          },
          provenance: [{ kind: "Measured" }],
          metrics: { wasteBytes: 4084 },
        },
      ],
      totalObjects: 100,
    };

    const answer = buildRulesModeAnswer("What should I investigate first?", context);
    expect(answer.provenance).toBe("rules");
    expect(answer.model).toBe("rules");
    expect(answer.answerSummary).toMatch(/0x10/);
    expect(answer.answerSummary).toMatch(/com\.example\.Cache/);
    expect(answer.answerSummary).toMatch(/Collection waste: java\.util\.HashMap/);
    expect(answer.answerSummary).toMatch(/HashMap uses 1024 slots for 3 entries/);
    expect(answer.answerSummary).not.toMatch(/\/var\/heaps/);
    expect(answer.answerSummary).not.toMatch(/prior advisory text/i);
  });

  it("describes workflow kind and step without an internal id", () => {
    const answer = buildRulesModeAnswer("What next?", {
      heapDisplayName: "fixture.hprof",
      workflowKind: "Tune GC",
      workflowStep: "thread_local_review",
      selection: {},
      measuredFindings: [],
    });

    expect(answer.answerSummary).toContain("Tune GC");
    expect(answer.answerSummary).toContain("thread_local_review");
    expect(answer.answerSummary).not.toContain("wf-internal");
  });
});

describe("assistant-bridge-client history eviction", () => {
  it("evicts the oldest turns when appending past the 12-turn default", () => {
    let history: AssistantChatTurn[] = [];
    for (let i = 0; i < DEFAULT_HISTORY_MAX_TURNS; i++) {
      history = appendBoundedTurn(history, {
        question: `q${i}`,
        answerSummary: `a${i}`,
        provenance: "rules",
        model: "rules",
      });
    }

    expect(history).toHaveLength(12);
    expect(history[0]?.question).toBe("q0");

    history = appendBoundedTurn(history, {
      question: "q12",
      answerSummary: "a12",
      provenance: "rules",
      model: "rules",
    });

    expect(history).toHaveLength(12);
    expect(history[0]?.question).toBe("q1");
    expect(history[11]?.question).toBe("q12");
  });

  it("never retains more than the hard maximum of 32 even if a larger limit is requested", () => {
    let history: AssistantChatTurn[] = [];
    for (let i = 0; i < HARD_MAX_HISTORY_TURNS + 5; i++) {
      history = appendBoundedTurn(
        history,
        {
          question: `q${i}`,
          answerSummary: `a${i}`,
          provenance: "rules",
          model: "rules",
        },
        100,
      );
    }

    expect(history).toHaveLength(HARD_MAX_HISTORY_TURNS);
    expect(history[0]?.question).toBe("q5");
  });

  it("rejects NaN and non-integer limits instead of allowing unbounded growth", () => {
    let history: AssistantChatTurn[] = [];
    for (let i = 0; i < DEFAULT_HISTORY_MAX_TURNS + 3; i++) {
      history = appendBoundedTurn(
        history,
        {
          question: `q${i}`,
          answerSummary: `a${i}`,
          provenance: "rules",
          model: "rules",
        },
        Number.NaN,
      );
    }
    expect(history).toHaveLength(DEFAULT_HISTORY_MAX_TURNS);
  });
});

describe("assistant-bridge-client provider availability", () => {
  it("reports provider chat unavailable when the host bridge is missing", () => {
    expect(isProviderChatAvailable()).toBe(false);
  });

  it("returns unavailable for provider chat when the bridge method is absent", async () => {
    const result = await runProviderChat({
      sessionId: "sess-1",
      question: "Why is this retaining?",
    });
    expect(result).toEqual({ status: "unavailable" });
  });

  it("surfaces bridge errors without inventing a provider response", async () => {
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      chatSession: async () => {
        throw new Error("provider_timeout");
      },
    };

    const result = await runProviderChat({
      sessionId: "sess-1",
      question: "Why is this retaining?",
    });
    expect(result).toEqual({ status: "error", error: "provider_timeout" });
  });

  it("formats machine-readable recovery guidance for timeouts", () => {
    expect(providerRecoveryGuidance("provider_timeout")).toBe(
      "recovery=rules_mode_available; error=provider_timeout",
    );
  });

  it("redacts path- and secret-like fragments from recovery guidance", () => {
    const guidance = providerRecoveryGuidance(
      "failed at /home/user/secret/app.hprof with api_key=sk-live-ABCDEFGHIJKLMNOP",
    );
    expect(guidance).toContain("recovery=rules_mode_available");
    expect(guidance).not.toContain("/home/user");
    expect(guidance).not.toContain("sk-live");
    expect(guidance).toMatch(/\[redacted/);
  });
});

describe("assistant-bridge-client create + ask fallback", () => {
  const context: AssistantSessionContext = {
    heapDisplayName: "fixture.hprof",
    selection: { leakId: "leak-1" },
    measuredFindings: [],
    totalObjects: 10,
  };

  it("creates an opaque session and returns outbound metadata notice", async () => {
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      createAiSession: async () => ({
        session_id: "mcp-1",
        display_name: "fixture.hprof",
        outbound_metadata: {
          sends: ["heap_summary_stats", "focused_leak_id_class_severity_description"],
          never_sends: ["api_keys", "absolute_heap_paths"],
        },
      }),
      chatSession: async () => ({ summary: "ok", model: "rules" }),
    };

    const created = await createAiSession({ sourceId: "src-opaque" });
    expect(created.status).toBe("ready");
    if (created.status === "ready") {
      expect(created.data.sessionId).toBe("mcp-1");
      expect(created.data.outboundNotice).toMatch(/api_keys/i);
      expect(created.data.outboundNotice).not.toMatch(/sk-/i);
      expect(created.data.displayName).toBe("fixture.hprof");
    }
  });

  it("uses chatSession with provider provenance when the bridge succeeds", async () => {
    let chatCalls = 0;
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      createAiSession: async () => ({ session_id: "mcp-2", display_name: "fixture.hprof" }),
      chatSession: async (input) => {
        chatCalls += 1;
        expect(input.sessionId).toBe("mcp-2");
        expect(input.focusLeakId).toBe("leak-1");
        return { summary: "Provider guidance about leak-1.", model: "gpt-test" };
      },
    };

    const result = await askWithProviderFallback({
      question: "What next?",
      context,
    });

    expect(chatCalls).toBe(1);
    expect(result.turn.provenance).toBe("provider");
    expect(result.turn.model).toBe("gpt-test");
    expect(result.turn.answerSummary).toMatch(/Provider guidance/);
    expect(result.sessionId).toBe("mcp-2");
    expect(result.outboundNotice ?? OUTBOUND_METADATA_NOTICE).toBeTruthy();
  });

  it("falls back to rules with recovery guidance on provider error", async () => {
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      createAiSession: async () => ({ session_id: "mcp-3" }),
      chatSession: async () => {
        throw new Error("provider_timeout");
      },
    };

    const result = await askWithProviderFallback({
      question: "What next?",
      context,
      sessionId: "mcp-3",
    });

    expect(result.turn.provenance).toBe("fallback");
    expect(result.turn.model).toBe("rules");
    expect(result.notice).toBe("recovery=rules_mode_available; error=provider_timeout");
    expect(result.turn.answerSummary).not.toMatch(/sk-/i);
  });

  it("rejects summaries that look like API keys", async () => {
    window.__MNEMOSYNE_ASSISTANT_BRIDGE__ = {
      chatSession: async () => ({
        summary: "use key sk-abcdefghijklmnopqrstuvwxyz",
        model: "gpt-test",
      }),
    };

    const result = await runProviderChat({
      sessionId: "mcp-4",
      question: "leak?",
    });
    expect(result.status).toBe("error");
  });
});

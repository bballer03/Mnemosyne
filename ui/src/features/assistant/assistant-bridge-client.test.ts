import "../../test/setup";

import { afterEach, describe, expect, it } from "bun:test";

import {
  DEFAULT_HISTORY_MAX_TURNS,
  HARD_MAX_HISTORY_TURNS,
  appendBoundedTurn,
  buildRulesModeAnswer,
  displayHeapBasename,
  isProviderChatAvailable,
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
  it("builds a rules-mode answer from measured focus context without a host bridge", () => {
    const context: AssistantSessionContext = {
      heapDisplayName: "fixture.hprof",
      focusLeakId: "leak-cache-1",
      focusLeakClassName: "com.example.Cache",
      focusLeakSeverity: "HIGH",
      focusLeakDescription: "Cache retaining 12 MB",
      totalObjects: 100,
    };

    const answer = buildRulesModeAnswer("What should I investigate first?", context);
    expect(answer.provenance).toBe("rules");
    expect(answer.model).toBe("rules");
    expect(answer.answerSummary).toMatch(/leak-cache-1|com\.example\.Cache|Cache retaining/i);
    expect(answer.answerSummary).not.toMatch(/\/var\/heaps/);
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
});

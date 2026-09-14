// M23.A investigation-session bridge helpers.
//
// Compose shipped AI-session contracts (`AiChatTurn`, 12/32 history bounds from
// `core::mcp::session`) for the React workspace. Does not invent a new AI
// provider: rules-mode answers are composed from already-loaded measured facts;
// optional `__MNEMOSYNE_ASSISTANT_BRIDGE__.chatSession` is reserved for 23.C
// Tauri wiring over MCP `chat_session`.

export const DEFAULT_HISTORY_MAX_TURNS = 12;
export const HARD_MAX_HISTORY_TURNS = 32;

export type AssistantProvenance = "rules" | "provider" | "fallback";

export type AssistantChatTurn = {
  question: string;
  answerSummary: string;
  provenance: AssistantProvenance;
  model: string;
};

export type AssistantSessionContext = {
  heapDisplayName: string;
  sourceId?: string;
  workflowId?: string;
  workflowStep?: string;
  focusLeakId?: string;
  focusLeakClassName?: string;
  focusLeakSeverity?: string;
  focusLeakDescription?: string;
  focusObjectId?: string;
  totalObjects?: number;
};

export type AssistantHostBridge = {
  chatSession?: (input: {
    sessionId: string;
    question: string;
    focusLeakId?: string;
  }) => Promise<unknown>;
  getAiSession?: (sessionId: string) => Promise<unknown>;
};

declare global {
  interface Window {
    __MNEMOSYNE_ASSISTANT_BRIDGE__?: AssistantHostBridge;
  }
}

export type AssistantBridgeResult<T> =
  | { status: "unavailable" }
  | { status: "ready"; data: T }
  | { status: "error"; error: string };

export function displayHeapBasename(pathOrName: string): string {
  const parts = pathOrName.split(/[/\\]/);
  return parts[parts.length - 1] || pathOrName;
}

export function effectiveHistoryLimit(configured?: number): number {
  if (configured === undefined) {
    return DEFAULT_HISTORY_MAX_TURNS;
  }
  if (!Number.isFinite(configured) || !Number.isInteger(configured)) {
    return DEFAULT_HISTORY_MAX_TURNS;
  }
  if (configured < 1) {
    return 1;
  }
  return Math.min(configured, HARD_MAX_HISTORY_TURNS);
}

/** Opaque desktop source ids must not look like filesystem paths. */
export function isOpaqueSourceId(sourceId: string): boolean {
  const trimmed = sourceId.trim();
  if (!trimmed) {
    return false;
  }
  if (trimmed.includes("/") || trimmed.includes("\\") || trimmed.includes("..")) {
    return false;
  }
  if (/^[A-Za-z]:/.test(trimmed)) {
    return false;
  }
  return true;
}

export function appendBoundedTurn(
  history: AssistantChatTurn[],
  turn: AssistantChatTurn,
  maxTurns: number = DEFAULT_HISTORY_MAX_TURNS,
): AssistantChatTurn[] {
  const limit = effectiveHistoryLimit(maxTurns);
  const next = [...history, turn];
  if (next.length <= limit) {
    return next;
  }
  return next.slice(next.length - limit);
}

export function buildRulesModeAnswer(
  question: string,
  context: AssistantSessionContext,
): AssistantChatTurn {
  const heap = displayHeapBasename(context.heapDisplayName);
  const focusParts: string[] = [];

  if (context.focusLeakId) {
    focusParts.push(`focused leak ${context.focusLeakId}`);
  }
  if (context.focusLeakClassName) {
    focusParts.push(context.focusLeakClassName);
  }
  if (context.focusLeakSeverity) {
    focusParts.push(`severity ${context.focusLeakSeverity}`);
  }
  if (context.focusLeakDescription) {
    focusParts.push(context.focusLeakDescription);
  }
  if (context.focusObjectId) {
    focusParts.push(`object ${context.focusObjectId}`);
  }

  const focusLine =
    focusParts.length > 0
      ? focusParts.join(" — ")
      : "no focused leak yet; open Dashboard or run a triage workflow first";

  const objectLine =
    typeof context.totalObjects === "number"
      ? ` Measured heap ${heap} reports ${context.totalObjects} objects.`
      : ` Measured heap ${heap}.`;

  const workflowLine = context.workflowId
    ? ` Active workflow ${context.workflowId}${
        context.workflowStep ? ` at step ${context.workflowStep}` : ""
      }.`
    : "";

  return {
    question,
    answerSummary: `Rules-mode guidance (offline): investigate ${focusLine}.${objectLine}${workflowLine} Use the deterministic workbench links for GC paths, dominators, and inspector views — this text is advisory, not a measured fact.`,
    provenance: "rules",
    model: "rules",
  };
}

function getAssistantBridge(): AssistantHostBridge | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  return window.__MNEMOSYNE_ASSISTANT_BRIDGE__;
}

export function isProviderChatAvailable(): boolean {
  return typeof getAssistantBridge()?.chatSession === "function";
}

export async function runProviderChat(input: {
  sessionId: string;
  question: string;
  focusLeakId?: string;
}): Promise<AssistantBridgeResult<{ summary: string; model: string }>> {
  const bridge = getAssistantBridge();
  if (!bridge?.chatSession) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.chatSession(input);
    if (typeof raw !== "object" || raw === null) {
      throw new TypeError("Invalid assistant bridge payload: chatSession result must be an object.");
    }
    const record = raw as Record<string, unknown>;
    const summary =
      typeof record.summary === "string"
        ? record.summary
        : typeof record.answer_summary === "string"
          ? record.answer_summary
          : undefined;
    const model = typeof record.model === "string" ? record.model : "provider";
    if (!summary) {
      throw new TypeError("Invalid assistant bridge payload: expected summary string.");
    }
    return { status: "ready", data: { summary, model } };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown chatSession bridge failure.",
    };
  }
}

// M23 investigation-session bridge helpers.
//
// Compose shipped AI-session contracts (`AiChatTurn`, 12/32 history bounds from
// `core::mcp::session`) for the React workspace. Rules-mode answers are composed
// locally from measured facts. Optional `__MNEMOSYNE_ASSISTANT_BRIDGE__` wires
// Tauri create/resume/get/close/chat over MCP `chat_session` semantics (M23.C).

export const DEFAULT_HISTORY_MAX_TURNS = 12;
export const HARD_MAX_HISTORY_TURNS = 32;

/** What provider outbound calls may include — never absolute paths or API keys. */
export const OUTBOUND_METADATA_NOTICE =
  "Provider outbound metadata (when enabled): heap summary stats, focused leak id/class/severity/description, and bounded chat history. Never sent: API keys, absolute heap paths, or raw heap field values.";

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
  createAiSession?: (input?: { sourceId?: string }) => Promise<unknown>;
  resumeAiSession?: (sessionId: string) => Promise<unknown>;
  getAiSession?: (sessionId: string) => Promise<unknown>;
  closeAiSession?: (sessionId: string) => Promise<unknown>;
  chatSession?: (input: {
    sessionId: string;
    question: string;
    focusLeakId?: string;
  }) => Promise<unknown>;
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

export type CreatedAiSession = {
  sessionId: string;
  displayName?: string;
  outboundNotice: string;
};

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

function readSessionId(raw: unknown): string | undefined {
  if (typeof raw !== "object" || raw === null) {
    return undefined;
  }
  const record = raw as Record<string, unknown>;
  if (typeof record.session_id === "string" && record.session_id.trim()) {
    return record.session_id;
  }
  if (typeof record.sessionId === "string" && record.sessionId.trim()) {
    return record.sessionId;
  }
  return undefined;
}

function formatOutboundNotice(raw: unknown): string {
  if (typeof raw !== "object" || raw === null) {
    return OUTBOUND_METADATA_NOTICE;
  }
  const meta = (raw as Record<string, unknown>).outbound_metadata;
  if (typeof meta !== "object" || meta === null) {
    return OUTBOUND_METADATA_NOTICE;
  }
  const record = meta as Record<string, unknown>;
  const sends = Array.isArray(record.sends)
    ? record.sends.filter((item): item is string => typeof item === "string")
    : [];
  const never = Array.isArray(record.never_sends)
    ? record.never_sends.filter((item): item is string => typeof item === "string")
    : [];
  if (sends.length === 0 && never.length === 0) {
    return OUTBOUND_METADATA_NOTICE;
  }
  return `Provider outbound metadata: sends ${sends.join(", ") || "(see notice)"}. Never sends: ${
    never.join(", ") || "API keys, absolute heap paths, raw heap field values"
  }.`;
}

/** Machine-readable recovery hint for provider/timeout failures. */
export function providerRecoveryGuidance(error: string): string {
  const normalized = error.trim() || "provider_error";
  return `recovery=rules_mode_available; error=${normalized}`;
}

export async function createAiSession(input?: {
  sourceId?: string;
}): Promise<AssistantBridgeResult<CreatedAiSession>> {
  const bridge = getAssistantBridge();
  if (!bridge?.createAiSession) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.createAiSession(input);
    const sessionId = readSessionId(raw);
    if (!sessionId) {
      throw new TypeError("Invalid assistant bridge payload: expected session_id.");
    }
    const record =
      typeof raw === "object" && raw !== null ? (raw as Record<string, unknown>) : {};
    const displayName =
      typeof record.display_name === "string"
        ? record.display_name
        : typeof record.displayName === "string"
          ? record.displayName
          : undefined;
    return {
      status: "ready",
      data: {
        sessionId,
        displayName,
        outboundNotice: formatOutboundNotice(raw),
      },
    };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown createAiSession bridge failure.",
    };
  }
}

export async function closeAiSession(
  sessionId: string,
): Promise<AssistantBridgeResult<{ sessionId: string; closed: boolean }>> {
  const bridge = getAssistantBridge();
  if (!bridge?.closeAiSession) {
    return { status: "unavailable" };
  }

  try {
    const raw = await bridge.closeAiSession(sessionId);
    if (typeof raw !== "object" || raw === null) {
      throw new TypeError("Invalid assistant bridge payload: closeAiSession result must be an object.");
    }
    return { status: "ready", data: { sessionId, closed: true } };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown closeAiSession bridge failure.",
    };
  }
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
    // Never surface wire/prompt bodies or anything that looks like an API key.
    if (typeof summary === "string" && /(?:api[_-]?key|sk-[A-Za-z0-9])/i.test(summary)) {
      throw new TypeError("Provider response redacted: summary must not contain secrets.");
    }
    return { status: "ready", data: { summary, model } };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown chatSession bridge failure.",
    };
  }
}

/**
 * Ask via chatSession when the host bridge is present; fall back to local rules
 * on unavailable/error. Always keeps 12/32 history bounds at the caller.
 */
export async function askWithProviderFallback(input: {
  question: string;
  context: AssistantSessionContext;
  sessionId?: string;
  sourceId?: string;
}): Promise<{
  turn: AssistantChatTurn;
  sessionId?: string;
  notice?: string;
  outboundNotice?: string;
}> {
  if (!isProviderChatAvailable()) {
    return { turn: buildRulesModeAnswer(input.question, input.context) };
  }

  let sessionId = input.sessionId;
  let outboundNotice: string | undefined;

  if (!sessionId) {
    const created = await createAiSession({
      sourceId: input.sourceId ?? input.context.sourceId,
    });
    if (created.status === "ready") {
      sessionId = created.data.sessionId;
      outboundNotice = created.data.outboundNotice;
    } else if (created.status === "unavailable") {
      return {
        turn: {
          ...buildRulesModeAnswer(input.question, input.context),
          provenance: "fallback",
        },
        notice: providerRecoveryGuidance("create_ai_session_unavailable"),
      };
    } else {
      return {
        turn: {
          ...buildRulesModeAnswer(input.question, input.context),
          provenance: "fallback",
        },
        notice: providerRecoveryGuidance(created.error),
      };
    }
  }

  const result = await runProviderChat({
    sessionId: sessionId!,
    question: input.question,
    focusLeakId: input.context.focusLeakId,
  });

  if (result.status === "ready") {
    return {
      turn: {
        question: input.question,
        answerSummary: result.data.summary,
        provenance: "provider",
        model: result.data.model,
      },
      sessionId,
      outboundNotice,
    };
  }

  const error =
    result.status === "error" ? result.error : "chat_session_unavailable";
  return {
    turn: {
      ...buildRulesModeAnswer(input.question, input.context),
      provenance: "fallback",
    },
    sessionId,
    notice: providerRecoveryGuidance(error),
    outboundNotice,
  };
}

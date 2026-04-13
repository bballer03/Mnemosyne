# Milestone 5 — AI / MCP / Differentiation

> **Status:** ✅ Complete for the approved milestone scope — provider/privacy hardening, CLI-first conversation, persisted MCP AI sessions, the first AI-backed fix-generation slice, and evidence-first request/response MCP hardening are shipped; broader conversation semantics, native local-provider transports beyond OpenAI-compatible endpoints, and transport streaming remain post-M5 follow-on only if later evidence justifies them
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-04-13

---

## Objective

Wire real AI capabilities into Mnemosyne and make MCP integration production-ready, transforming the tool from a heap analyzer with deterministic stubs into a genuine AI-powered memory debugging copilot.

## Context

AI-assisted analysis is Mnemosyne's key differentiator. The architecture is now realized for the approved milestone scope: `AiConfig` supports `rules`, `stub`, and `provider` modes, `AiInsights` and `AiWireExchange` remain the stable contracts, provider-backed execution is verified for OpenAI-compatible cloud and local endpoints plus Anthropic endpoints, the stdio MCP server exposes explicit AI session lifecycle methods on top of the analysis surface with `list_tools` discovery plus structured `error_details`, Step `14(d)` covers provider-mode prompt redaction plus hashed audit logging via `[ai.privacy]` before external calls, and Step `14(e)` ships both a CLI-first `mnemosyne-cli chat <heap.hprof>` slice with bounded in-process history and persisted MCP AI sessions for resumed `chat_session` / `explain_leak` / `propose_fix` follow-up. Evidence-first transport hardening verifies delayed AI-backed responses and larger single-response payloads against the existing stdio request/response model while exposing dedicated provider timeout and provider failure error codes.

The remaining work is post-M5 follow-on rather than unfinished core milestone scope: broader conversation/exploration semantics, native local-provider transports beyond OpenAI-compatible endpoints, and response streaming only if future validation shows the current request/response contract is insufficient.

M1.5 must complete before wiring AI to analysis results — sending empty/heuristic data to an LLM produces misleading output. M3 enriches the analysis context that makes AI insights valuable.

## Scope

### LLM Integration
1. **HTTP client + completion transport layer** — request/response provider support for multiple backends
2. **OpenAI backend** — GPT-4/GPT-4o implementation
3. **Anthropic backend** — Claude implementation
4. **Local model support** — OpenAI-compatible local endpoint support for self-hosted or offline-friendly deployments within the approved scope
5. **Configurable prompt templates** — YAML-defined prompts with context injection

### AI-Powered Features
6. **Real AI insights** — `generate_ai_insights()` calls a real LLM with heap analysis context
7. **AI-driven leak explanations** — pass retained-size data + reference chains to LLM
8. **AI-driven fix suggestions** — LLM-generated context-aware code patches (replacing templates)
9. **Conversation mode** — interactive Q&A about a heap dump, delivered as a CLI-first slice plus persisted MCP AI session follow-up

### MCP Hardening
10. **Tool descriptions** — proper MCP tool metadata for IDE discovery
11. **Transport streaming follow-on** — only if tests show the current request/response transport is insufficient
12. **Error contracts** — structured MCP error responses with error codes
13. **Session management** — maintain analysis context across multiple MCP calls

### Privacy & Safety
14. **Data redaction** — configurable rules for stripping sensitive data before LLM calls
15. **Token budget management** — truncation strategy for large heap contexts
16. **Audit logging** — record hashed metadata for the redacted outbound prompt sent to external LLMs

## Non-scope

- Core analysis algorithm changes (M3)
- Web UI for AI interaction (M4)
- Fine-tuned models or model training
- Self-hosted LLM infrastructure recommendations
- Changes to the 5 existing report formats
- New analysis features (M3)

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────────┐
│                    CLI / MCP                                       │
│                                                                    │
│  mnemosyne analyze --ai    MCP: explain_leak / propose_fix        │
│  mnemosyne explain         MCP: session-based conversation / fix   │
│  mnemosyne chat                                                    │
└──────────────┬─────────────────────────────────────────────────────┘
               │
┌──────────────┼─────────────────────────────────────────────────────┐
│              ▼    AI ORCHESTRATION LAYER (new)                     │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │  Prompt/Template Layer                                      │  │
│  │                                                             │  │
│  │  Embedded default YAML templates plus optional              │  │
│  │  `[ai.prompts].template_dir` override files drive           │  │
│  │  provider instructions, leak explanations, fix prompts,     │  │
│  │  and conversation context shaping.                          │  │
│  └──────────────┬──────────────────────────────────────────────┘  │
│                 │                                                   │
│  ┌──────────────▼──────────────────────────────────────────────┐  │
│  │  LLM Completion Layer (core/src/llm.rs)                     │  │
│  │                                                             │  │
│  │  LlmCompletionRequest                                       │  │
│  │      -> complete()                                          │  │
│  │      -> LlmCompletionResponse                               │  │
│  │                                                             │  │
│  │  ┌──────────────┐  ┌───────────┐  ┌──────────────────────┐  │  │
│  │  │ OpenAI-compat│  │ Anthropic │  │ OpenAI-compat local  │  │  │
│  │  │ cloud endpoint│ │ messages  │  │ endpoint via         │  │  │
│  │  │ (blocking)    │ │ (blocking)│  │ `ai.endpoint`        │  │  │
│  │  └──────────────┘  └───────────┘  └──────────────────────┘  │  │
│  └──────────────┬──────────────────────────────────────────────┘  │
│                 │                                                   │
│  ┌──────────────▼──────────────────────────────────────────────┐  │
│  │  Privacy / Redaction Layer                                  │  │
│  │  • Strip string values matching configurable patterns       │  │
│  │  • Truncate to token budget                                 │  │
│  │  • Audit log: hash of data sent, model, timestamp           │  │
│  └─────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
               │
┌──────────────┼─────────────────────────────────────────────────────┐
│              ▼    EXISTING CORE (unchanged)                        │
│  hprof/ │ graph/ │ analysis/ │ report/ │ mcp/ │ ...               │
└────────────────────────────────────────────────────────────────────┘
```

## Module/File Impact

| File | Change Type | Description |
|---|---|---|
| `core/src/llm.rs` | New | Shipped blocking completion transport for OpenAI-compatible and Anthropic execution, with OpenAI-compatible local endpoint support via `ai.endpoint` |
| `core/src/analysis/ai.rs` | Rewritten | Wire `generate_ai_insights()` and chat turns to `rules`, `stub`, or provider-backed completion |
| `core/src/fix/generator.rs` | Enhanced | Provider-backed one-file / one-snippet fix generation with heuristic fallback |
| `core/src/mcp/server.rs` | Enhanced | Error contracts, request/response hardening, and AI-session-aware MCP handlers |
| `core/src/mcp/session.rs` | New | Persisted MCP AI session storage and lifecycle support |
| `core/src/config.rs` | Enhanced | Privacy/redaction config, prompt-template override settings, provider/session config |
| `cli/src/config_loader.rs` | Enhanced | Provider/session config loading and prompt-template override plumbing |
| `core/Cargo.toml` | Updated | reqwest and serde_yaml deps for provider transport and prompt templates |
| `cli/src/main.rs` | Updated | `--ai` flag wiring, `chat` subcommand |

## API/CLI/Reporting Impact

### Changed CLI Commands
- `mnemosyne analyze --ai` — now calls real LLM (previously: deterministic stub)
- `mnemosyne explain --leak-id ID` — LLM-generated explanation with heap context

### New CLI Commands
- `mnemosyne chat <heap.hprof>` — CLI-only first slice of conversation mode: analyze once, print the top 3 leak candidates, and support `/focus <leak-id>`, `/list`, `/help`, and `/exit` with bounded in-process history

### Changed MCP Handlers
- `explain_leak` — returns LLM-generated explanation (previously: template text)
- `propose_fix` — now attempts provider-backed one-file / one-snippet fix generation when source context is available, otherwise falls back to heuristic patches with explicit provenance

### Post-M5 Follow-On Only
- Broader conversation/exploration semantics beyond the shipped leak-focused chat plus persisted MCP session follow-up
- Native local-provider transports beyond the shipped OpenAI-compatible local endpoint path
- Response streaming only if future validation shows the current request/response transport is insufficient
- Broader AI-specific error-code coverage only when additional provider failure classes prove necessary

### Config Changes
```toml
[ai]
provider = "openai"          # openai | anthropic | local
enabled = true                # default runtime value is false
mode = "provider"            # default runtime value is rules
model = "gpt-4o"
temperature = 0.3
api_key_env = "OPENAI_API_KEY"
endpoint = "http://localhost:11434/v1"  # optional: OpenAI-compatible local endpoint
max_tokens = 4096

[ai.privacy]
redact_heap_path = true
redact_patterns = ["\\b\\d{16}\\b", "password=.*"]  # regex patterns to strip from the outbound provider prompt
audit_log = true

[ai.prompts]
template_dir = "~/.config/mnemosyne/prompts"  # custom prompt overrides
```

Current runtime note: `redact_heap_path`, `redact_patterns`, and provider-mode `audit_log` are implemented, `max_tokens` now acts as a minimal prompt-budget guard for provider-mode leak context, `mnemosyne-cli chat` reuses that same provider/privacy path, and MCP now persists explicit heap-bound AI sessions with `create_ai_session`, `resume_ai_session`, `get_ai_session`, `close_ai_session`, and `chat_session`. Broader token-accounting, native non-OpenAI-compatible local transports, and transport streaming remain post-M5 follow-on work.

## Data Model Changes

### New Types (core::llm)
- `LlmCompletionRequest` — prompt text plus `AiConfig`
- `LlmCompletionResponse` — provider response text

### Current runtime boundaries
- `complete()` is the shipped entrypoint and returns one blocking request/response completion.
- OpenAI-compatible cloud and local providers share the chat-completions path; Anthropic uses the messages API path.
- Provider failures surface through shared `CoreError` variants rather than a dedicated `LlmError` type.
- Trait-based provider abstractions, streaming chunks, and richer transport-specific model types are post-M5 follow-on only.

### Prompt/template runtime notes
- Embedded default YAML templates plus optional `[ai.prompts].template_dir` overrides drive prompt rendering in the shipped implementation.
- These templates are runtime assets rather than a dedicated prompts source directory in the current tree.

### Updated Types
- `AiInsights` — now populated from real LLM response (previously: template text)
- `AiWireExchange` — now contains real prompt/response pairs
- `AiConfig` — add privacy, prompt template, and provider-specific fields

### Preserved Types
- `ProvenanceKind::Placeholder` — continues to mark stub/mock data in test/offline mode

## Validation/Testing Strategy

### Unit Tests
- LLM completion transport: OpenAI-compatible and Anthropic request/response handling returns expected content
- Prompt template engine: YAML parsing, variable injection, context preparation
- Token budget: truncation respects budget, prioritizes by retained size
- Privacy redaction: patterns strip correctly, audit log records hashes
- Error handling: provider unavailable, token limit exceeded, invalid response

### Integration Tests
- End-to-end with mock provider: `analyze --ai` produces AI insights section
- MCP explain/propose with mock provider: responses include LLM-generated content
- Configuration: API key loading from env, provider selection, prompt override
- Request/response hardening: delayed AI-backed responses and larger single-response payloads stay correct over the shipped stdio transport

### Contract Tests
- AI responses include provenance markers (no marker = real LLM, ProvenanceKind::Placeholder = stub/offline)
- TOON wire format for AI exchanges preserved
- MCP error codes for AI failures are structured and documented

Remaining follow-on validation should focus on broader conversation grounding, native local-provider transports, and streaming only if future evidence justifies it.

## Rollout / Milestone State

### Shipped milestone slices
1. **LLM/provider transport:** shipped for `rules`, `stub`, and provider-backed execution through the current completion layer
2. **Prompt engine:** shipped with embedded YAML templates, optional override files, context preparation, and the current prompt-budget guard
3. **AI features:** shipped for AI insights, leak explanations, the first AI-backed fix-generation slice, and CLI-first conversation
4. **MCP hardening:** shipped for request/response transport hardening, structured error codes, persisted session management, and tool descriptions
5. **Privacy and provider scope:** shipped for prompt redaction, hashed audit logging, Anthropic support, and OpenAI-compatible local endpoints

### Post-M5 follow-on only
1. **Broader conversation/exploration semantics** beyond the shipped leak-focused/session-backed surfaces
2. **Native local-provider transports** beyond OpenAI-compatible local endpoints
3. **Transport streaming** only if future evidence proves the current contract insufficient

## Risks and Open Questions

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **M1.5 + M3 must complete first** | Certain | Blocking | Without real analysis data, AI insights are meaningless |
| LLM response quality may be inconsistent | High | Medium | Structured prompts, response validation, fallback to template |
| API cost for OpenAI/Anthropic may deter users | Medium | Medium | Token budget management; local model as free alternative |
| Privacy concerns with sending heap data to cloud LLMs | High | High | Configurable redaction, audit logging, local model option |
| Prompt engineering requires iteration | High | Medium | YAML templates allow non-code iteration; A/B prompt testing |
| reqwest dependency adds compile time and binary size | Medium | Low | Feature-gate AI behind `ai` cargo feature |

### Open Questions
1. Should AI be feature-gated behind a cargo feature? (Recommendation: yes, `--features ai`)
2. Should prompt templates ship in the binary or as external files? (Recommendation: embed defaults, allow overrides)
3. What token budget for heap context? (Recommendation: 2048 tokens for context, 2048 for response)
4. Should conversation mode support file context (source code alongside heap)? (Recommendation: yes, with opt-in)
5. How to handle LLM hallucinations about heap data? (Recommendation: cross-reference AI claims against actual ObjectGraph data)

### Dependencies
- **Blocked by:** M1.5 (real data for meaningful AI input), M3 (richer analysis context)
- **Blocks:** M6 (AI features are a key differentiator for community adoption)

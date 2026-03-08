# Milestone 5 — AI / MCP / Differentiation

> **Status:** ⚬ Pending  
> **Design Owner:** Design Consulting Agent  
> **Last Updated:** 2026-03-08

---

## Objective

Wire real AI capabilities into Mnemosyne and make MCP integration production-ready, transforming the tool from a heap analyzer with deterministic stubs into a genuine AI-powered memory debugging copilot.

## Context

AI-assisted analysis is Mnemosyne's key differentiator. No other heap analysis tool provides LLM-backed explanations, AI-generated fix suggestions, or natural-language Q&A about heap dumps. The architecture is ready: `AiConfig` with provider/model/temperature fields exists, `AiInsights` and `AiWireExchange` types are defined, the `generate_ai_insights()` function has a clean interface, and the MCP server exposes 7 JSON-RPC handlers. But today, all AI calls terminate in a deterministic stub that returns template text with zero LLM calls and zero HTTP client dependencies.

M1.5 must complete before wiring AI to analysis results — sending empty/heuristic data to an LLM produces misleading output. M3 enriches the analysis context that makes AI insights valuable.

## Scope

### LLM Integration
1. **HTTP client + LLM abstraction layer** — trait-based interface supporting multiple providers
2. **OpenAI backend** — GPT-4/GPT-4o implementation
3. **Anthropic backend** — Claude implementation
4. **Local model support** — llama.cpp or Ollama integration for offline use
5. **Configurable prompt templates** — YAML-defined prompts with context injection

### AI-Powered Features
6. **Real AI insights** — `generate_ai_insights()` calls a real LLM with heap analysis context
7. **AI-driven leak explanations** — pass retained-size data + reference chains to LLM
8. **AI-driven fix suggestions** — LLM-generated context-aware code patches (replacing templates)
9. **Conversation mode** — interactive Q&A about a heap dump via CLI or MCP

### MCP Hardening
10. **Tool descriptions** — proper MCP tool metadata for IDE discovery
11. **Streaming responses** — progressive result delivery for long-running analysis
12. **Error contracts** — structured MCP error responses with error codes
13. **Session management** — maintain analysis context across multiple MCP calls

### Privacy & Safety
14. **Data redaction** — configurable rules for stripping sensitive data before LLM calls
15. **Token budget management** — truncation strategy for large heap contexts
16. **Audit logging** — log what data was sent to external LLMs

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
│  mnemosyne explain         MCP: session-based conversation        │
│  mnemosyne chat                                                    │
└──────────────┬─────────────────────────────────────────────────────┘
               │
┌──────────────┼─────────────────────────────────────────────────────┐
│              ▼    AI ORCHESTRATION LAYER (new)                     │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │  Prompt Engine (core/src/prompts/)                          │  │
│  │                                                             │  │
│  │  YAML templates:                                            │  │
│  │    leak_explanation.yaml  ─── context: retained sizes,      │  │
│  │    fix_suggestion.yaml        reference chains, class info   │  │
│  │    heap_summary.yaml     ─── truncation budget management   │  │
│  │    conversation.yaml     ─── multi-turn context window      │  │
│  └──────────────┬──────────────────────────────────────────────┘  │
│                 │                                                   │
│  ┌──────────────▼──────────────────────────────────────────────┐  │
│  │  LLM Abstraction (core/src/llm/)                            │  │
│  │                                                             │  │
│  │  trait LlmProvider {                                        │  │
│  │    async fn complete(prompt, config) -> LlmResponse;        │  │
│  │    async fn stream(prompt, config) -> Stream<LlmChunk>;     │  │
│  │  }                                                          │  │
│  │                                                             │  │
│  │  ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌──────────┐  │  │
│  │  │  OpenAI  │  │ Anthropic │  │  Ollama  │  │  Mock    │  │  │
│  │  │ (reqwest)│  │ (reqwest) │  │ (local)  │  │ (test)   │  │  │
│  │  └──────────┘  └───────────┘  └──────────┘  └──────────┘  │  │
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
| `core/src/llm/mod.rs` | New | LLM abstraction: `LlmProvider` trait, `LlmResponse`, `LlmConfig` |
| `core/src/llm/openai.rs` | New | OpenAI backend (reqwest HTTP client) |
| `core/src/llm/anthropic.rs` | New | Anthropic backend |
| `core/src/llm/ollama.rs` | New | Ollama/local model backend |
| `core/src/llm/mock.rs` | New | Mock provider for testing (replaces current stub) |
| `core/src/prompts/mod.rs` | New | Prompt template engine |
| `core/src/prompts/*.yaml` | New | YAML prompt templates |
| `core/src/analysis/ai.rs` | Rewritten | Wire to real LLM via `LlmProvider` trait |
| `core/src/mcp/server.rs` | Enhanced | Streaming, error contracts, session management |
| `core/src/config.rs` | Enhanced | Privacy/redaction config, prompt template paths |
| `core/Cargo.toml` | Updated | reqwest, serde_yaml, tiktoken deps |
| `cli/src/main.rs` | Updated | `--ai` flag wiring, `chat` subcommand |

## API/CLI/Reporting Impact

### Changed CLI Commands
- `mnemosyne analyze --ai` — now calls real LLM (previously: deterministic stub)
- `mnemosyne explain --leak-id ID` — LLM-generated explanation with heap context

### New CLI Commands
- `mnemosyne chat <heap.hprof>` — interactive conversation mode about a heap dump

### Changed MCP Handlers
- `explain_leak` — returns LLM-generated explanation (previously: template text)
- `propose_fix` — returns LLM-generated code patches (previously: template patches)

### New MCP Capabilities
- Streaming responses for long-running AI calls
- Session context: subsequent MCP calls share analysis state
- Error codes for AI-specific failures (provider unavailable, token limit, etc.)

### Config Changes
```toml
[ai]
provider = "openai"          # openai | anthropic | ollama | mock
model = "gpt-4o"
temperature = 0.3
api_key_env = "OPENAI_API_KEY"
max_tokens = 4096

[ai.privacy]
redact_patterns = ["\\b\\d{16}\\b", "password=.*"]  # regex patterns to strip
redact_string_values = true    # replace string object values with <REDACTED>
audit_log = true               # log hashes of data sent to LLM

[ai.prompts]
template_dir = "~/.config/mnemosyne/prompts"  # custom prompt overrides
```

## Data Model Changes

### New Types (core::llm)
- `LlmProvider` (trait) — `complete()`, `stream()`, `name()`, `supports_streaming()`
- `LlmRequest` — prompt text, system message, model config, max tokens
- `LlmResponse` — response text, model used, token usage, latency
- `LlmChunk` — streaming response chunk
- `LlmError` — provider-specific error types

### New Types (core::prompts)
- `PromptTemplate` — YAML-defined template with variables and context injection rules
- `PromptContext` — heap data context prepared for injection (retained sizes, top leaks, reference chains)
- `TokenBudget` — max tokens, truncation strategy (top-N, by-retained-size, etc.)

### Updated Types
- `AiInsights` — now populated from real LLM response (previously: template text)
- `AiWireExchange` — now contains real prompt/response pairs
- `AiConfig` — add privacy, prompt template, and provider-specific fields

### Preserved Types
- `ProvenanceKind::Placeholder` — continues to mark stub/mock data in test/offline mode

## Validation/Testing Strategy

### Unit Tests
- LLM provider trait: mock provider returns expected responses
- Prompt template engine: YAML parsing, variable injection, context preparation
- Token budget: truncation respects budget, prioritizes by retained size
- Privacy redaction: patterns strip correctly, audit log records hashes
- Error handling: provider unavailable, token limit exceeded, invalid response

### Integration Tests
- End-to-end with mock provider: `analyze --ai` produces AI insights section
- MCP explain/propose with mock provider: responses include LLM-generated content
- Configuration: API key loading from env, provider selection, prompt override
- Streaming: MCP streaming response delivers chunks

### Contract Tests
- AI responses include provenance markers (no marker = real LLM, ProvenanceKind::Placeholder = stub/offline)
- TOON wire format for AI exchanges preserved
- MCP error codes for AI failures are structured and documented

### Manual Testing Checklist
- [ ] `analyze --ai` with OpenAI API key produces meaningful insights
- [ ] `explain` with real leak ID produces context-aware explanation
- [ ] `chat` mode allows multi-turn conversation
- [ ] Ollama/local model fallback works offline
- [ ] Privacy redaction strips sensitive strings before LLM call
- [ ] Token budget prevents oversized prompts

## Rollout/Implementation Phases

### Phase 1 — LLM Abstraction (effort: Large)
1. Define `LlmProvider` trait
2. Implement mock provider (replaces current stub)
3. Implement OpenAI provider (reqwest + async)
4. Update `AiConfig` with provider selection

### Phase 2 — Prompt Engine (effort: Large)
5. YAML prompt template parser
6. Context preparation (extract top leaks, retained sizes, reference chains)
7. Token budget management (truncation strategy)
8. Wire `generate_ai_insights()` to prompt engine + LLM provider

### Phase 3 — AI Features (effort: Large)
9. AI-driven leak explanations (explain command)
10. AI-driven fix suggestions (context-aware patches)
11. Conversation mode (multi-turn context window)

### Phase 4 — MCP Hardening (effort: Medium)
12. Streaming responses
13. Structured error codes
14. Session management
15. Tool descriptions for IDE discovery

### Phase 5 — Privacy & Additional Providers (effort: Medium)
16. Privacy/redaction layer
17. Audit logging
18. Anthropic backend
19. Ollama/local backend

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

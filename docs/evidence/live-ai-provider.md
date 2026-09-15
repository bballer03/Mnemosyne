# Live AI provider path evidence

**Branch:** `feature/mat-equivalent-live-ai`  
**Date:** 2026-09-15  
**Evidence class:** local HTTP integration plus source trace; no external-provider credential or production model call

## Credential and local-provider discovery

The WSL environment and `~/.config` were checked without printing values:

- `env_OPENAI_API_KEY=false`
- `env_ANTHROPIC_API_KEY=false`
- `config_OPENAI_API_KEY=false`
- `config_ANTHROPIC_API_KEY=false`
- `http://127.0.0.1:11434/api/tags=false`
- `http://127.0.0.1:11434/v1/models=false`

No live OpenAI or Anthropic key was used. No Ollama-compatible endpoint responded on the default local port.

## Runtime trace

### Configuration

`cli/src/config_loader.rs::load_app_config` selects the first config source in this order: explicit `--config`, `MNEMOSYNE_CONFIG`, project `.mnemosyne.toml`, user config, system config, then defaults. File `[ai]` / `[llm]` values populate `AiConfig`; `MNEMOSYNE_AI_MODE`, `MNEMOSYNE_AI_PROVIDER`, `MNEMOSYNE_AI_ENDPOINT`, and `MNEMOSYNE_AI_API_KEY_ENV` can override them.

`api_key_env` is an environment-variable name. `core/src/llm.rs::resolved_api_key` resolves its value only when provider mode performs a completion. If omitted, OpenAI and Anthropic use `OPENAI_API_KEY` and `ANTHROPIC_API_KEY`; local providers have no default key. `resolved_endpoint` defaults OpenAI and Anthropic to their public API bases, while local mode requires `ai.endpoint`.

### CLI chat

`cli/src/main.rs::handle_chat` first analyzes the heap with AI disabled, then routes each question through `generate_ai_chat_turn_async`. Provider mode moves the blocking request to `spawn_blocking`, builds and redacts the TOON prompt, then calls `core::llm::complete`. OpenAI and local modes POST to `<endpoint>/chat/completions`; Anthropic POSTs to `<endpoint>/messages`. The returned provider text must parse as the existing TOON `AiInsights` contract.

### MCP chat

`mnemosyne-cli serve` receives the same loaded `AppConfig`. MCP `chat_session` loads the persisted heap-bound session, applies its focus/shortlist and bounded history, then calls the same `generate_ai_chat_turn_async` provider path. Provider transport failures remain `AiProviderError` / `AiProviderTimeout`, serialized by MCP as `provider_error` / `provider_timeout`.

### Desktop chat

The UI's `__MNEMOSYNE_ASSISTANT_BRIDGE__.chatSession` invokes the Tauri `chat_session` command. That command passes the native session's `AppConfig` to `chat_session_for_session`, which calls the same core provider function and strips raw wire prompt/response bodies from the UI payload.

However, `HeapSession::new()` currently initializes `AppConfig::default()`, and no desktop config-loader path updates it. The default is AI disabled with `mode = "rules"`. Therefore external-provider desktop chat is **not proven or currently operator-configurable through the CLI config chain**. This evidence proves CLI/MCP provider execution only.

## Added integration evidence

`cli/tests/integration.rs::test_serve_chat_session_provider_mode_round_trips_through_local_http` starts a loopback mock OpenAI-compatible server and invokes the real `mnemosyne-cli serve` stdio surface with a persisted AI session.

The test proves:

- `[ai].mode = "provider"` and `provider = "local"` reach the HTTP transport.
- `endpoint` selects the loopback `/v1/chat/completions` route.
- `api_key_env` is resolved and used as bearer authentication without embedding a credential in config.
- the MCP `chat_session` question reaches the provider prompt.
- `redact_heap_path = true` replaces the persisted heap path before transmission.
- the provider JSON envelope and TOON content parse into a successful MCP `AiInsights` response.

Captured redacted exchange:

```text
MCP request: <REDACTED_CHAT_SESSION_REQUEST>
HTTP authorization: <REDACTED_TEST_BEARER>
Outbound provider prompt: <REDACTED_PROVIDER_PROMPT>
Inbound provider response: <REDACTED_PROVIDER_RESPONSE>
MCP result: <REDACTED_AI_INSIGHTS_RESULT>
```

Focused command:

```bash
cargo test -p mnemosyne-cli --test integration \
  test_serve_chat_session_provider_mode_round_trips_through_local_http \
  -- --exact --nocapture
```

Observed result: `1 passed; 0 failed`.

Broader verification:

```bash
cargo test -p mnemosyne-cli --test integration
cargo test -p mnemosyne-core provider_mode --lib
```

Observed results:

- CLI integration: `104 passed; 0 failed`.
- Core provider-mode unit/integration paths: `6 passed; 0 failed`.

## Default and failure behavior

- `AiConfig::default()` remains `enabled = false`, `mode = "rules"`.
- CLI chat and MCP sessions perform initial heap analysis with AI disabled.
- Provider mode is opt-in through config or environment overrides.
- Existing CLI integration coverage verifies structured MCP `provider_error` and `provider_timeout` responses.
- Rules mode remains available without a key or endpoint.

## Honest proof boundary

Proven here: local process → CLI config loading → MCP stdio `chat_session` → persisted session context → redaction → real loopback HTTP request → OpenAI-compatible response parsing → structured MCP result.

Not proven here: a live OpenAI/Anthropic network round-trip, production model response quality, TLS/public-network behavior, provider billing/rate limits, Ollama operation, or packaged desktop provider chat.

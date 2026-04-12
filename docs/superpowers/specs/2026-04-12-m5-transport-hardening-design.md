# M5 Transport Hardening Design

## Goal

Decide whether the current stdio MCP transport needs streaming by collecting direct evidence under realistic AI-backed workloads, then implement only the minimum hardening required by that evidence.

## Why This Slice Exists

Milestone 5 now has provider-backed AI execution, structured MCP `error_details`, CLI chat, persisted MCP AI sessions, and an AI-backed one-file / one-snippet fix-generation path. The remaining open transport question is whether the current line-delimited stdio request/response model is still acceptable for long-running or larger AI-backed calls.

The current roadmap and status language already treat streaming as conditional, not mandatory. This slice makes that decision explicit and test-backed instead of speculative.

## Current Runtime Baseline

The current MCP transport lives in `core/src/mcp/server.rs:serve()`.

Current behavior:

- reads one JSON request per stdin line
- executes `handle_request()`
- writes exactly one JSON response line per request
- preserves the legacy string `error` field on failure
- attaches machine-readable `error_details` on failure
- returns the normal final payloads for `AiInsights`, `FixResponse`, and other existing result shapes

Current AI-heavy MCP methods most likely to stress the transport:

- `chat_session`
- `explain_leak`
- `propose_fix`
- `create_ai_session` when analysis is large enough to be noticeable

## Problem Statement

We do not yet know whether the current request/response-only stdio transport is good enough for Mnemosyne's realistic MCP usage.

The risk is not only raw correctness. It is also whether clients can tolerate:

- delayed AI-backed responses with no intermediate signal
- larger single-response JSON payloads
- provider failure or timeout conditions surfacing only at the end of the request

Adding streaming without evidence would introduce unnecessary transport and documentation churn. Not adding it when the current model is inadequate would leave a real product gap in M5.

## Design Principles

1. Evidence before protocol change.
2. Keep stable analysis and AI contracts unchanged unless transport evidence forces otherwise.
3. Prefer transport-layer additions over payload-shape rewrites.
4. Keep non-AI MCP methods stateless and unchanged.
5. If the current transport is acceptable, stop there.

## Scope

### In Scope

- black-box verification of `mnemosyne serve` under AI-backed delay and payload stress
- explicit thresholds for deciding whether current stdio request/response is acceptable
- tightening AI/provider/timeout transport error handling if tests show gaps
- documenting the accepted transport model in `docs/api.md`
- adding the smallest additive progress/event protocol only if tests prove it is necessary

### Out of Scope

- changing `AiInsights`, `AiWireExchange`, or `AiWireFormat::Toon`
- redesigning the persisted MCP session model
- changing CLI `chat` behavior
- redesigning non-AI MCP methods
- broader product work outside transport hardening

## Success Criteria

This slice is complete when all of the following are true:

1. The repository has repeatable tests that characterize current MCP transport behavior for delayed and larger AI-backed responses.
2. The branch records an explicit decision: either the current transport is sufficient, or additive progress/streaming is required.
3. If no streaming is required, docs and tests clearly establish the request/response contract and hardened error behavior.
4. If streaming is required, the implementation is additive, AI-method-scoped, and preserves final result payload contracts.

## Decision Framework

### Phase A: Evidence Collection

Add tests that answer these questions:

1. Does `serve` still emit exactly one valid JSON response line per request under delayed AI-backed execution?
2. Do delayed provider-backed requests still produce the same success/error envelope without corruption or partial writes?
3. Do provider failure and timeout scenarios remain machine-distinguishable through `error_details`?
4. Are current `AiInsights` and `FixResponse` payload sizes still practical within the one-line JSON envelope for expected use?

### Phase B: Decision Gate

If all evidence says the current transport is acceptable:

- keep the request/response-only protocol
- document that AI-backed MCP calls are synchronous over stdio
- harden only the missing error/size behaviors proven by tests

If evidence shows the current transport is inadequate:

- add an additive event/progress protocol only for AI-backed methods
- keep the final success/error response line as the authoritative completion result
- do not change the final `result` payload shapes already used by clients

## Acceptability Thresholds

The current transport is considered acceptable if all of these hold in tests:

1. One response line per request remains intact, even under delayed AI/provider execution.
2. Final responses remain parseable JSON with the existing `success`, `result`, `error`, and `error_details` envelope.
3. Provider timeout and provider failure remain machine-distinguishable from invalid input.
4. AI-backed response sizes stay within a tested envelope with no observed truncation, corruption, or line-splitting behavior.

If any of those fail in realistic tests, the transport is not acceptable as-is.

## Preferred Implementation Path

### Path A: No Streaming Needed

This is the preferred outcome.

Implementation would include:

- stronger CLI integration coverage for `serve`
- stronger core tests around delayed success and delayed failure
- clearer AI/provider timeout/error classification where needed
- docs that state the transport remains one-request/one-response for all current methods, including AI-backed ones

This path keeps M5 small and honest.

### Path B: Additive AI Progress Events

Only used if Phase A proves the current model insufficient.

Constraints:

- progress/event messages are additive transport metadata, not replacements for final responses
- final success/error response remains the current authoritative response contract
- non-AI methods remain unchanged
- `AiInsights`, `FixResponse`, `AiWireExchange`, and session payloads stay structurally unchanged

Recommended event shape if needed:

```json
{
  "id": 9,
  "event": "progress",
  "method": "chat_session",
  "stage": "awaiting_provider",
  "message": "Waiting for AI provider response"
}
```

This is intentionally transport-scoped and separate from the final `RpcResponse` envelope.

## File Impact

### Primary Files

- `core/src/mcp/server.rs`
  - keep the transport boundary here
  - add only the minimal transport hardening required by evidence

- `cli/tests/integration.rs`
  - add black-box `mnemosyne serve` transport tests
  - this file should remain the main evidence source for transport behavior seen by real clients

- `docs/api.md`
  - document either the confirmed request/response-only model or the additive event extension

### Possible Secondary Files

- `core/src/errors.rs`
  - only if transport-related AI/provider/timeout distinctions require a clearer core error boundary

- `STATUS.md`
  - update the M5 transport note once the decision is made

- `docs/design/milestone-5-ai-mcp-differentiation.md`
  - update milestone status from “streaming remains the main open item” to the tested outcome

## Testing Strategy

### Core Tests

Add targeted `core::mcp::server` tests for:

- delayed success path remains well-formed
- delayed error path remains well-formed
- timeout/provider failures preserve structured machine-readable error classification

These should stay focused on transport behavior, not duplicate provider implementation tests already covered elsewhere.

### CLI Integration Tests

Add `mnemosyne serve` tests that:

- send one request over stdin and assert one response line on stdout
- exercise AI-backed delay using the same local HTTP test-server style already used in provider integration tests
- verify provider failure and invalid request shapes stay distinguishable at the stdio boundary
- verify larger AI/fix payloads remain parseable in a single-line response model

### Contract Tests

Preserve and explicitly re-check:

- final `RpcResponse` envelope shape
- legacy `error` field presence on failure
- `error_details` machine readability
- unchanged `AiInsights` / `FixResponse` result payloads

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Transport tests prove current model acceptable and no code change feels “small” | High | Low | Accept that outcome and stop at verification + docs |
| Transport tests are unrealistic and overstate the need for streaming | Medium | Medium | Keep tests black-box and close to real provider-backed behavior |
| A new event protocol accidentally becomes a second response contract | Medium | High | Keep all progress messages additive and retain current final `RpcResponse` as authoritative |
| Hardening spreads into general MCP redesign | Medium | High | Keep scope limited to AI-backed transport pressure and error behavior |

## Open Questions Resolved By This Design

1. Should streaming be treated as mandatory M5 scope?
   Answer: no. It must be justified by tests.

2. Should stable AI payload contracts change as part of transport hardening?
   Answer: no. Any transport change should be additive.

3. Should non-AI MCP methods participate in any new progress protocol?
   Answer: no. Keep any additive progress/event behavior scoped to AI-backed methods.

## Recommended Outcome

The expected best outcome is that the current stdio request/response model remains acceptable once it has better evidence, stronger error-path coverage, and clearer documentation.

If the evidence disproves that, then the next-best outcome is a narrow additive event/progress layer for AI-backed MCP calls only.

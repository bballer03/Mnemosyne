# MCP Agent-Loop Transcripts (M18.F)

Captured against the live `core::mcp::server::handle_request` dispatcher using the same synthetic HPROF fixture as the core MCP unit tests (`build_graph_fixture`). Absolute temp paths are replaced with `<heap>`; snapshot digests with `<snapshot-key>`; artifact IDs with `<artifact-id>`. No heap-derived string contents or host filesystem roots appear below.

## 1. Policy gate with baseline (`ci_check`)

### Request

```json
{
  "id": 1,
  "method": "ci_check",
  "params": {
    "heap_path": "<heap>",
    "baseline": "<heap>",
    "policy_toml": "[[rule]]\nid = \"no-runaway-growth\"\npredicate = \"object_growth_threshold\"\nop = \"<=\"\nvalue = 999999999999\nseverity = \"error\"\n"
  }
}
```

### Response (shape)

```json
{
  "exit_code": 0,
  "result": {
    "evaluations": [
      {
        "rule_id": "no-runaway-growth",
        "passed": true
      }
    ],
    "skipped": [],
    "violations": []
  }
}
```

Evidence: `mcp_ci_check_object_growth_with_baseline_evaluates_instead_of_skipping` in `core/src/mcp/server.rs` (same request shape; exit code `0`; `object_growth_threshold` evaluated, not skipped).

`baseline_snapshot` is the alternative to a second heap path — pass a store key from `save_snapshot` / `list_snapshots` instead of `baseline`.

## 2. Snapshot → diff → flamegraph

### 2a. `save_snapshot`

```json
{"id":1,"method":"save_snapshot","params":{"heap_path":"<heap>"}}
```

Response includes `heap_sha256` (`<snapshot-key>`), `object_count`, and `has_field_data`. Evidence: `mcp_save_snapshot_open_list_remove_lifecycle_with_synthetic_heap`.

### 2b. `diff_heaps`

```json
{
  "id": 2,
  "method": "diff_heaps",
  "params": {
    "before_path": "<heap>",
    "after_path": "<heap>",
    "cross_reference_leaks": false
  }
}
```

Additive `cross_reference_leaks` defaults off. Response carries the existing object/class diff envelope (no absolute store paths).

### 2c. `generate_flamegraph` (managed artifact)

```json
{
  "id": 3,
  "method": "generate_flamegraph",
  "params": {
    "heap_path": "<heap>",
    "mode": "deep",
    "format": "folded-stack",
    "snapshot": "<snapshot-key>"
  }
}
```

Response shape (inline when ≤256 KiB rendered bytes; otherwise `delivery=managed`):

```json
{
  "artifact_id": "<artifact-id>",
  "format": "folded-stack",
  "media_type": "text/plain",
  "byte_length": 0,
  "sha256": "<hex>",
  "created_at": "<RFC3339 UTC>",
  "expires_at": "<RFC3339 UTC>",
  "delivery": "inline",
  "content_base64": "<omitted in docs>"
}
```

Follow with `read_artifact` / `delete_artifact` using only `<artifact-id>` — never a filesystem path. Evidence: M18.E artifact contract tests in `core/src/mcp/artifact.rs` and `generate_flamegraph` dispatch in `server.rs`.

## Tool discovery note

`list_tools` advertises the live schemas for `ci_check` (including exclusive `baseline` / `baseline_snapshot`), `diff_heaps.cross_reference_leaks`, `save_snapshot` / `remove_snapshot`, `generate_flamegraph`, `read_artifact`, and `delete_artifact`. Prefer `list_tools` over stale prose when wiring an agent.

## Sanitization rules used here

- Heap paths → `<heap>`
- Snapshot keys → `<snapshot-key>`
- Artifact IDs → `<artifact-id>`
- Base64 flamegraph bodies omitted (binary noise; not required to understand the loop)
- No class names / field values from fixture heaps are quoted as investigation facts

# API Reference

Mnemosyne exposes a line-delimited JSON protocol over stdio through `mnemosyne-cli serve`.

This file documents the live wire contract implemented in `core/src/mcp/server.rs`. It is intentionally narrower than JSON-RPC 2.0:

- requests are single-line JSON objects written to stdin
- responses are single-line JSON objects written to stdout
- there is no `jsonrpc` field
- there is no `data` envelope inside `result`
- failures preserve the plain-string `error` field and now also attach machine-readable `error_details`

## Transport

Start the server:

```bash
mnemosyne-cli serve
```

Send one JSON object per line:

```json
{"id":1,"method":"parse_heap","params":{"path":"heap.hprof"}}
```

Successful responses:

```json
{
  "id": 1,
  "success": true,
  "result": {},
  "error": null
}
```

Failed responses:

```json
{
  "id": 1,
  "success": false,
  "result": null,
  "error": "Invalid input: unsupported MCP method: nope",
  "error_details": {
    "code": "invalid_input",
    "message": "Invalid input: unsupported MCP method: nope",
    "details": {
      "detail": "unsupported MCP method: nope"
    }
  }
}
```

## Method List

The current server supports these methods:

- `list_tools`
- `parse_heap`
- `detect_leaks`
- `analyze_heap`
- `query_heap`
- `map_to_code`
- `find_gc_path`
- `explain_leak`
- `propose_fix`

There is currently no `apply_fix` handler.

`list_tools` is the discovery surface for machine-readable tool descriptions and parameter metadata.

## Common Types

These serialized values show up in multiple responses.

### Error Details

Failures include:

- `error`: a backward-compatible string summary
- `error_details.code`: a stable machine-readable code such as `invalid_input` or `config_error`
- `error_details.message`: the same human-readable message carried in `error`
- `error_details.details`: optional structured context such as `path`, `detail`, `phase`, or `suggestion`

### Provenance Marker

Fallback, synthetic, partial, and placeholder output is labeled explicitly:

```json
{
  "kind": "SYNTHETIC",
  "detail": "Fix suggestions are generated heuristically from leak summaries."
}
```

`kind` is one of:

- `SYNTHETIC`
- `PARTIAL`
- `FALLBACK`
- `PLACEHOLDER`

### Leak Severity

Serialized as uppercase enum names:

- `LOW`
- `MEDIUM`
- `HIGH`
- `CRITICAL`

### Leak Kind

Serialized as uppercase enum names:

- `UNKNOWN`
- `CACHE`
- `COROUTINE`
- `THREAD`
- `HTTP_RESPONSE`
- `CLASS_LOADER`
- `COLLECTION`
- `LISTENER`

### Histogram Grouping

`analyze_heap` accepts `histogram_group_by` as:

- `class`
- `package`
- `class_loader`

## `list_tools`

Return the live MCP tool catalog with descriptions and parameter metadata.

### Request

```json
{
  "id": 0,
  "method": "list_tools",
  "params": {}
}
```

### Result Shape

`result` contains a `tools` array:

```json
{
  "tools": [
    {
      "name": "analyze_heap",
      "description": "Run the full analysis pipeline and return the serialized analysis response.",
      "params": [
        {
          "name": "heap_path",
          "type": "string",
          "required": true,
          "description": "Path to the heap dump."
        }
      ]
    }
  ]
}
```

## `parse_heap`

Parse an HPROF file and return a lightweight heap summary.

### Request

```json
{
  "id": 1,
  "method": "parse_heap",
  "params": {
    "path": "heap.hprof",
    "include_strings": false,
    "max_objects": 500000
  }
}
```

### Params

- `path` string, required
- `include_strings` boolean, optional, currently accepted but not surfaced in the summary
- `max_objects` number, optional, defaults to config `parser.max_objects`

### Result Shape

`result` is a serialized `HeapSummary`:

```json
{
  "heap_path": "heap.hprof",
  "total_objects": 1234567,
  "total_size_bytes": 2576980377,
  "classes": [
    {
      "name": "INSTANCE_DUMP",
      "instances": 345678,
      "total_size_bytes": 441450000,
      "percentage": 50.1
    }
  ],
  "generated_at": "2026-04-12T10:00:00Z",
  "header": {
    "format": "JAVA PROFILE 1.0.2",
    "identifier_size": 8,
    "timestamp_millis": 1709836800000
  },
  "total_records": 5678901,
  "record_stats": [
    {
      "tag": 28,
      "name": "HEAP_DUMP_SEGMENT",
      "count": 12,
      "bytes": 882376704
    }
  ]
}
```

## `detect_leaks`

Run leak detection against a heap path using the configured analysis defaults plus request overrides.

### Request

```json
{
  "id": 2,
  "method": "detect_leaks",
  "params": {
    "heap_path": "heap.hprof",
    "package": "com.example",
    "min_severity": "HIGH",
    "leak_types": ["CACHE", "THREAD"]
  }
}
```

### Params

- `heap_path` string, required
- `package` string, optional, single package filter for this MCP method
- `min_severity` string, optional
- `leak_types` string array, optional

### Result Shape

`result` is a JSON array of `LeakInsight` objects:

```json
[
  {
    "id": "leak-usersession-1",
    "class_name": "com.example.UserSessionCache",
    "leak_kind": "CACHE",
    "severity": "HIGH",
    "retained_size_bytes": 536870912,
    "shallow_size_bytes": 4096,
    "suspect_score": 0.87,
    "instances": 125432,
    "description": "Cache growing unbounded, cleanup thread blocked",
    "provenance": []
  }
]
```

## `analyze_heap`

Run the full analysis pipeline and return the serialized `AnalyzeResponse`.

### Request

```json
{
  "id": 3,
  "method": "analyze_heap",
  "params": {
    "heap_path": "heap.hprof",
    "min_severity": "MEDIUM",
    "packages": ["com.example"],
    "leak_types": ["CACHE"],
    "histogram_group_by": "package",
    "enable_ai": true,
    "enable_classloaders": true,
    "enable_threads": true,
    "enable_strings": true,
    "enable_collections": true,
    "enable_top_instances": true,
    "top_n": 10,
    "min_collection_capacity": 32,
    "min_duplicate_count": 3
  }
}
```

### Params

- `heap_path` string, required
- `min_severity` string, optional
- `packages` string array, optional
- `leak_types` string array, optional
- `histogram_group_by` string, optional, defaults to `class`
- `enable_ai` boolean, optional
- `enable_classloaders` boolean, optional
- `enable_threads` boolean, optional
- `enable_strings` boolean, optional
- `enable_collections` boolean, optional
- `enable_top_instances` boolean, optional
- `top_n` number, optional
- `min_collection_capacity` number, optional
- `min_duplicate_count` number, optional

### Result Shape

`result` is a serialized `AnalyzeResponse` object with optional sections omitted when not requested or not available:

```json
{
  "summary": {
    "heap_path": "heap.hprof",
    "total_objects": 1234567,
    "total_size_bytes": 2576980377,
    "classes": [],
    "generated_at": "2026-04-12T10:00:00Z",
    "header": {
      "format": "JAVA PROFILE 1.0.2",
      "identifier_size": 8,
      "timestamp_millis": 1709836800000
    },
    "total_records": 5678901,
    "record_stats": []
  },
  "leaks": [],
  "recommendations": [],
  "elapsed": {
    "secs": 1,
    "nanos": 500000000
  },
  "graph": {
    "node_count": 1200000,
    "edge_count": 4300000,
    "dominators": []
  },
  "ai": {
    "model": "gpt-4.1-mini",
    "summary": "Top leak is retaining a large share of the heap.",
    "recommendations": [
      "Review cleanup and ownership boundaries."
    ],
    "confidence": 0.74,
    "wire": {
      "format": "Toon",
      "prompt": "...",
      "response": "..."
    }
  },
  "histogram": {
    "group_by": "package",
    "entries": [],
    "total_instances": 1200000,
    "total_shallow_size": 900000000
  },
  "unreachable": {
    "total_count": 42,
    "total_shallow_size": 8192,
    "by_class": []
  },
  "provenance": []
}
```

Notes:

- the field name is `unreachable`, not `unreachable_objects`
- `ai` is omitted when AI is disabled or unavailable
- optional report sections are omitted when `None`
- the server serializes the raw Rust structs directly under `result`

## `query_heap`

Execute the OQL-style heap query engine over the parsed object graph.

### Request

```json
{
  "id": 4,
  "method": "query_heap",
  "params": {
    "heap_path": "heap.hprof",
    "query": "SELECT @objectId, @className FROM \"com.example.BigCache\" LIMIT 25"
  }
}
```

### Result Shape

`result` is a `QueryResult`:

```json
{
  "columns": ["@objectId", "@className"],
  "rows": [
    [
      { "Id": 4096 },
      { "Str": "com.example.BigCache" }
    ]
  ],
  "total_matched": 1,
  "truncated": false
}
```

`CellValue` uses serde's externally tagged enum encoding. Common cells look like:

- `{ "Id": 4096 }`
- `{ "Str": "com.example.BigCache" }`
- `{ "Int": 8192 }`
- `"Null"`

## `map_to_code`

Map a leak identifier to likely source files using lightweight path and symbol heuristics.

### Request

```json
{
  "id": 5,
  "method": "map_to_code",
  "params": {
    "leak_id": "com.example.Cache::deadbeef",
    "class": "com.example.Cache",
    "project_root": "D:/repo",
    "include_git_info": true
  }
}
```

### Params

- `leak_id` string, required
- `class` string, optional
- `project_root` string, required
- `include_git_info` boolean, optional, defaults to `true`

### Result Shape

`result` is a `SourceMapResult`:

```json
{
  "leak_id": "com.example.Cache::deadbeef",
  "locations": [
    {
      "file": "D:/repo/src/main/java/com/example/Cache.java",
      "line": 42,
      "symbol": "public final class Cache {",
      "code_snippet": "class Cache {\n  ...\n}",
      "git": {
        "author": "Example Author",
        "commit": "abc123",
        "date": "2026-04-12 10:00:00 +0000",
        "message": "Add cache cleanup"
      }
    }
  ]
}
```

If no matching source file is found, Mnemosyne returns a synthetic fallback location under `.mnemosyne/unmapped/...` with `git: null`.

## `find_gc_path`

Find a path from an object to a GC root.

### Request

```json
{
  "id": 6,
  "method": "find_gc_path",
  "params": {
    "heap_path": "heap.hprof",
    "object_id": "0x1000",
    "max_depth": 8
  }
}
```

### Result Shape

`result` is a `GcPathResult`:

```json
{
  "object_id": "0x0000000000001000",
  "path": [
    {
      "object_id": "0x0000000000000001",
      "class_name": "java.lang.Thread",
      "field": "ROOT",
      "is_root": true
    },
    {
      "object_id": "0x0000000000001000",
      "class_name": "com.example.Cache",
      "field": "entries",
      "is_root": false
    }
  ],
  "path_length": 2,
  "provenance": []
}
```

The implementation tries, in order:

1. full `ObjectGraph` BFS
2. budget-limited fallback graph parsing
3. synthetic path construction from summary data

Synthetic or fallback paths are labeled in `provenance`.

## `explain_leak`

Generate AI insights for the whole heap or a single leak candidate.

### Request

```json
{
  "id": 7,
  "method": "explain_leak",
  "params": {
    "heap_path": "heap.hprof",
    "leak_id": "leak-usersession-1",
    "min_severity": "HIGH"
  }
}
```

### Result Shape

`result` is the raw `AiInsights` object:

```json
{
  "model": "gpt-4.1-mini",
  "summary": "Top leak is retaining a large share of the heap.",
  "recommendations": [
    "Review cleanup and ownership boundaries."
  ],
  "confidence": 0.74,
  "wire": {
    "format": "Toon",
    "prompt": "...",
    "response": "..."
  }
}
```

Notes:

- the `wire.format` enum currently serializes as `Toon`
- `AiInsights`, `AiWireExchange`, and `AiWireFormat::Toon` are stable shared contracts in this branch
- provider-backed AI exists, but consumers should still treat the generated text as advisory

## `propose_fix`

Generate AI-backed fix suggestions for a leak candidate when provider mode and source context are available; otherwise return heuristic fallback guidance.

### Request

```json
{
  "id": 8,
  "method": "propose_fix",
  "params": {
    "heap_path": "heap.hprof",
    "leak_id": "leak-usersession-1",
    "project_root": "D:/repo",
    "style": "Minimal"
  }
}
```

### Params

- `heap_path` string, required
- `leak_id` string, optional
- `project_root` string, optional
- `style` string, optional, defaults to `Minimal`

`style` currently serializes in Rust enum casing:

- `Minimal`
- `Defensive`
- `Comprehensive`

### Result Shape

`result` is a `FixResponse`:

```json
{
  "suggestions": [
    {
      "leak_id": "leak-usersession-1",
      "class_name": "com.example.UserSessionCache",
      "target_file": "D:/repo/src/main/java/com/example/UserSessionCache.java",
      "description": "Add guard clauses so com.example.UserSessionCache releases references when exceeding safe capacity.",
      "diff": "--- a/...\n+++ b/...\n@@\n-// TODO...\n+if (cache.size() > SAFE_CAPACITY) {\n+    cache.clear();\n+}\n",
      "confidence": 0.75,
      "style": "Minimal"
    }
  ],
  "project_root": "D:/repo",
  "provenance": [
    {
      "kind": "SYNTHETIC",
      "detail": "Fix suggestions are generated heuristically from leak summaries."
    },
    {
      "kind": "FALLBACK",
      "detail": "Provider-backed fix generation was unavailable; returned heuristic guidance instead."
    },
    {
      "kind": "PLACEHOLDER",
      "detail": "Static-analysis-backed remediation is not wired yet; this is placeholder guidance."
    }
  ]
}
```

When `project_root` yields a mapped source file plus a small local snippet and provider mode is active, Mnemosyne can return an AI-backed patch suggestion in the same `FixResponse` shape. When that path is unavailable or fails validation, it falls back to heuristic placeholder guidance with explicit provenance markers.

## CLI Relationship

The MCP server shares core logic with the CLI, but the command names are not one-to-one API wrappers. The live CLI command surface is:

- `parse`
- `leaks`
- `analyze`
- `diff`
- `map`
- `gc-path`
- `query`
- `explain`
- `fix`
- `serve`
- `config`

For quick operator examples, see `docs/QUICKSTART.md`. For command-line usage, prefer `mnemosyne-cli --help` and the subcommand help text as the runtime source of truth.

# MCP API Reference

This document describes all available Model Context Protocol (MCP) commands for Mnemosyne.

## Table of Contents

- [Connection](#connection)
- [Commands](#commands)
  - [parse_heap](#parse_heap)
  - [detect_leaks](#detect_leaks)
  - [map_to_code](#map_to_code)
  - [find_gc_path](#find_gc_path)
  - [explain_leak](#explain_leak)
  - [propose_fix](#propose_fix)
  - [apply_fix](#apply_fix)
- [Data Types](#data-types)
- [Error Handling](#error-handling)

---

## Connection

Mnemosyne runs as an MCP server that communicates via stdio.

### Starting the Server

```bash
mnemosyne serve
```

### Configuration

MCP clients connect using configuration files. See [README.md](../README.md#-mcp-integration) for IDE-specific setup.

---

## Commands

### parse_heap

Parse a heap dump file and return a summary.

#### Request

```json
{
  "method": "parse_heap",
  "params": {
    "path": "/path/to/heap.hprof",
    "options": {
      "include_strings": false,
      "max_objects": null
    }
  }
}
```

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | Yes | Path to the `.hprof` file |
| `options.include_strings` | boolean | No | Include string table in output (default: false) |
| `options.max_objects` | number | No | Limit number of objects to parse (for testing) |

#### Response

```json
{
  "success": true,
  "data": {
    "total_size_bytes": 2453291008,
    "total_objects": 1234567,
    "total_classes": 4321,
    "gc_roots": 156,
    "top_classes": [
      {
        "name": "java.lang.String",
        "instances": 421032,
        "total_size_bytes": 441651200,
        "percentage": 18.0
      }
    ],
    "parse_time_ms": 3245
  }
}
```

---

### detect_leaks

Detect potential memory leaks in a parsed heap dump.

#### Request

```json
{
  "method": "detect_leaks",
  "params": {
    "heap_path": "/path/to/heap.hprof",
    "filters": {
      "package": "com.example",
      "min_severity": "MEDIUM",
      "leak_types": ["COROUTINE", "THREAD", "CACHE"]
    }
  }
}
```

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `heap_path` | string | Yes | Path to the heap dump |
| `filters.package` | string | No | Only analyze classes in this package |
| `filters.min_severity` | string | No | Minimum severity: LOW, MEDIUM, HIGH, CRITICAL |
| `filters.leak_types` | array | No | Types of leaks to detect |

#### Response

```json
{
  "success": true,
  "data": {
    "leaks": [
      {
        "id": "leak-001",
        "class": "com.example.UserSessionCache",
        "severity": "HIGH",
        "leak_type": "CACHE",
        "instances": 125432,
        "retained_size_bytes": 536870912,
        "gc_root": {
          "type": "THREAD",
          "name": "session-cleanup",
          "state": "BLOCKED"
        },
        "description": "UserSessionCache retaining stale sessions"
      }
    ],
    "total_leaks": 1,
    "analysis_time_ms": 1842
  }
}
```

---

### map_to_code

Map leaked objects to source code locations.

#### Request

```json
{
  "method": "map_to_code",
  "params": {
    "leak_id": "com.example.UserSessionCache::ff12ab90",
    "class": "com.example.UserSessionCache",
    "project_root": "/path/to/project",
    "include_git_info": true
  }
}
```

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `leak_id` | string | Yes | ID from `detect_leaks` response |
| `class` | string | No | Fully-qualified class name (improves accuracy) |
| `project_root` | string | Yes | Root directory of source code |
| `include_git_info` | boolean | No | Include git blame/history (default: true) |

#### Response

```json
{
  "success": true,
  "data": {
    "locations": [
      {
        "file": "src/main/java/com/example/UserSessionCache.java",
        "line": 45,
        "symbol": "public void addSession(...)",
        "code_snippet": "cache.put(sessionId, session);",
        "git": {
          "author": "John Doe",
          "commit": "abc123def456",
          "date": "2025-11-15T10:30:00Z",
          "message": "Add session caching"
        }
      }
    ]
  }
}
```

> **Note:** When no matching file is found, Mnemosyne will return a placeholder entry that explains how to provide better hints (e.g., `class`) for the next attempt.

---

### find_gc_path

Find the path from an object to its GC root.

#### Request

```json
{
  "method": "find_gc_path",
  "params": {
    "heap_path": "/path/to/heap.hprof",
    "object_id": "0x7f8a9c123456",
    "max_depth": 5
  }
}
```

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `heap_path` | string | Yes | Path to the heap dump |
| `object_id` | string | Yes | Hex ID of the object |
| `max_depth` | number | No | Maximum path depth to search |

#### Response

```json
{
  "success": true,
  "data": {
    "object_id": "0x0000000033333333",
    "path": [
      {
        "object_id": "0x0000000044444444",
        "class_name": "com.example.Leaky",
        "field": "ROOT Unknown",
        "is_root": true
      },
      {
        "object_id": "0x0000000033333333",
        "class_name": "java.lang.Object",
        "field": "leakyField",
        "is_root": false
      }
    ],
    "path_length": 2
  }
}
```

The server now streams real GC roots, class dumps, instance dumps, and object arrays to build these paths. If a heap omits the required records—or exceeds the configured sampling budget—the API falls back to the legacy synthetic chain so clients never receive an empty response.

---

### explain_leak

Get an AI-generated explanation for a detected leak.

#### Request

```json
{
  "method": "explain_leak",
  "params": {
    "heap_path": "/path/to/heap.hprof",
    "leak_id": "com.example.UserSessionCache::ff12ab90",
    "min_severity": "LOW"
  }
}
```

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `heap_path` | string | Yes | Heap dump to inspect |
| `leak_id` | string | No | Target leak ID or class (falls back to top leak) |
| `min_severity` | string | No | Minimum severity to consider (default: LOW) |

#### Response

```json
{
  "success": true,
  "data": {
    "model": "gpt-4.1-mini",
    "summary": "UserSessionCache is retaining ~512.00 MB via 125432 instances; prioritize freeing it to reclaim 21.0% of the heap.",
    "recommendations": [
      "Guard UserSessionCache lifetimes: ensure cleanup hooks dispose unused entries.",
      "Add targeted instrumentation (counters, timers) around the suspected allocation sites.",
      "Review threading / coroutine lifecycles anchoring these objects to a GC root."
    ],
    "confidence": 0.78,
    "wire": {
      "format": "Toon",
      "prompt": "TOON v1\nsection request\n  intent=explain_leak\n  heap_path=/path/to/heap.hprof\n  total_bytes=2453291008\n  total_objects=1234567\n  leak_sampled=1\nsection leaks\n  leak#0\n    id=com.example.UserSessionCache::ff12ab90\n    class=com.example.UserSessionCache\n    kind=Cache\n    severity=High\n    retained_mb=512.00\n    instances=125432\n    description=UserSessionCache dominates 21% of the heap\n",
      "response": "TOON v1\nsection response\n  model=gpt-4.1-mini\n  confidence_pct=78\n  summary=com.example.UserSessionCache retains ~512.00 MB via 125432 instances (severity High).\nsection remediation\n  priority=high\n  retained_percent=21.0\n"
    }
  }
}
```

The `wire` block always contains the exact TOON payload Mnemosyne would send to (and expect from) a real LLM. Clients that want to broker their own AI requests can forward this payload without parsing human-readable prose.

---

### propose_fix

Generate code fix suggestions for a leak.

#### Request

```json
{
  "method": "propose_fix",
  "params": {
    "heap_path": "/path/to/heap.hprof",
    "leak_id": "com.example.UserSessionCache::ff12ab90",
    "project_root": "/path/to/project",
    "style": "DEFENSIVE"
  }
}
```

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `heap_path` | string | Yes | Heap dump used for leak context |
| `leak_id` | string | No | Target leak ID/class (defaults to top leak) |
| `project_root` | string | No | Source root for path hints |
| `style` | string | No | MINIMAL, DEFENSIVE, or COMPREHENSIVE (default: MINIMAL) |

#### Response

```json
{
  "success": true,
  "data": {
    "suggestions": [
      {
        "leak_id": "com.example.UserSessionCache::ff12ab90",
        "class_name": "com.example.UserSessionCache",
        "target_file": "src/main/java/com/example/UserSessionCache.java",
        "description": "Wrap com.example.UserSessionCache allocations in try-with-resources / finally blocks to avoid lingering references.",
        "diff": "--- a/...\n+++ b/...\n@@ public void retain(...)\n-Resource r = allocator.acquire();\n+try (Resource r = allocator.acquire()) {\n+    // existing logic\n+}\n",
        "confidence": 0.72,
        "style": "DEFENSIVE"
      }
    ]
  }
}
```

---

### apply_fix

Apply a proposed fix to the source code.

#### Request

```json
{
  "method": "apply_fix",
  "params": {
    "fix_index": 0,
    "create_backup": true,
    "dry_run": false
  }
}
```

#### Response

```json
{
  "success": true,
  "data": {
    "files_modified": 1,
    "backup_path": "/path/to/project/.mnemosyne/backup-2025-11-30-123456"
  }
}
```

---

## Data Types

### Severity Levels

- `LOW`: Minor issues, informational
- `MEDIUM`: Noticeable memory usage, should investigate
- `HIGH`: Significant leak, fix soon
- `CRITICAL`: Severe leak, fix immediately

### Leak Types

- `COROUTINE`: Suspended coroutines never resumed
- `THREAD`: Threads that should have terminated
- `HTTP_RESPONSE`: Unclosed HTTP responses
- `CLASSLOADER`: ClassLoader preventing unloading
- `CACHE`: Unbounded cache growth
- `COLLECTION`: Collection growing without bounds
- `LISTENER`: Event listeners not unregistered

---

## Error Handling

All errors follow this format:

```json
{
  "success": false,
  "error": {
    "code": "PARSE_ERROR",
    "message": "Failed to parse heap dump: Invalid HPROF magic number",
    "details": {
      "file": "/path/to/heap.hprof",
      "offset": 0
    }
  }
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `FILE_NOT_FOUND` | Heap dump file doesn't exist |
| `PARSE_ERROR` | Invalid or corrupted heap dump |
| `ANALYSIS_ERROR` | Error during leak detection |
| `MAPPING_ERROR` | Failed to map to source code |
| `GIT_ERROR` | Git operation failed |
| `AI_ERROR` | LLM service error |
| `INVALID_PARAMS` | Invalid request parameters |

---

## Rate Limits

AI-powered commands (`explain_leak`, `propose_fix`) are subject to LLM API rate limits.

**Recommendations:**
- Cache results when possible
- Use `dry_run` mode for testing
- Batch multiple analyses

---

## Examples

See [examples/](examples/) directory for complete usage examples.

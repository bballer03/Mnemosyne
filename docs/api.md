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
    "object_id": "0x7f8a9c123456",
    "path": [
      {
        "object_id": "0x7f8a9c123456",
        "class_name": "com.example.Session",
        "field": null,
        "is_root": false
      },
      {
        "object_id": "0x12d687",
        "class_name": "com.example.Session$Holder",
        "field": "value",
        "is_root": false
      },
      {
        "object_id": "GC_ROOT_Thread[root]",
        "class_name": "java.lang.Thread",
        "field": "Thread[root]",
        "is_root": true
      }
    ],
    "path_length": 3
  }
}
```

---

### explain_leak

Get an AI-generated explanation for a detected leak.

#### Request

```json
{
  "method": "explain_leak",
  "params": {
    "leak_id": "leak-001",
    "heap_path": "/path/to/heap.hprof",
    "include_recommendations": true
  }
}
```

#### Response

```json
{
  "success": true,
  "data": {
    "explanation": "The UserSessionCache is retaining stale sessions because the cleanup thread is deadlocked...",
    "root_cause": "Thread deadlock preventing cache cleanup",
    "impact": "512 MB of memory retained unnecessarily",
    "recommendations": [
      "Add timeout to cache.cleanup() method",
      "Use ConcurrentHashMap instead of synchronized HashMap",
      "Consider using weak references for session storage"
    ]
  }
}
```

---

### propose_fix

Generate code fix suggestions for a leak.

#### Request

```json
{
  "method": "propose_fix",
  "params": {
    "leak_id": "leak-001",
    "project_root": "/path/to/project",
    "fix_style": "MINIMAL"
  }
}
```

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `leak_id` | string | Yes | Leak to fix |
| `project_root` | string | Yes | Source code root |
| `fix_style` | string | No | MINIMAL, DEFENSIVE, or COMPREHENSIVE |

#### Response

```json
{
  "success": true,
  "data": {
    "fixes": [
      {
        "file": "src/main/java/com/example/UserSessionCache.java",
        "description": "Add timeout to cleanup method",
        "diff": "--- a/UserSessionCache.java\n+++ b/UserSessionCache.java\n...",
        "confidence": 0.95
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

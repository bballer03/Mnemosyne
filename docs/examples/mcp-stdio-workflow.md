# MCP Stdio Workflow

This example shows one compact stdio session against the live Mnemosyne MCP surface. Start the server in one terminal, then write single-line JSON requests to stdin from your MCP client or a local harness.

## 1. Start the Server

```bash
mnemosyne-cli serve
```

## 2. Discover the Live Tool Catalog

```json
{"id":1,"method":"list_tools","params":{}}
```

Use `list_tools` first if your client wants the machine-readable method list and parameter metadata.

## 3. Parse the Heap

```json
{"id":2,"method":"parse_heap","params":{"path":"heap.hprof","max_objects":500000}}
```

## 4. Detect Leaks with Incident Filters

```json
{"id":3,"method":"detect_leaks","params":{"heap_path":"heap.hprof","package":"com.example","min_severity":"HIGH","leak_types":["CACHE","THREAD"]}}
```

## 5. Run Full Analysis

```json
{"id":4,"method":"analyze_heap","params":{"heap_path":"heap.hprof","packages":["com.example"],"histogram_group_by":"package","enable_threads":true,"enable_strings":true,"enable_collections":true,"enable_classloaders":true,"enable_top_instances":true,"top_n":10,"min_collection_capacity":32}}
```

## 6. Create an AI Follow-Up Session

```json
{"id":5,"method":"create_ai_session","params":{"heap_path":"heap.hprof","min_severity":"HIGH","packages":["com.example"],"leak_types":["CACHE"]}}
```

## 7. Ask a Follow-Up Question in the Session

```json
{"id":6,"method":"chat_session","params":{"session_id":"mcp-1712920000000000000","question":"What should I fix first?","focus_leak_id":"leak-usersession-1"}}
```

## Notes

- Responses remain single-line JSON over stdio, including the AI-session methods.
- [`docs/api.md`](../api.md) is the source of truth for the live wire format, params, and result shapes.

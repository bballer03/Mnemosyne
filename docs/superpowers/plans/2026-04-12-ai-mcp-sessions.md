# AI MCP Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add persisted, heap-bound MCP AI sessions with explicit lifecycle methods and session-backed `chat_session`, `explain_leak`, and `propose_fix` flows while preserving the current `heap_path`-based contracts.

**Architecture:** Keep persistence and lifecycle orchestration inside the MCP layer by adding a small `core/src/mcp/session.rs` helper for session models and file-backed storage. Reuse the existing analysis and AI layers unchanged where possible: create sessions from one `analyze_heap()` pass, persist `summary + leaks + shortlist + focus + bounded history`, and feed that state into the existing `generate_ai_chat_turn_async()`, `generate_ai_insights_async()`, and fix-generation helpers.

**Tech Stack:** Rust, existing `mnemosyne-core` analysis/AI/fix modules, serde JSON persistence, Tokio async tests, tempfile-based filesystem tests, MCP stdio contract tests

---

## File Map

- `core/src/mcp/session.rs`
  - new MCP-local session model and file-backed store
  - owns session IDs, timestamps, JSON persistence, history trimming, and store/load/delete helpers
- `core/src/mcp/mod.rs`
  - export the new MCP session module for internal server use
- `core/src/config.rs`
  - add `[ai.sessions]` config surface for optional storage directory override
- `core/src/mcp/server.rs`
  - add new MCP methods to `tool_catalog()` and `handle_request()`
  - route `create_ai_session`, `resume_ai_session`, `get_ai_session`, `close_ai_session`, and `chat_session`
  - add session-backed branches for `explain_leak` and `propose_fix`
  - map session-specific errors into structured MCP `error_details`
- `core/src/fix/generator.rs`
  - add the smallest internal helper needed to generate fixes from persisted `summary + leaks` context without changing public fix request/response types
- `docs/api.md`
  - document new MCP methods and dual `heap_path` / `session_id` explain/fix behavior
- `README.md`
  - update MCP capability list to include explicit AI session methods
- `ARCHITECTURE.md`
  - update MCP architecture/status notes for persisted AI sessions
- `STATUS.md`
  - move MCP/session follow-through from pending to shipped/partial based on runtime truth
- `docs/roadmap.md`
  - mark the MCP session/context slice complete once verified
- `docs/design/milestone-5-ai-mcp-differentiation.md`
  - sync the design doc with the shipped MCP session model
- `OVERNIGHT_SUMMARY.md`
  - record the batch and verification results

### Task 1: Add Session Config And Store Red Tests

**Files:**
- Modify: `core/src/config.rs`
- Add: `core/src/mcp/session.rs`

- [ ] **Step 1: Write a failing config unit test for `[ai.sessions]` parsing**

Add this test near the existing config parsing tests in `cli/src/config_loader.rs`:

```rust
#[test]
fn parses_ai_session_directory_config() {
    let config = parse_test_config(
        r#"
[ai.sessions]
directory = "C:/tmp/mnemosyne-sessions"
"#,
    );

    assert_eq!(
        config.ai.sessions.directory.as_deref(),
        Some("C:/tmp/mnemosyne-sessions")
    );
}
```

- [ ] **Step 2: Write a failing session-store round-trip test**

Create `core/src/mcp/session.rs` with this initial failing test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{AiChatTurn, LeakInsight, LeakKind, LeakSeverity};
    use crate::hprof::HeapSummary;

    #[test]
    fn session_store_round_trips_persisted_session() {
        let temp = tempfile::tempdir().unwrap();
        let store = McpSessionStore::new(temp.path().to_path_buf());
        let session = PersistedAiSession {
            session_version: 1,
            session_id: "session-123".into(),
            created_at: "2026-04-12T00:00:00Z".into(),
            updated_at: "2026-04-12T00:00:00Z".into(),
            heap_path: "heap.hprof".into(),
            analysis: SessionAnalysisSnapshot {
                min_severity: LeakSeverity::High,
                packages: vec!["com.example".into()],
                leak_types: vec![LeakKind::Cache],
                summary: HeapSummary::default(),
                leaks: vec![LeakInsight {
                    id: "L1".into(),
                    class_name: "com.example.Cache".into(),
                    leak_kind: LeakKind::Cache,
                    severity: LeakSeverity::High,
                    retained_size_bytes: 42,
                    shallow_size_bytes: None,
                    suspect_score: None,
                    instances: 1,
                    description: "cache leak".into(),
                    provenance: Vec::new(),
                }],
                top_leaks: vec!["L1".into()],
            },
            conversation: SessionConversationSnapshot {
                focus_leak_id: Some("L1".into()),
                history: vec![AiChatTurn {
                    question: "why?".into(),
                    answer_summary: "because cache".into(),
                }],
            },
        };

        store.save(&session).unwrap();
        let loaded = store.load("session-123").unwrap();

        assert_eq!(loaded.session_id, session.session_id);
        assert_eq!(loaded.analysis.top_leaks, vec!["L1"]);
        assert_eq!(loaded.conversation.focus_leak_id.as_deref(), Some("L1"));
    }
}
```

- [ ] **Step 3: Write a failing history-trimming test**

In the same test module, add:

```rust
#[test]
fn append_turn_trims_history_to_three_entries() {
    let mut history = vec![
        AiChatTurn { question: "q1".into(), answer_summary: "a1".into() },
        AiChatTurn { question: "q2".into(), answer_summary: "a2".into() },
        AiChatTurn { question: "q3".into(), answer_summary: "a3".into() },
    ];

    trim_history(&mut history, AiChatTurn {
        question: "q4".into(),
        answer_summary: "a4".into(),
    });

    assert_eq!(history.len(), 3);
    assert_eq!(history[0].question, "q2");
    assert_eq!(history[2].question, "q4");
}
```

- [ ] **Step 4: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-cli parses_ai_session_directory_config -- --exact --nocapture
cargo test -p mnemosyne-core session_store_round_trips_persisted_session --lib -- --exact --nocapture
cargo test -p mnemosyne-core append_turn_trims_history_to_three_entries --lib -- --exact --nocapture
```

Expected: FAIL because session config and store types do not exist yet.

### Task 2: Implement Session Config And File-Backed Store

**Files:**
- Modify: `core/src/config.rs`
- Add: `core/src/mcp/session.rs`
- Modify: `core/src/mcp/mod.rs`
- Modify: `cli/src/config_loader.rs`

- [ ] **Step 1: Add AI session config types to `core/src/config.rs`**

Add these types below `AiPromptConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AiSessionConfig {
    pub directory: Option<String>,
}
```

Then add the new field to `AiConfig`:

```rust
pub struct AiConfig {
    pub enabled: bool,
    pub provider: AiProvider,
    pub model: String,
    pub temperature: f32,
    pub mode: AiMode,
    pub tasks: Vec<AiTaskDefinition>,
    pub privacy: AiPrivacyConfig,
    pub prompts: AiPromptConfig,
    pub sessions: AiSessionConfig,
    pub endpoint: Option<String>,
    pub api_key_env: Option<String>,
    pub max_tokens: Option<u32>,
    #[serde(default = "AiConfig::default_timeout_secs")]
    pub timeout_secs: u64,
}
```

And update `impl Default for AiConfig`:

```rust
            privacy: AiPrivacyConfig::default(),
            prompts: AiPromptConfig::default(),
            sessions: AiSessionConfig::default(),
            endpoint: None,
```

- [ ] **Step 2: Parse `[ai.sessions]` in the CLI config loader**

In `cli/src/config_loader.rs`, extend the partial config types to include sessions:

```rust
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PartialAiSessionConfig {
    directory: Option<String>,
}
```

Add the field to `PartialAiConfig`:

```rust
    sessions: Option<PartialAiSessionConfig>,
```

Then update `apply_ai_section()` to apply it:

```rust
    let sessions = section.sessions;
    // existing fields...
    if let Some(value) = sessions.and_then(|sessions| sessions.directory) {
        cfg.sessions.directory = Some(value);
    }
```

- [ ] **Step 3: Implement the session model and store in `core/src/mcp/session.rs`**

Create the file with this minimal production code skeleton:

```rust
use crate::{
    analysis::{AiChatTurn, LeakInsight, LeakKind, LeakSeverity},
    errors::{CoreError, CoreResult},
    hprof::HeapSummary,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub const MCP_SESSION_VERSION: u32 = 1;
pub const MAX_SESSION_HISTORY: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionAnalysisSnapshot {
    pub min_severity: LeakSeverity,
    pub packages: Vec<String>,
    pub leak_types: Vec<LeakKind>,
    pub summary: HeapSummary,
    pub leaks: Vec<LeakInsight>,
    pub top_leaks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionConversationSnapshot {
    pub focus_leak_id: Option<String>,
    pub history: Vec<AiChatTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedAiSession {
    pub session_version: u32,
    pub session_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub heap_path: String,
    pub analysis: SessionAnalysisSnapshot,
    pub conversation: SessionConversationSnapshot,
}

#[derive(Debug, Clone)]
pub struct McpSessionStore {
    root: PathBuf,
}

impl McpSessionStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure_root(&self) -> CoreResult<()> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    pub fn save(&self, session: &PersistedAiSession) -> CoreResult<()> {
        self.ensure_root()?;
        let target = self.path_for(&session.session_id);
        let temp = self.path_for(&format!("{}.tmp", session.session_id));
        let bytes = serde_json::to_vec_pretty(session)?;
        fs::write(&temp, bytes)?;
        fs::rename(&temp, &target)?;
        Ok(())
    }

    pub fn load(&self, session_id: &str) -> CoreResult<PersistedAiSession> {
        let path = self.path_for(session_id);
        let bytes = fs::read(&path).map_err(|err| map_load_error(session_id, err))?;
        let session: PersistedAiSession = serde_json::from_slice(&bytes)
            .map_err(|err| CoreError::InvalidInput(format!("session load failed: {err}")))?;
        if session.session_version != MCP_SESSION_VERSION {
            return Err(CoreError::Unsupported(format!(
                "session_version {} is unsupported",
                session.session_version
            )));
        }
        Ok(session)
    }

    pub fn delete(&self, session_id: &str) -> CoreResult<()> {
        let path = self.path_for(session_id);
        fs::remove_file(path).map_err(|err| map_load_error(session_id, err))?;
        Ok(())
    }

    fn path_for(&self, session_id: &str) -> PathBuf {
        self.root.join(format!("{session_id}.json"))
    }
}

pub fn trim_history(history: &mut Vec<AiChatTurn>, turn: AiChatTurn) {
    history.push(turn);
    if history.len() > MAX_SESSION_HISTORY {
        let excess = history.len() - MAX_SESSION_HISTORY;
        history.drain(0..excess);
    }
}

pub fn top_leak_ids(leaks: &[LeakInsight]) -> Vec<String> {
    leaks.iter().take(3).map(|leak| leak.id.clone()).collect()
}

pub fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("mcp-{}", nanos)
}

pub fn timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

fn map_load_error(session_id: &str, err: std::io::Error) -> CoreError {
    if err.kind() == std::io::ErrorKind::NotFound {
        return CoreError::InvalidInput(format!("session not found: {session_id}"));
    }
    CoreError::Io(err)
}
```

- [ ] **Step 4: Export the new session module**

In `core/src/mcp/mod.rs`, change:

```rust
pub mod server;

pub use server::*;
```

to:

```rust
pub mod server;
pub mod session;

pub use server::*;
```

- [ ] **Step 5: Run the focused tests and make them pass**

Run:

```bash
cargo test -p mnemosyne-cli parses_ai_session_directory_config -- --exact --nocapture
cargo test -p mnemosyne-core session_store_round_trips_persisted_session --lib -- --exact --nocapture
cargo test -p mnemosyne-core append_turn_trims_history_to_three_entries --lib -- --exact --nocapture
```

Expected: PASS.

### Task 3: Add MCP Lifecycle Red Tests

**Files:**
- Modify: `core/src/mcp/server.rs`

- [ ] **Step 1: Write a failing `create_ai_session` MCP test**

Add this test in the existing `#[cfg(test)] mod tests` block in `core/src/mcp/server.rs`:

```rust
#[tokio::test]
async fn handle_request_create_ai_session_returns_session_metadata() {
    let fixture = build_graph_fixture();
    let mut heap = NamedTempFile::new().unwrap();
    heap.write_all(&fixture).unwrap();

    let mut config = AppConfig::default();
    config.ai.sessions.directory = Some(tempfile::tempdir().unwrap().path().display().to_string());

    let result = handle_request(
        RpcRequest {
            id: json!(11),
            method: "create_ai_session".into(),
            params: json!({
                "heap_path": heap.path().to_string_lossy().into_owned()
            }),
        },
        &config,
    )
    .await
    .unwrap();

    assert!(result.get("session_id").and_then(Value::as_str).is_some());
    assert_eq!(result.get("focus_leak_id"), Some(&Value::Null));
    assert!(result.get("top_leaks").and_then(Value::as_array).is_some());
}
```

- [ ] **Step 2: Write a failing `resume_ai_session` MCP test**

Add:

```rust
#[tokio::test]
async fn handle_request_resume_ai_session_reads_persisted_state() {
    let fixture = build_graph_fixture();
    let mut heap = NamedTempFile::new().unwrap();
    heap.write_all(&fixture).unwrap();
    let sessions = tempfile::tempdir().unwrap();

    let mut config = AppConfig::default();
    config.ai.sessions.directory = Some(sessions.path().display().to_string());

    let created = handle_request(
        RpcRequest {
            id: json!(12),
            method: "create_ai_session".into(),
            params: json!({
                "heap_path": heap.path().to_string_lossy().into_owned()
            }),
        },
        &config,
    )
    .await
    .unwrap();

    let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

    let resumed = handle_request(
        RpcRequest {
            id: json!(13),
            method: "resume_ai_session".into(),
            params: json!({ "session_id": session_id }),
        },
        &config,
    )
    .await
    .unwrap();

    assert_eq!(resumed.get("session_id"), Some(&json!(session_id)));
    assert!(resumed.get("history").and_then(Value::as_array).is_some());
}
```

- [ ] **Step 3: Write a failing `close_ai_session` MCP test**

Add:

```rust
#[tokio::test]
async fn handle_request_close_ai_session_removes_persisted_state() {
    let fixture = build_graph_fixture();
    let mut heap = NamedTempFile::new().unwrap();
    heap.write_all(&fixture).unwrap();
    let sessions = tempfile::tempdir().unwrap();

    let mut config = AppConfig::default();
    config.ai.sessions.directory = Some(sessions.path().display().to_string());

    let created = handle_request(
        RpcRequest {
            id: json!(14),
            method: "create_ai_session".into(),
            params: json!({
                "heap_path": heap.path().to_string_lossy().into_owned()
            }),
        },
        &config,
    )
    .await
    .unwrap();

    let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

    handle_request(
        RpcRequest {
            id: json!(15),
            method: "close_ai_session".into(),
            params: json!({ "session_id": session_id }),
        },
        &config,
    )
    .await
    .unwrap();

    let err = handle_request(
        RpcRequest {
            id: json!(16),
            method: "resume_ai_session".into(),
            params: json!({ "session_id": session_id }),
        },
        &config,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("session"));
}
```

- [ ] **Step 4: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-core handle_request_create_ai_session_returns_session_metadata --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_resume_ai_session_reads_persisted_state --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_close_ai_session_removes_persisted_state --lib -- --exact --nocapture
```

Expected: FAIL because the new MCP methods do not exist yet.

### Task 4: Implement MCP Session Lifecycle Methods

**Files:**
- Modify: `core/src/mcp/server.rs`
- Modify: `core/src/mcp/session.rs`

- [ ] **Step 1: Add MCP param structs for session lifecycle methods**

In `core/src/mcp/server.rs`, add these new request param structs near the existing MCP param definitions:

```rust
#[derive(Debug, Deserialize)]
struct CreateAiSessionParams {
    heap_path: String,
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
    #[serde(default)]
    packages: Vec<String>,
    #[serde(default)]
    leak_types: Vec<LeakKind>,
}

#[derive(Debug, Deserialize)]
struct SessionIdParams {
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct ChatSessionParams {
    session_id: String,
    question: String,
    #[serde(default)]
    focus_leak_id: Option<String>,
}
```

- [ ] **Step 2: Add session-store resolution helpers**

In `core/src/mcp/server.rs`, add these helper functions above `handle_request()`:

```rust
fn session_store(config: &AppConfig) -> crate::mcp::session::McpSessionStore {
    let root = config
        .ai
        .sessions
        .directory
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(default_session_directory);
    crate::mcp::session::McpSessionStore::new(root)
}

fn default_session_directory() -> PathBuf {
    if let Some(mut dir) = dirs::data_local_dir() {
        dir.push("mnemosyne");
        dir.push("ai-sessions");
        return dir;
    }

    let mut fallback = std::env::temp_dir();
    fallback.push("mnemosyne-ai-sessions");
    fallback
}
```

- [ ] **Step 3: Add `tool_catalog()` entries for the new MCP methods**

In `tool_catalog()`, add new tool definitions for:

```rust
{
    "name": "create_ai_session",
    "description": "Analyze a heap once and persist an AI follow-up session.",
    "params": [
        { "name": "heap_path", "type": "string", "required": true, "description": "Path to the heap dump." },
        { "name": "min_severity", "type": "string", "required": false, "description": "Optional minimum leak severity override." },
        { "name": "packages", "type": "array<string>", "required": false, "description": "Optional package prefix filters." },
        { "name": "leak_types", "type": "array<string>", "required": false, "description": "Optional leak-kind filter list." }
    ]
},
{
    "name": "resume_ai_session",
    "description": "Resume a persisted AI follow-up session by session_id.",
    "params": [
        { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." }
    ]
},
{
    "name": "get_ai_session",
    "description": "Inspect compact metadata for a persisted AI session.",
    "params": [
        { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." }
    ]
},
{
    "name": "close_ai_session",
    "description": "Delete a persisted AI follow-up session.",
    "params": [
        { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." }
    ]
},
{
    "name": "chat_session",
    "description": "Ask a follow-up AI question against a persisted session.",
    "params": [
        { "name": "session_id", "type": "string", "required": true, "description": "Persisted AI session identifier." },
        { "name": "question", "type": "string", "required": true, "description": "Follow-up question to ask." },
        { "name": "focus_leak_id", "type": "string", "required": false, "description": "Optional leak identifier to focus the turn." }
    ]
}
```

- [ ] **Step 4: Implement lifecycle branches in `handle_request()`**

Add new match arms in `handle_request()` that follow this structure:

```rust
        "create_ai_session" => {
            let params: CreateAiSessionParams = serde_json::from_value(packet.params)?;
            let mut request_config = config.clone();
            if !params.packages.is_empty() {
                request_config.analysis.packages = params.packages.clone();
            }
            if !params.leak_types.is_empty() {
                request_config.analysis.leak_types = params.leak_types.clone();
            }
            request_config.ai.enabled = false;

            let mut leak_options = LeakDetectionOptions::from(&request_config.analysis);
            if let Some(sev) = params.min_severity {
                leak_options.min_severity = sev;
            }

            let analysis = analyze_heap(AnalyzeRequest {
                heap_path: params.heap_path.clone(),
                config: request_config,
                leak_options: leak_options.clone(),
                enable_ai: false,
                histogram_group_by: HistogramGroupBy::Class,
                ..AnalyzeRequest::default()
            })
            .await?;

            let session = crate::mcp::session::PersistedAiSession {
                session_version: crate::mcp::session::MCP_SESSION_VERSION,
                session_id: crate::mcp::session::new_session_id(),
                created_at: crate::mcp::session::timestamp_now(),
                updated_at: crate::mcp::session::timestamp_now(),
                heap_path: params.heap_path,
                analysis: crate::mcp::session::SessionAnalysisSnapshot {
                    min_severity: leak_options.min_severity,
                    packages: leak_options.package_filters,
                    leak_types: leak_options.leak_types,
                    summary: analysis.summary,
                    leaks: analysis.leaks.clone(),
                    top_leaks: crate::mcp::session::top_leak_ids(&analysis.leaks),
                },
                conversation: crate::mcp::session::SessionConversationSnapshot {
                    focus_leak_id: None,
                    history: Vec::new(),
                },
            };

            let store = session_store(config);
            store.save(&session)?;
            Ok(serde_json::json!({
                "session_id": session.session_id,
                "created_at": session.created_at,
                "updated_at": session.updated_at,
                "heap_path": session.heap_path,
                "summary": session.analysis.summary,
                "leak_count": session.analysis.leaks.len(),
                "top_leaks": session.analysis.top_leaks,
                "focus_leak_id": session.conversation.focus_leak_id,
            }))
        }
```

Implement `resume_ai_session`, `get_ai_session`, and `close_ai_session` the same way the spec requires, using the store load/delete helpers and returning compact JSON values.

- [ ] **Step 5: Run the lifecycle MCP tests and make them pass**

Run:

```bash
cargo test -p mnemosyne-core handle_request_create_ai_session_returns_session_metadata --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_resume_ai_session_reads_persisted_state --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_close_ai_session_removes_persisted_state --lib -- --exact --nocapture
```

Expected: PASS.

### Task 5: Add Session-Backed AI Follow-Up Red Tests

**Files:**
- Modify: `core/src/mcp/server.rs`
- Modify: `core/src/fix/generator.rs`

- [ ] **Step 1: Write a failing `chat_session` MCP test**

Add this test to `core/src/mcp/server.rs`:

```rust
#[tokio::test]
async fn handle_request_chat_session_updates_history_and_focus() {
    let fixture = build_graph_fixture();
    let mut heap = NamedTempFile::new().unwrap();
    heap.write_all(&fixture).unwrap();
    let sessions = tempfile::tempdir().unwrap();

    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.mode = crate::config::AiMode::Rules;
    config.ai.sessions.directory = Some(sessions.path().display().to_string());

    let created = handle_request(
        RpcRequest {
            id: json!(21),
            method: "create_ai_session".into(),
            params: json!({
                "heap_path": heap.path().to_string_lossy().into_owned()
            }),
        },
        &config,
    )
    .await
    .unwrap();

    let session_id = created.get("session_id").and_then(Value::as_str).unwrap();
    let leak_id = created
        .get("top_leaks")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .unwrap();

    let ai = handle_request(
        RpcRequest {
            id: json!(22),
            method: "chat_session".into(),
            params: json!({
                "session_id": session_id,
                "question": "What should I fix first?",
                "focus_leak_id": leak_id
            }),
        },
        &config,
    )
    .await
    .unwrap();

    assert!(ai.get("summary").and_then(Value::as_str).is_some());

    let resumed = handle_request(
        RpcRequest {
            id: json!(23),
            method: "resume_ai_session".into(),
            params: json!({ "session_id": session_id }),
        },
        &config,
    )
    .await
    .unwrap();

    assert_eq!(resumed.get("focus_leak_id"), Some(&json!(leak_id)));
    assert_eq!(
        resumed
            .get("history")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
}
```

- [ ] **Step 2: Write a failing session-backed `explain_leak` MCP test**

Add:

```rust
#[tokio::test]
async fn handle_request_explain_leak_supports_session_id() {
    let fixture = build_graph_fixture();
    let mut heap = NamedTempFile::new().unwrap();
    heap.write_all(&fixture).unwrap();
    let sessions = tempfile::tempdir().unwrap();

    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.mode = crate::config::AiMode::Rules;
    config.ai.sessions.directory = Some(sessions.path().display().to_string());

    let created = handle_request(
        RpcRequest {
            id: json!(24),
            method: "create_ai_session".into(),
            params: json!({
                "heap_path": heap.path().to_string_lossy().into_owned()
            }),
        },
        &config,
    )
    .await
    .unwrap();

    let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

    let explained = handle_request(
        RpcRequest {
            id: json!(25),
            method: "explain_leak".into(),
            params: json!({ "session_id": session_id }),
        },
        &config,
    )
    .await
    .unwrap();

    assert!(explained.get("summary").and_then(Value::as_str).is_some());
}
```

- [ ] **Step 3: Write a failing session-backed `propose_fix` MCP test**

Add:

```rust
#[tokio::test]
async fn handle_request_propose_fix_supports_session_id() {
    let fixture = build_graph_fixture();
    let mut heap = NamedTempFile::new().unwrap();
    heap.write_all(&fixture).unwrap();
    let sessions = tempfile::tempdir().unwrap();

    let mut config = AppConfig::default();
    config.ai.enabled = false;
    config.ai.sessions.directory = Some(sessions.path().display().to_string());

    let created = handle_request(
        RpcRequest {
            id: json!(26),
            method: "create_ai_session".into(),
            params: json!({
                "heap_path": heap.path().to_string_lossy().into_owned()
            }),
        },
        &config,
    )
    .await
    .unwrap();

    let session_id = created.get("session_id").and_then(Value::as_str).unwrap();

    let fix = handle_request(
        RpcRequest {
            id: json!(27),
            method: "propose_fix".into(),
            params: json!({
                "session_id": session_id,
                "style": "Minimal"
            }),
        },
        &config,
    )
    .await
    .unwrap();

    assert!(fix.get("suggestions").and_then(Value::as_array).is_some());
}
```

- [ ] **Step 4: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-core handle_request_chat_session_updates_history_and_focus --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_explain_leak_supports_session_id --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_propose_fix_supports_session_id --lib -- --exact --nocapture
```

Expected: FAIL because the session-backed follow-up paths do not exist yet.

### Task 6: Implement Session-Backed Chat, Explain, And Fix

**Files:**
- Modify: `core/src/mcp/server.rs`
- Modify: `core/src/mcp/session.rs`
- Modify: `core/src/fix/generator.rs`

- [ ] **Step 1: Add a minimal internal fix helper for persisted leak context**

In `core/src/fix/generator.rs`, add this helper near `propose_fix_with_config()`:

```rust
pub(crate) async fn propose_fix_for_leaks_with_config(
    leaks: &[LeakInsight],
    request: &FixRequest,
    base_config: &AppConfig,
) -> CoreResult<FixResponse> {
    if let Some(ref target) = request.leak_id {
        validate_leak_id(leaks, target)?;
    }

    let leaks = focus_leaks(leaks, request.leak_id.as_deref());
    let Some(leak) = leaks.into_iter().next() else {
        return Ok(FixResponse {
            suggestions: Vec::new(),
            project_root: request.project_root.clone(),
            provenance: Vec::new(),
        });
    };

    let mut provenance = Vec::new();
    let suggestion = if base_config.ai.enabled
        && matches!(base_config.ai.mode, crate::config::AiMode::Provider)
    {
        if let Some(root) = request.project_root.as_deref() {
            if let Some(source) = source_snippet_for_leak(&leak, root) {
                let prompt = build_provider_fix_prompt(
                    &leak,
                    &request.style,
                    &source.diff_target_file,
                    &source.snippet,
                    &request.heap_path,
                );
                let prompt = crate::analysis::redact_provider_prompt(prompt, &base_config.ai)?;
                let ai_config = base_config.ai.clone();
                let draft_result = tokio::task::spawn_blocking(move || {
                    crate::analysis::complete_provider_prompt(prompt, &ai_config)
                        .and_then(|raw| parse_provider_fix_response(&raw))
                })
                .await;

                match draft_result.map_err(|err| crate::CoreError::Other(err.into())) {
                    Ok(Ok(draft))
                        if validate_provider_fix_diff(&draft.diff, &source.diff_target_file) =>
                    {
                        FixSuggestion {
                            leak_id: leak.id.clone(),
                            class_name: leak.class_name.clone(),
                            target_file: source.target_file,
                            description: draft.description,
                            diff: draft.diff,
                            confidence: draft.confidence,
                            style: request.style.clone(),
                        }
                    }
                    _ => {
                        provenance = fallback_provenance(
                            "Provider-backed fix generation was unavailable; returned heuristic guidance instead.",
                        );
                        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
                    }
                }
            } else {
                provenance = fallback_provenance(
                    "Provider-backed fix generation was skipped because source targeting or snippet extraction was unavailable.",
                );
                build_suggestion(&leak, request.project_root.as_deref(), &request.style)
            }
        } else {
            provenance = fallback_provenance(
                "Provider-backed fix generation was skipped because project_root was not provided.",
            );
            build_suggestion(&leak, request.project_root.as_deref(), &request.style)
        }
    } else {
        provenance = fallback_provenance(
            "Provider-backed fix generation was skipped because AI provider mode is not active.",
        );
        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
    };

    Ok(FixResponse {
        suggestions: vec![suggestion],
        project_root: request.project_root.clone(),
        provenance,
    })
}
```

Then change `propose_fix_with_config()` to reuse it after `analyze_heap()`:

```rust
    propose_fix_for_leaks_with_config(&analysis.leaks, &request, &config).await
```

- [ ] **Step 2: Broaden `ExplainLeakParams` and `ProposeFixParams` to allow `session_id`**

In `core/src/mcp/server.rs`, change the param structs to:

```rust
#[derive(Debug, Deserialize)]
struct ExplainLeakParams {
    #[serde(default)]
    heap_path: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    leak_id: Option<String>,
    #[serde(default)]
    min_severity: Option<LeakSeverity>,
}

#[derive(Debug, Deserialize)]
struct ProposeFixParams {
    #[serde(default)]
    heap_path: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    leak_id: Option<String>,
    #[serde(default)]
    project_root: Option<PathBuf>,
    #[serde(default = "default_fix_style")]
    style: FixStyle,
}
```

- [ ] **Step 3: Implement `chat_session` in `handle_request()`**

Add a new match arm in `handle_request()` with this structure:

```rust
        "chat_session" => {
            let params: ChatSessionParams = serde_json::from_value(packet.params)?;
            let store = session_store(config);
            let mut session = store.load(&params.session_id)?;

            if let Some(ref target) = params.focus_leak_id {
                validate_leak_id(&session.analysis.leaks, target)?;
            }

            let active_focus = params
                .focus_leak_id
                .as_deref()
                .or(session.conversation.focus_leak_id.as_deref());
            let leaks = match active_focus {
                Some(focus) => focus_leaks(&session.analysis.leaks, Some(focus)),
                None => focus_leaks(&session.analysis.leaks, None),
                    };

            let mut ai_config = config.ai.clone();
            ai_config.enabled = true;
            let ai = generate_ai_chat_turn_async(
                &session.analysis.summary,
                &leaks,
                &params.question,
                &session.conversation.history,
                active_focus,
                &ai_config,
            )
            .await?;

            if let Some(target) = params.focus_leak_id {
                session.conversation.focus_leak_id = Some(target);
            }
            crate::mcp::session::trim_history(
                &mut session.conversation.history,
                crate::analysis::AiChatTurn {
                    question: params.question,
                    answer_summary: ai.summary.clone(),
                },
            );
            session.updated_at = crate::mcp::session::timestamp_now();
            store.save(&session)?;
            Ok(serde_json::to_value(ai)?)
        }
```

- [ ] **Step 4: Implement session-backed `explain_leak` and `propose_fix` branches**

Replace the existing match arms with dual-path branches:

```rust
        "explain_leak" => {
            let params: ExplainLeakParams = serde_json::from_value(packet.params)?;
            match (&params.heap_path, &params.session_id) {
                (Some(_), Some(_)) | (None, None) => {
                    Err(CoreError::InvalidInput(
                        "exactly one of heap_path or session_id is required".into(),
                    ))
                }
                (Some(heap_path), None) => {
                    // keep the existing heap-based path, adapted to optional heap_path
                }
                (None, Some(session_id)) => {
                    if params.min_severity.is_some() {
                        return Err(CoreError::InvalidInput(
                            "min_severity is not supported for session-backed explain_leak"
                                .into(),
                        ));
                    }
                    let store = session_store(config);
                    let session = store.load(session_id)?;
                    let leak_id = params
                        .leak_id
                        .clone()
                        .or(session.conversation.focus_leak_id.clone());
                    if let Some(ref target) = leak_id {
                        validate_leak_id(&session.analysis.leaks, target)?;
                    }
                    let focused = focus_leaks(&session.analysis.leaks, leak_id.as_deref());
                    let mut ai_config = config.ai.clone();
                    ai_config.enabled = true;
                    let ai = generate_ai_insights_async(
                        &session.analysis.summary,
                        &focused,
                        &ai_config,
                    )
                    .await?;
                    Ok(serde_json::to_value(ai)?)
                }
            }
        }
```

And for `propose_fix`:

```rust
        "propose_fix" => {
            let params: ProposeFixParams = serde_json::from_value(packet.params)?;
            match (&params.heap_path, &params.session_id) {
                (Some(_), Some(_)) | (None, None) => Err(CoreError::InvalidInput(
                    "exactly one of heap_path or session_id is required".into(),
                )),
                (Some(heap_path), None) => {
                    let response = propose_fix_with_config(
                        FixRequest {
                            heap_path: heap_path.clone(),
                            leak_id: params.leak_id,
                            style: params.style,
                            project_root: params.project_root,
                        },
                        config,
                    )
                    .await?;
                    Ok(serde_json::to_value(response)?)
                }
                (None, Some(session_id)) => {
                    let store = session_store(config);
                    let session = store.load(session_id)?;
                    let leak_id = params
                        .leak_id
                        .clone()
                        .or(session.conversation.focus_leak_id.clone());
                    let response = crate::fix::propose_fix_for_leaks_with_config(
                        &session.analysis.leaks,
                        &FixRequest {
                            heap_path: session.heap_path,
                            leak_id,
                            style: params.style,
                            project_root: params.project_root,
                        },
                        config,
                    )
                    .await?;
                    Ok(serde_json::to_value(response)?)
                }
            }
        }
```

- [ ] **Step 5: Run the focused follow-up tests and make them pass**

Run:

```bash
cargo test -p mnemosyne-core handle_request_chat_session_updates_history_and_focus --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_explain_leak_supports_session_id --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_propose_fix_supports_session_id --lib -- --exact --nocapture
```

Expected: PASS.

### Task 7: Add Regression Tests For Error Semantics And Catalog Metadata

**Files:**
- Modify: `core/src/mcp/server.rs`

- [ ] **Step 1: Write a failing error test for conflicting `heap_path` and `session_id`**

Add:

```rust
#[tokio::test]
async fn handle_request_explain_leak_rejects_conflicting_context_sources() {
    let err = handle_request(
        RpcRequest {
            id: json!(31),
            method: "explain_leak".into(),
            params: json!({
                "heap_path": "heap.hprof",
                "session_id": "session-123"
            }),
        },
        &AppConfig::default(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("exactly one of heap_path or session_id"));
}
```

- [ ] **Step 2: Write a failing catalog test for new tools**

Add:

```rust
#[tokio::test]
async fn handle_request_list_tools_includes_ai_session_methods() {
    let result = handle_request(
        RpcRequest {
            id: json!(32),
            method: "list_tools".into(),
            params: Value::Null,
        },
        &AppConfig::default(),
    )
    .await
    .unwrap();

    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools array");

    for name in [
        "create_ai_session",
        "resume_ai_session",
        "get_ai_session",
        "close_ai_session",
        "chat_session",
    ] {
        assert!(tools.iter().any(|tool| tool.get("name") == Some(&json!(name))), "missing {name}");
    }
}
```

- [ ] **Step 3: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p mnemosyne-core handle_request_explain_leak_rejects_conflicting_context_sources --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_list_tools_includes_ai_session_methods --lib -- --exact --nocapture
```

Expected: FAIL until the new validation and catalog entries are in place.

### Task 8: Finish Error Mapping, Catalog, And Documentation Sync

**Files:**
- Modify: `core/src/mcp/server.rs`
- Modify: `README.md`
- Modify: `ARCHITECTURE.md`
- Modify: `STATUS.md`
- Modify: `docs/api.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/design/milestone-5-ai-mcp-differentiation.md`
- Modify: `OVERNIGHT_SUMMARY.md`

- [ ] **Step 1: Add explicit session error mapping in `RpcErrorDetails::from_core_error()`**

In `core/src/mcp/server.rs`, add `InvalidInput` classification branches before the generic `invalid_input` fallback:

```rust
            CoreError::InvalidInput(detail) if detail.starts_with("session not found:") => Self {
                code: "session_not_found",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::InvalidInput(detail) if detail.starts_with("session load failed:") => Self {
                code: "session_load_failed",
                message,
                details: Some(json!({ "detail": detail })),
            },
            CoreError::Unsupported(detail) if detail.contains("session_version") => Self {
                code: "session_version_unsupported",
                message,
                details: Some(json!({ "detail": detail })),
            },
```

And add an `Io` classification branch for persistence failures when needed at the call sites by wrapping save/delete failures with `CoreError::InvalidInput` or `CoreError::Other(anyhow::anyhow!(...))` containing `session persist failed:` text.

- [ ] **Step 2: Make the validation and catalog tests pass**

Run:

```bash
cargo test -p mnemosyne-core handle_request_explain_leak_rejects_conflicting_context_sources --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_list_tools_includes_ai_session_methods --lib -- --exact --nocapture
```

Expected: PASS.

- [ ] **Step 3: Update user-facing docs to match the shipped MCP session behavior**

Apply these content updates:

In `README.md`, update the MCP method list from:

```markdown
- list_tools
- parse_heap
- analyze_heap
- query_heap
- detect_leaks
- map_to_code
- find_gc_path
- explain_leak
- propose_fix
```

to:

```markdown
- list_tools
- parse_heap
- analyze_heap
- query_heap
- detect_leaks
- map_to_code
- find_gc_path
- create_ai_session
- resume_ai_session
- get_ai_session
- close_ai_session
- chat_session
- explain_leak
- propose_fix
```

In `docs/api.md`, add an MCP section documenting:

- `create_ai_session`
- `resume_ai_session`
- `get_ai_session`
- `close_ai_session`
- `chat_session`
- `explain_leak` with `heap_path` or `session_id`
- `propose_fix` with `heap_path` or `session_id`

In `STATUS.md` and `ARCHITECTURE.md`, replace wording that says MCP/session semantics remain pending with wording that says persisted AI follow-up sessions are shipped, while streaming and broader workspace sessions remain future work.

In `docs/roadmap.md` and `docs/design/milestone-5-ai-mcp-differentiation.md`, mark the MCP session/context slice complete and describe the actual explicit lifecycle model.

In `OVERNIGHT_SUMMARY.md`, add a short bullet list for the session batch and its verification commands.

- [ ] **Step 4: Run targeted docs-adjacent verification**

Run:

```bash
cargo test -p mnemosyne-core handle_request_list_tools_returns_descriptions --lib -- --exact --nocapture
cargo test -p mnemosyne-core handle_request_list_tools_includes_ai_session_methods --lib -- --exact --nocapture
```

Expected: PASS.

### Task 9: Run Full Verification

**Files:**
- None

- [ ] **Step 1: Run the targeted MCP and fix suite**

Run:

```bash
cargo test -p mnemosyne-core mcp::server::tests -- --nocapture
cargo test -p mnemosyne-core fix::generator::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run the full workspace test suite**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 3: Run formatting and lint-style verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS.

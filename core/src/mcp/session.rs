use crate::{
    analysis::{AiChatTurn, LeakInsight, LeakKind, LeakSeverity},
    errors::{CoreError, CoreResult},
    hprof::HeapSummary,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MCP_SESSION_VERSION: u32 = 1;
pub const MAX_SESSION_HISTORY: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAnalysisSnapshot {
    pub min_severity: LeakSeverity,
    pub packages: Vec<String>,
    pub leak_types: Vec<LeakKind>,
    pub summary: HeapSummary,
    pub leaks: Vec<LeakInsight>,
    pub top_leaks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConversationSnapshot {
    pub focus_leak_id: Option<String>,
    pub history: Vec<AiChatTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let target = self.path_for(&session.session_id)?;
        let temp = self.root.join(format!("{}.tmp", session.session_id));
        let payload = serde_json::to_vec_pretty(session)?;
        fs::write(&temp, payload)?;
        replace_session_file(&temp, &target)?;
        Ok(())
    }

    pub fn load(&self, session_id: &str) -> CoreResult<PersistedAiSession> {
        let path = self.path_for(session_id)?;
        let bytes = fs::read(&path).map_err(|err| map_load_error(session_id, err))?;
        let session: PersistedAiSession = serde_json::from_slice(&bytes).map_err(|err| {
            CoreError::InvalidInput(format!("session load failed: {session_id}: {err}"))
        })?;
        if session.session_id != session_id {
            return Err(CoreError::InvalidInput(format!(
                "session load failed: {session_id}: embedded session_id {} does not match requested session_id {session_id}",
                session.session_id
            )));
        }
        if session.session_version != MCP_SESSION_VERSION {
            return Err(CoreError::Unsupported(format!(
                "session_version {} is unsupported",
                session.session_version
            )));
        }
        Ok(session)
    }

    pub fn delete(&self, session_id: &str) -> CoreResult<()> {
        let path = self.path_for(session_id)?;
        fs::remove_file(path).map_err(|err| map_load_error(session_id, err))?;
        Ok(())
    }

    fn path_for(&self, session_id: &str) -> CoreResult<PathBuf> {
        validate_session_id(session_id)?;
        Ok(self.root.join(format!("{session_id}.json")))
    }
}

trait SessionFileOps {
    fn exists(&self, path: &Path) -> bool;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
}

struct StdSessionFileOps;

impl SessionFileOps for StdSessionFileOps {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

fn replace_session_file(temp: &Path, target: &Path) -> io::Result<()> {
    replace_session_file_with_ops(temp, target, &StdSessionFileOps)
}

fn replace_session_file_with_ops(
    temp: &Path,
    target: &Path,
    ops: &dyn SessionFileOps,
) -> io::Result<()> {
    if !ops.exists(target) {
        return ops.rename(temp, target);
    }

    let backup = backup_path(target);
    if ops.exists(&backup) {
        ops.remove_file(&backup)?;
    }

    ops.rename(target, &backup)?;
    match ops.rename(temp, target) {
        Ok(()) => {
            ops.remove_file(&backup)?;
            Ok(())
        }
        Err(err) => {
            let _ = ops.rename(&backup, target);
            Err(err)
        }
    }
}

fn backup_path(target: &Path) -> PathBuf {
    let mut backup = target.as_os_str().to_os_string();
    backup.push(".bak");
    PathBuf::from(backup)
}

fn validate_session_id(session_id: &str) -> CoreResult<()> {
    if session_id.is_empty()
        || !session_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err(CoreError::InvalidInput(format!(
            "invalid session_id: {session_id}"
        )));
    }

    Ok(())
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
    format!("mcp-{nanos}")
}

pub fn timestamp_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs.to_string()
}

fn map_load_error(session_id: &str, err: std::io::Error) -> CoreError {
    if err.kind() == std::io::ErrorKind::NotFound {
        return CoreError::InvalidInput(format!("session not found: {session_id}"));
    }
    CoreError::Io(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{AiChatTurn, LeakInsight};
    use std::{
        cell::RefCell,
        collections::HashMap,
        io,
        path::{Path, PathBuf},
    };

    struct MockFileOps {
        entries: RefCell<HashMap<PathBuf, &'static str>>,
        fail_rename: Option<(PathBuf, PathBuf)>,
    }

    impl MockFileOps {
        fn new(entries: impl IntoIterator<Item = (PathBuf, &'static str)>) -> Self {
            Self {
                entries: RefCell::new(entries.into_iter().collect()),
                fail_rename: None,
            }
        }

        fn with_failed_rename(mut self, from: PathBuf, to: PathBuf) -> Self {
            self.fail_rename = Some((from, to));
            self
        }

        fn entry(&self, path: &Path) -> Option<&'static str> {
            self.entries.borrow().get(path).copied()
        }
    }

    impl SessionFileOps for MockFileOps {
        fn exists(&self, path: &Path) -> bool {
            self.entries.borrow().contains_key(path)
        }

        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            if self
                .fail_rename
                .as_ref()
                .is_some_and(|(fail_from, fail_to)| fail_from == from && fail_to == to)
            {
                return Err(io::Error::other("simulated rename failure"));
            }

            let mut entries = self.entries.borrow_mut();
            let value = entries
                .remove(from)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing source"))?;
            entries.insert(to.to_path_buf(), value);
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            self.entries
                .borrow_mut()
                .remove(path)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing path"))?;
            Ok(())
        }
    }

    fn sample_session(session_id: &str) -> PersistedAiSession {
        PersistedAiSession {
            session_version: 1,
            session_id: session_id.into(),
            created_at: "2026-04-12T00:00:00Z".into(),
            updated_at: "2026-04-12T00:00:00Z".into(),
            heap_path: "heap.hprof".into(),
            analysis: SessionAnalysisSnapshot {
                min_severity: LeakSeverity::High,
                packages: vec!["com.example".into()],
                leak_types: vec![LeakKind::Cache],
                summary: HeapSummary {
                    heap_path: "heap.hprof".into(),
                    total_objects: 0,
                    total_size_bytes: 0,
                    classes: Vec::new(),
                    generated_at: SystemTime::UNIX_EPOCH,
                    header: None,
                    total_records: 0,
                    record_stats: Vec::new(),
                },
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
        }
    }

    #[test]
    fn session_store_round_trips_persisted_session() {
        let temp = tempfile::tempdir().unwrap();
        let store = McpSessionStore::new(temp.path().to_path_buf());
        let session = sample_session("session-123");

        store.save(&session).unwrap();
        let loaded = store.load("session-123").unwrap();

        assert_eq!(loaded.session_id, session.session_id);
        assert_eq!(loaded.analysis.top_leaks, vec!["L1"]);
        assert_eq!(loaded.conversation.focus_leak_id.as_deref(), Some("L1"));
    }

    #[test]
    fn append_turn_trims_history_to_three_entries() {
        let mut history = vec![
            AiChatTurn {
                question: "q1".into(),
                answer_summary: "a1".into(),
            },
            AiChatTurn {
                question: "q2".into(),
                answer_summary: "a2".into(),
            },
            AiChatTurn {
                question: "q3".into(),
                answer_summary: "a3".into(),
            },
        ];

        trim_history(
            &mut history,
            AiChatTurn {
                question: "q4".into(),
                answer_summary: "a4".into(),
            },
        );

        assert_eq!(history.len(), 3);
        assert_eq!(history[0].question, "q2");
        assert_eq!(history[2].question, "q4");
    }

    #[test]
    fn session_store_rejects_path_traversal_session_ids() {
        let parent = tempfile::tempdir().unwrap();
        let sessions = parent.path().join("sessions");
        let store = McpSessionStore::new(sessions);
        let escaped_path = parent.path().join("escaped.json");
        std::fs::write(
            &escaped_path,
            serde_json::to_vec_pretty(&sample_session("escaped")).unwrap(),
        )
        .unwrap();

        let err = store.load("../escaped").unwrap_err();

        assert!(err.to_string().contains("invalid session_id"));
    }

    #[test]
    fn session_store_rejects_embedded_session_id_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let store = McpSessionStore::new(temp.path().to_path_buf());
        let session = sample_session("session-actual");
        let path = temp.path().join("session-expected.json");
        std::fs::write(path, serde_json::to_vec_pretty(&session).unwrap()).unwrap();

        let err = store.load("session-expected").unwrap_err();

        assert!(err
            .to_string()
            .contains("session load failed: session-expected"));
        assert!(err.to_string().contains("session-actual"));
    }

    #[test]
    fn replace_session_file_restores_original_when_install_fails() {
        let temp = PathBuf::from("session.tmp");
        let target = PathBuf::from("session.json");
        let backup = backup_path(&target);
        let ops = MockFileOps::new([
            (temp.clone(), "new-session"),
            (target.clone(), "old-session"),
        ])
        .with_failed_rename(temp.clone(), target.clone());

        let err = replace_session_file_with_ops(&temp, &target, &ops).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(ops.entry(&target), Some("old-session"));
        assert_eq!(ops.entry(&temp), Some("new-session"));
        assert_eq!(ops.entry(&backup), None);
    }
}

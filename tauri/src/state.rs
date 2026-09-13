use std::sync::{
    atomic::{AtomicU64, Ordering},
    RwLock,
};

use mnemosyne_core::config::AppConfig;
use mnemosyne_core::hprof::ObjectGraph;

/// Shared heap session state managed by Tauri.
///
/// The parsed `ObjectGraph` is held behind an `RwLock` so
/// multiple frontend queries can read concurrently while
/// load/unload operations acquire exclusive access.
pub struct HeapSession {
    pub graph: RwLock<Option<ObjectGraph>>,
    /// Lazily populated when `inspect_object` is called with
    /// `retain_field_data: true` against a lean session graph.
    pub field_data_graph: RwLock<Option<ObjectGraph>>,
    /// Incremented on every `load_heap` / `unload_heap` so in-flight field-data
    /// reparse work can detect a replaced session before installing its cache.
    pub session_epoch: AtomicU64,
    pub config: RwLock<AppConfig>,
    pub heap_path: RwLock<Option<String>>,
}

impl HeapSession {
    pub fn new() -> Self {
        Self {
            graph: RwLock::new(None),
            field_data_graph: RwLock::new(None),
            session_epoch: AtomicU64::new(0),
            config: RwLock::new(AppConfig::default()),
            heap_path: RwLock::new(None),
        }
    }

    pub fn bump_session_epoch(&self) -> u64 {
        self.session_epoch.fetch_add(1, Ordering::Release)
    }
}
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, RwLock,
    },
};

use mnemosyne_core::config::AppConfig;
use mnemosyne_core::{hprof::ObjectGraph, DominatorTree};
use mnemosyne_desktop_session::OperationRegistry;

/// Shared heap session state managed by Tauri.
///
/// The parsed `ObjectGraph` and its `DominatorTree` are retained behind
/// `RwLock`s so frontend queries can reuse one analyzed pair while
/// load/unload operations acquire exclusive access.
pub struct HeapSession {
    /// Serializes session mutation (load/unload and field-data cache install)
    /// so epoch/path checks cannot interleave with cache writes.
    pub session_mutation: Mutex<()>,
    pub graph: RwLock<Option<ObjectGraph>>,
    pub dominator: RwLock<Option<DominatorTree>>,
    /// Lazily populated when `inspect_object` is called with
    /// `retain_field_data: true` against a lean session graph.
    pub field_data_graph: RwLock<Option<ObjectGraph>>,
    /// Incremented on every `load_heap` / `unload_heap` so in-flight field-data
    /// reparse work can detect a replaced session before installing its cache.
    pub session_epoch: AtomicU64,
    pub config: RwLock<AppConfig>,
    pub heap_path: RwLock<Option<String>>,
    /// Opaque source IDs → absolute paths retained only on the native side.
    pub selected_sources: Mutex<HashMap<String, String>>,
    pub operations: OperationRegistry,
}

impl HeapSession {
    pub fn new() -> Self {
        Self {
            session_mutation: Mutex::new(()),
            graph: RwLock::new(None),
            dominator: RwLock::new(None),
            field_data_graph: RwLock::new(None),
            session_epoch: AtomicU64::new(0),
            config: RwLock::new(AppConfig::default()),
            heap_path: RwLock::new(None),
            selected_sources: Mutex::new(HashMap::new()),
            operations: OperationRegistry::default(),
        }
    }

    pub fn bump_session_epoch(&self) -> u64 {
        self.session_epoch.fetch_add(1, Ordering::Release)
    }
}

use std::sync::RwLock;

use mnemosyne_core::config::AppConfig;
use mnemosyne_core::hprof::ObjectGraph;

/// Shared heap session state managed by Tauri.
///
/// The parsed `ObjectGraph` is held behind an `RwLock` so
/// multiple frontend queries can read concurrently while
/// load/unload operations acquire exclusive access.
pub struct HeapSession {
    pub graph: RwLock<Option<ObjectGraph>>,
    pub config: RwLock<AppConfig>,
    pub heap_path: RwLock<Option<String>>,
}

impl HeapSession {
    pub fn new() -> Self {
        Self {
            graph: RwLock::new(None),
            config: RwLock::new(AppConfig::default()),
            heap_path: RwLock::new(None),
        }
    }
}
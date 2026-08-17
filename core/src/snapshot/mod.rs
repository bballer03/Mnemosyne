//! Snapshot persistence: serialize an already-parsed `(ObjectGraph,
//! DominatorTree)` pair to disk so a later invocation can skip the HPROF
//! binary parse entirely (parse-once-query-many, M9).
//!
//! Structural template: [`crate::mcp::session::McpSessionStore`]. This
//! module mirrors its shape exactly rather than reinventing it:
//! - Atomic write: serialize to `<key>.tmp`, then a rename-based swap into
//!   `<key>.json` via [`crate::mcp::session::replace_session_file`] (backs
//!   up any existing target and restores it if the final rename fails --
//!   required for correctness on Windows, where a plain `fs::rename` fails
//!   outright when the destination already exists).
//! - `schema_version: u32` versioning convention, mirroring
//!   `PersistedAiSession::session_version`.
//!
//! Slice 9.A scope only: [`SnapshotStore::new`], [`SnapshotStore::ensure_root`],
//! [`SnapshotStore::save`], [`SnapshotStore::load`]. Listing, removal,
//! cache-key resolution by heap path, and staleness detection (schema
//! mismatch / stale source hash) are Slice 9.B. CLI/MCP wiring is Slices
//! 9.C/9.D. See `docs/design/milestone-9-snapshot-persistence.md`.

use std::{fmt::Write as _, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    errors::{CoreError, CoreResult},
    graph::DominatorTree,
    hprof::ObjectGraph,
    mcp::session::{replace_session_file, timestamp_now},
};

/// Snapshot payload schema version. Bump this whenever the serialized shape
/// of `ObjectGraph`/`DominatorTree` (or `SnapshotPayload` itself) changes in
/// a wire-incompatible way. `mnemosyne_version` is informational only --
/// `schema_version` is the enforced compatibility contract.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Small header describing a saved snapshot. Serialized alongside the full
/// payload today (Slice 9.A); a future slice may also persist it standalone
/// for cheap `snapshot list` reads without deserializing the full graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub schema_version: u32,
    pub heap_sha256: String,
    /// Recorded for display purposes only (e.g. a future `snapshot list`).
    /// Never trusted for cache-identity -- `heap_sha256` is.
    pub heap_path: String,
    /// Creation timestamp. Uses the same epoch-seconds-as-string convention
    /// as `PersistedAiSession::created_at` (via
    /// [`crate::mcp::session::timestamp_now`]): this workspace has no
    /// RFC3339-formatting crate as a dependency, and Slice 9.A does not add
    /// one, so the existing timestamp approach is reused verbatim rather
    /// than introducing a second, inconsistent convention.
    pub created_at: String,
    pub mnemosyne_version: String,
    pub object_count: usize,
    /// True when any object in the graph carries non-empty `field_data`,
    /// i.e. the graph was parsed with `ParseOptions { retain_field_data: true }`.
    pub has_field_data: bool,
}

/// The full on-disk snapshot contents: the manifest header plus the
/// serialized object graph and dominator tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPayload {
    pub manifest: SnapshotManifest,
    pub object_graph: ObjectGraph,
    pub dominator_tree: DominatorTree,
}

/// On-disk store for snapshot payloads, keyed by heap-file SHA-256 hash.
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure_root(&self) -> CoreResult<()> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    /// Hash `heap_path`'s bytes, build a [`SnapshotPayload`] from `graph`
    /// and `dominator`, and atomically write it to `<sha256>.json` under
    /// this store's root. Returns the manifest that was written -- notably
    /// including `heap_sha256`, the key `load` expects.
    pub fn save(
        &self,
        heap_path: &str,
        graph: &ObjectGraph,
        dominator: &DominatorTree,
    ) -> CoreResult<SnapshotManifest> {
        self.ensure_root()?;

        let heap_bytes = fs::read(heap_path)?;
        let heap_sha256 = sha256_hex(&heap_bytes);

        let manifest = SnapshotManifest {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            heap_sha256: heap_sha256.clone(),
            heap_path: heap_path.to_string(),
            created_at: timestamp_now(),
            mnemosyne_version: env!("CARGO_PKG_VERSION").to_string(),
            object_count: graph.objects.len(),
            has_field_data: graph.objects.values().any(|obj| !obj.field_data.is_empty()),
        };

        let payload = SnapshotPayload {
            manifest: manifest.clone(),
            object_graph: graph.clone(),
            dominator_tree: dominator.clone(),
        };

        let target = self.path_for(&heap_sha256);
        let temp = self.root.join(format!("{heap_sha256}.tmp"));
        let bytes = serde_json::to_vec_pretty(&payload)?;
        fs::write(&temp, &bytes)?;
        replace_session_file(&temp, &target)?;

        Ok(manifest)
    }

    /// Load a previously saved snapshot by its SHA-256 key.
    ///
    /// Returns a structured `snapshot_not_found`/`snapshot_corrupt` error
    /// (never a panic) when the key doesn't exist or the file can't be
    /// deserialized -- e.g. truncated by a crash mid-write, or edited by
    /// hand into invalid JSON.
    pub fn load(&self, key: &str) -> CoreResult<SnapshotPayload> {
        let path = self.path_for(key);
        let bytes = fs::read(&path).map_err(|err| map_load_error(key, err))?;
        let payload: SnapshotPayload =
            serde_json::from_slice(&bytes).map_err(|err| snapshot_corrupt(key, err))?;
        Ok(payload)
    }

    fn path_for(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.json"))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn map_load_error(key: &str, err: std::io::Error) -> CoreError {
    if err.kind() == std::io::ErrorKind::NotFound {
        return snapshot_error(
            "snapshot_not_found",
            format!("no snapshot found for key '{key}'"),
        );
    }
    CoreError::Io(err)
}

fn snapshot_corrupt(key: &str, err: serde_json::Error) -> CoreError {
    snapshot_error(
        "snapshot_corrupt",
        format!("failed to deserialize snapshot '{key}': {err}"),
    )
}

/// Structured, greppable error identifier, following the same
/// `CoreError::Unsupported("<code>: <detail>")` convention used by M8/M10
/// (see `core::diff::diff_feature_unavailable`).
fn snapshot_error(code: &str, message: impl Into<String>) -> CoreError {
    CoreError::Unsupported(format!("{code}: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_dominator_tree;
    use crate::hprof::{GcRoot, GcRootType, HeapObject, ObjectKind};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);
        graph.objects.insert(
            1,
            HeapObject {
                id: 1,
                class_id: 0x100,
                shallow_size: 10,
                references: vec![2],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            2,
            HeapObject {
                id: 2,
                class_id: 0x100,
                shallow_size: 20,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.gc_roots.push(GcRoot {
            object_id: 1,
            root_type: GcRootType::StickyClass,
        });
        graph
    }

    fn write_temp_heap(bytes: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn save_then_load_round_trips_manifest_fields() {
        let store_dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(store_dir.path().to_path_buf());
        let heap_file = write_temp_heap(b"pretend hprof bytes");
        let graph = make_graph();
        let dominator = build_dominator_tree(&graph);

        let manifest = store
            .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
            .unwrap();

        assert_eq!(manifest.schema_version, SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(manifest.object_count, 2);
        assert!(!manifest.has_field_data);

        let loaded = store.load(&manifest.heap_sha256).unwrap();
        assert_eq!(loaded.manifest.heap_sha256, manifest.heap_sha256);
        assert_eq!(loaded.object_graph.objects.len(), 2);
    }

    #[test]
    fn load_missing_key_returns_snapshot_not_found() {
        let store_dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(store_dir.path().to_path_buf());

        let err = store.load("does-not-exist").unwrap_err();

        assert!(err.to_string().contains("snapshot_not_found"));
    }

    #[test]
    fn load_corrupt_file_returns_structured_error_not_panic() {
        let store_dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(store_dir.path().to_path_buf());
        store.ensure_root().unwrap();
        fs::write(store_dir.path().join("deadbeef.json"), b"{ not valid json").unwrap();

        let err = store.load("deadbeef").unwrap_err();

        assert!(err.to_string().contains("snapshot_corrupt"));
    }
}

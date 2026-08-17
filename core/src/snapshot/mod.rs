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
//! Slice 9.A shipped [`SnapshotStore::new`], [`SnapshotStore::ensure_root`],
//! [`SnapshotStore::save`], [`SnapshotStore::load`]. Slice 9.B added
//! [`SnapshotStore::list`], [`SnapshotStore::remove`], and
//! [`SnapshotStore::find_fresh_for_heap`] (cache-key resolution by heap path
//! plus staleness detection). Slice 9.C (this pass) adds
//! [`SnapshotStore::load_checked`] -- the loud counterpart to
//! `find_fresh_for_heap` used by CLI `--snapshot <key>` call sites -- and
//! wires everything into the CLI (`cli/src/main.rs`). MCP wiring remains
//! Slice 9.D. See `docs/design/milestone-9-snapshot-persistence.md`.
//!
//! ## Design note: where staleness checking lives (Slice 9.B decision)
//!
//! [`SnapshotStore::load`] (Slice 9.A) deliberately does **not** gain a
//! staleness check in this slice, even though the concept (schema-version /
//! source-hash mismatch) now exists via [`SnapshotStore::find_fresh_for_heap`].
//! This is intentional, not an oversight:
//!
//! - Per the design doc §7, the *auto-discovery* path (no explicit
//!   `--snapshot <key>` from the user) must treat every kind of miss --
//!   "never cached", "schema mismatch", "source hash changed" -- identically
//!   and silently: fall through to a normal parse. That is exactly what
//!   [`SnapshotStore::find_fresh_for_heap`] implements: `Ok(None)` for all
//!   three, never an error.
//! - The *explicit* path (`--snapshot <key>` / `snapshot load <key>`) is
//!   supposed to be loud instead: a stale or mismatched explicit key should
//!   surface `snapshot_schema_mismatch` / `snapshot_stale_source` as
//!   distinct structured errors (§6.1, §7's exit-code table 11/12), not the
//!   generic `snapshot_corrupt`/`snapshot_not_found` `load` returns today.
//!   That behavior is user-facing CLI/MCP plumbing -- it needs the resolved
//!   `heap_path` the *user* asked to validate against, which only exists at
//!   the Slice 9.C/9.D call sites (`load(key)` alone has no heap path to
//!   re-hash against; the key might not even be a hash, per its own
//!   `key: sha256 hash OR direct file path` contract). Teaching `load`
//!   itself to loudly detect schema mismatches (it already *can* -- the
//!   deserialized manifest's `schema_version` is right there) is cheap and
//!   arguably belongs here, but doing the *source-hash* half of loud
//!   staleness detection inside `load` would require also threading a heap
//!   path into a method whose whole point is "load by key, no heap path
//!   needed." Splitting loud-schema-in-`load` from loud-source-hash-in-9.C
//!   would leave the two staleness triggers inconsistent about which layer
//!   owns them. **Decision: keep `load` exactly as Slice 9.A shipped it
//!   (key -> payload or `snapshot_not_found`/`snapshot_corrupt`), and defer
//!   both loud staleness errors (`snapshot_schema_mismatch`,
//!   `snapshot_stale_source`) to Slice 9.C**, where the CLI/MCP call site
//!   has both the resolved key *and* the user-provided heap path available
//!   to build the complete, consistent error. Slice 9.B only implements the
//!   silent auto-discovery half of staleness (`find_fresh_for_heap`).
//!
//!   **Resolution (Slice 9.C):** the loud-staleness logic landed as a new
//!   [`SnapshotStore::load_checked`] method on this same store, not inlined
//!   separately at each CLI call site. Both loud checks (schema version,
//!   source hash) need the same two inputs (`key`, `heap_path`) and the
//!   same `sha256_hex` helper this module already owns privately, so a
//!   shared method avoids duplicating hashing/error-construction logic
//!   across five CLI command handlers (`analyze`, `leaks`, `gc-path`,
//!   `inspect`, `query`) plus `snapshot load`. `load` itself remains
//!   untouched, preserving Slice 9.A's contract for every existing caller.

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

    /// Load a previously saved snapshot by `key`, then loudly validate it
    /// against `heap_path` (Slice 9.C).
    ///
    /// This is the resolution of the open question left by Slice 9.B's
    /// module-level doc comment above: `load` alone (key -> payload) has no
    /// heap path to re-hash against, so it can only ever surface
    /// `snapshot_not_found`/`snapshot_corrupt`. An *explicit*
    /// `--snapshot <key>` CLI/MCP call site, however, always has both the
    /// resolved key and the user-provided heap path, so this method lives
    /// on `SnapshotStore` (not inlined at each call site) to keep the
    /// staleness rules in one place and reusable across every command that
    /// wires `--snapshot` (`analyze`, `leaks`, `gc-path`, `inspect`,
    /// `query`) plus `snapshot load`.
    ///
    /// Checks, in order:
    /// 1. `manifest.schema_version` against [`SNAPSHOT_SCHEMA_VERSION`] --
    ///    mismatch surfaces `snapshot_schema_mismatch` (never silently
    ///    proceeds with a payload shape the running binary may not
    ///    understand).
    /// 2. `heap_path`'s *current* on-disk SHA-256 against
    ///    `manifest.heap_sha256` -- mismatch surfaces `snapshot_stale_source`
    ///    (the heap file changed since the snapshot was taken).
    ///
    /// Unlike [`SnapshotStore::find_fresh_for_heap`] (silent, `Ok(None)` on
    /// any staleness), both checks here are loud, structured errors --
    /// exactly the distinction the design doc's exit-code table (§7, codes
    /// 11/12) requires for explicit `--snapshot` usage.
    pub fn load_checked(&self, key: &str, heap_path: &str) -> CoreResult<SnapshotPayload> {
        let payload = self.load(key)?;

        if payload.manifest.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(snapshot_error(
                "snapshot_schema_mismatch",
                format!(
                    "cached snapshot schema_version={}, running binary expects schema_version={SNAPSHOT_SCHEMA_VERSION}",
                    payload.manifest.schema_version
                ),
            ));
        }

        let heap_bytes = fs::read(heap_path)?;
        let current_sha256 = sha256_hex(&heap_bytes);
        if payload.manifest.heap_sha256 != current_sha256 {
            return Err(snapshot_error(
                "snapshot_stale_source",
                format!(
                    "heap file '{heap_path}' sha256 no longer matches the cached snapshot for key '{key}'"
                ),
            ));
        }

        Ok(payload)
    }

    /// List the manifests of every snapshot currently in this store.
    ///
    /// Scans the store root for `*.json` files and extracts just the
    /// `manifest` field of each (via `serde_json::Value`, not a full
    /// `SnapshotPayload` deserialize -- cheaper, and per §6 point 2 of the
    /// design doc the manifest is specifically meant to be "cheap to read
    /// without deserializing the full payload"). A file that fails to parse,
    /// or whose `manifest` field doesn't match [`SnapshotManifest`]'s shape,
    /// is skipped (with a `tracing::warn`) rather than failing the whole
    /// listing -- one corrupt entry must not hide every other valid one from
    /// `snapshot list`.
    pub fn list(&self) -> CoreResult<Vec<SnapshotManifest>> {
        self.ensure_root()?;

        let mut manifests = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            match read_manifest_only(&path) {
                Ok(manifest) => manifests.push(manifest),
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "skipping unreadable snapshot entry during list()"
                    );
                }
            }
        }

        Ok(manifests)
    }

    /// Delete the snapshot stored under `key`.
    ///
    /// Returns `snapshot_not_found` (not a silent success) if no snapshot
    /// exists for `key` -- a `remove` of a nonexistent key is a user error
    /// worth surfacing, not a no-op to swallow.
    pub fn remove(&self, key: &str) -> CoreResult<()> {
        let path = self.path_for(key);
        fs::remove_file(&path).map_err(|err| map_load_error(key, err))?;
        Ok(())
    }

    /// Auto-discovery lookup: hash `heap_path`'s current on-disk bytes and
    /// return the cached snapshot for that hash, if one exists and is
    /// current.
    ///
    /// Returns `Ok(None)` -- never an error -- for every flavor of "no usable
    /// cache entry": nothing has ever been saved for this heap, the cached
    /// entry's `schema_version` no longer matches [`SNAPSHOT_SCHEMA_VERSION`],
    /// its recorded `heap_sha256` doesn't match the hash we just computed
    /// (re-derived explicitly here rather than assumed, guarding against a
    /// hand-edited manifest whose `heap_sha256` field was changed without
    /// renaming the file), or the cached entry is corrupt. Per §7 of the
    /// design doc, auto-discovery misses -- including staleness -- are the
    /// expected first-run/cold-cache state, not a failure; only an
    /// *explicit* `--snapshot <key>` / `snapshot load <key>` (Slice 9.C) is
    /// supposed to surface loud `snapshot_schema_mismatch` /
    /// `snapshot_stale_source` errors. A genuine I/O error reading
    /// `heap_path` itself (e.g. it doesn't exist) still propagates as an
    /// error -- that's a problem with the caller's input, not a cache-miss.
    pub fn find_fresh_for_heap(&self, heap_path: &str) -> CoreResult<Option<SnapshotPayload>> {
        let heap_bytes = fs::read(heap_path)?;
        let current_sha256 = sha256_hex(&heap_bytes);

        let path = self.path_for(&current_sha256);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(CoreError::Io(err)),
        };

        let payload: SnapshotPayload = match serde_json::from_slice(&bytes) {
            Ok(payload) => payload,
            Err(_) => return Ok(None), // corrupt cache entry: treat as a miss, not an error
        };

        if payload.manifest.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Ok(None);
        }
        if payload.manifest.heap_sha256 != current_sha256 {
            return Ok(None);
        }

        Ok(Some(payload))
    }

    fn path_for(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.json"))
    }
}

/// Deserialize just the `manifest` field of a snapshot file at `path`,
/// without paying the cost of deserializing the full `object_graph`/
/// `dominator_tree` payload alongside it. Used by [`SnapshotStore::list`].
fn read_manifest_only(path: &std::path::Path) -> CoreResult<SnapshotManifest> {
    let bytes = fs::read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let manifest_value = value
        .get("manifest")
        .ok_or_else(|| CoreError::Unsupported("missing 'manifest' field".to_string()))?;
    let manifest: SnapshotManifest = serde_json::from_value(manifest_value.clone())?;
    Ok(manifest)
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

    #[test]
    fn load_checked_returns_payload_when_fresh() {
        let store_dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(store_dir.path().to_path_buf());
        let heap_file = write_temp_heap(b"pretend hprof bytes");
        let graph = make_graph();
        let dominator = build_dominator_tree(&graph);

        let manifest = store
            .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
            .unwrap();

        let checked = store
            .load_checked(&manifest.heap_sha256, heap_file.path().to_str().unwrap())
            .unwrap();
        assert_eq!(checked.manifest.heap_sha256, manifest.heap_sha256);
    }

    #[test]
    fn load_checked_detects_schema_mismatch() {
        let store_dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(store_dir.path().to_path_buf());
        let heap_file = write_temp_heap(b"pretend hprof bytes");
        let graph = make_graph();
        let dominator = build_dominator_tree(&graph);

        let mut manifest = store
            .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
            .unwrap();

        // Hand-edit the persisted manifest's schema_version to simulate a
        // snapshot saved by an older/newer binary.
        let payload_path = store_dir
            .path()
            .join(format!("{}.json", manifest.heap_sha256));
        let mut payload: SnapshotPayload =
            serde_json::from_slice(&fs::read(&payload_path).unwrap()).unwrap();
        payload.manifest.schema_version = SNAPSHOT_SCHEMA_VERSION + 1;
        manifest.schema_version = payload.manifest.schema_version;
        fs::write(&payload_path, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();

        let err = store
            .load_checked(&manifest.heap_sha256, heap_file.path().to_str().unwrap())
            .unwrap_err();

        assert!(err.to_string().contains("snapshot_schema_mismatch"));
    }

    #[test]
    fn load_checked_detects_stale_source() {
        let store_dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(store_dir.path().to_path_buf());
        let mut heap_file = write_temp_heap(b"pretend hprof bytes v1");
        let graph = make_graph();
        let dominator = build_dominator_tree(&graph);

        let manifest = store
            .save(heap_file.path().to_str().unwrap(), &graph, &dominator)
            .unwrap();

        // Mutate the heap file's bytes after the snapshot was saved.
        heap_file.as_file_mut().set_len(0).unwrap();
        heap_file.write_all(b"different bytes now").unwrap();
        heap_file.flush().unwrap();

        let err = store
            .load_checked(&manifest.heap_sha256, heap_file.path().to_str().unwrap())
            .unwrap_err();

        assert!(err.to_string().contains("snapshot_stale_source"));
    }

    #[test]
    fn load_checked_missing_key_returns_snapshot_not_found() {
        let store_dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(store_dir.path().to_path_buf());
        let heap_file = write_temp_heap(b"pretend hprof bytes");

        let err = store
            .load_checked("does-not-exist", heap_file.path().to_str().unwrap())
            .unwrap_err();

        assert!(err.to_string().contains("snapshot_not_found"));
    }
}

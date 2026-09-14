use crate::errors::{CoreError, CoreResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const ARTIFACT_TTL: Duration = Duration::from_secs(24 * 60 * 60);
pub const ARTIFACT_MAX_BYTES: u64 = 16_777_216;
pub const ARTIFACT_INLINE_MAX_BYTES: u64 = 262_144;

#[derive(Debug, Serialize)]
pub struct ArtifactResponse {
    pub artifact_id: String,
    pub format: String,
    pub media_type: String,
    pub byte_length: u64,
    pub sha256: String,
    pub created_at: String,
    pub expires_at: String,
    pub delivery: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ArtifactChunk {
    pub artifact_id: String,
    pub offset_bytes: u64,
    pub next_offset_bytes: u64,
    pub eof: bool,
    pub content_base64: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Manifest {
    artifact_id: String,
    format: String,
    media_type: String,
    byte_length: u64,
    sha256: String,
    created_secs: u64,
    expires_secs: u64,
}

pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn cleanup_expired(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        let now = now_secs();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(manifest) = read_manifest(&path) else {
                continue;
            };
            if !valid_id(&manifest.artifact_id)
                || path.file_stem().and_then(|stem| stem.to_str())
                    != Some(manifest.artifact_id.as_str())
            {
                continue;
            }
            if manifest.expires_secs <= now {
                let _ = fs::remove_file(self.data_path(&manifest.artifact_id));
                let _ = fs::remove_file(path);
            }
        }
    }

    pub fn create(
        &self,
        format: &str,
        media_type: &str,
        render: impl FnOnce(&mut CountingWriter) -> CoreResult<()>,
    ) -> CoreResult<ArtifactResponse> {
        fs::create_dir_all(&self.root).map_err(store_io)?;
        self.cleanup_expired();
        let temp = self.root.join(format!(".tmp-{}", random_id()?));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(store_io)?;
        #[cfg(unix)]
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600)).map_err(store_io)?;
        let mut writer = CountingWriter::new(file);
        let result = render(&mut writer);
        let observed = writer.bytes;
        let digest = writer.digest.clone().finalize();
        let mut file = writer.into_inner();
        let _ = file.flush();
        drop(file);
        if observed > ARTIFACT_MAX_BYTES || writer_exceeded(&result) {
            let _ = fs::remove_file(&temp);
            return Err(CoreError::Unsupported(format!(
                "artifact_size_limit_exceeded: exceeded render limit (limit_bytes={ARTIFACT_MAX_BYTES}, observed_bytes={observed}, format={format})"
            )));
        }
        if let Err(error) = result {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        let id = random_id()?;
        let data_path = self.data_path(&id);
        fs::rename(&temp, &data_path).map_err(store_io)?;
        let created_secs = now_secs();
        let manifest = Manifest {
            artifact_id: id.clone(),
            format: format.into(),
            media_type: media_type.into(),
            byte_length: observed,
            sha256: hex(&digest),
            created_secs,
            expires_secs: created_secs + ARTIFACT_TTL.as_secs(),
        };
        let manifest_path = self.manifest_path(&id);
        let json = serde_json::to_vec(&manifest)?;
        if let Err(err) = write_private_atomic(&manifest_path, &json) {
            let _ = fs::remove_file(&data_path);
            return Err(err);
        }
        let content_base64 = if observed <= ARTIFACT_INLINE_MAX_BYTES {
            Some(base64_encode(&fs::read(&data_path).map_err(store_io)?))
        } else {
            None
        };
        Ok(response_from_manifest(manifest, content_base64))
    }

    pub fn read(&self, id: &str, offset: u64, max_bytes: u64) -> CoreResult<ArtifactChunk> {
        self.cleanup_expired();
        let manifest = self.load_manifest(id)?;
        if manifest.expires_secs <= now_secs() {
            self.remove_files(&manifest.artifact_id);
            return Err(artifact_error("artifact_expired"));
        }
        if max_bytes > ARTIFACT_INLINE_MAX_BYTES {
            return Err(CoreError::InvalidInput(
                "max_bytes may not exceed 262144".into(),
            ));
        }
        if offset > manifest.byte_length {
            return Err(CoreError::InvalidInput(
                "offset_bytes must not exceed artifact length".into(),
            ));
        }
        let mut file =
            File::open(self.data_path(id)).map_err(|_| artifact_error("artifact_not_found"))?;
        file.seek(SeekFrom::Start(offset)).map_err(store_io)?;
        let amount = max_bytes.min(manifest.byte_length - offset);
        let mut bytes = vec![0; amount as usize];
        file.read_exact(&mut bytes).map_err(store_io)?;
        let next = offset + amount;
        Ok(ArtifactChunk {
            artifact_id: id.into(),
            offset_bytes: offset,
            next_offset_bytes: next,
            eof: next == manifest.byte_length,
            content_base64: base64_encode(&bytes),
        })
    }

    pub fn delete(&self, id: &str) -> CoreResult<()> {
        self.cleanup_expired();
        let manifest = self.load_manifest(id)?;
        self.remove_files(&manifest.artifact_id);
        Ok(())
    }

    fn load_manifest(&self, id: &str) -> CoreResult<Manifest> {
        if !valid_id(id) {
            return Err(artifact_error("artifact_not_found"));
        }
        let manifest = read_manifest(&self.manifest_path(id))
            .map_err(|_| artifact_error("artifact_not_found"))?;
        if manifest.artifact_id != id {
            return Err(artifact_error("artifact_not_found"));
        }
        Ok(manifest)
    }

    fn data_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.data"))
    }
    fn manifest_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }
    fn remove_files(&self, id: &str) {
        let _ = fs::remove_file(self.data_path(id));
        let _ = fs::remove_file(self.manifest_path(id));
    }
}

pub fn default_artifact_dir() -> PathBuf {
    if let Ok(value) = std::env::var("MNEMOSYNE_ARTIFACT_DIR") {
        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }
    if let Some(mut dir) = dirs::cache_dir() {
        dir.push("mnemosyne");
        dir.push("artifacts");
        return dir;
    }
    std::env::temp_dir().join("mnemosyne").join("artifacts")
}

pub struct CountingWriter {
    file: File,
    pub bytes: u64,
    digest: Sha256,
}
impl CountingWriter {
    fn new(file: File) -> Self {
        Self {
            file,
            bytes: 0,
            digest: Sha256::new(),
        }
    }
    fn into_inner(self) -> File {
        self.file
    }
}
impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.saturating_add(buf.len() as u64);
        if self.bytes > ARTIFACT_MAX_BYTES {
            return Err(io::Error::other("artifact size limit exceeded"));
        }
        self.digest.update(buf);
        self.file.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn writer_exceeded(result: &CoreResult<()>) -> bool {
    matches!(result, Err(CoreError::Io(err)) if err.to_string().contains("artifact size limit exceeded"))
}
fn read_manifest(path: &Path) -> io::Result<Manifest> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}
fn write_private_atomic(path: &Path, bytes: &[u8]) -> CoreResult<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("manifest.json");
    let tmp = path.with_file_name(format!(".{file_name}.tmp-{}", random_id()?));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .map_err(store_io)?;
    #[cfg(unix)]
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(store_io)?;
    file.write_all(bytes).map_err(store_io)?;
    file.flush().map_err(store_io)?;
    drop(file);
    fs::rename(tmp, path).map_err(store_io)
}
fn store_io(err: io::Error) -> CoreError {
    CoreError::Unsupported(format!("artifact_store_error: {}", err.kind()))
}
fn artifact_error(code: &str) -> CoreError {
    CoreError::Unsupported(code.into())
}
fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_id() -> CoreResult<String> {
    let mut bytes = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| CoreError::Unsupported("artifact_random_id_unavailable".into()))?;
    Ok(hex(&bytes))
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| {
            [
                DIGITS[(b >> 4) as usize] as char,
                DIGITS[(b & 15) as usize] as char,
            ]
        })
        .collect()
}
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
fn response_from_manifest(m: Manifest, content_base64: Option<String>) -> ArtifactResponse {
    ArtifactResponse {
        artifact_id: m.artifact_id,
        format: m.format,
        media_type: m.media_type,
        byte_length: m.byte_length,
        sha256: m.sha256,
        created_at: rfc3339(m.created_secs),
        expires_at: rfc3339(m.expires_secs),
        delivery: if content_base64.is_some() {
            "inline"
        } else {
            "managed"
        },
        content_base64,
    }
}
fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!(
        "{year:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn inline_and_managed_delivery_have_bounded_reads() {
        let dir = tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().to_path_buf());
        let inline = store
            .create("svg", "image/svg+xml", |w| {
                w.write_all(b"small").map_err(CoreError::from)
            })
            .unwrap();
        assert_eq!(inline.delivery, "inline");
        assert!(inline.content_base64.is_some());

        let managed = store
            .create("folded-stack", "text/plain", |w| {
                w.write_all(&vec![b'x'; ARTIFACT_INLINE_MAX_BYTES as usize + 1])
                    .map_err(CoreError::from)
            })
            .unwrap();
        assert_eq!(managed.delivery, "managed");
        assert!(managed.content_base64.is_none());
        let chunk = store.read(&managed.artifact_id, 0, 10).unwrap();
        assert_eq!(chunk.next_offset_bytes, 10);
        assert!(!chunk.eof);
    }

    #[test]
    fn traversal_and_missing_ids_are_not_found() {
        let store = ArtifactStore::new(tempdir().unwrap().path().to_path_buf());
        assert!(store
            .read("../secret", 0, 1)
            .unwrap_err()
            .to_string()
            .contains("artifact_not_found"));
        assert!(store
            .delete("not-an-id")
            .unwrap_err()
            .to_string()
            .contains("artifact_not_found"));
    }

    #[test]
    fn expired_artifact_is_deleted_and_unreadable() {
        let dir = tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().to_path_buf());
        let response = store
            .create("json", "application/json", |w| {
                w.write_all(b"{}").map_err(CoreError::from)
            })
            .unwrap();
        let path = store.manifest_path(&response.artifact_id);
        let mut manifest = read_manifest(&path).unwrap();
        manifest.expires_secs = 0;
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(store.read(&response.artifact_id, 0, 1).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn oversized_render_is_rejected_without_artifact() {
        let dir = tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().to_path_buf());
        let error = store
            .create("json", "application/json", |w| {
                w.write_all(&vec![b'x'; ARTIFACT_MAX_BYTES as usize + 1])
                    .map_err(CoreError::from)
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("artifact_size_limit_exceeded"));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn read_rejects_chunks_larger_than_inline_limit() {
        let store = ArtifactStore::new(tempdir().unwrap().path().to_path_buf());
        let response = store
            .create("json", "application/json", |writer| {
                writer.write_all(b"{}").map_err(CoreError::from)
            })
            .unwrap();

        let error = store
            .read(&response.artifact_id, 0, ARTIFACT_INLINE_MAX_BYTES + 1)
            .unwrap_err()
            .to_string();
        assert!(error.contains("max_bytes may not exceed 262144"));
    }

    #[test]
    fn reading_an_artifact_cleans_up_other_expired_artifacts() {
        let dir = tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().to_path_buf());
        let expired = store
            .create("json", "application/json", |writer| {
                writer.write_all(b"expired").map_err(CoreError::from)
            })
            .unwrap();
        let live = store
            .create("json", "application/json", |writer| {
                writer.write_all(b"live").map_err(CoreError::from)
            })
            .unwrap();

        let expired_manifest_path = store.manifest_path(&expired.artifact_id);
        let mut manifest = read_manifest(&expired_manifest_path).unwrap();
        manifest.expires_secs = 0;
        fs::write(
            &expired_manifest_path,
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        store.read(&live.artifact_id, 0, 4).unwrap();
        assert!(!expired_manifest_path.exists());
        assert!(store.manifest_path(&live.artifact_id).exists());
    }
}

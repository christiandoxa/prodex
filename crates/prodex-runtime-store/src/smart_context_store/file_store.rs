use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

const RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub fn runtime_smart_context_artifact_store_to_json(
    store: &RuntimeSmartContextArtifactStore,
) -> Result<String, RuntimeSmartContextArtifactStoreJsonError> {
    let document = RuntimeSmartContextArtifactStoreWrite {
        version: store.version,
        artifacts: store.artifacts.values().collect(),
    };
    let mut output = serde_json::to_string_pretty(&document)
        .map_err(|error| RuntimeSmartContextArtifactStoreJsonError::new(error.to_string()))?;
    output.push('\n');
    Ok(output)
}

pub fn runtime_smart_context_artifact_store_from_json(
    input: &str,
) -> Result<RuntimeSmartContextArtifactStore, RuntimeSmartContextArtifactStoreJsonError> {
    let document = serde_json::from_str::<RuntimeSmartContextArtifactStoreRead>(input)
        .map_err(|error| RuntimeSmartContextArtifactStoreJsonError::new(error.to_string()))?;
    let version = document.version;
    if version != crate::RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "unsupported smart-context artifact store version {version}"
        )));
    }

    let mut artifacts = BTreeMap::new();
    match document.artifacts {
        RuntimeSmartContextArtifactsRead::Array(items) => {
            for item in items {
                let artifact = item.into_artifact(None)?;
                artifacts.insert(artifact.key.clone(), artifact);
            }
        }
        RuntimeSmartContextArtifactsRead::Object(entries) => {
            for (key, item) in entries {
                let artifact = item.into_artifact(Some(&key))?;
                if artifact.key != key {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
                        "artifact key mismatch for {key}"
                    )));
                }
                artifacts.insert(key, artifact);
            }
        }
    }

    Ok(RuntimeSmartContextArtifactStore { version, artifacts })
}

#[derive(Serialize)]
struct RuntimeSmartContextArtifactStoreWrite<'a> {
    version: u32,
    artifacts: Vec<&'a RuntimeSmartContextArtifact>,
}

#[derive(Deserialize)]
struct RuntimeSmartContextArtifactStoreRead {
    #[serde(default = "runtime_smart_context_artifact_store_version")]
    version: u32,
    artifacts: RuntimeSmartContextArtifactsRead,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RuntimeSmartContextArtifactsRead {
    Array(Vec<RuntimeSmartContextArtifactRead>),
    Object(BTreeMap<String, RuntimeSmartContextArtifactRead>),
}

#[derive(Deserialize)]
struct RuntimeSmartContextArtifactRead {
    #[serde(default)]
    key: Option<String>,
    content_hash: String,
    byte_len: usize,
    created_at: i64,
    last_accessed_at: i64,
    content: String,
}

impl RuntimeSmartContextArtifactRead {
    fn into_artifact(
        self,
        key_hint: Option<&str>,
    ) -> Result<RuntimeSmartContextArtifact, RuntimeSmartContextArtifactStoreJsonError> {
        let artifact = RuntimeSmartContextArtifact {
            key: self
                .key
                .or_else(|| key_hint.map(str::to_string))
                .ok_or_else(|| RuntimeSmartContextArtifactStoreJsonError::new("missing key"))?,
            content_hash: self.content_hash,
            byte_len: self.byte_len,
            created_at: self.created_at,
            last_accessed_at: self.last_accessed_at,
            content: self.content,
        };
        runtime_smart_context_validate_artifact(&artifact)?;
        Ok(artifact)
    }
}

fn runtime_smart_context_artifact_store_version() -> u32 {
    crate::RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION
}

pub fn load_runtime_smart_context_artifact_store(
    path: impl AsRef<Path>,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> io::Result<RuntimeSmartContextArtifactStore> {
    let path = path.as_ref();
    let Some(content) = runtime_smart_context_read_artifact_store(path)? else {
        return Ok(RuntimeSmartContextArtifactStore::default());
    };
    let store = runtime_smart_context_artifact_store_from_json(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(compact_runtime_smart_context_artifact_store(
        store, now, policy,
    ))
}

pub fn save_runtime_smart_context_artifact_store(
    path: impl AsRef<Path>,
    store: &RuntimeSmartContextArtifactStore,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> io::Result<RuntimeSmartContextArtifactStore> {
    let path = path.as_ref();
    runtime_smart_context_prepare_artifact_store_parent(path)?;
    let _lock = runtime_smart_context_lock_artifact_store(path)?;
    let compacted = compact_runtime_smart_context_artifact_store(store.clone(), now, policy);
    runtime_smart_context_write_artifact_store(path, &compacted)?;
    Ok(compacted)
}

pub fn save_merged_runtime_smart_context_artifact_store(
    path: impl AsRef<Path>,
    store: &RuntimeSmartContextArtifactStore,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> io::Result<RuntimeSmartContextArtifactStore> {
    let path = path.as_ref();
    runtime_smart_context_prepare_artifact_store_parent(path)?;
    let _lock = runtime_smart_context_lock_artifact_store(path)?;
    let existing = load_runtime_smart_context_artifact_store(path, now, policy)?;
    let merged = merge_runtime_smart_context_artifact_stores(existing, store.clone());
    let compacted = compact_runtime_smart_context_artifact_store(merged, now, policy);
    runtime_smart_context_write_artifact_store(path, &compacted)?;
    Ok(compacted)
}

pub(super) fn runtime_smart_context_write_artifact_store(
    path: &Path,
    store: &RuntimeSmartContextArtifactStore,
) -> io::Result<()> {
    runtime_smart_context_prepare_artifact_store_parent(path)?;
    let (temp_path, mut temp_file) = runtime_smart_context_create_artifact_store_temp(path)?;
    let bytes = runtime_smart_context_artifact_store_to_json(store)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if let Err(error) = temp_file
        .write_all(bytes.as_bytes())
        .and_then(|()| temp_file.sync_all())
    {
        drop(temp_file);
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    drop(temp_file);
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    runtime_smart_context_sync_artifact_store_parent(path)?;
    Ok(())
}

fn runtime_smart_context_prepare_artifact_store_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn runtime_smart_context_lock_artifact_store(path: &Path) -> io::Result<fs::File> {
    let lock_path = runtime_smart_context_artifact_store_lock_path(path);
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(lock_path)?;
    lock.lock()?;
    Ok(lock)
}

fn runtime_smart_context_create_artifact_store_temp(
    path: &Path,
) -> io::Result<(PathBuf, fs::File)> {
    for _ in 0..16 {
        let temp_path = runtime_smart_context_artifact_store_temp_path(path);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "failed to allocate smart-context artifact store temp file",
    ))
}

#[cfg(unix)]
fn runtime_smart_context_sync_artifact_store_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn runtime_smart_context_sync_artifact_store_parent(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn runtime_smart_context_read_artifact_store(path: &Path) -> io::Result<Option<String>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file()
        || metadata.len() > RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES
    {
        return Ok(None);
    }

    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let opened_metadata = file.metadata()?;
    if !runtime_smart_context_same_artifact_store_file(&metadata, &opened_metadata) {
        return Ok(None);
    }

    let mut content = String::new();
    file.take(RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES.saturating_add(1))
        .read_to_string(&mut content)?;
    if content.len() as u64 > RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES {
        return Ok(None);
    }
    Ok(Some(content))
}

#[cfg(unix)]
fn runtime_smart_context_same_artifact_store_file(
    before: &fs::Metadata,
    after: &fs::Metadata,
) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn runtime_smart_context_same_artifact_store_file(
    _before: &fs::Metadata,
    _after: &fs::Metadata,
) -> bool {
    true
}

pub(super) fn runtime_smart_context_artifact_store_temp_path(path: &Path) -> PathBuf {
    let counter = RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or("smart-context-artifacts.json");
    path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        counter
    ))
}

fn runtime_smart_context_artifact_store_lock_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or("smart-context-artifacts.json");
    path.with_file_name(format!(".{file_name}.lock"))
}

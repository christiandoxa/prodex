use super::json::{RuntimeSmartContextJsonParser, RuntimeSmartContextJsonValue};
use super::*;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

const RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub fn runtime_smart_context_artifact_store_to_json(
    store: &RuntimeSmartContextArtifactStore,
) -> String {
    let mut output = String::new();
    output.push_str("{\n  \"version\": ");
    output.push_str(&store.version.to_string());
    output.push_str(",\n  \"artifacts\": [");
    if !store.artifacts.is_empty() {
        output.push('\n');
    }
    let len = store.artifacts.len();
    for (index, artifact) in store.artifacts.values().enumerate() {
        output.push_str("    {\n");
        output.push_str("      \"key\": ");
        output.push_str(&runtime_smart_context_json_string(&artifact.key));
        output.push_str(",\n      \"content_hash\": ");
        output.push_str(&runtime_smart_context_json_string(&artifact.content_hash));
        output.push_str(",\n      \"byte_len\": ");
        output.push_str(&artifact.byte_len.to_string());
        output.push_str(",\n      \"created_at\": ");
        output.push_str(&artifact.created_at.to_string());
        output.push_str(",\n      \"last_accessed_at\": ");
        output.push_str(&artifact.last_accessed_at.to_string());
        output.push_str(",\n      \"content\": ");
        output.push_str(&runtime_smart_context_json_string(&artifact.content));
        output.push_str("\n    }");
        if index + 1 != len {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("  ]\n}\n");
    output
}

pub fn runtime_smart_context_artifact_store_from_json(
    input: &str,
) -> Result<RuntimeSmartContextArtifactStore, RuntimeSmartContextArtifactStoreJsonError> {
    let value = RuntimeSmartContextJsonParser::parse(input)?;
    let root = runtime_smart_context_json_object(&value, "root")?;
    let version = match root.get("version") {
        Some(value) => runtime_smart_context_json_u32(value, "version")?,
        None => crate::RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION,
    };
    if version != crate::RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "unsupported smart-context artifact store version {version}"
        )));
    }

    let artifacts_value = root
        .get("artifacts")
        .ok_or_else(|| RuntimeSmartContextArtifactStoreJsonError::new("missing artifacts"))?;
    let mut artifacts = BTreeMap::new();
    match artifacts_value {
        RuntimeSmartContextJsonValue::Array(items) => {
            for item in items {
                let artifact = runtime_smart_context_artifact_from_json_value(item, None)?;
                artifacts.insert(artifact.key.clone(), artifact);
            }
        }
        RuntimeSmartContextJsonValue::Object(entries) => {
            for (key, item) in entries {
                let artifact = runtime_smart_context_artifact_from_json_value(item, Some(key))?;
                if artifact.key != *key {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
                        "artifact key mismatch for {key}"
                    )));
                }
                artifacts.insert(key.clone(), artifact);
            }
        }
        _ => {
            return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "artifacts must be array or object",
            ));
        }
    }

    Ok(RuntimeSmartContextArtifactStore { version, artifacts })
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
    let compacted = compact_runtime_smart_context_artifact_store(store.clone(), now, policy);
    runtime_smart_context_write_artifact_store(path.as_ref(), &compacted)?;
    Ok(compacted)
}

pub fn save_merged_runtime_smart_context_artifact_store(
    path: impl AsRef<Path>,
    store: &RuntimeSmartContextArtifactStore,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> io::Result<RuntimeSmartContextArtifactStore> {
    let path = path.as_ref();
    let existing = load_runtime_smart_context_artifact_store(path, now, policy)?;
    let merged = merge_runtime_smart_context_artifact_stores(existing, store.clone());
    save_runtime_smart_context_artifact_store(path, &merged, now, policy)
}

pub(super) fn runtime_smart_context_write_artifact_store(
    path: &Path,
    store: &RuntimeSmartContextArtifactStore,
) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let temp_path = runtime_smart_context_artifact_store_temp_path(path);
    fs::write(
        &temp_path,
        runtime_smart_context_artifact_store_to_json(store),
    )?;
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
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

fn runtime_smart_context_json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn runtime_smart_context_json_object<'a>(
    value: &'a RuntimeSmartContextJsonValue,
    context: &str,
) -> Result<
    &'a BTreeMap<String, RuntimeSmartContextJsonValue>,
    RuntimeSmartContextArtifactStoreJsonError,
> {
    match value {
        RuntimeSmartContextJsonValue::Object(object) => Ok(object),
        _ => Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "{context} must be object"
        ))),
    }
}

fn runtime_smart_context_json_string_value(
    value: &RuntimeSmartContextJsonValue,
    field: &str,
) -> Result<String, RuntimeSmartContextArtifactStoreJsonError> {
    match value {
        RuntimeSmartContextJsonValue::String(value) => Ok(value.clone()),
        _ => Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "{field} must be string"
        ))),
    }
}

fn runtime_smart_context_json_i64(
    value: &RuntimeSmartContextJsonValue,
    field: &str,
) -> Result<i64, RuntimeSmartContextArtifactStoreJsonError> {
    match value {
        RuntimeSmartContextJsonValue::Number(value) => Ok(*value),
        _ => Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "{field} must be number"
        ))),
    }
}

fn runtime_smart_context_json_u32(
    value: &RuntimeSmartContextJsonValue,
    field: &str,
) -> Result<u32, RuntimeSmartContextArtifactStoreJsonError> {
    let number = runtime_smart_context_json_i64(value, field)?;
    u32::try_from(number)
        .map_err(|_| RuntimeSmartContextArtifactStoreJsonError::new(format!("{field} must be u32")))
}

fn runtime_smart_context_json_usize(
    value: &RuntimeSmartContextJsonValue,
    field: &str,
) -> Result<usize, RuntimeSmartContextArtifactStoreJsonError> {
    let number = runtime_smart_context_json_i64(value, field)?;
    usize::try_from(number).map_err(|_| {
        RuntimeSmartContextArtifactStoreJsonError::new(format!("{field} must be usize"))
    })
}

fn runtime_smart_context_json_required_string(
    object: &BTreeMap<String, RuntimeSmartContextJsonValue>,
    field: &str,
) -> Result<String, RuntimeSmartContextArtifactStoreJsonError> {
    let value = object.get(field).ok_or_else(|| {
        RuntimeSmartContextArtifactStoreJsonError::new(format!("missing {field}"))
    })?;
    runtime_smart_context_json_string_value(value, field)
}

fn runtime_smart_context_json_optional_string(
    object: &BTreeMap<String, RuntimeSmartContextJsonValue>,
    field: &str,
) -> Result<Option<String>, RuntimeSmartContextArtifactStoreJsonError> {
    object
        .get(field)
        .map(|value| runtime_smart_context_json_string_value(value, field))
        .transpose()
}

fn runtime_smart_context_json_required_i64(
    object: &BTreeMap<String, RuntimeSmartContextJsonValue>,
    field: &str,
) -> Result<i64, RuntimeSmartContextArtifactStoreJsonError> {
    let value = object.get(field).ok_or_else(|| {
        RuntimeSmartContextArtifactStoreJsonError::new(format!("missing {field}"))
    })?;
    runtime_smart_context_json_i64(value, field)
}

fn runtime_smart_context_json_required_usize(
    object: &BTreeMap<String, RuntimeSmartContextJsonValue>,
    field: &str,
) -> Result<usize, RuntimeSmartContextArtifactStoreJsonError> {
    let value = object.get(field).ok_or_else(|| {
        RuntimeSmartContextArtifactStoreJsonError::new(format!("missing {field}"))
    })?;
    runtime_smart_context_json_usize(value, field)
}

fn runtime_smart_context_artifact_from_json_value(
    value: &RuntimeSmartContextJsonValue,
    key_hint: Option<&str>,
) -> Result<RuntimeSmartContextArtifact, RuntimeSmartContextArtifactStoreJsonError> {
    let object = runtime_smart_context_json_object(value, "artifact")?;
    let key = runtime_smart_context_json_optional_string(object, "key")?
        .or_else(|| key_hint.map(str::to_string))
        .ok_or_else(|| RuntimeSmartContextArtifactStoreJsonError::new("missing key"))?;
    let artifact = RuntimeSmartContextArtifact {
        key,
        content_hash: runtime_smart_context_json_required_string(object, "content_hash")?,
        byte_len: runtime_smart_context_json_required_usize(object, "byte_len")?,
        created_at: runtime_smart_context_json_required_i64(object, "created_at")?,
        last_accessed_at: runtime_smart_context_json_required_i64(object, "last_accessed_at")?,
        content: runtime_smart_context_json_required_string(object, "content")?,
    };
    runtime_smart_context_validate_artifact(&artifact)?;
    Ok(artifact)
}

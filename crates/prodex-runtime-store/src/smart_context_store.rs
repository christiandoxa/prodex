use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) static RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct RuntimeMergedStateAndContinuations {
    pub state: AppState,
    pub continuations: RuntimeContinuationStore<ResponseProfileBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextArtifactStorePolicy {
    pub ttl_seconds: i64,
    pub max_entries: usize,
}

impl Default for RuntimeSmartContextArtifactStorePolicy {
    fn default() -> Self {
        Self {
            ttl_seconds: 24 * 60 * 60,
            max_entries: 1_024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextArtifact {
    pub key: String,
    pub content_hash: String,
    pub byte_len: usize,
    pub created_at: i64,
    pub last_accessed_at: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextArtifactStore {
    pub version: u32,
    pub artifacts: BTreeMap<String, RuntimeSmartContextArtifact>,
}

impl Default for RuntimeSmartContextArtifactStore {
    fn default() -> Self {
        Self {
            version: RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION,
            artifacts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextLineRange {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextExtractedLineRange {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextStaleContextSnapshot<'a> {
    pub hash: Option<&'a str>,
    pub byte_len: usize,
    pub token_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSmartContextStaleContextPruningInput<'a> {
    pub previous: Option<RuntimeSmartContextStaleContextSnapshot<'a>>,
    pub current: RuntimeSmartContextStaleContextSnapshot<'a>,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSmartContextStaleContextPruningKind {
    TooSmall,
    NoPrevious,
    ExactReuse,
    Changed,
}

impl RuntimeSmartContextStaleContextPruningKind {
    pub fn can_prune_payload(self) -> bool {
        matches!(self, Self::ExactReuse | Self::Changed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextStaleContextPruningDecision {
    pub kind: RuntimeSmartContextStaleContextPruningKind,
    pub summary: String,
    pub previous_hash: Option<String>,
    pub current_hash: Option<String>,
    pub previous_byte_len: Option<usize>,
    pub current_byte_len: usize,
    pub previous_token_len: Option<usize>,
    pub current_token_len: usize,
    pub reusable_byte_len: usize,
    pub reusable_token_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSmartContextArtifactStoreJsonError {
    pub message: String,
}

impl RuntimeSmartContextArtifactStoreJsonError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RuntimeSmartContextArtifactStoreJsonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeSmartContextArtifactStoreJsonError {}

mod json;

use self::json::{RuntimeSmartContextJsonParser, RuntimeSmartContextJsonValue};

pub fn runtime_smart_context_artifact_content_hash(content: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in content {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

pub fn runtime_smart_context_stale_context_pruning_decision(
    input: RuntimeSmartContextStaleContextPruningInput<'_>,
) -> RuntimeSmartContextStaleContextPruningDecision {
    let current_hash = runtime_smart_context_normalized_hash(input.current.hash);
    let previous = input.previous;
    let previous_hash = previous.and_then(|snapshot| {
        runtime_smart_context_normalized_hash(snapshot.hash).map(str::to_string)
    });
    let current_hash_owned = current_hash.map(str::to_string);
    let previous_byte_len = previous.map(|snapshot| snapshot.byte_len);
    let previous_token_len = previous.map(|snapshot| snapshot.token_len);

    let current_is_small = input.current.byte_len < RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_BYTES
        && input.current.token_len < RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_TOKENS;
    if current_is_small {
        return RuntimeSmartContextStaleContextPruningDecision {
            kind: RuntimeSmartContextStaleContextPruningKind::TooSmall,
            summary: format!(
                "static-context: no-op reason=too_small current_hash={} current_bytes={} current_tokens={}",
                runtime_smart_context_hash_summary(current_hash),
                input.current.byte_len,
                input.current.token_len
            ),
            previous_hash,
            current_hash: current_hash_owned,
            previous_byte_len,
            current_byte_len: input.current.byte_len,
            previous_token_len,
            current_token_len: input.current.token_len,
            reusable_byte_len: 0,
            reusable_token_len: 0,
        };
    }

    let Some(previous) = previous else {
        return RuntimeSmartContextStaleContextPruningDecision {
            kind: RuntimeSmartContextStaleContextPruningKind::NoPrevious,
            summary: format!(
                "static-context: no-op reason=no_previous current_hash={} current_bytes={} current_tokens={}",
                runtime_smart_context_hash_summary(current_hash),
                input.current.byte_len,
                input.current.token_len
            ),
            previous_hash,
            current_hash: current_hash_owned,
            previous_byte_len,
            current_byte_len: input.current.byte_len,
            previous_token_len,
            current_token_len: input.current.token_len,
            reusable_byte_len: 0,
            reusable_token_len: 0,
        };
    };

    let previous_hash_ref = runtime_smart_context_normalized_hash(previous.hash);
    let hashes_match = previous_hash_ref
        .zip(current_hash)
        .is_some_and(|(previous_hash, current_hash)| previous_hash == current_hash);
    let hash_mismatch = previous_hash_ref
        .zip(current_hash)
        .is_some_and(|(previous_hash, current_hash)| previous_hash != current_hash);
    let caller_reports_unchanged_with_matching_size = !input.changed
        && !hash_mismatch
        && previous.byte_len == input.current.byte_len
        && previous.token_len == input.current.token_len;

    if hashes_match || caller_reports_unchanged_with_matching_size {
        return RuntimeSmartContextStaleContextPruningDecision {
            kind: RuntimeSmartContextStaleContextPruningKind::ExactReuse,
            summary: format!(
                "static-context: reuse hash={} bytes={} tokens={} saved_bytes={} saved_tokens={}",
                runtime_smart_context_hash_summary(current_hash.or(previous_hash_ref)),
                input.current.byte_len,
                input.current.token_len,
                input.current.byte_len,
                input.current.token_len
            ),
            previous_hash,
            current_hash: current_hash_owned,
            previous_byte_len,
            current_byte_len: input.current.byte_len,
            previous_token_len,
            current_token_len: input.current.token_len,
            reusable_byte_len: input.current.byte_len,
            reusable_token_len: input.current.token_len,
        };
    }

    RuntimeSmartContextStaleContextPruningDecision {
        kind: RuntimeSmartContextStaleContextPruningKind::Changed,
        summary: format!(
            "static-context: changed previous_hash={} current_hash={} previous_bytes={} current_bytes={} previous_tokens={} current_tokens={} byte_delta={} token_delta={}",
            runtime_smart_context_hash_summary(previous_hash_ref),
            runtime_smart_context_hash_summary(current_hash),
            previous.byte_len,
            input.current.byte_len,
            previous.token_len,
            input.current.token_len,
            runtime_smart_context_signed_delta(input.current.byte_len, previous.byte_len),
            runtime_smart_context_signed_delta(input.current.token_len, previous.token_len)
        ),
        previous_hash,
        current_hash: current_hash_owned,
        previous_byte_len,
        current_byte_len: input.current.byte_len,
        previous_token_len,
        current_token_len: input.current.token_len,
        reusable_byte_len: 0,
        reusable_token_len: 0,
    }
}

pub(super) fn runtime_smart_context_normalized_hash(hash: Option<&str>) -> Option<&str> {
    hash.map(str::trim).filter(|hash| !hash.is_empty())
}

pub(super) fn runtime_smart_context_hash_summary(hash: Option<&str>) -> &str {
    hash.unwrap_or("none")
}

pub(super) fn runtime_smart_context_signed_delta(current: usize, previous: usize) -> String {
    if current >= previous {
        format!("+{}", current - previous)
    } else {
        format!("-{}", previous - current)
    }
}

pub fn runtime_smart_context_artifact_from_content(
    key: impl Into<String>,
    content: impl Into<String>,
    now: i64,
) -> RuntimeSmartContextArtifact {
    let content = content.into();
    RuntimeSmartContextArtifact {
        key: key.into(),
        content_hash: runtime_smart_context_artifact_content_hash(content.as_bytes()),
        byte_len: content.len(),
        created_at: now,
        last_accessed_at: now,
        content,
    }
}

pub fn runtime_smart_context_upsert_artifact(
    store: &mut RuntimeSmartContextArtifactStore,
    key: impl Into<String>,
    content: impl Into<String>,
    now: i64,
) -> RuntimeSmartContextArtifact {
    let key = key.into();
    let content = content.into();
    let content_hash = runtime_smart_context_artifact_content_hash(content.as_bytes());
    let created_at = store
        .artifacts
        .get(&key)
        .filter(|artifact| artifact.content_hash == content_hash)
        .map(|artifact| artifact.created_at)
        .unwrap_or(now);
    let artifact = RuntimeSmartContextArtifact {
        key: key.clone(),
        content_hash,
        byte_len: content.len(),
        created_at,
        last_accessed_at: now,
        content,
    };
    store.artifacts.insert(key, artifact.clone());
    artifact
}

pub fn runtime_smart_context_touch_artifact<'a>(
    store: &'a mut RuntimeSmartContextArtifactStore,
    key: &str,
    now: i64,
) -> Option<&'a RuntimeSmartContextArtifact> {
    store.artifacts.get_mut(key).map(|artifact| {
        artifact.last_accessed_at = artifact.last_accessed_at.max(now);
        &*artifact
    })
}

pub fn runtime_smart_context_extract_line_range(
    content: &str,
    range: RuntimeSmartContextLineRange,
) -> Option<RuntimeSmartContextExtractedLineRange> {
    if range.start_line == 0 || range.end_line < range.start_line {
        return None;
    }

    let mut extracted = String::new();
    let mut end_line = None;
    for (index, line) in content.split_inclusive('\n').enumerate() {
        let line_number = index + 1;
        if line_number < range.start_line {
            continue;
        }
        if line_number > range.end_line {
            break;
        }
        extracted.push_str(line);
        end_line = Some(line_number);
    }

    end_line.map(|end_line| RuntimeSmartContextExtractedLineRange {
        start_line: range.start_line,
        end_line,
        content: extracted,
    })
}

pub fn runtime_smart_context_artifact_line_range(
    artifact: &RuntimeSmartContextArtifact,
    range: RuntimeSmartContextLineRange,
) -> Option<RuntimeSmartContextExtractedLineRange> {
    runtime_smart_context_extract_line_range(&artifact.content, range)
}

pub fn merge_runtime_smart_context_artifact_stores(
    mut existing: RuntimeSmartContextArtifactStore,
    incoming: RuntimeSmartContextArtifactStore,
) -> RuntimeSmartContextArtifactStore {
    existing.version = RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION;
    for (key, mut incoming_artifact) in incoming.artifacts {
        incoming_artifact.key = key.clone();
        existing
            .artifacts
            .entry(key)
            .and_modify(|current| {
                *current = runtime_smart_context_merge_artifact(current, &incoming_artifact);
            })
            .or_insert(incoming_artifact);
    }
    existing
}

pub fn compact_runtime_smart_context_artifact_store(
    mut store: RuntimeSmartContextArtifactStore,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> RuntimeSmartContextArtifactStore {
    store.version = RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION;
    store.artifacts.retain(|key, artifact| {
        artifact.key == *key
            && runtime_smart_context_artifact_should_retain_for_ttl(artifact, now, policy)
    });

    if store.artifacts.len() > policy.max_entries {
        let excess = store.artifacts.len() - policy.max_entries;
        let mut coldest = store
            .artifacts
            .iter()
            .map(|(key, artifact)| {
                (
                    key.clone(),
                    (
                        runtime_smart_context_artifact_retention_time(artifact),
                        artifact.created_at,
                        artifact.byte_len,
                    ),
                )
            })
            .collect::<Vec<_>>();
        coldest.sort_by_key(|(_, retention)| *retention);
        for (key, _) in coldest.into_iter().take(excess) {
            store.artifacts.remove(&key);
        }
    }

    store
}

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
        None => RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION,
    };
    if version != RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION {
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
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RuntimeSmartContextArtifactStore::default());
        }
        Err(error) => return Err(error),
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

pub(super) fn runtime_smart_context_merge_artifact(
    current: &RuntimeSmartContextArtifact,
    incoming: &RuntimeSmartContextArtifact,
) -> RuntimeSmartContextArtifact {
    if current.content_hash == incoming.content_hash && current.content == incoming.content {
        let mut merged = if incoming.last_accessed_at >= current.last_accessed_at {
            incoming.clone()
        } else {
            current.clone()
        };
        merged.created_at = current.created_at.min(incoming.created_at);
        merged.last_accessed_at = current.last_accessed_at.max(incoming.last_accessed_at);
        return merged;
    }

    let incoming_rank = (
        runtime_smart_context_artifact_retention_time(incoming),
        incoming.created_at,
        incoming.byte_len,
    );
    let current_rank = (
        runtime_smart_context_artifact_retention_time(current),
        current.created_at,
        current.byte_len,
    );
    if incoming_rank >= current_rank {
        incoming.clone()
    } else {
        current.clone()
    }
}

pub(super) fn runtime_smart_context_artifact_retention_time(
    artifact: &RuntimeSmartContextArtifact,
) -> i64 {
    artifact.created_at.max(artifact.last_accessed_at)
}

pub(super) fn runtime_smart_context_artifact_should_retain_for_ttl(
    artifact: &RuntimeSmartContextArtifact,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> bool {
    policy.ttl_seconds <= 0
        || now.saturating_sub(runtime_smart_context_artifact_retention_time(artifact))
            <= policy.ttl_seconds
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

pub(super) fn runtime_smart_context_validate_artifact(
    artifact: &RuntimeSmartContextArtifact,
) -> Result<(), RuntimeSmartContextArtifactStoreJsonError> {
    if artifact.key.is_empty() {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(
            "artifact key must not be empty",
        ));
    }
    if artifact.byte_len != artifact.content.len() {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "artifact {} byte_len mismatch",
            artifact.key
        )));
    }
    let content_hash = runtime_smart_context_artifact_content_hash(artifact.content.as_bytes());
    if artifact.content_hash != content_hash {
        return Err(RuntimeSmartContextArtifactStoreJsonError::new(format!(
            "artifact {} content_hash mismatch",
            artifact.key
        )));
    }
    Ok(())
}

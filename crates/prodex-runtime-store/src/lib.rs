//! Runtime store merge, compaction, and small persisted cache helpers.
//!
//! Runtime state save orchestration stays in the binary crate. This crate keeps
//! reusable merge/retention primitives and the smart-context artifact JSON cache
//! boundary.

use prodex_runtime_state::{
    RuntimeContinuationBindingLifecycle, RuntimeContinuationBindingStatus,
    RuntimeContinuationStatuses, RuntimeContinuationStore, RuntimeProfileBackoffs,
    RuntimeProfileHealth, RuntimeProfileUsageSnapshot, RuntimeRouteKind, RuntimeStateSaveSections,
    RuntimeStateSaveSelectedSnapshot, RuntimeStateSaveStateSection,
};
use prodex_state::{
    AppState, AppStateCompactionPolicy, ProfileEntry, ResponseProfileBinding,
    merge_profile_bindings, prune_profile_bindings,
    prune_profile_bindings_for_housekeeping_without_retention,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const RUNTIME_SCORE_RETENTION_SECONDS: i64 = if cfg!(test) { 120 } else { 14 * 24 * 60 * 60 };
pub const RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS: i64 =
    if cfg!(test) { 120 } else { 7 * 24 * 60 * 60 };
pub const RUNTIME_PROFILE_HEALTH_DECAY_SECONDS: i64 = if cfg!(test) { 2 } else { 60 };
pub const RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS: i64 = if cfg!(test) { 4 } else { 180 };
pub const RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS: i64 = if cfg!(test) { 8 } else { 300 };
pub const RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS: i64 = if cfg!(test) { 2 } else { 15 };
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS: i64 = 20;
pub const RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS: i64 = if cfg!(test) { 320 } else { 600 };
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;
pub const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS: i64 =
    if cfg!(test) { 20 } else { 60 };
pub const RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS: i64 = if cfg!(test) { 12 } else { 1_800 };
pub const RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE: u32 = 4;
pub const RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_VERSION: u32 = 1;

static RUNTIME_SMART_CONTEXT_ARTIFACT_STORE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeSmartContextJsonValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<RuntimeSmartContextJsonValue>),
    Object(BTreeMap<String, RuntimeSmartContextJsonValue>),
}

struct RuntimeSmartContextJsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> RuntimeSmartContextJsonParser<'a> {
    fn parse(
        input: &'a str,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        let mut parser = Self {
            input: input.as_bytes(),
            pos: 0,
        };
        let value = parser.parse_value()?;
        parser.skip_whitespace();
        if parser.pos != parser.input.len() {
            return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "trailing JSON content",
            ));
        }
        Ok(value)
    }

    fn parse_value(
        &mut self,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        self.skip_whitespace();
        match self.peek_byte() {
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(RuntimeSmartContextJsonValue::Null)
            }
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(RuntimeSmartContextJsonValue::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(RuntimeSmartContextJsonValue::Bool(false))
            }
            Some(b'"') => self
                .parse_string()
                .map(RuntimeSmartContextJsonValue::String),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self
                .parse_number()
                .map(RuntimeSmartContextJsonValue::Number),
            Some(_) => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON value",
            )),
            None => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "unexpected end of JSON",
            )),
        }
    }

    fn parse_array(
        &mut self,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.consume_byte(b']') {
            return Ok(RuntimeSmartContextJsonValue::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume_byte(b']') {
                return Ok(RuntimeSmartContextJsonValue::Array(items));
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_object(
        &mut self,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        let mut entries = BTreeMap::new();
        if self.consume_byte(b'}') {
            return Ok(RuntimeSmartContextJsonValue::Object(entries));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect_byte(b':')?;
            let value = self.parse_value()?;
            entries.insert(key, value);
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                return Ok(RuntimeSmartContextJsonValue::Object(entries));
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_string(&mut self) -> Result<String, RuntimeSmartContextArtifactStoreJsonError> {
        self.expect_byte(b'"')?;
        let mut output = String::new();
        loop {
            let start = self.pos;
            while let Some(byte) = self.peek_byte() {
                match byte {
                    b'"' | b'\\' | 0x00..=0x1f => break,
                    _ => self.pos += 1,
                }
            }
            if self.pos > start {
                let chunk = std::str::from_utf8(&self.input[start..self.pos]).map_err(|_| {
                    RuntimeSmartContextArtifactStoreJsonError::new("invalid UTF-8 string")
                })?;
                output.push_str(chunk);
            }
            match self.next_byte() {
                Some(b'"') => return Ok(output),
                Some(b'\\') => output.push(self.parse_escape()?),
                Some(0x00..=0x1f) => {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                        "unescaped control character in string",
                    ));
                }
                Some(_) => unreachable!("string scanner stops only on delimiter or control"),
                None => {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                        "unterminated JSON string",
                    ));
                }
            }
        }
    }

    fn parse_escape(&mut self) -> Result<char, RuntimeSmartContextArtifactStoreJsonError> {
        match self.next_byte() {
            Some(b'"') => Ok('"'),
            Some(b'\\') => Ok('\\'),
            Some(b'/') => Ok('/'),
            Some(b'b') => Ok('\u{08}'),
            Some(b'f') => Ok('\u{0c}'),
            Some(b'n') => Ok('\n'),
            Some(b'r') => Ok('\r'),
            Some(b't') => Ok('\t'),
            Some(b'u') => {
                let codepoint = self.parse_hex4()?;
                char::from_u32(codepoint).ok_or_else(|| {
                    RuntimeSmartContextArtifactStoreJsonError::new("invalid unicode escape")
                })
            }
            Some(_) => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON escape",
            )),
            None => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "unterminated JSON escape",
            )),
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, RuntimeSmartContextArtifactStoreJsonError> {
        let mut codepoint = 0_u32;
        for _ in 0..4 {
            let Some(byte) = self.next_byte() else {
                return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                    "short unicode escape",
                ));
            };
            let digit = match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'a'..=b'f' => u32::from(byte - b'a' + 10),
                b'A'..=b'F' => u32::from(byte - b'A' + 10),
                _ => {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                        "invalid unicode escape",
                    ));
                }
            };
            codepoint = (codepoint << 4) | digit;
        }
        Ok(codepoint)
    }

    fn parse_number(&mut self) -> Result<i64, RuntimeSmartContextArtifactStoreJsonError> {
        let start = self.pos;
        self.consume_byte(b'-');
        let digit_start = self.pos;
        while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.pos == digit_start {
            return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON number",
            ));
        }
        let text = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| RuntimeSmartContextArtifactStoreJsonError::new("invalid JSON number"))?;
        text.parse::<i64>()
            .map_err(|_| RuntimeSmartContextArtifactStoreJsonError::new("invalid JSON number"))
    }

    fn expect_literal(
        &mut self,
        literal: &[u8],
    ) -> Result<(), RuntimeSmartContextArtifactStoreJsonError> {
        if self
            .input
            .get(self.pos..self.pos.saturating_add(literal.len()))
            == Some(literal)
        {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON literal",
            ))
        }
    }

    fn expect_byte(
        &mut self,
        expected: u8,
    ) -> Result<(), RuntimeSmartContextArtifactStoreJsonError> {
        match self.next_byte() {
            Some(byte) if byte == expected => Ok(()),
            _ => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "unexpected JSON token",
            )),
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek_byte() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn next_byte(&mut self) -> Option<u8> {
        let byte = self.input.get(self.pos).copied()?;
        self.pos += 1;
        Some(byte)
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }
}

pub fn runtime_smart_context_artifact_content_hash(content: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in content {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
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

fn runtime_smart_context_merge_artifact(
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

fn runtime_smart_context_artifact_retention_time(artifact: &RuntimeSmartContextArtifact) -> i64 {
    artifact.created_at.max(artifact.last_accessed_at)
}

fn runtime_smart_context_artifact_should_retain_for_ttl(
    artifact: &RuntimeSmartContextArtifact,
    now: i64,
    policy: RuntimeSmartContextArtifactStorePolicy,
) -> bool {
    policy.ttl_seconds <= 0
        || now.saturating_sub(runtime_smart_context_artifact_retention_time(artifact))
            <= policy.ttl_seconds
}

fn runtime_smart_context_write_artifact_store(
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

fn runtime_smart_context_artifact_store_temp_path(path: &Path) -> PathBuf {
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

fn runtime_smart_context_validate_artifact(
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

pub fn merge_runtime_state_snapshot_for_save(
    existing: AppState,
    snapshot: &AppState,
    now: i64,
    policy: AppStateCompactionPolicy,
) -> AppState {
    prodex_state::merge_runtime_state_snapshot_with_policy(existing, snapshot, now, policy)
}

pub fn merge_runtime_state_and_continuations_for_save(
    existing_state: AppState,
    state_snapshot: &AppState,
    existing_continuations: &RuntimeContinuationStore<ResponseProfileBinding>,
    continuation_snapshot: &RuntimeContinuationStore<ResponseProfileBinding>,
    now: i64,
    state_policy: AppStateCompactionPolicy,
    continuation_policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeMergedStateAndContinuations {
    let mut state =
        merge_runtime_state_snapshot_for_save(existing_state, state_snapshot, now, state_policy);
    let continuations = merge_runtime_continuation_store(
        existing_continuations,
        continuation_snapshot,
        &state.profiles,
        now,
        continuation_policy,
    );
    state.response_profile_bindings =
        runtime_external_response_profile_bindings(&continuations.response_profile_bindings);
    state.session_profile_bindings = continuations.session_profile_bindings.clone();
    RuntimeMergedStateAndContinuations {
        state,
        continuations,
    }
}

pub fn runtime_state_save_selected_snapshot_from_parts<P, C, H, U, B>(
    paths: &P,
    state: &AppState,
    continuations: &C,
    profile_scores: &BTreeMap<String, H>,
    usage_snapshots: &BTreeMap<String, U>,
    backoffs: &B,
    sections: RuntimeStateSaveSections,
) -> RuntimeStateSaveSelectedSnapshot<P, AppState, ProfileEntry, C, H, U, B>
where
    P: Clone,
    C: Clone,
    H: Clone,
    U: Clone,
    B: Clone,
{
    let state_snapshot = match sections.state {
        RuntimeStateSaveStateSection::None => None,
        RuntimeStateSaveStateSection::Core => Some(AppState {
            active_profile: state.active_profile.clone(),
            profiles: state.profiles.clone(),
            last_run_selected_at: state.last_run_selected_at.clone(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        }),
        RuntimeStateSaveStateSection::Full => Some(state.clone()),
    };
    let profiles = state_snapshot.is_none().then(|| state.profiles.clone());
    RuntimeStateSaveSelectedSnapshot {
        paths: paths.clone(),
        state: state_snapshot,
        profiles,
        continuations: sections.continuations.then(|| continuations.clone()),
        profile_scores: sections.profile_scores.then(|| profile_scores.clone()),
        usage_snapshots: sections.usage_snapshots.then(|| usage_snapshots.clone()),
        backoffs: sections.backoffs.then(|| backoffs.clone()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStateSelectedSnapshotLoadPlan {
    pub needs_existing_state: bool,
    pub needs_existing_continuations: bool,
}

pub fn runtime_state_selected_snapshot_load_plan<P, S, E, C, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<P, S, E, C, H, U, B>,
) -> RuntimeStateSelectedSnapshotLoadPlan {
    RuntimeStateSelectedSnapshotLoadPlan {
        needs_existing_state: snapshot.state.is_some() || snapshot.profiles.is_none(),
        needs_existing_continuations: snapshot.continuations.is_some(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStateSelectedSnapshotPreparePlan {
    pub load: RuntimeStateSelectedSnapshotLoadPlan,
    pub writes_state: bool,
    pub writes_continuations: bool,
    pub writes_profile_scores: bool,
    pub writes_usage_snapshots: bool,
    pub writes_backoffs: bool,
}

pub fn runtime_state_selected_snapshot_prepare_plan<P, S, E, C, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<P, S, E, C, H, U, B>,
) -> RuntimeStateSelectedSnapshotPreparePlan {
    RuntimeStateSelectedSnapshotPreparePlan {
        load: runtime_state_selected_snapshot_load_plan(snapshot),
        writes_state: snapshot.state.is_some(),
        writes_continuations: snapshot.continuations.is_some(),
        writes_profile_scores: snapshot.profile_scores.is_some(),
        writes_usage_snapshots: snapshot.usage_snapshots.is_some(),
        writes_backoffs: snapshot.backoffs.is_some(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStateSelectedSnapshotMergedSections {
    pub profiles: BTreeMap<String, ProfileEntry>,
    pub state: Option<AppState>,
    pub continuations: Option<RuntimeContinuationStore<ResponseProfileBinding>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStateSelectedSnapshotMergeError {
    MissingExistingState,
    MissingProfiles,
    MissingExistingContinuations,
}

impl std::fmt::Display for RuntimeStateSelectedSnapshotMergeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingExistingState => formatter.write_str("missing existing state"),
            Self::MissingProfiles => formatter.write_str("missing profiles"),
            Self::MissingExistingContinuations => {
                formatter.write_str("missing existing continuations")
            }
        }
    }
}

impl std::error::Error for RuntimeStateSelectedSnapshotMergeError {}

pub fn runtime_state_selected_snapshot_profiles_for_merge<P, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<
        P,
        AppState,
        ProfileEntry,
        RuntimeContinuationStore<ResponseProfileBinding>,
        H,
        U,
        B,
    >,
    existing_state: Option<&AppState>,
    now: i64,
    state_policy: AppStateCompactionPolicy,
) -> Result<BTreeMap<String, ProfileEntry>, RuntimeStateSelectedSnapshotMergeError> {
    if let Some(state_snapshot) = snapshot.state.as_ref() {
        let existing_state =
            existing_state.ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingState)?;
        return Ok(merge_runtime_state_snapshot_for_save(
            existing_state.clone(),
            state_snapshot,
            now,
            state_policy,
        )
        .profiles);
    }

    snapshot
        .profiles
        .clone()
        .or_else(|| existing_state.map(|state| state.profiles.clone()))
        .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingProfiles)
}

pub fn merge_runtime_state_selected_snapshot_sections<P, H, U, B>(
    snapshot: &RuntimeStateSaveSelectedSnapshot<
        P,
        AppState,
        ProfileEntry,
        RuntimeContinuationStore<ResponseProfileBinding>,
        H,
        U,
        B,
    >,
    existing_state: Option<&AppState>,
    existing_continuations: Option<&RuntimeContinuationStore<ResponseProfileBinding>>,
    now: i64,
    state_policy: AppStateCompactionPolicy,
    continuation_policy: RuntimeContinuationCompactionPolicy,
) -> Result<RuntimeStateSelectedSnapshotMergedSections, RuntimeStateSelectedSnapshotMergeError> {
    let mut state = None;
    let mut continuations = None;
    let profiles = if let Some(state_snapshot) = snapshot.state.as_ref() {
        let existing_state =
            existing_state.ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingState)?;
        if let Some(continuation_snapshot) = snapshot.continuations.as_ref() {
            let existing_continuations = existing_continuations
                .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingContinuations)?;
            let merged = merge_runtime_state_and_continuations_for_save(
                existing_state.clone(),
                state_snapshot,
                existing_continuations,
                continuation_snapshot,
                now,
                state_policy,
                continuation_policy,
            );
            let profiles = merged.state.profiles.clone();
            state = Some(merged.state);
            continuations = Some(merged.continuations);
            profiles
        } else {
            let merged_state = merge_runtime_state_snapshot_for_save(
                existing_state.clone(),
                state_snapshot,
                now,
                state_policy,
            );
            let profiles = merged_state.profiles.clone();
            state = Some(merged_state);
            profiles
        }
    } else {
        let profiles = snapshot
            .profiles
            .clone()
            .or_else(|| existing_state.map(|state| state.profiles.clone()))
            .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingProfiles)?;
        if let Some(continuation_snapshot) = snapshot.continuations.as_ref() {
            let existing_continuations = existing_continuations
                .ok_or(RuntimeStateSelectedSnapshotMergeError::MissingExistingContinuations)?;
            continuations = Some(merge_runtime_continuation_store(
                existing_continuations,
                continuation_snapshot,
                &profiles,
                now,
                continuation_policy,
            ));
        }
        profiles
    };

    Ok(RuntimeStateSelectedSnapshotMergedSections {
        profiles,
        state,
        continuations,
    })
}

pub fn compact_runtime_usage_snapshots<W>(
    mut snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let oldest_allowed = now.saturating_sub(RUNTIME_USAGE_SNAPSHOT_RETENTION_SECONDS);
    snapshots.retain(|profile_name, snapshot| {
        profiles.contains_key(profile_name) && snapshot.checked_at >= oldest_allowed
    });
    snapshots
}

pub fn merge_runtime_usage_snapshots<W: Clone>(
    existing: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    incoming: &BTreeMap<String, RuntimeProfileUsageSnapshot<W>>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileUsageSnapshot<W>> {
    let mut merged = existing.clone();
    for (profile_name, snapshot) in incoming {
        let should_replace = merged
            .get(profile_name)
            .is_none_or(|current| current.checked_at <= snapshot.checked_at);
        if should_replace {
            merged.insert(profile_name.clone(), snapshot.clone());
        }
    }
    compact_runtime_usage_snapshots(merged, profiles, now)
}

pub fn compact_runtime_profile_scores(
    mut scores: BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let oldest_allowed = now.saturating_sub(RUNTIME_SCORE_RETENTION_SECONDS);
    scores.retain(|key, value| {
        profiles.contains_key(runtime_profile_score_profile_name(key))
            && value.updated_at >= oldest_allowed
    });
    scores
}

pub fn merge_runtime_profile_scores(
    existing: &BTreeMap<String, RuntimeProfileHealth>,
    incoming: &BTreeMap<String, RuntimeProfileHealth>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> BTreeMap<String, RuntimeProfileHealth> {
    let mut merged = existing.clone();
    for (key, value) in incoming {
        let should_replace = merged
            .get(key)
            .is_none_or(|current| current.updated_at <= value.updated_at);
        if should_replace {
            merged.insert(key.clone(), value.clone());
        }
    }
    compact_runtime_profile_scores(merged, profiles, now)
}

pub fn runtime_profile_score_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub fn compact_runtime_profile_backoffs(
    mut backoffs: RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    backoffs
        .retry_backoff_until
        .retain(|profile_name, until| profiles.contains_key(profile_name) && *until > now);
    backoffs.transport_backoff_until.retain(|key, until| {
        runtime_profile_transport_backoff_key_matches_profiles(key, profiles) && *until > now
    });
    backoffs
        .route_circuit_open_until
        .retain(|route_profile_key, _| {
            profiles.contains_key(runtime_profile_route_circuit_profile_name(
                route_profile_key,
            ))
        });
    backoffs
}

pub fn merge_runtime_profile_backoffs(
    existing: &RuntimeProfileBackoffs,
    incoming: &RuntimeProfileBackoffs,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) -> RuntimeProfileBackoffs {
    let mut merged = existing.clone();
    for (profile_name, until) in &incoming.retry_backoff_until {
        merged
            .retry_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (profile_name, until) in &incoming.transport_backoff_until {
        merged
            .transport_backoff_until
            .insert(profile_name.clone(), *until);
    }
    for (route_profile_key, until) in &incoming.route_circuit_open_until {
        merged
            .route_circuit_open_until
            .insert(route_profile_key.clone(), *until);
    }
    compact_runtime_profile_backoffs(merged, profiles, now)
}

pub fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses",
        RuntimeRouteKind::Compact => "compact",
        RuntimeRouteKind::Websocket => "websocket",
        RuntimeRouteKind::Standard => "standard",
    }
}

pub fn runtime_route_kind_from_label(label: &str) -> Option<RuntimeRouteKind> {
    match label {
        "responses" => Some(RuntimeRouteKind::Responses),
        "compact" => Some(RuntimeRouteKind::Compact),
        "websocket" => Some(RuntimeRouteKind::Websocket),
        "standard" => Some(RuntimeRouteKind::Standard),
        _ => None,
    }
}

pub fn runtime_route_coupled_kinds(route_kind: RuntimeRouteKind) -> &'static [RuntimeRouteKind] {
    match route_kind {
        RuntimeRouteKind::Responses => &[RuntimeRouteKind::Websocket],
        RuntimeRouteKind::Websocket => &[RuntimeRouteKind::Responses],
        RuntimeRouteKind::Compact => &[RuntimeRouteKind::Standard],
        RuntimeRouteKind::Standard => &[RuntimeRouteKind::Compact],
    }
}

pub fn runtime_profile_effective_health_score(entry: &RuntimeProfileHealth, now: i64) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

pub fn runtime_profile_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

pub fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

pub fn runtime_profile_effective_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

pub fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_success__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    key.replacen("__route_circuit__", "__route_health__", 1)
}

pub fn runtime_profile_route_circuit_reopen_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit_reopen__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_global_health_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
) -> u32 {
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
}

pub fn runtime_profile_route_health_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(
        profile_health,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    )
}

pub fn runtime_profile_route_coupling_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_from_map(
                profile_health,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_bad_pairing_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            route_score
                .saturating_add(bad_pairing_score)
                .saturating_div(2)
        })
        .fold(0, u32::saturating_add)
}

pub fn runtime_profile_route_performance_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    let route_score = runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_performance_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
            .saturating_div(2)
        })
        .fold(0, u32::saturating_add);
    route_score.saturating_add(coupled_score)
}

pub fn runtime_profile_health_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_global_health_score(profile_health, profile_name, now)
        .saturating_add(runtime_profile_route_health_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_coupling_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

pub fn runtime_profile_health_sort_key(
    profile_name: &str,
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
        .saturating_add(runtime_profile_route_health_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_effective_score_from_map(
            profile_health,
            &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
            now,
            RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        ))
        .saturating_add(runtime_profile_route_coupling_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

pub fn runtime_previous_response_negative_cache_key(
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__previous_response_not_found__:{}:{}:{profile_name}",
        runtime_route_kind_label(route_kind),
        previous_response_id
    )
}

pub fn runtime_previous_response_negative_cache_failures(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_previous_response_negative_cache_key(
            previous_response_id,
            profile_name,
            route_kind,
        ),
        now,
        decay_seconds,
    )
}

pub fn runtime_previous_response_negative_cache_active(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
    decay_seconds: i64,
) -> bool {
    runtime_previous_response_negative_cache_failures(
        profile_health,
        previous_response_id,
        profile_name,
        route_kind,
        now,
        decay_seconds,
    ) > 0
}

pub fn clear_runtime_previous_response_negative_cache(
    profile_health: &mut BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
) -> bool {
    let mut changed = false;
    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Standard,
    ] {
        changed = profile_health
            .remove(&runtime_previous_response_negative_cache_key(
                previous_response_id,
                profile_name,
                route_kind,
            ))
            .is_some()
            || changed;
    }
    changed
}

pub fn runtime_profile_route_key_parts<'a>(
    key: &'a str,
    prefix: &str,
) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

pub fn runtime_profile_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_transport_backoff__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub fn runtime_profile_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_profile_route_key_parts(key, "__route_transport_backoff__:")
}

pub fn runtime_profile_transport_backoff_profile_name(key: &str) -> &str {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(_, profile_name)| profile_name)
        .unwrap_or(key)
}

pub fn runtime_profile_transport_backoff_key_valid(
    key: &str,
    valid_profiles: &BTreeSet<String>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && valid_profiles.contains(profile_name)
        })
        .unwrap_or_else(|| valid_profiles.contains(key))
}

pub fn runtime_profile_transport_backoff_until_from_map(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    [
        transport_backoff_until.get(&route_key).copied(),
        transport_backoff_until.get(profile_name).copied(),
    ]
    .into_iter()
    .flatten()
    .filter(|until| *until > now)
    .max()
}

pub fn runtime_profile_transport_backoff_max_until(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    now: i64,
) -> Option<i64> {
    transport_backoff_until
        .iter()
        .filter(|(key, until)| {
            runtime_profile_transport_backoff_profile_name(key) == profile_name && **until > now
        })
        .map(|(_, until)| *until)
        .max()
}

pub fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
        || runtime_profile_transport_backoff_until_from_map(
            transport_backoff_until,
            profile_name,
            route_kind,
            now,
        )
        .is_some()
        || route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
            .copied()
            .is_some_and(|until| until > now)
}

pub fn runtime_profile_backoff_sort_key(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (usize, i64, i64, i64) {
    let retry_until = retry_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let transport_until = runtime_profile_transport_backoff_until_from_map(
        transport_backoff_until,
        profile_name,
        route_kind,
        now,
    );
    let circuit_until = route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now);

    match (circuit_until, transport_until, retry_until) {
        (None, None, None) => (0, 0, 0, 0),
        (Some(circuit_until), None, None) => (1, circuit_until, 0, 0),
        (None, Some(transport_until), None) => (2, transport_until, 0, 0),
        (None, None, Some(retry_until)) => (3, retry_until, 0, 0),
        (Some(circuit_until), Some(transport_until), None) => (
            4,
            circuit_until.min(transport_until),
            circuit_until.max(transport_until),
            0,
        ),
        (Some(circuit_until), None, Some(retry_until)) => (
            5,
            circuit_until.min(retry_until),
            circuit_until.max(retry_until),
            0,
        ),
        (None, Some(transport_until), Some(retry_until)) => (
            6,
            transport_until.min(retry_until),
            transport_until.max(retry_until),
            0,
        ),
        (Some(circuit_until), Some(transport_until), Some(retry_until)) => (
            7,
            circuit_until.min(transport_until.min(retry_until)),
            circuit_until.max(transport_until.max(retry_until)),
            retry_until,
        ),
    }
}

pub fn runtime_soften_persisted_backoff_map_for_startup(
    backoffs: &mut BTreeMap<String, i64>,
    now: i64,
    max_future_seconds: i64,
) -> bool {
    let max_until = now.saturating_add(max_future_seconds.max(0));
    let mut changed = false;
    backoffs.retain(|_, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub fn runtime_profile_circuit_open_seconds(score: u32, reopen_stage: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3)
                .saturating_add(reopen_stage.min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS)
}

pub fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS)
}

pub fn runtime_profile_route_circuit_probe_seconds(
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    route_profile_key: &str,
    now: i64,
) -> i64 {
    let Some((route_label, profile_name)) =
        runtime_profile_route_key_parts(route_profile_key, "__route_circuit__:")
    else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let Some(route_kind) = runtime_route_kind_from_label(route_label) else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let score = runtime_profile_effective_health_score_from_map(
        profile_scores,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    );
    runtime_profile_circuit_half_open_probe_seconds(score)
}

pub fn runtime_soften_persisted_route_circuits_for_startup(
    route_circuit_open_until: &mut BTreeMap<String, i64>,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = false;
    route_circuit_open_until.retain(|route_profile_key, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let max_until = now.saturating_add(runtime_profile_route_circuit_probe_seconds(
            profile_scores,
            route_profile_key,
            now,
        ));
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub fn runtime_soften_persisted_backoffs_for_startup(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = runtime_soften_persisted_backoff_map_for_startup(
        &mut backoffs.transport_backoff_until,
        now,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

fn runtime_profile_transport_backoff_key_matches_profiles(
    key: &str,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && profiles.contains_key(profile_name)
        })
        .unwrap_or_else(|| profiles.contains_key(key))
}

pub const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
pub const RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX: &str = "__compact_turn_state__:";
pub const RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX: &str = "__response_turn_state__:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationCompactionPolicy {
    pub response_binding_limit: usize,
    pub turn_state_binding_limit: usize,
    pub session_id_binding_limit: usize,
    pub response_status_limit: usize,
    pub turn_state_status_limit: usize,
    pub session_id_status_limit: usize,
    pub suspect_grace_seconds: i64,
    pub dead_grace_seconds: i64,
    pub verified_stale_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub confidence_max: u32,
}

impl Default for RuntimeContinuationCompactionPolicy {
    fn default() -> Self {
        Self {
            response_binding_limit: 16_384,
            turn_state_binding_limit: 2_048,
            session_id_binding_limit: 2_048,
            response_status_limit: 16_384,
            turn_state_status_limit: 2_048,
            session_id_status_limit: 2_048,
            suspect_grace_seconds: 120,
            dead_grace_seconds: 900,
            verified_stale_seconds: 1_800,
            suspect_not_found_streak_limit: 2,
            confidence_max: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationStatusPolicy {
    pub touch_persist_interval_seconds: i64,
    pub suspect_grace_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub confidence_max: u32,
    pub verified_confidence_bonus: u32,
    pub touch_confidence_bonus: u32,
    pub suspect_confidence_penalty: u32,
}

pub fn runtime_continuation_store_from_app_state(
    state: &AppState,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    RuntimeContinuationStore {
        response_profile_bindings: runtime_external_response_profile_bindings(
            &state.response_profile_bindings,
        ),
        session_profile_bindings: state.session_profile_bindings.clone(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: runtime_external_session_id_bindings(&state.session_profile_bindings),
        statuses: RuntimeContinuationStatuses::default(),
    }
}

pub fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    format!("{RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX}{session_id}")
}

pub fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    format!("{RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX}{turn_state}")
}

pub fn runtime_response_turn_state_lineage_key(response_id: &str, turn_state: &str) -> String {
    format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{response_id}:{turn_state}",
        response_id.len()
    )
}

pub fn runtime_is_response_turn_state_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)
}

pub fn runtime_response_turn_state_lineage_parts(key: &str) -> Option<(&str, &str)> {
    let suffix = key.strip_prefix(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)?;
    let (response_len, rest) = suffix.split_once(':')?;
    let response_len = response_len.parse::<usize>().ok()?;
    let response_and_sep = rest.get(..response_len.saturating_add(1))?;
    if response_and_sep.as_bytes().get(response_len).copied() != Some(b':') {
        return None;
    }
    let response_id = response_and_sep.get(..response_len)?;
    let turn_state = rest.get(response_len.saturating_add(1)..)?;
    (!response_id.is_empty() && !turn_state.is_empty()).then_some((response_id, turn_state))
}

pub fn runtime_is_compact_session_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX)
}

pub fn runtime_external_response_profile_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_response_turn_state_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub fn runtime_external_session_id_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_compact_session_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub fn runtime_continuation_status_map(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &statuses.response,
        RuntimeContinuationBindingKind::TurnState => &statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &statuses.session_id,
    }
}

pub fn runtime_continuation_status_map_mut(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &mut BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &mut statuses.response,
        RuntimeContinuationBindingKind::TurnState => &mut statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &mut statuses.session_id,
    }
}

pub fn runtime_continuation_binding_lifecycle_rank(
    state: RuntimeContinuationBindingLifecycle,
) -> u8 {
    match state {
        RuntimeContinuationBindingLifecycle::Dead => 0,
        RuntimeContinuationBindingLifecycle::Suspect => 1,
        RuntimeContinuationBindingLifecycle::Warm => 2,
        RuntimeContinuationBindingLifecycle::Verified => 3,
    }
}

pub fn runtime_continuation_status_evidence_sort_key(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u32, u32, u32, u8, i64, i64, i64) {
    (
        runtime_continuation_binding_lifecycle_rank(status.state),
        status.confidence.min(policy.confidence_max),
        status.success_count,
        u32::MAX.saturating_sub(status.not_found_streak),
        if status.last_verified_route.is_some() {
            1
        } else {
            0
        },
        status.last_verified_at.unwrap_or(i64::MIN),
        status.last_touched_at.unwrap_or(i64::MIN),
        status.last_not_found_at.unwrap_or(i64::MIN),
    )
}

pub fn runtime_continuation_status_is_more_evidenced(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    runtime_continuation_status_evidence_sort_key(candidate, policy)
        > runtime_continuation_status_evidence_sort_key(current, policy)
}

pub fn runtime_continuation_status_should_replace(
    candidate: &RuntimeContinuationBindingStatus,
    current: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match (
        runtime_continuation_status_last_event_at(candidate),
        runtime_continuation_status_last_event_at(current),
    ) {
        (Some(candidate_at), Some(current_at)) if candidate_at != current_at => {
            return candidate_at > current_at;
        }
        (Some(_), None) => return true,
        (None, Some(_)) => return false,
        _ => {}
    }

    match (
        runtime_continuation_status_is_terminal(candidate, policy),
        runtime_continuation_status_is_terminal(current, policy),
    ) {
        (true, false) => return true,
        (false, true) => return false,
        _ => {}
    }

    runtime_continuation_status_is_more_evidenced(candidate, current, policy)
}

pub fn runtime_continuation_status_last_event_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    [
        status.last_not_found_at,
        status.last_verified_at,
        status.last_touched_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

pub fn runtime_binding_touch_should_persist(
    bound_at: i64,
    now: i64,
    touch_persist_interval_seconds: i64,
) -> bool {
    // Timestamps use second precision. Require strictly more than the interval
    // so a boundary-crossing lookup does not persist nearly a second early.
    now.saturating_sub(bound_at) > touch_persist_interval_seconds
}

pub fn runtime_continuation_status_is_terminal_for_status_policy(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Dead
        || status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (status.state == RuntimeContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

pub fn runtime_continuation_next_event_at(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> i64 {
    runtime_continuation_status_last_event_at(status)
        .filter(|last| *last >= now)
        .map_or(now, |last| last.saturating_add(1))
}

pub fn runtime_continuation_status_touches(
    status: &mut RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.last_touched_at = Some(event_at);
    if status.state == RuntimeContinuationBindingLifecycle::Suspect {
        if status
            .last_not_found_at
            .is_some_and(|last| event_at.saturating_sub(last) >= policy.suspect_grace_seconds)
        {
            status.state = RuntimeContinuationBindingLifecycle::Warm;
            status.not_found_streak = 0;
            status.last_not_found_at = None;
        }
        status.confidence = status
            .confidence
            .saturating_add(policy.touch_confidence_bonus)
            .min(policy.confidence_max);
    } else if status.state != RuntimeContinuationBindingLifecycle::Dead {
        status.confidence = status
            .confidence
            .saturating_add(policy.touch_confidence_bonus)
            .min(policy.confidence_max);
    }
    *status != previous
}

pub fn runtime_continuation_status_should_refresh_verified(
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    verified_route_label: Option<&str>,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let Some(status) = status else {
        return true;
    };

    if status.state != RuntimeContinuationBindingLifecycle::Verified {
        return true;
    }

    if status.last_verified_route.as_deref() != verified_route_label {
        return true;
    }

    status.last_verified_at.is_none_or(|last_verified_at| {
        runtime_binding_touch_should_persist(
            last_verified_at,
            now,
            policy.touch_persist_interval_seconds,
        )
    })
}

pub fn runtime_continuation_status_should_persist_touch(
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let Some(status) = status else {
        return true;
    };

    if status.state == RuntimeContinuationBindingLifecycle::Suspect
        && status.last_not_found_at.is_some_and(|last_not_found_at| {
            now.saturating_sub(last_not_found_at) >= policy.suspect_grace_seconds
        })
    {
        return true;
    }

    status.last_touched_at.is_none_or(|last_touched_at| {
        runtime_binding_touch_should_persist(
            last_touched_at,
            now,
            policy.touch_persist_interval_seconds,
        )
    })
}

pub fn runtime_mark_continuation_status_touched(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    runtime_continuation_status_touches(status, now, policy)
}

pub fn runtime_mark_continuation_status_verified(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route_label: Option<&str>,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.state = RuntimeContinuationBindingLifecycle::Verified;
    status.last_touched_at = Some(event_at);
    status.last_verified_at = Some(event_at);
    status.last_verified_route = verified_route_label.map(str::to_string);
    status.last_not_found_at = None;
    status.not_found_streak = 0;
    status.success_count = status.success_count.saturating_add(1);
    status.failure_count = 0;
    status.confidence = status
        .confidence
        .saturating_add(policy.verified_confidence_bonus)
        .min(policy.confidence_max);
    *status != previous
}

pub fn runtime_mark_continuation_status_suspect(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.not_found_streak = status.not_found_streak.saturating_add(1);
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.failure_count = status.failure_count.saturating_add(1);
    let previous_confidence = status.confidence;
    status.confidence = status
        .confidence
        .saturating_sub(policy.suspect_confidence_penalty);
    if previous_confidence == 0 {
        status.confidence = 1;
    }
    status.state = if status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (previous_confidence > 0 && status.confidence == 0)
    {
        RuntimeContinuationBindingLifecycle::Dead
    } else {
        RuntimeContinuationBindingLifecycle::Suspect
    };
    *status != previous
}

pub fn runtime_mark_continuation_status_dead(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.state = RuntimeContinuationBindingLifecycle::Dead;
    status.confidence = 0;
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.not_found_streak = status
        .not_found_streak
        .max(policy.suspect_not_found_streak_limit);
    status.failure_count = status.failure_count.saturating_add(1);
    *status != previous
}

pub fn runtime_continuation_status_recently_suspect(
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    policy: RuntimeContinuationStatusPolicy,
) -> bool {
    status.is_some_and(|status| {
        status.state == RuntimeContinuationBindingLifecycle::Suspect
            && !runtime_continuation_status_is_terminal_for_status_policy(status, policy)
            && status
                .last_not_found_at
                .is_some_and(|last| now.saturating_sub(last) < policy.suspect_grace_seconds)
    })
}

pub fn runtime_continuation_status_label(
    status: &RuntimeContinuationBindingStatus,
) -> &'static str {
    match status.state {
        RuntimeContinuationBindingLifecycle::Warm => "warm",
        RuntimeContinuationBindingLifecycle::Verified => "verified",
        RuntimeContinuationBindingLifecycle::Suspect => "suspect",
        RuntimeContinuationBindingLifecycle::Dead => "dead",
    }
}

pub fn runtime_continuation_status_is_terminal(
    status: &RuntimeContinuationBindingStatus,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Dead
        || status.not_found_streak >= policy.suspect_not_found_streak_limit
        || (status.state == RuntimeContinuationBindingLifecycle::Suspect
            && status.confidence == 0
            && status.failure_count > 0)
}

pub fn runtime_continuation_status_is_stale_verified(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    status.state == RuntimeContinuationBindingLifecycle::Verified
        && runtime_continuation_status_last_event_at(status)
            .is_some_and(|last| now.saturating_sub(last) >= policy.verified_stale_seconds)
}

pub fn runtime_age_stale_verified_continuation_status(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    let Some(status) = runtime_continuation_status_map_mut(statuses, kind).get_mut(key) else {
        return false;
    };
    if !runtime_continuation_status_is_stale_verified(status, now, policy) {
        return false;
    }
    status.state = RuntimeContinuationBindingLifecycle::Warm;
    true
}

pub fn runtime_continuation_status_should_retain_with_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => false,
        RuntimeContinuationBindingLifecycle::Verified
        | RuntimeContinuationBindingLifecycle::Warm => {
            status.confidence > 0
                || status.success_count > 0
                || status.last_verified_at.is_some()
                || status.last_touched_at.is_some()
        }
        RuntimeContinuationBindingLifecycle::Suspect => {
            status.not_found_streak < policy.suspect_not_found_streak_limit
                && status.confidence > 0
                && status
                    .last_not_found_at
                    .is_some_and(|last| now.saturating_sub(last) < policy.suspect_grace_seconds)
        }
    }
}

pub fn runtime_continuation_status_should_retain_without_binding(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match status.state {
        RuntimeContinuationBindingLifecycle::Dead => status
            .last_not_found_at
            .or(status.last_touched_at)
            .is_some_and(|last| now.saturating_sub(last) < policy.dead_grace_seconds),
        _ => runtime_continuation_status_should_retain_with_binding(status, now, policy),
    }
}

pub fn runtime_continuation_status_dead_at(
    status: &RuntimeContinuationBindingStatus,
) -> Option<i64> {
    (status.state == RuntimeContinuationBindingLifecycle::Dead)
        .then(|| status.last_not_found_at.or(status.last_touched_at))
        .flatten()
}

pub fn runtime_continuation_dead_status_shadowed_by_binding(
    binding: &ResponseProfileBinding,
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    runtime_continuation_status_dead_at(status).is_some_and(|dead_at| binding.bound_at > dead_at)
}

pub fn merge_runtime_continuation_status_map(
    existing: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    incoming: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    live_bindings: &BTreeMap<String, ResponseProfileBinding>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> BTreeMap<String, RuntimeContinuationBindingStatus> {
    let mut merged = existing.clone();
    for (key, status) in incoming {
        let should_replace = merged.get(key).is_none_or(|current| {
            runtime_continuation_status_should_replace(status, current, policy)
        });
        if should_replace {
            merged.insert(key.clone(), status.clone());
        }
    }
    merged.retain(|key, status| {
        live_bindings.contains_key(key)
            || runtime_continuation_status_should_retain_without_binding(status, now, policy)
    });
    merged
}

pub fn merge_runtime_continuation_statuses(
    existing: &RuntimeContinuationStatuses,
    incoming: &RuntimeContinuationStatuses,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStatuses {
    RuntimeContinuationStatuses {
        response: merge_runtime_continuation_status_map(
            &existing.response,
            &incoming.response,
            response_bindings,
            now,
            policy,
        ),
        turn_state: merge_runtime_continuation_status_map(
            &existing.turn_state,
            &incoming.turn_state,
            turn_state_bindings,
            now,
            policy,
        ),
        session_id: merge_runtime_continuation_status_map(
            &existing.session_id,
            &incoming.session_id,
            session_id_bindings,
            now,
            policy,
        ),
    }
}

pub fn compact_runtime_continuation_statuses(
    statuses: RuntimeContinuationStatuses,
    continuations: &RuntimeContinuationStore<ResponseProfileBinding>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStatuses {
    let mut merged = merge_runtime_continuation_statuses(
        &RuntimeContinuationStatuses::default(),
        &statuses,
        &continuations.response_profile_bindings,
        &continuations.turn_state_bindings,
        &continuations.session_id_bindings,
        now,
        policy,
    );
    merged.response.retain(|key, status| {
        if let Some(binding) = continuations.response_profile_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now, policy)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now, policy)
        }
    });
    merged.turn_state.retain(|key, status| {
        if let Some(binding) = continuations.turn_state_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now, policy)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now, policy)
        }
    });
    merged.session_id.retain(|key, status| {
        if let Some(binding) = continuations.session_id_bindings.get(key) {
            !runtime_continuation_dead_status_shadowed_by_binding(binding, status)
                && runtime_continuation_status_should_retain_with_binding(status, now, policy)
        } else {
            runtime_continuation_status_should_retain_without_binding(status, now, policy)
        }
    });
    prune_runtime_continuation_status_map(
        &mut merged.response,
        &continuations.response_profile_bindings,
        policy.response_status_limit,
        policy,
    );
    prune_runtime_continuation_status_map(
        &mut merged.turn_state,
        &continuations.turn_state_bindings,
        policy.turn_state_status_limit,
        policy,
    );
    prune_runtime_continuation_status_map(
        &mut merged.session_id,
        &continuations.session_id_bindings,
        policy.session_id_status_limit,
        policy,
    );
    merged
}

pub fn runtime_continuation_status_retention_sort_key(
    key: &str,
    status: &RuntimeContinuationBindingStatus,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let evidence = runtime_continuation_status_evidence_sort_key(status, policy);
    (
        if bindings.contains_key(key) { 1 } else { 0 },
        evidence.0,
        evidence.1,
        evidence.2,
        evidence.3,
        evidence.4,
        evidence.5,
        evidence.6,
        evidence.7,
        bindings
            .get(key)
            .map(|binding| binding.bound_at)
            .unwrap_or(i64::MIN),
    )
}

pub fn prune_runtime_continuation_status_map(
    statuses: &mut BTreeMap<String, RuntimeContinuationBindingStatus>,
    bindings: &BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
    policy: RuntimeContinuationCompactionPolicy,
) {
    if statuses.len() <= max_entries {
        return;
    }

    let excess = statuses.len() - max_entries;
    let mut coldest = statuses
        .iter()
        .map(|(key, status)| {
            (
                key.clone(),
                runtime_continuation_status_retention_sort_key(key, status, bindings, policy),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (key, _) in coldest.into_iter().take(excess) {
        statuses.remove(&key);
    }
}

pub fn runtime_continuation_binding_should_retain(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> bool {
    match status {
        Some(status) if runtime_continuation_dead_status_shadowed_by_binding(binding, status) => {
            true
        }
        Some(status) if runtime_continuation_status_is_terminal(status, policy) => false,
        Some(status) => runtime_continuation_status_should_retain_with_binding(status, now, policy),
        None => binding.bound_at <= now,
    }
}

pub fn runtime_continuation_binding_retention_sort_key(
    binding: &ResponseProfileBinding,
    status: Option<&RuntimeContinuationBindingStatus>,
    policy: RuntimeContinuationCompactionPolicy,
) -> (u8, u32, u32, u32, u8, i64, i64, i64, i64) {
    let evidence = status
        .map(|status| runtime_continuation_status_evidence_sort_key(status, policy))
        .unwrap_or((0, 0, 0, 0, 0, i64::MIN, i64::MIN, i64::MIN));
    (
        evidence.0,
        evidence.1,
        evidence.2,
        evidence.3,
        evidence.4,
        evidence.5,
        evidence.6,
        evidence.7,
        binding.bound_at,
    )
}

pub fn prune_runtime_continuation_response_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    statuses: &BTreeMap<String, RuntimeContinuationBindingStatus>,
    max_entries: usize,
    policy: RuntimeContinuationCompactionPolicy,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut coldest = bindings
        .iter()
        .map(|(response_id, binding)| {
            (
                response_id.clone(),
                runtime_continuation_binding_retention_sort_key(
                    binding,
                    statuses.get(response_id),
                    policy,
                ),
            )
        })
        .collect::<Vec<_>>();
    coldest.sort_by_key(|(_, retention)| *retention);

    for (response_id, _) in coldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

pub fn compact_runtime_continuation_store(
    mut continuations: RuntimeContinuationStore<ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.response_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_profile_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.turn_state_bindings,
        profiles,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut continuations.session_id_bindings,
        profiles,
    );
    continuations
        .response_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.response.get(key),
                now,
                policy,
            )
        });
    let response_turn_state_keys = continuations
        .response_profile_bindings
        .keys()
        .filter(|key| runtime_is_response_turn_state_lineage_key(key))
        .cloned()
        .collect::<Vec<_>>();
    for key in response_turn_state_keys {
        let Some((response_id, _)) = runtime_response_turn_state_lineage_parts(&key) else {
            continuations.response_profile_bindings.remove(&key);
            continue;
        };
        if continuations
            .response_profile_bindings
            .get(response_id)
            .is_none_or(|binding| !profiles.contains_key(&binding.profile_name))
        {
            continuations.response_profile_bindings.remove(&key);
        }
    }
    continuations.turn_state_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(
            binding,
            continuations.statuses.turn_state.get(key),
            now,
            policy,
        )
    });
    continuations
        .session_profile_bindings
        .retain(|key, binding| {
            runtime_continuation_binding_should_retain(
                binding,
                continuations.statuses.session_id.get(key),
                now,
                policy,
            )
        });
    continuations.session_id_bindings.retain(|key, binding| {
        runtime_continuation_binding_should_retain(
            binding,
            continuations.statuses.session_id.get(key),
            now,
            policy,
        )
    });
    prune_runtime_continuation_response_bindings(
        &mut continuations.response_profile_bindings,
        &continuations.statuses.response,
        policy.response_binding_limit,
        policy,
    );
    prune_profile_bindings(
        &mut continuations.turn_state_bindings,
        policy.turn_state_binding_limit,
    );
    prune_profile_bindings(
        &mut continuations.session_profile_bindings,
        policy.session_id_binding_limit,
    );
    prune_profile_bindings(
        &mut continuations.session_id_bindings,
        policy.session_id_binding_limit,
    );
    let statuses = std::mem::take(&mut continuations.statuses);
    continuations.statuses =
        compact_runtime_continuation_statuses(statuses, &continuations, now, policy);
    continuations
}

pub fn merge_runtime_continuation_store(
    existing: &RuntimeContinuationStore<ResponseProfileBinding>,
    incoming: &RuntimeContinuationStore<ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    policy: RuntimeContinuationCompactionPolicy,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    let response_profile_bindings = merge_profile_bindings(
        &existing.response_profile_bindings,
        &incoming.response_profile_bindings,
        profiles,
    );
    let turn_state_bindings = merge_profile_bindings(
        &existing.turn_state_bindings,
        &incoming.turn_state_bindings,
        profiles,
    );
    let session_id_bindings = merge_profile_bindings(
        &existing.session_id_bindings,
        &incoming.session_id_bindings,
        profiles,
    );
    let statuses = merge_runtime_continuation_statuses(
        &existing.statuses,
        &incoming.statuses,
        &response_profile_bindings,
        &turn_state_bindings,
        &session_id_bindings,
        now,
        policy,
    );
    compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings,
            session_profile_bindings: merge_profile_bindings(
                &existing.session_profile_bindings,
                &incoming.session_profile_bindings,
                profiles,
            ),
            turn_state_bindings: turn_state_bindings.clone(),
            session_id_bindings: session_id_bindings.clone(),
            statuses,
        },
        profiles,
        now,
        policy,
    )
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;

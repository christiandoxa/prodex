use super::{
    OUTPUT_CURSOR_VERSION, OUTPUT_READ_MAX_BYTES, OUTPUT_READ_MAX_LINE_BYTES,
    OUTPUT_READ_MAX_TEXT_BYTES, OUTPUT_READ_MAX_TOTAL_TEXT_BYTES, OUTPUT_SOURCE_PROBE_BYTES,
    OpenProcessFile, PromptInjectionError, PromptOutputEvent, ResolvedTarget, legacy_thread_id,
};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use uuid::Uuid;
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct OutputCursor {
    pub(crate) version: u8,
    pub(crate) prodex_pid: u32,
    pub(crate) prodex_birth: String,
    pub(crate) codex_pid: u32,
    pub(crate) codex_birth: String,
    pub(crate) thread_id: String,
    pub(crate) source_id: String,
    pub(crate) offset: u64,
    pub(crate) event_index: usize,
    pub(crate) checkpoint_id: String,
}

impl OutputCursor {
    pub(crate) fn matches(&self, target: &ResolvedTarget, source_id: &str) -> bool {
        self.version == OUTPUT_CURSOR_VERSION
            && self.prodex_pid == target.prodex.pid
            && target.prodex.birth_identity.as_deref() == Some(self.prodex_birth.as_str())
            && self.codex_pid == target.writer.pid
            && target.writer.birth_identity.as_deref() == Some(self.codex_birth.as_str())
            && self.thread_id == target.thread_id
            && self.source_id == source_id
    }
}

pub(crate) fn encode_output_cursor(
    mut cursor: OutputCursor,
) -> std::result::Result<String, PromptInjectionError> {
    cursor.version = OUTPUT_CURSOR_VERSION;
    let bytes = serde_json::to_vec(&cursor).map_err(|_| PromptInjectionError::OutputReadFailed)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn decode_output_cursor(
    value: &str,
) -> std::result::Result<OutputCursor, PromptInjectionError> {
    if value.is_empty() || value.len() > 16 * 1024 {
        return Err(PromptInjectionError::InvalidCursor);
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| PromptInjectionError::InvalidCursor)?;
    let cursor = serde_json::from_slice::<OutputCursor>(&bytes)
        .map_err(|_| PromptInjectionError::InvalidCursor)?;
    (cursor.version == OUTPUT_CURSOR_VERSION
        && cursor.prodex_pid > 0
        && cursor.codex_pid > 0
        && !cursor.prodex_birth.is_empty()
        && !cursor.codex_birth.is_empty()
        && Uuid::parse_str(&cursor.thread_id).is_ok()
        && !cursor.source_id.is_empty()
        && !cursor.checkpoint_id.is_empty()
        && cursor.event_index <= 65_536)
        .then_some(cursor)
        .ok_or(PromptInjectionError::InvalidCursor)
}

pub(crate) fn output_source_id(
    path: &Path,
    thread_id: &str,
) -> std::result::Result<String, PromptInjectionError> {
    let metadata = fs::metadata(path).map_err(|_| PromptInjectionError::OutputSourceUnavailable)?;
    if !metadata.is_file() {
        return Err(PromptInjectionError::OutputSourceUnavailable);
    }
    let mut file = prodex_core::open_regular_file_no_follow(path)
        .map_err(|_| PromptInjectionError::OutputSourceUnavailable)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)
        .map_err(|_| PromptInjectionError::OutputSourceUnavailable)?
    {
        return Err(PromptInjectionError::OutputSourceChanged);
    }
    let mut hasher = Sha256::new();
    hasher.update(thread_id.as_bytes());
    hasher.update(path.as_os_str().to_string_lossy().as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        hasher.update(metadata.dev().to_le_bytes());
        hasher.update(metadata.ino().to_le_bytes());
    }
    #[cfg(not(unix))]
    {
        hasher.update(metadata.len().to_le_bytes());
        hasher.update(
            metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |value| value.as_nanos() as u64)
                .to_le_bytes(),
        );
    }
    let mut prefix = vec![0; OUTPUT_SOURCE_PROBE_BYTES];
    let count = file
        .read(&mut prefix)
        .map_err(|_| PromptInjectionError::OutputSourceUnavailable)?;
    hasher.update(&prefix[..count]);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn source_checkpoint_id(
    path: &Path,
    offset: u64,
) -> std::result::Result<String, PromptInjectionError> {
    let metadata = fs::metadata(path).map_err(|_| PromptInjectionError::OutputSourceChanged)?;
    let mut file = prodex_core::open_regular_file_no_follow(path)
        .map_err(|_| PromptInjectionError::OutputSourceChanged)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)
        .map_err(|_| PromptInjectionError::OutputSourceChanged)?
    {
        return Err(PromptInjectionError::OutputSourceChanged);
    }
    let prefix_len = offset.min(OUTPUT_SOURCE_PROBE_BYTES as u64);
    let mut bytes = vec![0; prefix_len as usize];
    file.read_exact(&mut bytes)
        .map_err(|_| PromptInjectionError::OutputSourceChanged)?;
    let mut hasher = Sha256::new();
    hasher.update(offset.to_le_bytes());
    hasher.update(&bytes);
    if offset > 0 {
        let window_start = offset.saturating_sub(4096);
        file.seek(SeekFrom::Start(window_start))
            .map_err(|_| PromptInjectionError::OutputSourceChanged)?;
        let mut window = vec![0; (offset - window_start) as usize];
        file.read_exact(&mut window)
            .map_err(|_| PromptInjectionError::OutputSourceChanged)?;
        hasher.update(&window);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) struct OutputReadBatch {
    pub(crate) events: Vec<PromptOutputEvent>,
    pub(crate) next_offset: u64,
    pub(crate) next_event_index: usize,
    pub(crate) has_more: bool,
}

pub(crate) fn read_output_events(
    path: &Path,
    offset: u64,
    event_index: usize,
    limit: usize,
) -> std::result::Result<OutputReadBatch, PromptInjectionError> {
    let metadata = fs::metadata(path).map_err(|_| PromptInjectionError::OutputSourceUnavailable)?;
    if !metadata.is_file() || offset > metadata.len() {
        return Err(PromptInjectionError::OutputSourceChanged);
    }
    let mut file = prodex_core::open_regular_file_no_follow(path)
        .map_err(|_| PromptInjectionError::OutputReadFailed)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)
        .map_err(|_| PromptInjectionError::OutputReadFailed)?
    {
        return Err(PromptInjectionError::OutputSourceChanged);
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| PromptInjectionError::OutputReadFailed)?;
    let mut bytes = Vec::new();
    file.take((OUTPUT_READ_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| PromptInjectionError::OutputReadFailed)?;
    let bounded = bytes.len() <= OUTPUT_READ_MAX_BYTES;
    let complete_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    if !bounded && complete_len == 0 {
        return Err(PromptInjectionError::OutputReadFailed);
    }
    let complete = &bytes[..complete_len];
    let mut events = Vec::new();
    let mut total_text_bytes = 0_usize;
    let mut consumed = 0_usize;
    for line in complete.split_inclusive(|byte| *byte == b'\n') {
        let line_start = offset.saturating_add(consumed as u64);
        let raw_len = line.len();
        if events.len() >= limit {
            return Ok(OutputReadBatch {
                events,
                next_offset: line_start,
                next_event_index: 0,
                has_more: true,
            });
        }
        if line.len() > OUTPUT_READ_MAX_LINE_BYTES {
            consumed = consumed.saturating_add(raw_len);
            continue;
        }
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let Ok(line) = std::str::from_utf8(line) else {
            consumed = consumed.saturating_add(raw_len);
            continue;
        };
        let parsed = crate::app_commands::transcript_events_from_session_line(line)
            .into_iter()
            .filter(mcp_visible_transcript_event)
            .collect::<Vec<_>>();
        let start_index = if consumed == 0 { event_index } else { 0 };
        if start_index > parsed.len() {
            return Err(PromptInjectionError::OutputSourceChanged);
        }
        for (index, event) in parsed.iter().enumerate().skip(start_index) {
            if events.len() >= limit {
                return Ok(OutputReadBatch {
                    events,
                    next_offset: line_start,
                    next_event_index: index,
                    has_more: true,
                });
            }
            let output_event = output_event_from_transcript(
                line_start
                    .saturating_mul(65_536)
                    .saturating_add(index as u64),
                event.clone(),
            );
            if total_text_bytes.saturating_add(output_event.text.len())
                > OUTPUT_READ_MAX_TOTAL_TEXT_BYTES
            {
                return Ok(OutputReadBatch {
                    events,
                    next_offset: line_start,
                    next_event_index: index,
                    has_more: true,
                });
            }
            total_text_bytes = total_text_bytes.saturating_add(output_event.text.len());
            events.push(output_event);
        }
        consumed = consumed.saturating_add(raw_len);
    }
    let next_offset = offset.saturating_add(consumed as u64);
    Ok(OutputReadBatch {
        events,
        next_offset,
        next_event_index: 0,
        has_more: next_offset < metadata.len(),
    })
}

fn mcp_visible_transcript_event(event: &crate::app_commands::TranscriptEvent) -> bool {
    event.source == "assistant"
        || event.source == "user"
        || event.source == "tool-output"
        || event.source == "mcp"
        || event.source == "agent"
        || event.source == "tool"
        || event.source == "session-context"
        || event.source == "turn-context"
        || event.source.starts_with("tool-call:")
}

fn output_event_from_transcript(
    sequence: u64,
    event: crate::app_commands::TranscriptEvent,
) -> PromptOutputEvent {
    let (kind, name, status) = if event.source == "assistant" {
        ("assistant", None, None)
    } else if event.source == "user" {
        ("user", None, None)
    } else if let Some(name) = event.source.strip_prefix("tool-call:") {
        ("tool", Some(name.to_string()), Some("started".to_string()))
    } else if event.source == "tool-output" {
        ("tool", None, Some("completed".to_string()))
    } else {
        (event.source.as_str(), None, None)
    };
    PromptOutputEvent {
        sequence,
        timestamp: event.timestamp.chars().take(128).collect(),
        kind: kind.to_string(),
        name: name.map(|name| name.chars().take(256).collect()),
        status,
        text: redaction::redaction_redact_secret_like_text(
            &event
                .text
                .chars()
                .take(OUTPUT_READ_MAX_TEXT_BYTES)
                .collect::<String>(),
        ),
    }
}

pub(crate) fn valid_rollout_path_in_roots(
    path: &Path,
    roots: &[PathBuf],
    thread_id: &str,
) -> Option<PathBuf> {
    let roots = roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .collect::<Vec<_>>();
    let paths = if path.is_absolute() {
        vec![path.to_path_buf()]
    } else {
        roots.iter().map(|root| root.join(path)).collect()
    };
    paths.into_iter().find_map(|path| {
        if path.symlink_metadata().ok()?.file_type().is_symlink() {
            return None;
        }
        let canonical = path.canonicalize().ok()?;
        if !roots.iter().any(|root| canonical.starts_with(root)) {
            return None;
        }
        let name = canonical.file_name()?.to_str()?;
        (is_uncompressed_rollout_file_name(name)
            && legacy_thread_id(&canonical).as_deref() == Some(thread_id)
            && fs::metadata(&canonical).ok()?.is_file())
        .then_some(canonical)
    })
}

pub(crate) fn valid_rollout_path_in_authoritative_open_files(
    stored_path: &Path,
    roots: &[PathBuf],
    open_files: &[OpenProcessFile],
    thread_id: &str,
) -> Option<PathBuf> {
    let paths = if stored_path.is_absolute() {
        vec![stored_path.to_path_buf()]
    } else {
        roots.iter().map(|root| root.join(stored_path)).collect()
    };
    paths.into_iter().find_map(|path| {
        if path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
            || !roots.iter().any(|root| path.starts_with(root))
            || path.symlink_metadata().ok()?.file_type().is_symlink()
        {
            return None;
        }
        let canonical = path.canonicalize().ok()?;
        open_files.iter().find_map(|file| {
            let open_path = file.path.canonicalize().ok()?;
            let name = open_path.file_name()?.to_str()?;
            (open_path == canonical
                && is_uncompressed_rollout_file_name(name)
                && legacy_thread_id(&open_path).as_deref() == Some(thread_id)
                && fs::metadata(&open_path).ok()?.is_file())
            .then_some(open_path)
        })
    })
}

pub(crate) fn collect_exact_rollouts(
    root: &Path,
    thread_id: &str,
    output: &mut Vec<PathBuf>,
    depth: usize,
) -> std::result::Result<(), PromptInjectionError> {
    if depth > 8 {
        return Err(PromptInjectionError::OutputReadFailed);
    }
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_exact_rollouts(&path, thread_id, output, depth + 1)?;
        } else if file_type.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_uncompressed_rollout_file_name)
            && legacy_thread_id(&path).as_deref() == Some(thread_id)
            && !file_type.is_symlink()
            && let Ok(path) = path.canonicalize()
            && path.starts_with(root)
        {
            output.push(path);
        }
        if output.len() > 2 {
            return Err(PromptInjectionError::OutputSourceAmbiguous);
        }
    }
    Ok(())
}

fn is_uncompressed_rollout_file_name(name: &str) -> bool {
    name.starts_with("rollout-") && name.ends_with(".jsonl")
}

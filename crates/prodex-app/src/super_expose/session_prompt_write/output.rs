use super::{
    OUTPUT_CURSOR_VERSION, OUTPUT_READ_MAX_BYTES, OUTPUT_READ_MAX_LINE_BYTES,
    OUTPUT_READ_MAX_TOTAL_TEXT_BYTES, OUTPUT_SKIP_MAX_BYTES, OUTPUT_SOURCE_PROBE_BYTES,
    OUTPUT_VERIFY_MAX_LINE_BYTES, OpenProcessFile, PromptOutputEvent, ResolvedTarget,
    SessionPromptWriteError, legacy_thread_id,
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

pub(crate) fn output_cursor_anchor(
    target: &ResolvedTarget,
    (path, offset, source_id): &(PathBuf, u64, String),
) -> Option<String> {
    let prodex_birth = target.prodex.birth_identity.clone()?;
    let codex_birth = target.writer.birth_identity.clone()?;
    if output_source_id(path, &target.thread_id).ok().as_deref() != Some(source_id) {
        return None;
    }
    let checkpoint_id = source_checkpoint_id(path, *offset).ok()?;
    if output_source_id(path, &target.thread_id).ok().as_deref() != Some(source_id) {
        return None;
    }
    encode_output_cursor(OutputCursor {
        version: OUTPUT_CURSOR_VERSION,
        prodex_pid: target.prodex.pid,
        prodex_birth,
        codex_pid: target.writer.pid,
        codex_birth,
        thread_id: target.thread_id.clone(),
        source_id: source_id.clone(),
        offset: *offset,
        event_index: 0,
        checkpoint_id,
    })
    .ok()
}

impl OutputCursor {
    pub(crate) fn matches(&self, target: &ResolvedTarget, source_id: &str) -> bool {
        self.version == OUTPUT_CURSOR_VERSION
            && self.prodex_pid == target.prodex.pid
            && target.prodex.birth_identity.as_deref() == Some(self.prodex_birth.as_str())
            && self.codex_pid == target.writer.pid
            && target.writer.birth_identity.as_deref() == Some(self.codex_birth.as_str())
            && prodex_mojo_core::json::session_selector_matches(
                &self.thread_id,
                &target.thread_id,
                true,
            )
            .is_ok_and(|matches| matches)
            && self.source_id == source_id
    }
}

pub(crate) fn encode_output_cursor(
    mut cursor: OutputCursor,
) -> std::result::Result<String, SessionPromptWriteError> {
    cursor.version = OUTPUT_CURSOR_VERSION;
    let bytes =
        serde_json::to_vec(&cursor).map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn decode_output_cursor(
    value: &str,
) -> std::result::Result<OutputCursor, SessionPromptWriteError> {
    if value.is_empty() || value.len() > 16 * 1024 {
        return Err(SessionPromptWriteError::InvalidCursor);
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| SessionPromptWriteError::InvalidCursor)?;
    let mut cursor = serde_json::from_slice::<OutputCursor>(&bytes)
        .map_err(|_| SessionPromptWriteError::InvalidCursor)?;
    let Ok(thread_id) = Uuid::parse_str(&cursor.thread_id) else {
        return Err(SessionPromptWriteError::InvalidCursor);
    };
    cursor.thread_id = thread_id.to_string();
    (cursor.version == OUTPUT_CURSOR_VERSION
        && cursor.prodex_pid > 0
        && cursor.codex_pid > 0
        && !cursor.prodex_birth.is_empty()
        && !cursor.codex_birth.is_empty()
        && !cursor.thread_id.is_empty()
        && !cursor.source_id.is_empty()
        && !cursor.checkpoint_id.is_empty()
        && cursor.event_index <= 65_536)
        .then_some(cursor)
        .ok_or(SessionPromptWriteError::InvalidCursor)
}

pub(crate) fn output_source_id(
    path: &Path,
    thread_id: &str,
) -> std::result::Result<String, SessionPromptWriteError> {
    let metadata =
        fs::metadata(path).map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)?;
    if !metadata.is_file() {
        return Err(SessionPromptWriteError::OutputSourceUnavailable);
    }
    let file = prodex_core::open_regular_file_no_follow(path)
        .map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)
        .map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)?
    {
        return Err(SessionPromptWriteError::OutputSourceChanged);
    }
    // A rollout is append-only while its owning Codex session is alive. Content belongs in the
    // cursor checkpoint below; including a variable-length prefix here would make normal growth
    // look like a source replacement when a file crosses the probe-size boundary.
    let mut hasher = Sha256::new();
    hasher.update(thread_id.as_bytes());
    hasher.update(path.as_os_str().to_string_lossy().as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        hasher.update(metadata.dev().to_le_bytes());
        hasher.update(metadata.ino().to_le_bytes());
    }
    #[cfg(windows)]
    {
        let identity = prodex_core::opened_file_identity(&file)
            .map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)?;
        hasher.update(identity.to_le_bytes());
    }
    #[cfg(not(any(unix, windows)))]
    {
        hasher.update(
            metadata
                .created()
                .ok()
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |value| value.as_nanos() as u64)
                .to_le_bytes(),
        );
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn source_checkpoint_id(
    path: &Path,
    offset: u64,
) -> std::result::Result<String, SessionPromptWriteError> {
    let metadata = fs::metadata(path).map_err(|_| SessionPromptWriteError::OutputSourceChanged)?;
    let mut file = prodex_core::open_regular_file_no_follow(path)
        .map_err(|_| SessionPromptWriteError::OutputSourceChanged)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)
        .map_err(|_| SessionPromptWriteError::OutputSourceChanged)?
    {
        return Err(SessionPromptWriteError::OutputSourceChanged);
    }
    let prefix_len = offset.min(OUTPUT_SOURCE_PROBE_BYTES as u64);
    let mut bytes = vec![0; prefix_len as usize];
    file.read_exact(&mut bytes)
        .map_err(|_| SessionPromptWriteError::OutputSourceChanged)?;
    let mut hasher = Sha256::new();
    hasher.update(offset.to_le_bytes());
    hasher.update(&bytes);
    if offset > 0 {
        let window_start = offset.saturating_sub(4096);
        file.seek(SeekFrom::Start(window_start))
            .map_err(|_| SessionPromptWriteError::OutputSourceChanged)?;
        let mut window = vec![0; (offset - window_start) as usize];
        file.read_exact(&mut window)
            .map_err(|_| SessionPromptWriteError::OutputSourceChanged)?;
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
) -> std::result::Result<OutputReadBatch, SessionPromptWriteError> {
    let (source_len, complete, skipped_to) = read_complete_output(path, offset)?;
    if let Some(next_offset) = skipped_to {
        let events = if limit > 0 {
            vec![output_gap_event(offset, "oversized_record")?]
        } else {
            Vec::new()
        };
        return Ok(OutputReadBatch {
            events,
            next_offset,
            next_event_index: 0,
            has_more: next_offset < source_len,
        });
    }
    read_output_lines(&complete, offset, event_index, limit, source_len)
}

pub(crate) fn rollout_contains_exact_user_message(
    path: &Path,
    mut offset: u64,
    expected: &str,
) -> std::result::Result<bool, SessionPromptWriteError> {
    let mut scanned = 0_u64;
    loop {
        let (source_len, complete, skipped_to) = read_complete_output(path, offset)?;
        if let Some(next_offset) = skipped_to {
            let skipped = next_offset.saturating_sub(offset);
            scanned = scanned.saturating_add(skipped);
            if scanned > OUTPUT_SKIP_MAX_BYTES as u64 {
                return Ok(false);
            }
            offset = next_offset;
            continue;
        }

        let (found, consumed) = scan_exact_user_message_lines(&complete, expected);
        if found {
            return Ok(true);
        }
        if consumed == 0 {
            return Ok(false);
        }
        scanned = scanned.saturating_add(consumed);
        if scanned > OUTPUT_SKIP_MAX_BYTES as u64 {
            return Ok(false);
        }
        offset = offset.saturating_add(consumed);
        if offset >= source_len {
            return Ok(false);
        }
    }
}

fn scan_exact_user_message_lines(complete: &[u8], expected: &str) -> (bool, u64) {
    let mut consumed = 0_u64;
    for line in complete.split_inclusive(|byte| *byte == b'\n') {
        if exact_user_message_line(line, expected) {
            return (true, consumed);
        }
        consumed = consumed.saturating_add(line.len() as u64);
    }
    (false, consumed)
}

fn exact_user_message_line(raw_line: &[u8], expected: &str) -> bool {
    if raw_line.len() > OUTPUT_VERIFY_MAX_LINE_BYTES {
        return false;
    }
    let line = raw_line.strip_suffix(b"\n").unwrap_or(raw_line);
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
        return false;
    };
    let Some(payload) = value.get("payload") else {
        return false;
    };
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("event_msg") => {
            payload.get("type").and_then(serde_json::Value::as_str) == Some("user_message")
                && payload.get("message").and_then(serde_json::Value::as_str) == Some(expected)
        }
        Some("response_item") => {
            crate::app_commands::transcript_exact_visible_user_message(payload)
                .is_some_and(|message| message == expected)
        }
        _ => false,
    }
}

fn read_complete_output(
    path: &Path,
    offset: u64,
) -> std::result::Result<(u64, Vec<u8>, Option<u64>), SessionPromptWriteError> {
    read_complete_output_with_limit(path, offset, OUTPUT_READ_MAX_BYTES)
}

fn read_complete_output_with_limit(
    path: &Path,
    offset: u64,
    max_bytes: usize,
) -> std::result::Result<(u64, Vec<u8>, Option<u64>), SessionPromptWriteError> {
    let metadata =
        fs::metadata(path).map_err(|_| SessionPromptWriteError::OutputSourceUnavailable)?;
    if !metadata.is_file() || offset > metadata.len() {
        return Err(SessionPromptWriteError::OutputSourceChanged);
    }
    let mut file = prodex_core::open_regular_file_no_follow(path)
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    if !prodex_core::opened_file_matches_path(&metadata, path, &file)
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?
    {
        return Err(SessionPromptWriteError::OutputSourceChanged);
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    let bounded = bytes.len() <= max_bytes;
    let complete_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    if !bounded && complete_len == 0 {
        let Some(next_offset) = skip_oversized_line(&mut file, offset, metadata.len())? else {
            return Ok((metadata.len(), Vec::new(), None));
        };
        return Ok((metadata.len(), Vec::new(), Some(next_offset)));
    }
    Ok((metadata.len(), bytes[..complete_len].to_vec(), None))
}

fn skip_oversized_line(
    file: &mut fs::File,
    offset: u64,
    source_len: u64,
) -> std::result::Result<Option<u64>, SessionPromptWriteError> {
    let mut skipped = 0_usize;
    let mut chunk = vec![0_u8; 64 * 1024];
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    while skipped < OUTPUT_SKIP_MAX_BYTES {
        let remaining = OUTPUT_SKIP_MAX_BYTES - skipped;
        let chunk_len = chunk.len().min(remaining);
        let count = file
            .read(&mut chunk[..chunk_len])
            .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
        if count == 0 {
            return Ok(None);
        }
        if let Some(index) = chunk[..count].iter().position(|byte| *byte == b'\n') {
            return Ok(Some(
                offset
                    .saturating_add(skipped as u64)
                    .saturating_add(index as u64 + 1),
            ));
        }
        skipped = skipped.saturating_add(count);
    }
    if offset.saturating_add(skipped as u64) >= source_len {
        Ok(None)
    } else {
        Err(SessionPromptWriteError::RecoveryFailed)
    }
}

fn read_output_lines(
    complete: &[u8],
    offset: u64,
    event_index: usize,
    limit: usize,
    source_len: u64,
) -> std::result::Result<OutputReadBatch, SessionPromptWriteError> {
    let mut events = Vec::new();
    let mut total_text_bytes = 0_usize;
    let mut consumed = 0_usize;
    for line in complete.split_inclusive(|byte| *byte == b'\n') {
        let line_start = offset.saturating_add(consumed as u64);
        let raw_len = line.len();
        match output_line_plan(line, events.len() >= limit)? {
            prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineAction::Limit => {
                return Ok(OutputReadBatch {
                    events,
                    next_offset: line_start,
                    next_event_index: 0,
                    has_more: true,
                });
            }
            prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineAction::Oversized => {
                let next_offset = line_start.saturating_add(raw_len as u64);
                events.push(output_gap_event(line_start, "oversized_record")?);
                consumed = consumed.saturating_add(raw_len);
                if events.len() >= limit {
                    return Ok(OutputReadBatch {
                        events,
                        next_offset,
                        next_event_index: 0,
                        has_more: next_offset < source_len,
                    });
                }
                continue;
            }
            prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineAction::InvalidUtf8 => {
                events.push(output_gap_event(line_start, "invalid_utf8")?);
                consumed = consumed.saturating_add(raw_len);
                continue;
            }
            prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineAction::Malformed => {
                events.push(output_gap_event(line_start, "malformed_record")?);
                consumed = consumed.saturating_add(raw_len);
                continue;
            }
            prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineAction::Process => {}
        }
        if let Some(batch) = read_output_line(
            line,
            line_start,
            if consumed == 0 { event_index } else { 0 },
            limit,
            &mut events,
            &mut total_text_bytes,
        )? {
            return Ok(batch);
        }
        consumed = consumed.saturating_add(raw_len);
    }
    let next_offset = offset.saturating_add(consumed as u64);
    Ok(OutputReadBatch {
        events,
        next_offset,
        next_event_index: 0,
        has_more: next_offset < source_len,
    })
}

fn read_output_line(
    raw_line: &[u8],
    line_start: u64,
    start_index: usize,
    limit: usize,
    events: &mut Vec<PromptOutputEvent>,
    total_text_bytes: &mut usize,
) -> std::result::Result<Option<OutputReadBatch>, SessionPromptWriteError> {
    let line = raw_line.strip_suffix(b"\n").unwrap_or(raw_line);
    let line = std::str::from_utf8(line).map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    let mut visible_index = 0_usize;
    for event in crate::app_commands::transcript_events_from_session_line(line) {
        let sequence = line_start
            .saturating_mul(65_536)
            .saturating_add(visible_index as u64);
        let Some(output_event) = output_event_from_transcript(sequence, event)? else {
            continue;
        };
        if visible_index < start_index {
            visible_index = visible_index.saturating_add(1);
            continue;
        }
        if events.len() >= limit {
            return Ok(Some(OutputReadBatch {
                events: std::mem::take(events),
                next_offset: line_start,
                next_event_index: visible_index,
                has_more: true,
            }));
        }
        if total_text_bytes.saturating_add(output_event.text.len())
            > OUTPUT_READ_MAX_TOTAL_TEXT_BYTES
        {
            return Ok(Some(OutputReadBatch {
                events: std::mem::take(events),
                next_offset: line_start,
                next_event_index: visible_index,
                has_more: true,
            }));
        }
        *total_text_bytes = total_text_bytes.saturating_add(output_event.text.len());
        events.push(output_event);
        visible_index = visible_index.saturating_add(1);
    }
    if start_index > visible_index {
        return Err(SessionPromptWriteError::OutputSourceChanged);
    }
    Ok(None)
}

fn raw_line_is_visible_user_message(
    raw_line: &[u8],
) -> std::result::Result<bool, SessionPromptWriteError> {
    if raw_line.len() > OUTPUT_VERIFY_MAX_LINE_BYTES {
        return Ok(false);
    }
    let line = raw_line.strip_suffix(b"\n").unwrap_or(raw_line);
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
        return Ok(false);
    };
    let Some(payload) = value.get("payload") else {
        return Ok(false);
    };
    let record_type = match value.get("type").and_then(serde_json::Value::as_str) {
        Some("event_msg") => 1,
        Some("response_item") => 2,
        _ => 0,
    };
    let payload_user_message = if record_type == 1 {
        payload.get("type").and_then(serde_json::Value::as_str) == Some("user_message")
            && payload
                .get("message")
                .and_then(serde_json::Value::as_str)
                .is_some()
    } else {
        payload.get("type").and_then(serde_json::Value::as_str) == Some("message")
    };
    let role_user = payload.get("role").and_then(serde_json::Value::as_str) == Some("user");
    let kinds = payload
        .get("internal_chat_message_metadata_passthrough")
        .and_then(|metadata| metadata.get("content_item_kinds"))
        .and_then(serde_json::Value::as_array);
    let metadata_present = kinds.is_some();
    let kinds_nonempty = kinds.is_some_and(|kinds| !kinds.is_empty());
    let kinds_all_user_text =
        kinds.is_some_and(|kinds| kinds.iter().all(|kind| kind.as_str() == Some("user.text")));
    prodex_mojo_core::session_cli_policy::prompt_write_user_message_visible(
        record_type,
        payload_user_message,
        role_user,
        metadata_present,
        kinds_nonempty,
        kinds_all_user_text,
    )
    .map_err(|_| SessionPromptWriteError::OutputReadFailed)
}

fn transcript_record_shape_is_valid(
    line: &str,
) -> std::result::Result<bool, SessionPromptWriteError> {
    let value = serde_json::from_str::<serde_json::Value>(line)
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    let record_type = match value.get("type").and_then(serde_json::Value::as_str) {
        Some("event_msg") => 1,
        Some("response_item") => 2,
        Some("session_meta") => 3,
        Some("turn_context") => 4,
        Some(_) => 5,
        None => 0,
    };
    let payload = value.get("payload");
    prodex_mojo_core::session_cli_policy::prompt_write_record_shape(
        record_type,
        payload.is_some(),
        payload.is_some_and(serde_json::Value::is_object),
        payload
            .and_then(|payload| payload.get("type"))
            .and_then(serde_json::Value::as_str)
            .is_some(),
    )
    .map_err(|_| SessionPromptWriteError::OutputReadFailed)
}

fn output_gap_event(
    sequence: u64,
    reason: &'static str,
) -> std::result::Result<PromptOutputEvent, SessionPromptWriteError> {
    let reason_code = match reason {
        "oversized_record" => 0,
        "invalid_utf8" => 1,
        "malformed_record" => 2,
        _ => return Err(SessionPromptWriteError::OutputReadFailed),
    };
    let text = prodex_mojo_core::session_cli_policy::prompt_write_gap_text(reason_code)
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    Ok(PromptOutputEvent {
        sequence: sequence.saturating_mul(65_536),
        timestamp: "-".to_string(),
        kind: "gap".to_string(),
        name: Some(reason.to_string()),
        status: Some("skipped".to_string()),
        text,
    })
}

fn output_line_plan(
    raw_line: &[u8],
    limit_reached: bool,
) -> std::result::Result<
    prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineAction,
    SessionPromptWriteError,
> {
    let line = raw_line.strip_suffix(b"\n").unwrap_or(raw_line);
    let utf8_valid = std::str::from_utf8(line).is_ok();
    let json_valid = utf8_valid && serde_json::from_slice::<serde_json::Value>(line).is_ok();
    let shape_valid = if json_valid {
        let line =
            std::str::from_utf8(line).map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
        transcript_record_shape_is_valid(line)?
    } else {
        false
    };
    prodex_mojo_core::session_cli_policy::prompt_write_output_line_plan(
        prodex_mojo_core::session_cli_policy::SessionPromptWriteOutputLineInput {
            raw_length: raw_line.len(),
            read_limit: OUTPUT_READ_MAX_LINE_BYTES,
            verify_limit: OUTPUT_VERIFY_MAX_LINE_BYTES,
            utf8_valid,
            json_valid,
            shape_valid,
            visible_user_message: raw_line_is_visible_user_message(raw_line)?,
            limit_reached,
        },
    )
    .map_err(|_| SessionPromptWriteError::OutputReadFailed)
}

fn output_event_from_transcript(
    sequence: u64,
    event: crate::app_commands::TranscriptEvent,
) -> std::result::Result<Option<PromptOutputEvent>, SessionPromptWriteError> {
    let plan = prodex_mojo_core::session_cli_policy::transcript_output_plan(&event.source)
        .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    if !plan.visible {
        return Ok(None);
    }
    let kind = match plan.kind {
        prodex_mojo_core::session_cli_policy::TranscriptOutputKind::Assistant => "assistant",
        prodex_mojo_core::session_cli_policy::TranscriptOutputKind::User => "user",
        prodex_mojo_core::session_cli_policy::TranscriptOutputKind::Tool => "tool",
        prodex_mojo_core::session_cli_policy::TranscriptOutputKind::Other => event.source.as_str(),
    };
    let status = match plan.status {
        prodex_mojo_core::session_cli_policy::TranscriptOutputStatus::None => None,
        prodex_mojo_core::session_cli_policy::TranscriptOutputStatus::Started => Some("started"),
        prodex_mojo_core::session_cli_policy::TranscriptOutputStatus::Completed => {
            Some("completed")
        }
    };
    let name = plan
        .name
        .map(|(start, end)| {
            let name = event
                .source
                .get(start..end)
                .ok_or(SessionPromptWriteError::OutputReadFailed)?;
            prodex_mojo_core::session_cli_policy::transcript_output_bounded(
                name,
                prodex_mojo_core::session_cli_policy::TranscriptOutputTextMode::Name,
            )
            .map(Some)
            .map_err(|_| SessionPromptWriteError::OutputReadFailed)
        })
        .transpose()?
        .flatten();
    let timestamp = prodex_mojo_core::session_cli_policy::transcript_output_bounded(
        &event.timestamp,
        prodex_mojo_core::session_cli_policy::TranscriptOutputTextMode::Timestamp,
    )
    .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    let redacted = redaction::redaction_redact_secret_like_text(&event.text);
    let text = prodex_mojo_core::session_cli_policy::transcript_output_bounded(
        &redacted,
        prodex_mojo_core::session_cli_policy::TranscriptOutputTextMode::Text,
    )
    .map_err(|_| SessionPromptWriteError::OutputReadFailed)?;
    Ok(Some(PromptOutputEvent {
        sequence,
        timestamp,
        kind: kind.to_string(),
        name,
        status: status.map(str::to_string),
        text,
    }))
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
) -> std::result::Result<(), SessionPromptWriteError> {
    if depth > 8 {
        return Err(SessionPromptWriteError::OutputReadFailed);
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
            return Err(SessionPromptWriteError::OutputSourceAmbiguous);
        }
    }
    Ok(())
}

fn is_uncompressed_rollout_file_name(name: &str) -> bool {
    name.starts_with("rollout-") && name.ends_with(".jsonl")
}

pub(crate) fn rollout_path_exists_in_roots(path: &Path, roots: &[PathBuf]) -> bool {
    if path.is_absolute() {
        return fs::symlink_metadata(path).is_ok();
    }
    roots
        .iter()
        .any(|root| fs::symlink_metadata(root.join(path)).is_ok())
}

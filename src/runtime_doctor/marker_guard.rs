use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

const RUNTIME_LOG_FUNCTIONS: &[&str] = &["runtime_proxy_log", "runtime_proxy_log_to_path"];

const RUNTIME_MARKER_PREFIXES: &[&str] = &[
    "chain",
    "compact",
    "compat",
    "continuation",
    "first",
    "local",
    "precommit",
    "previous_response",
    "profile",
    "quota",
    "responses",
    "runtime_proxy",
    "selection",
    "stale",
    "state",
    "stream",
    "upstream",
    "websocket",
];

const RUNTIME_DOCTOR_MARKER_GUARD_ALLOWLIST: &[&str] = &[
    "continuation_journal_save_inline",
    "continuation_journal_save_suppressed",
    "local_close",
    "local_connection_closed",
    "precommit_hold",
    "profile_commit",
    "profile_probe_refresh_suppressed",
    "state_save_inline",
    "state_save_suppressed",
    "stream_complete",
    "stream_start",
    "upstream_connect_ok",
    "upstream_connect_start",
    "upstream_stream_end",
    "websocket_reuse_start",
];

#[test]
fn runtime_doctor_marker_registry_covers_runtime_log_markers() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut source_files = Vec::new();
    collect_rust_source_files(&manifest_dir.join("src"), &mut source_files);

    let known_markers = RUNTIME_DOCTOR_MARKERS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let allowlist = RUNTIME_DOCTOR_MARKER_GUARD_ALLOWLIST
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::<String, BTreeSet<String>>::new();

    for path in source_files {
        if path.ends_with("runtime_doctor.rs")
            || path.components().any(|c| c.as_os_str() == "runtime_doctor")
        {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        for function_name in RUNTIME_LOG_FUNCTIONS {
            collect_runtime_log_marker_candidates(
                &manifest_dir,
                &path,
                &source,
                function_name,
                &mut candidates,
            );
        }
    }

    let missing = candidates
        .into_iter()
        .filter(|(marker, _)| !known_markers.contains(marker.as_str()))
        .filter(|(marker, _)| !allowlist.contains(marker.as_str()))
        .collect::<BTreeMap<_, _>>();

    if !missing.is_empty() {
        let details = missing
            .iter()
            .map(|(marker, locations)| {
                let sample_locations = locations
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n  ");
                format!("{marker}\n  {sample_locations}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "runtime doctor marker registry missing runtime log marker(s). Add diagnostic markers to RUNTIME_DOCTOR_MARKERS, or classify noisy lifecycle tokens in RUNTIME_DOCTOR_MARKER_GUARD_ALLOWLIST.\n{details}"
        );
    }
}

fn collect_rust_source_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn collect_runtime_log_marker_candidates(
    manifest_dir: &Path,
    path: &Path,
    source: &str,
    function_name: &str,
    candidates: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let mut offset = 0;
    while let Some(relative_position) = source[offset..].find(function_name) {
        let call_start = offset + relative_position;
        offset = call_start + function_name.len();
        if !is_identifier_boundary(source, call_start, function_name.len()) {
            continue;
        }
        let Some((call_end, call_body)) = runtime_log_call_body(source, offset) else {
            continue;
        };
        offset = call_end;
        let line_number = source[..call_start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        let display_path = path.strip_prefix(manifest_dir).unwrap_or(path);
        let location = format!("{}:{line_number}", display_path.display());
        for message in runtime_log_message_format_strings(call_body) {
            for marker in runtime_log_marker_tokens(&message) {
                candidates
                    .entry(marker)
                    .or_default()
                    .insert(location.clone());
            }
        }
    }
}

fn is_identifier_boundary(source: &str, start: usize, len: usize) -> bool {
    let before = start
        .checked_sub(1)
        .and_then(|index| source.as_bytes().get(index))
        .copied();
    let after = source.as_bytes().get(start + len).copied();
    !before.is_some_and(is_rust_identifier_byte) && !after.is_some_and(is_rust_identifier_byte)
}

fn is_rust_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn runtime_log_call_body(source: &str, search_from: usize) -> Option<(usize, &str)> {
    let open = source[search_from..].find('(')? + search_from;
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth = depth.saturating_add(1),
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((index + 1, &source[open + 1..index]));
                }
            }
            b'"' => index = skip_normal_string(bytes, index)?,
            b'\'' => index = skip_char_literal(bytes, index)?,
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = skip_line_comment(bytes, index);
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index)?;
            }
            b'r' if raw_string_hash_count(bytes, index).is_some() => {
                index = skip_raw_string(bytes, index)?;
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn runtime_log_message_format_strings(call_body: &str) -> Vec<String> {
    let mut messages = Vec::new();
    let mut offset = 0;
    while let Some(relative_position) = call_body[offset..].find("format!") {
        let format_start = offset + relative_position;
        offset = format_start + "format!".len();
        let Some(open) = call_body[offset..]
            .find('(')
            .map(|position| offset + position)
        else {
            continue;
        };
        let mut string_start = open + 1;
        while call_body
            .as_bytes()
            .get(string_start)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'&')
        {
            string_start += 1;
        }
        if let Some((end, message)) = read_string_literal(call_body, string_start) {
            messages.push(message);
            offset = end;
        }
    }
    if !messages.is_empty() {
        return messages;
    }

    let mut index = 0;
    while index < call_body.len() {
        if let Some((_, message)) = read_string_literal(call_body, index) {
            return vec![message];
        }
        index += 1;
    }
    Vec::new()
}

fn runtime_log_marker_tokens(message: &str) -> Vec<String> {
    message
        .split_whitespace()
        .filter_map(|token| {
            let token = token.trim_matches(|c: char| {
                matches!(
                    c,
                    '`' | '\'' | '"' | '[' | ']' | '(' | ')' | '{' | '}' | ',' | '.' | ';' | ':'
                )
            });
            if token.contains(['=', '{', '}', '/', '-']) || !runtime_marker_like_token(token) {
                return None;
            }
            Some(token.to_string())
        })
        .collect()
}

fn runtime_marker_like_token(token: &str) -> bool {
    !token.is_empty()
        && token.as_bytes()[0].is_ascii_lowercase()
        && token.contains('_')
        && token
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && RUNTIME_MARKER_PREFIXES
            .iter()
            .any(|prefix| token.starts_with(&format!("{prefix}_")))
}

fn read_string_literal(source: &str, start: usize) -> Option<(usize, String)> {
    let bytes = source.as_bytes();
    if bytes.get(start) == Some(&b'"') {
        let end = skip_normal_string(bytes, start)?;
        return Some((end + 1, source[start + 1..end].to_string()));
    }
    if bytes.get(start) == Some(&b'r') {
        let hash_count = raw_string_hash_count(bytes, start)?;
        let content_start = start + 2 + hash_count;
        let end = raw_string_end(bytes, content_start, hash_count)?;
        return Some((end + 1 + hash_count, source[content_start..end].to_string()));
    }
    None
}

fn raw_string_hash_count(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'r') {
        return None;
    }
    let mut index = start + 1;
    while bytes.get(index) == Some(&b'#') {
        index += 1;
    }
    (bytes.get(index) == Some(&b'"')).then_some(index - start - 1)
}

fn raw_string_end(bytes: &[u8], content_start: usize, hash_count: usize) -> Option<usize> {
    let mut index = content_start;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && (0..hash_count).all(|offset| bytes.get(index + 1 + offset) == Some(&b'#'))
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn skip_raw_string(bytes: &[u8], start: usize) -> Option<usize> {
    let hash_count = raw_string_hash_count(bytes, start)?;
    let content_start = start + 2 + hash_count;
    raw_string_end(bytes, content_start, hash_count).map(|end| end + hash_count)
}

fn skip_normal_string(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

fn skip_char_literal(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'\'' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

fn skip_line_comment(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 2;
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

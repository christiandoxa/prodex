use serde_json::Value;
use std::collections::HashMap;

use crate::runtime_mem_lookup_json_path;

const RUNTIME_MEM_SHORT_ARTIFACT_REF_PREFIXES: &[&str] = &["p:", "psc:"];

pub fn runtime_mem_content_hash(text: &str) -> String {
    format!("sc:{:016x}", runtime_mem_fnv1a64(text.as_bytes()))
}

pub(crate) fn runtime_mem_first_artifact_ref_text_at_paths(
    event: &Value,
    paths: &[&str],
) -> Option<String> {
    paths.iter().find_map(|path| {
        runtime_mem_lookup_json_path(event, path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| {
                runtime_mem_normalize_prodex_artifact_ref(text).unwrap_or_else(|| text.to_string())
            })
    })
}

pub(crate) fn runtime_mem_first_prodex_artifact_ref_at_paths(
    event: &Value,
    paths: &[&str],
    content: &str,
) -> Option<String> {
    paths
        .iter()
        .find_map(|path| {
            runtime_mem_lookup_json_path(event, path)
                .and_then(Value::as_str)
                .and_then(runtime_mem_normalize_prodex_artifact_ref)
        })
        .or_else(|| runtime_mem_extract_artifact_marker(event))
        .or_else(|| runtime_mem_prodex_artifact_ref(None, content))
}

pub(crate) fn runtime_mem_prodex_artifact_ref(
    artifact_ref: Option<&str>,
    content: &str,
) -> Option<String> {
    if let Some(normalized) = artifact_ref.and_then(runtime_mem_normalize_prodex_artifact_ref) {
        return Some(normalized);
    }
    if let Some(artifact_ref) = artifact_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let combined = format!("{artifact_ref}\n{content}");
        if let Some(normalized) = runtime_mem_normalize_prodex_artifact_ref(&combined) {
            return Some(normalized);
        }
    }
    runtime_mem_extract_artifact_marker_from_text(content)
}

pub(crate) fn runtime_mem_normalize_prodex_artifact_ref(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(reference) = runtime_mem_parse_non_alias_artifact_ref(text) {
        return Some(reference);
    }
    let aliases = runtime_mem_artifact_aliases_from_text(text);
    runtime_mem_extract_artifact_marker_from_text_with_aliases(text, &aliases)
}

pub(crate) fn runtime_mem_artifact_recall_summary(
    artifact_ref: &str,
    content_hash: &str,
    original_bytes: usize,
) -> String {
    format!("{artifact_ref} [mem art; h={content_hash}; b={original_bytes}]")
}

pub(crate) fn runtime_mem_duplicate_recall_summary(
    original_id: &str,
    content_hash: &str,
    original_bytes: usize,
) -> String {
    format!("mem dup: original={original_id}; h={content_hash}; b={original_bytes}")
}

pub(crate) fn runtime_mem_extract_artifact_marker(value: &Value) -> Option<String> {
    let aliases = runtime_mem_artifact_aliases(value);
    runtime_mem_extract_artifact_marker_with_aliases(value, &aliases)
}

fn runtime_mem_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn runtime_mem_extract_artifact_marker_with_aliases(
    value: &Value,
    aliases: &HashMap<String, String>,
) -> Option<String> {
    match value {
        Value::String(text) => {
            runtime_mem_extract_artifact_marker_from_text_with_aliases(text, aliases)
        }
        Value::Array(values) => values
            .iter()
            .find_map(|value| runtime_mem_extract_artifact_marker_with_aliases(value, aliases)),
        Value::Object(object) => object
            .values()
            .find_map(|value| runtime_mem_extract_artifact_marker_with_aliases(value, aliases)),
        _ => None,
    }
}

fn runtime_mem_extract_artifact_marker_from_text(text: &str) -> Option<String> {
    let aliases = runtime_mem_artifact_aliases_from_text(text);
    runtime_mem_extract_artifact_marker_from_text_with_aliases(text, &aliases)
}

fn runtime_mem_extract_artifact_marker_from_text_with_aliases(
    text: &str,
    aliases: &HashMap<String, String>,
) -> Option<String> {
    runtime_mem_artifact_ref_tokens(text)
        .into_iter()
        .find_map(|token| runtime_mem_parse_artifact_ref_token(token, aliases))
}

fn runtime_mem_artifact_aliases(value: &Value) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    runtime_mem_collect_artifact_aliases(value, &mut aliases);
    aliases
}

fn runtime_mem_collect_artifact_aliases(value: &Value, aliases: &mut HashMap<String, String>) {
    match value {
        Value::String(text) => runtime_mem_collect_artifact_aliases_from_text(text, aliases),
        Value::Array(values) => {
            for value in values {
                runtime_mem_collect_artifact_aliases(value, aliases);
            }
        }
        Value::Object(object) => {
            for value in object.values() {
                runtime_mem_collect_artifact_aliases(value, aliases);
            }
        }
        _ => {}
    }
}

pub(crate) fn runtime_mem_artifact_aliases_from_text(text: &str) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    runtime_mem_collect_artifact_aliases_from_text(text, &mut aliases);
    aliases
}

fn runtime_mem_collect_artifact_aliases_from_text(
    text: &str,
    aliases: &mut HashMap<String, String>,
) {
    if !text.contains('@') || !text.contains('=') {
        return;
    }
    for token in runtime_mem_artifact_ref_tokens(text) {
        if let Some((alias, reference)) = runtime_mem_parse_artifact_alias(token) {
            aliases.entry(alias).or_insert(reference);
        }
    }
}

fn runtime_mem_parse_artifact_alias(token: &str) -> Option<(String, String)> {
    let token = runtime_mem_trim_artifact_ref_token(token);
    let (alias, reference) = token.split_once('=')?;
    if !runtime_mem_artifact_alias_valid(alias) {
        return None;
    }
    let reference = runtime_mem_parse_non_alias_artifact_ref(reference)?;
    Some((alias.to_string(), reference))
}

pub(crate) fn runtime_mem_parse_artifact_ref_token(
    token: &str,
    aliases: &HashMap<String, String>,
) -> Option<String> {
    let token = runtime_mem_trim_artifact_ref_token(token);
    if let Some((_, reference)) = token.split_once('=')
        && token.starts_with('@')
    {
        return runtime_mem_parse_non_alias_artifact_ref(reference);
    }
    runtime_mem_parse_alias_artifact_ref(token, aliases)
        .or_else(|| runtime_mem_parse_non_alias_artifact_ref(token))
}

fn runtime_mem_parse_alias_artifact_ref(
    token: &str,
    aliases: &HashMap<String, String>,
) -> Option<String> {
    let (alias, suffix) = runtime_mem_split_artifact_alias_ref(token)?;
    aliases
        .get(alias)
        .map(|reference| format!("{reference}{suffix}"))
}

fn runtime_mem_split_artifact_alias_ref(token: &str) -> Option<(&str, &str)> {
    let rest = token.strip_prefix('@')?;
    let digit_len = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .map(char::len_utf8)
        .sum::<usize>();
    if digit_len == 0 {
        return None;
    }
    let alias_end = 1 + digit_len;
    Some((&token[..alias_end], &token[alias_end..]))
}

fn runtime_mem_artifact_alias_valid(alias: &str) -> bool {
    alias
        .strip_prefix('@')
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()))
}

fn runtime_mem_parse_non_alias_artifact_ref(token: &str) -> Option<String> {
    let token = runtime_mem_trim_artifact_ref_token(token);
    if token
        .strip_prefix("prodex-artifact:")
        .is_some_and(|value| !value.is_empty())
        || token
            .strip_prefix("prodex://artifact/")
            .is_some_and(|value| !value.is_empty())
    {
        return Some(token.to_string());
    }

    for prefix in RUNTIME_MEM_SHORT_ARTIFACT_REF_PREFIXES {
        if token
            .strip_prefix(prefix)
            .is_some_and(runtime_mem_short_artifact_ref_tail_valid)
        {
            return Some(token.to_string());
        }
    }

    None
}

fn runtime_mem_short_artifact_ref_tail_valid(tail: &str) -> bool {
    let tail = tail.strip_prefix("sc:").unwrap_or(tail);
    let artifact_id = tail
        .split_once('#')
        .map(|(artifact_id, _)| artifact_id)
        .or_else(|| tail.split_once('?').map(|(artifact_id, _)| artifact_id))
        .unwrap_or(tail);
    !artifact_id.is_empty()
        && artifact_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

pub(crate) fn runtime_mem_artifact_ref_tokens(text: &str) -> Vec<&str> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ')' | ']' | '}'))
        .collect()
}

fn runtime_mem_trim_artifact_ref_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | ':'
                | ';'
                | '.'
                | '!'
                | '?'
                | '('
                | '['
                | '{'
                | '<'
                | ')'
                | ']'
                | '}'
                | '>'
        )
    })
}

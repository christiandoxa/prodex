use super::constants::SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX;
use super::static_context::{
    runtime_smart_context_static_prompt_field_key,
    runtime_smart_context_value_is_static_context_item,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RuntimeSmartContextLineRange {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RuntimeSmartContextArtifactReference {
    pub(super) id: String,
    pub(super) marker: String,
    pub(super) line_range: Option<RuntimeSmartContextLineRange>,
    pub(super) line_ranges: Vec<RuntimeSmartContextLineRange>,
}

pub(super) fn runtime_smart_context_collect_rehydratable_artifact_ref_ids(
    value: &serde_json::Value,
) -> Vec<String> {
    runtime_smart_context_collect_rehydratable_artifact_refs(value)
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn runtime_smart_context_collect_rehydratable_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let mut refs = BTreeSet::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_rehydratable_artifact_refs_from_value(value, &aliases, &mut refs);
    refs.into_iter().collect()
}

pub(super) fn runtime_smart_context_collect_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let mut refs = BTreeSet::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_artifact_refs_from_value(value, &aliases, &mut refs);
    refs.into_iter().collect()
}

fn runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, String>,
    refs: &mut BTreeSet<RuntimeSmartContextArtifactReference>,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                    item, aliases, refs,
                );
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                    item, aliases, refs,
                );
            }
        }
        _ => runtime_smart_context_collect_artifact_refs_from_value(value, aliases, refs),
    }
}

fn runtime_smart_context_collect_artifact_refs_from_value(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, String>,
    refs: &mut BTreeSet<RuntimeSmartContextArtifactReference>,
) {
    match value {
        serde_json::Value::String(text)
            if text.contains("prodex-artifact:")
                || text.contains(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX)
                || text.contains('@')
                || text.contains("prodex smart context artifact")
                || text.contains("prodex-sc ") =>
        {
            for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases)
            {
                refs.insert(reference);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_smart_context_collect_artifact_aliases(
    value: &serde_json::Value,
) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    runtime_smart_context_collect_artifact_aliases_from_value(value, &mut aliases);
    aliases
}

fn runtime_smart_context_collect_artifact_aliases_from_value(
    value: &serde_json::Value,
    aliases: &mut BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::String(text) if text.contains('@') && text.contains('=') => {
            for token in runtime_smart_context_artifact_ref_tokens(text) {
                if let Some((alias, id)) = runtime_smart_context_parse_artifact_alias(token) {
                    aliases.entry(alias).or_insert(id);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_smart_context_artifact_ref_occurrences_from_text(
    text: &str,
    aliases: &BTreeMap<String, String>,
) -> Vec<RuntimeSmartContextArtifactReference> {
    runtime_smart_context_artifact_ref_tokens(text)
        .into_iter()
        .filter_map(|token| {
            runtime_smart_context_parse_artifact_reference_with_aliases(token, aliases)
        })
        .collect()
}

fn runtime_smart_context_artifact_ref_tokens(text: &str) -> Vec<&str> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ')' | ']' | '}'))
        .collect()
}

fn runtime_smart_context_parse_artifact_alias(token: &str) -> Option<(String, String)> {
    let token = runtime_smart_context_trim_artifact_ref_token(token);
    let (alias, reference) = token.split_once('=')?;
    if !runtime_smart_context_artifact_alias_valid(alias) {
        return None;
    }
    let reference = runtime_smart_context_parse_non_alias_artifact_reference(reference)?;
    Some((alias.to_string(), reference.id))
}

fn runtime_smart_context_parse_artifact_reference_with_aliases(
    token: &str,
    aliases: &BTreeMap<String, String>,
) -> Option<RuntimeSmartContextArtifactReference> {
    let token = runtime_smart_context_trim_artifact_ref_token(token);
    if token.starts_with('@') && token.contains('=') {
        return None;
    }
    if let Some(reference) = runtime_smart_context_parse_alias_artifact_reference(token, aliases) {
        return Some(reference);
    }
    runtime_smart_context_parse_non_alias_artifact_reference(token)
}

fn runtime_smart_context_parse_alias_artifact_reference(
    token: &str,
    aliases: &BTreeMap<String, String>,
) -> Option<RuntimeSmartContextArtifactReference> {
    let (alias, suffix) = runtime_smart_context_split_artifact_alias_ref(token)?;
    let id = aliases.get(alias)?;
    let line_ranges = runtime_smart_context_parse_line_ranges(suffix);
    Some(RuntimeSmartContextArtifactReference {
        id: id.clone(),
        marker: token.to_string(),
        line_range: line_ranges.first().copied(),
        line_ranges,
    })
}

fn runtime_smart_context_split_artifact_alias_ref(token: &str) -> Option<(&str, &str)> {
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

pub(super) fn runtime_smart_context_artifact_alias_valid(alias: &str) -> bool {
    alias
        .strip_prefix('@')
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()))
}

fn runtime_smart_context_trim_artifact_ref_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | ':'
                | ';'
                | '.'
                | ','
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

pub(super) fn runtime_smart_context_parse_non_alias_artifact_reference(
    token: &str,
) -> Option<RuntimeSmartContextArtifactReference> {
    let token = runtime_smart_context_trim_artifact_ref_token(token);
    let raw = if let Some(raw) = token.strip_prefix("prodex-artifact:") {
        Cow::Borrowed(raw)
    } else if let Some(raw) = token.strip_prefix(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX) {
        if raw.starts_with("sc:") {
            Cow::Borrowed(raw)
        } else {
            Cow::Owned(format!("sc:{raw}"))
        }
    } else {
        Cow::Borrowed(token)
    };
    let raw = raw.as_ref();
    if !raw.starts_with("sc:") {
        return None;
    }

    let mut id_end = 3usize;
    for (offset, ch) in raw[3..].char_indices() {
        if ch.is_ascii_hexdigit() {
            id_end = 3 + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    if id_end == 3 {
        return None;
    }

    let line_ranges = runtime_smart_context_parse_line_ranges(&raw[id_end..]);
    Some(RuntimeSmartContextArtifactReference {
        id: raw[..id_end].to_string(),
        marker: token.to_string(),
        line_range: line_ranges.first().copied(),
        line_ranges,
    })
}

fn runtime_smart_context_parse_line_ranges(suffix: &str) -> Vec<RuntimeSmartContextLineRange> {
    let Some(suffix) = suffix
        .strip_prefix('#')
        .or_else(|| suffix.strip_prefix(':'))
        .or_else(|| suffix.strip_prefix('?'))
    else {
        return Vec::new();
    };
    let suffix = suffix.strip_prefix("lines=").unwrap_or(suffix);
    suffix
        .split(',')
        .filter_map(runtime_smart_context_parse_line_range_segment)
        .collect()
}

fn runtime_smart_context_parse_line_range_segment(
    suffix: &str,
) -> Option<RuntimeSmartContextLineRange> {
    let suffix = suffix
        .strip_prefix('L')
        .or_else(|| suffix.strip_prefix('l'))
        .unwrap_or(suffix);
    let (start, end) = suffix.split_once('-').unwrap_or((suffix, suffix));
    let start = runtime_smart_context_parse_line_number(start)?;
    let end = runtime_smart_context_parse_line_number(end)?;
    (start > 0 && end >= start).then_some(RuntimeSmartContextLineRange { start, end })
}

fn runtime_smart_context_parse_line_number(value: &str) -> Option<usize> {
    value
        .strip_prefix('L')
        .or_else(|| value.strip_prefix('l'))
        .unwrap_or(value)
        .parse::<usize>()
        .ok()
}

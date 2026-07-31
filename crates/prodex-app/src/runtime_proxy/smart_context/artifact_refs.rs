use super::constants::SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX;
use super::runtime_smart_context_artifact_id_valid;
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
    if let Some(object) = value.as_object() {
        for (key, item) in object {
            if runtime_smart_context_static_prompt_field_key(key) {
                continue;
            }
            runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                item, aliases, refs,
            );
        }
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                item, aliases, refs,
            );
        }
        return;
    }
    runtime_smart_context_collect_artifact_refs_from_value(value, aliases, refs);
}

fn runtime_smart_context_collect_artifact_refs_from_value(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, String>,
    refs: &mut BTreeSet<RuntimeSmartContextArtifactReference>,
) {
    if let Some(text) = value.as_str() {
        if runtime_smart_context_may_contain_artifact_ref(text) {
            for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases)
            {
                refs.insert(reference);
            }
        }
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
        }
        return;
    }
    if let Some(object) = value.as_object() {
        for item in object.values() {
            runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
        }
    }
}

fn runtime_smart_context_may_contain_artifact_ref(text: &str) -> bool {
    text.contains("prodex-artifact:")
        || text.contains(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX)
        || text.contains("psc2:")
        || text.contains('@')
        || text.contains("prodex smart context artifact")
        || text.contains("prodex-sc ")
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
    if let Some(text) = value.as_str() {
        runtime_smart_context_collect_artifact_aliases_from_text(text, aliases);
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
        }
        return;
    }
    if let Some(object) = value.as_object() {
        for item in object.values() {
            runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
        }
    }
}

fn runtime_smart_context_collect_artifact_aliases_from_text(
    text: &str,
    aliases: &mut BTreeMap<String, String>,
) {
    if !text.contains('@') || !text.contains('=') {
        return;
    }
    for (alias, id) in runtime_smart_context_artifact_ref_tokens(text)
        .into_iter()
        .filter_map(runtime_smart_context_parse_artifact_alias)
    {
        aliases.entry(alias).or_insert(id);
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
    let raw = runtime_smart_context_normalize_artifact_ref(token);
    let raw = raw.as_ref();
    let prefix_len = runtime_smart_context_artifact_prefix_len(raw)?;
    let id_end = runtime_smart_context_artifact_id_end(raw, prefix_len)?;
    let id = &raw[..id_end];
    runtime_smart_context_artifact_id_valid(id).then_some(())?;

    let line_ranges = runtime_smart_context_parse_line_ranges(&raw[id_end..]);
    Some(RuntimeSmartContextArtifactReference {
        id: id.to_string(),
        marker: token.to_string(),
        line_range: line_ranges.first().copied(),
        line_ranges,
    })
}

fn runtime_smart_context_normalize_artifact_ref<'a>(token: &'a str) -> Cow<'a, str> {
    if let Some(raw) = token.strip_prefix("prodex-artifact:") {
        Cow::Borrowed(raw)
    } else if let Some(raw) = token.strip_prefix("psc2:") {
        Cow::Owned(format!("sc2:{raw}"))
    } else if let Some(raw) = token.strip_prefix(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX) {
        if raw.starts_with("sc:") {
            Cow::Borrowed(raw)
        } else {
            Cow::Owned(format!("sc:{raw}"))
        }
    } else {
        Cow::Borrowed(token)
    }
}

fn runtime_smart_context_artifact_prefix_len(raw: &str) -> Option<usize> {
    raw.strip_prefix("sc2:")
        .map(|_| 4)
        .or_else(|| raw.strip_prefix("sc:").map(|_| 3))
}

fn runtime_smart_context_artifact_id_end(raw: &str, prefix_len: usize) -> Option<usize> {
    let id_end = raw[prefix_len..]
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_hexdigit())
        .last()
        .map_or(prefix_len, |(offset, ch)| {
            prefix_len + offset + ch.len_utf8()
        });
    (id_end > prefix_len).then_some(id_end)
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

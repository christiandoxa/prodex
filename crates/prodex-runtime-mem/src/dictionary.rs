use crate::shadow::{runtime_mem_short_shadow_event, runtime_mem_value_is_text};
use crate::{
    RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX_LEGACY,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY,
    RUNTIME_MEM_SUPER_SLIM_V2_INTERN_REF_FIELD, RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS,
    RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD, RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
    runtime_mem_lookup_json_path,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

mod artifact_ref_dedupe;
mod intern;
mod terms;

use self::terms::runtime_mem_super_slim_v2_inline_dictionary_terms;
pub(crate) use artifact_ref_dedupe::RuntimeMemSuperSlimV2ArtifactRefDedupeState;
pub(crate) use intern::RuntimeMemSuperSlimV2InternState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeMemSuperSlimV2DictionaryMode {
    Exact,
    Prefix,
}

impl RuntimeMemSuperSlimV2DictionaryMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT,
            Self::Prefix => RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX,
        }
    }

    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value {
            RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT
            | RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY => Some(Self::Exact),
            RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX
            | RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX_LEGACY => Some(Self::Prefix),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeMemSuperSlimV2DictionaryEntry {
    pub(super) mode: RuntimeMemSuperSlimV2DictionaryMode,
    pub(super) value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeMemSuperSlimV2DictionaryCandidate {
    pub(crate) field: String,
    pub(crate) mode: RuntimeMemSuperSlimV2DictionaryMode,
    placement: RuntimeMemSuperSlimV2DictionaryPlacement,
    pub(crate) value: String,
    event_indexes: Vec<usize>,
}

impl RuntimeMemSuperSlimV2DictionaryCandidate {
    fn compact_ref(&self, dictionary_index: usize, value: &str) -> Option<String> {
        let base_ref = runtime_mem_super_slim_v2_dictionary_ref(&self.field, dictionary_index);
        match self.mode {
            RuntimeMemSuperSlimV2DictionaryMode::Exact => (value == self.value).then_some(base_ref),
            RuntimeMemSuperSlimV2DictionaryMode::Prefix => value
                .strip_prefix(&self.value)
                .filter(|suffix| !suffix.is_empty())
                .map(|suffix| format!("{base_ref}+{suffix}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeMemSuperSlimV2DictionaryPlacement {
    WholeField,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMemSuperSlimV2DictionaryRef<'a> {
    pub(super) field: &'a str,
    pub(super) index: usize,
    pub(super) suffix: Option<&'a str>,
}

pub(crate) fn runtime_mem_super_slim_v2_compact_dictionary_events(
    mut events: Vec<Value>,
) -> Vec<Value> {
    const MAX_DICTIONARY_EVENTS_PER_SHADOW: usize = 32;
    while runtime_mem_super_slim_v2_dictionary_event_count(&events)
        < MAX_DICTIONARY_EVENTS_PER_SHADOW
    {
        let Some(candidate) = runtime_mem_super_slim_v2_best_dictionary_candidate(&events) else {
            break;
        };
        events = runtime_mem_super_slim_v2_apply_dictionary_candidate(events, &candidate);
    }
    events
}

fn runtime_mem_super_slim_v2_dictionary_event_count(events: &[Value]) -> usize {
    events
        .iter()
        .filter(|event| {
            event.get("t").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .count()
}

fn runtime_mem_super_slim_v2_best_dictionary_candidate(
    events: &[Value],
) -> Option<RuntimeMemSuperSlimV2DictionaryCandidate> {
    runtime_mem_super_slim_v2_dictionary_candidates(events)
        .into_iter()
        .filter_map(|candidate| {
            runtime_mem_super_slim_v2_candidate_savings(events, &candidate)
                .map(|savings| (candidate, savings))
        })
        .max_by_key(|(candidate, savings)| {
            (
                *savings,
                candidate.event_indexes.len(),
                candidate.value.len(),
            )
        })
        .map(|(candidate, _)| candidate)
}

pub(crate) fn runtime_mem_super_slim_v2_candidate_savings(
    events: &[Value],
    candidate: &RuntimeMemSuperSlimV2DictionaryCandidate,
) -> Option<usize> {
    let dictionary_index =
        runtime_mem_super_slim_v2_next_dictionary_index(events, &candidate.field);
    let dictionary_event_len =
        runtime_mem_json_value_len(&runtime_mem_super_slim_v2_dictionary_event(
            &candidate.field,
            dictionary_index,
            candidate.mode,
            &candidate.value,
        ))
        .saturating_add(1);
    if dictionary_event_len == usize::MAX {
        return None;
    }

    let mut field_savings = 0usize;
    for event_index in &candidate.event_indexes {
        let Some(event) = events.get(*event_index) else {
            continue;
        };
        let Some(value) = event.get(&candidate.field).and_then(Value::as_str) else {
            continue;
        };
        let compacted = match candidate.placement {
            RuntimeMemSuperSlimV2DictionaryPlacement::WholeField => {
                candidate.compact_ref(dictionary_index, value)
            }
            RuntimeMemSuperSlimV2DictionaryPlacement::Inline => {
                let compact_ref = format!(
                    "{{{}}}",
                    runtime_mem_super_slim_v2_dictionary_ref(&candidate.field, dictionary_index)
                );
                let compacted = value.replace(candidate.value.as_str(), &compact_ref);
                (compacted != value).then_some(compacted)
            }
        };
        let Some(compacted) = compacted else {
            continue;
        };

        let original_len = runtime_mem_json_value_len(event).saturating_add(1);
        let compacted_len = original_len
            .saturating_sub(runtime_mem_json_string_len(value))
            .saturating_add(runtime_mem_json_string_len(&compacted));
        if compacted_len < original_len {
            field_savings = field_savings.saturating_add(original_len - compacted_len);
        }
    }

    field_savings
        .checked_sub(dictionary_event_len)
        .filter(|savings| *savings > 0)
}

pub(crate) fn runtime_mem_super_slim_v2_dictionary_candidates(
    events: &[Value],
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let mut candidates = Vec::new();
    for field in ["i", "n", "c", "r", "s"] {
        candidates.extend(runtime_mem_super_slim_v2_exact_dictionary_candidates(
            events, field,
        ));
    }
    for field in ["i", "c", "r", "s"] {
        candidates.extend(runtime_mem_super_slim_v2_prefix_dictionary_candidates(
            events, field,
        ));
    }
    for field in ["c", "r", "s"] {
        candidates.extend(runtime_mem_super_slim_v2_inline_dictionary_candidates(
            events, field,
        ));
    }
    candidates
}

fn runtime_mem_super_slim_v2_exact_dictionary_candidates(
    events: &[Value],
    field: &str,
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let mut values = HashMap::<String, Vec<usize>>::new();
    for (event_index, value) in runtime_mem_super_slim_v2_field_strings(events, field) {
        values.entry(value).or_default().push(event_index);
    }
    values
        .into_iter()
        .filter(|(_, event_indexes)| event_indexes.len() > 1)
        .map(
            |(value, event_indexes)| RuntimeMemSuperSlimV2DictionaryCandidate {
                field: field.to_string(),
                mode: RuntimeMemSuperSlimV2DictionaryMode::Exact,
                placement: RuntimeMemSuperSlimV2DictionaryPlacement::WholeField,
                value,
                event_indexes,
            },
        )
        .collect()
}

fn runtime_mem_super_slim_v2_prefix_dictionary_candidates(
    events: &[Value],
    field: &str,
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let values = runtime_mem_super_slim_v2_field_strings(events, field);
    let mut prefixes = HashSet::<String>::new();
    for left_index in 0..values.len() {
        for right_index in (left_index + 1)..values.len() {
            let Some(prefix) =
                runtime_mem_common_char_prefix(&values[left_index].1, &values[right_index].1)
            else {
                continue;
            };
            prefixes.insert(prefix);
        }
    }

    prefixes
        .into_iter()
        .filter_map(|prefix| {
            let event_indexes = values
                .iter()
                .filter(|(_, value)| value.starts_with(&prefix) && value.len() > prefix.len())
                .map(|(event_index, _)| *event_index)
                .collect::<Vec<_>>();
            (event_indexes.len() > 1).then_some(RuntimeMemSuperSlimV2DictionaryCandidate {
                field: field.to_string(),
                mode: RuntimeMemSuperSlimV2DictionaryMode::Prefix,
                placement: RuntimeMemSuperSlimV2DictionaryPlacement::WholeField,
                value: prefix,
                event_indexes,
            })
        })
        .collect()
}

fn runtime_mem_super_slim_v2_inline_dictionary_candidates(
    events: &[Value],
    field: &str,
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let values = runtime_mem_super_slim_v2_inline_field_strings(events, field);
    let mut term_events = HashMap::<String, Vec<usize>>::new();
    let mut term_occurrences = HashMap::<String, usize>::new();
    for (event_index, value) in &values {
        for term in runtime_mem_super_slim_v2_inline_dictionary_terms(value) {
            let occurrences = value.matches(term.as_str()).count();
            if occurrences == 0 {
                continue;
            }
            let events = term_events.entry(term.clone()).or_default();
            if !events.iter().any(|seen_index| seen_index == event_index) {
                events.push(*event_index);
            }
            *term_occurrences.entry(term).or_default() += occurrences;
        }
    }
    term_events
        .into_iter()
        .filter(|(term, event_indexes)| {
            term_occurrences.get(term).copied().unwrap_or_default() > 1 && !event_indexes.is_empty()
        })
        .map(
            |(value, event_indexes)| RuntimeMemSuperSlimV2DictionaryCandidate {
                field: field.to_string(),
                mode: RuntimeMemSuperSlimV2DictionaryMode::Exact,
                placement: RuntimeMemSuperSlimV2DictionaryPlacement::Inline,
                value,
                event_indexes,
            },
        )
        .collect()
}

fn runtime_mem_super_slim_v2_field_strings(events: &[Value], field: &str) -> Vec<(usize, String)> {
    events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let event_type = runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)?;
            if !runtime_mem_super_slim_v2_field_can_use_dictionary(event_type, field) {
                return None;
            }
            let value = event.get(field)?.as_str()?;
            if value.is_empty()
                || runtime_mem_super_slim_v2_contains_dictionary_ref_marker(value)
                || runtime_mem_super_slim_v2_parse_dictionary_ref(value).is_some()
            {
                return None;
            }
            Some((event_index, value.to_string()))
        })
        .collect()
}

fn runtime_mem_super_slim_v2_inline_field_strings(
    events: &[Value],
    field: &str,
) -> Vec<(usize, String)> {
    events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let event_type = runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)?;
            if !runtime_mem_super_slim_v2_field_can_use_dictionary(event_type, field) {
                return None;
            }
            let value = event.get(field)?.as_str()?;
            if value.is_empty() || runtime_mem_super_slim_v2_parse_dictionary_ref(value).is_some() {
                return None;
            }
            Some((event_index, value.to_string()))
        })
        .collect()
}

fn runtime_mem_super_slim_v2_field_can_use_dictionary(event_type: &str, field: &str) -> bool {
    match event_type {
        RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE => matches!(field, "i" | "n" | "c"),
        RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE => matches!(field, "i" | "r" | "s"),
        RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE => matches!(field, "r" | "s"),
        RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE => field == "s",
        _ => false,
    }
}

fn runtime_mem_push_dictionary_term(terms: &mut Vec<String>, term: String) {
    let term = term.trim();
    if term.len() < RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS
        || runtime_mem_super_slim_v2_contains_dictionary_ref_marker(term)
        || terms.iter().any(|existing| existing == term)
    {
        return;
    }
    terms.push(term.to_string());
}

pub(crate) fn runtime_mem_super_slim_v2_apply_dictionary_candidate(
    mut events: Vec<Value>,
    candidate: &RuntimeMemSuperSlimV2DictionaryCandidate,
) -> Vec<Value> {
    let Some(insert_at) = candidate.event_indexes.iter().copied().min() else {
        return events;
    };
    let dictionary_index =
        runtime_mem_super_slim_v2_next_dictionary_index(&events, &candidate.field);

    for event_index in &candidate.event_indexes {
        let Some(value) = events
            .get(*event_index)
            .and_then(|event| event.get(&candidate.field))
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let compacted = match candidate.placement {
            RuntimeMemSuperSlimV2DictionaryPlacement::WholeField => {
                candidate.compact_ref(dictionary_index, &value)
            }
            RuntimeMemSuperSlimV2DictionaryPlacement::Inline => {
                let compact_ref = format!(
                    "{{{}}}",
                    runtime_mem_super_slim_v2_dictionary_ref(&candidate.field, dictionary_index)
                );
                let compacted = value.replace(candidate.value.as_str(), &compact_ref);
                (compacted != value).then_some(compacted)
            }
        };
        if let Some(compacted) = compacted
            && let Some(object) = events.get_mut(*event_index).and_then(Value::as_object_mut)
        {
            object.insert(candidate.field.clone(), Value::String(compacted));
        }
    }

    events.insert(
        insert_at,
        runtime_mem_super_slim_v2_dictionary_event(
            &candidate.field,
            dictionary_index,
            candidate.mode,
            &candidate.value,
        ),
    );
    events
}

fn runtime_mem_super_slim_v2_next_dictionary_index(events: &[Value], field: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "k").and_then(Value::as_str) == Some(field)
        })
        .filter_map(|event| {
            runtime_mem_lookup_json_path(event, "i")
                .and_then(runtime_mem_super_slim_v2_dictionary_index)
        })
        .max()
        .map_or(0, |index| index + 1)
}

fn runtime_mem_super_slim_v2_dictionary_event(
    field: &str,
    dictionary_index: usize,
    mode: RuntimeMemSuperSlimV2DictionaryMode,
    value: &str,
) -> Value {
    let mut event = runtime_mem_short_shadow_event(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE);
    let index = dictionary_index.to_string();
    event.insert("k".to_string(), Value::String(field.to_string()));
    event.insert("i".to_string(), Value::String(index.clone()));
    event.insert("m".to_string(), Value::String(mode.as_str().to_string()));
    event.insert("v".to_string(), Value::String(value.to_string()));
    event.insert(
        "s".to_string(),
        Value::String(format!("dict {field}#{index} {}={value}", mode.as_str())),
    );
    Value::Object(event)
}

fn runtime_mem_super_slim_v2_dictionary_ref(field: &str, dictionary_index: usize) -> String {
    format!("{RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX}{field}#{dictionary_index}")
}

pub(super) fn runtime_mem_super_slim_v2_parse_dictionary_ref(
    value: &str,
) -> Option<RuntimeMemSuperSlimV2DictionaryRef<'_>> {
    let rest = value
        .strip_prefix(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX)
        .or_else(|| value.strip_prefix(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY))?;
    let (field, index_and_suffix) = rest.split_once('#')?;
    if field.is_empty() || !field.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return None;
    }
    let (index, suffix) = index_and_suffix
        .split_once('+')
        .map_or((index_and_suffix, None), |(index, suffix)| {
            (index, Some(suffix))
        });
    let index = index.parse::<usize>().ok()?;
    Some(RuntimeMemSuperSlimV2DictionaryRef {
        field,
        index,
        suffix,
    })
}

fn runtime_mem_super_slim_v2_contains_dictionary_ref_marker(value: &str) -> bool {
    value.contains(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX)
        || value.contains(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY)
}

pub(super) fn runtime_mem_super_slim_v2_dictionary_index(value: &Value) -> Option<usize> {
    value
        .as_str()
        .and_then(|value| value.parse::<usize>().ok())
        .or_else(|| value.as_u64().and_then(|value| usize::try_from(value).ok()))
}

#[cfg(test)]
pub(crate) fn runtime_mem_jsonl_events_len(events: &[Value]) -> usize {
    events
        .iter()
        .map(|event| runtime_mem_json_value_len(event).saturating_add(1))
        .sum()
}

pub(super) fn runtime_mem_super_slim_v2_emitted_artifact_ref(event: &Value) -> Option<String> {
    let event_type = runtime_mem_lookup_json_path(event, "t")?.as_str()?;
    if !matches!(
        event_type,
        RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE
            | RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE
    ) {
        return None;
    }
    runtime_mem_lookup_json_path(event, "r")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn runtime_mem_common_char_prefix(left: &str, right: &str) -> Option<String> {
    let mut prefix_len = 0usize;
    for ((left_index, left_char), (right_index, right_char)) in
        left.char_indices().zip(right.char_indices())
    {
        if left_char != right_char {
            break;
        }
        prefix_len = left_index + left_char.len_utf8();
        if left_index != right_index {
            break;
        }
    }
    if prefix_len < RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        return None;
    }
    Some(left[..prefix_len].to_string())
}

fn runtime_mem_json_value_len(value: &Value) -> usize {
    serde_json::to_string(value).map_or(usize::MAX, |value| value.len())
}

fn runtime_mem_json_string_len(value: &str) -> usize {
    let mut len = 2usize;
    for ch in value.chars() {
        len = len.saturating_add(match ch {
            '"' | '\\' => 2,
            '\u{08}' | '\u{0c}' | '\n' | '\r' | '\t' => 2,
            '\u{00}'..='\u{1f}' => 6,
            _ => ch.len_utf8(),
        });
    }
    len
}

pub(crate) fn runtime_mem_value_is_text_or_v2_intern_marker(value: &Value) -> bool {
    runtime_mem_value_is_text(value)
        || value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_INTERN_REF_FIELD)
            .and_then(Value::as_u64)
            .is_some()
        || value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD)
            .and_then(Value::as_array)
            .is_some_and(|marker| {
                marker.first().and_then(Value::as_u64).is_some()
                    && marker.get(1).and_then(Value::as_str).is_some()
            })
}

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
    RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD, RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD,
    RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER,
    RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
    runtime_mem_lookup_json_path,
};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct RuntimeMemSuperSlimV2ArtifactRefDedupeState {
    previous_emitted_ref: Option<String>,
}

impl RuntimeMemSuperSlimV2ArtifactRefDedupeState {
    pub(crate) fn dedupe_consecutive_event_ref(&mut self, mut event: Value) -> Value {
        let Some(artifact_ref) = runtime_mem_super_slim_v2_emitted_artifact_ref(&event) else {
            self.previous_emitted_ref = None;
            return event;
        };

        if self.previous_emitted_ref.as_deref() == Some(artifact_ref.as_str()) {
            if let Some(object) = event.as_object_mut() {
                object.remove("r");
                object.insert(
                    RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD.to_string(),
                    Value::String(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER.to_string()),
                );
            }
        } else {
            self.previous_emitted_ref = Some(artifact_ref);
        }

        event
    }
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeMemSuperSlimV2InternState {
    exact_values: HashMap<String, Vec<String>>,
    prefix_values: HashMap<String, Vec<String>>,
    previous_full_values: HashMap<String, Vec<String>>,
    dictionary_values: HashMap<String, HashMap<usize, RuntimeMemSuperSlimV2DictionaryEntry>>,
}

impl RuntimeMemSuperSlimV2InternState {
    pub(crate) fn expand_event(&mut self, mut event: Value) -> Option<Value> {
        let Some(event_type) = runtime_mem_lookup_json_path(&event, "t").and_then(Value::as_str)
        else {
            return Some(event);
        };

        if event_type == RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE {
            self.remember_dictionary_event(&event);
            return None;
        }

        let fields: &[&str] = match event_type {
            RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE => &["i", "n", "c"],
            RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE => &["i", "s", "r"],
            RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE => &["s", "r"],
            RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE => &["s"],
            _ => &[],
        };
        for field in fields {
            self.expand_field(&mut event, field);
        }
        Some(event)
    }

    fn expand_field(&mut self, event: &mut Value, field: &str) {
        let Some(raw_value) = event.get(field).cloned() else {
            return;
        };
        if let Some(value) = raw_value.as_str() {
            if let Some(resolved) = self.resolve_dictionary_ref(field, value) {
                if let Some(object) = event.as_object_mut() {
                    object.insert(field.to_string(), Value::String(resolved.clone()));
                }
                self.remember_resolved(field, &resolved);
            } else if let Some(resolved) = self.resolve_inline_dictionary_refs(field, value) {
                if let Some(object) = event.as_object_mut() {
                    object.insert(field.to_string(), Value::String(resolved.clone()));
                }
                self.remember_resolved(field, &resolved);
            } else if runtime_mem_super_slim_v2_parse_dictionary_ref(value).is_none() {
                self.remember_resolved(field, value);
            }
            return;
        }

        let Some(value) = self.resolve_marker(field, &raw_value) else {
            return;
        };

        if let Some(object) = event.as_object_mut() {
            object.insert(field.to_string(), Value::String(value.clone()));
        }
        self.remember_resolved(field, &value);
    }

    fn resolve_marker(&self, field: &str, value: &Value) -> Option<String> {
        if let Some(index) = value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_INTERN_REF_FIELD)
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
        {
            return self
                .exact_values
                .get(field)
                .and_then(|values| values.get(index))
                .cloned();
        }

        let marker = value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD)
            .and_then(Value::as_array)?;
        let index = marker
            .first()
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())?;
        let suffix = marker.get(1).and_then(Value::as_str)?;
        self.prefix_values
            .get(field)
            .and_then(|prefixes| prefixes.get(index))
            .map(|prefix| format!("{prefix}{suffix}"))
    }

    fn remember_dictionary_event(&mut self, event: &Value) {
        let Some(field) = runtime_mem_lookup_json_path(event, "k")
            .and_then(Value::as_str)
            .filter(|field| !field.is_empty())
        else {
            return;
        };
        let Some(index) = runtime_mem_lookup_json_path(event, "i")
            .and_then(runtime_mem_super_slim_v2_dictionary_index)
        else {
            return;
        };
        let Some(mode) = runtime_mem_lookup_json_path(event, "m")
            .and_then(Value::as_str)
            .and_then(RuntimeMemSuperSlimV2DictionaryMode::from_str)
        else {
            return;
        };
        let Some(value) = runtime_mem_lookup_json_path(event, "v")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        self.dictionary_values
            .entry(field.to_string())
            .or_default()
            .insert(
                index,
                RuntimeMemSuperSlimV2DictionaryEntry {
                    mode,
                    value: value.to_string(),
                },
            );
    }

    fn resolve_dictionary_ref(&self, field: &str, value: &str) -> Option<String> {
        let reference = runtime_mem_super_slim_v2_parse_dictionary_ref(value)?;
        if reference.field != field {
            return None;
        }
        let entry = self
            .dictionary_values
            .get(field)
            .and_then(|entries| entries.get(&reference.index))?;
        match (entry.mode, reference.suffix) {
            (RuntimeMemSuperSlimV2DictionaryMode::Exact, None) => Some(entry.value.clone()),
            (RuntimeMemSuperSlimV2DictionaryMode::Prefix, Some(suffix)) => {
                Some(format!("{}{suffix}", entry.value))
            }
            (RuntimeMemSuperSlimV2DictionaryMode::Prefix, None) => Some(entry.value.clone()),
            (RuntimeMemSuperSlimV2DictionaryMode::Exact, Some(_)) => None,
        }
    }

    fn resolve_inline_dictionary_refs(&self, field: &str, value: &str) -> Option<String> {
        let marker_prefixes = [
            format!("{{{RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX}{field}#"),
            format!("{{{RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY}{field}#"),
        ];
        let mut output = String::new();
        let mut cursor = 0usize;
        let mut changed = false;
        while let Some((start, marker_len)) = marker_prefixes
            .iter()
            .filter_map(|marker_prefix| {
                value[cursor..]
                    .find(marker_prefix)
                    .map(|relative_start| (cursor + relative_start, marker_prefix.len()))
            })
            .min_by_key(|(start, _)| *start)
        {
            output.push_str(&value[cursor..start]);
            let digits_start = start + marker_len;
            let mut digits_end = digits_start;
            for (offset, ch) in value[digits_start..].char_indices() {
                if !ch.is_ascii_digit() {
                    break;
                }
                digits_end = digits_start + offset + ch.len_utf8();
            }
            if digits_end == digits_start || !value[digits_end..].starts_with('}') {
                output.push_str(&value[start..digits_end]);
                cursor = digits_end;
                continue;
            }
            let Some(index) = value[digits_start..digits_end].parse::<usize>().ok() else {
                output.push_str(&value[start..digits_end]);
                cursor = digits_end;
                continue;
            };
            let Some(entry) = self
                .dictionary_values
                .get(field)
                .and_then(|entries| entries.get(&index))
                .filter(|entry| entry.mode == RuntimeMemSuperSlimV2DictionaryMode::Exact)
            else {
                output.push_str(&value[start..digits_end]);
                cursor = digits_end;
                continue;
            };
            output.push_str(&entry.value);
            cursor = digits_end + 1;
            changed = true;
        }
        if !changed {
            return None;
        }
        output.push_str(&value[cursor..]);
        Some(output)
    }

    fn remember_resolved(&mut self, field: &str, value: &str) {
        self.remember_exact(field, value);
        self.remember_prefixes(field, value);
        self.previous_full_values
            .entry(field.to_string())
            .or_default()
            .push(value.to_string());
    }

    fn remember_exact(&mut self, field: &str, value: &str) {
        let values = self.exact_values.entry(field.to_string()).or_default();
        if !values.iter().any(|candidate| candidate == value) {
            values.push(value.to_string());
        }
    }

    fn remember_prefixes(&mut self, field: &str, value: &str) {
        let Some(previous_values) = self.previous_full_values.get(field) else {
            return;
        };
        let learned = previous_values
            .iter()
            .filter_map(|previous| runtime_mem_common_char_prefix(previous, value))
            .collect::<Vec<_>>();
        if learned.is_empty() {
            return;
        }
        let prefixes = self.prefix_values.entry(field.to_string()).or_default();
        for prefix in learned {
            if !prefixes.iter().any(|candidate| candidate == &prefix) {
                prefixes.push(prefix);
            }
        }
    }
}

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

    fn from_str(value: &str) -> Option<Self> {
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
struct RuntimeMemSuperSlimV2DictionaryEntry {
    mode: RuntimeMemSuperSlimV2DictionaryMode,
    value: String,
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
struct RuntimeMemSuperSlimV2DictionaryRef<'a> {
    field: &'a str,
    index: usize,
    suffix: Option<&'a str>,
}

pub(crate) fn runtime_mem_super_slim_v2_compact_dictionary_events(
    mut events: Vec<Value>,
) -> Vec<Value> {
    while let Some(candidate) = runtime_mem_super_slim_v2_best_dictionary_candidate(&events) {
        let compacted =
            runtime_mem_super_slim_v2_apply_dictionary_candidate(events.clone(), &candidate);
        if runtime_mem_jsonl_events_len(&compacted) >= runtime_mem_jsonl_events_len(&events) {
            break;
        }
        events = compacted;
    }
    events
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
        let mut compacted_event = event.clone();
        let Some(object) = compacted_event.as_object_mut() else {
            continue;
        };
        object.insert(candidate.field.clone(), Value::String(compacted));
        let compacted_len = runtime_mem_json_value_len(&compacted_event).saturating_add(1);
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
    let mut prefixes = Vec::<String>::new();
    for left_index in 0..values.len() {
        for right_index in (left_index + 1)..values.len() {
            let Some(prefix) =
                runtime_mem_common_char_prefix(&values[left_index].1, &values[right_index].1)
            else {
                continue;
            };
            if !prefixes.iter().any(|candidate| candidate == &prefix) {
                prefixes.push(prefix);
            }
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

fn runtime_mem_super_slim_v2_inline_dictionary_terms(value: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for token in runtime_mem_super_slim_v2_dictionary_tokens(value) {
        for term in runtime_mem_super_slim_v2_token_dictionary_terms(&token) {
            runtime_mem_push_dictionary_term(&mut terms, term);
        }
    }
    for term in runtime_mem_super_slim_v2_package_terms(value) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    terms
}

fn runtime_mem_super_slim_v2_dictionary_tokens(value: &str) -> Vec<String> {
    value
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
                )
        })
        .filter_map(|token| {
            let token = runtime_mem_super_slim_v2_trim_dictionary_token(token);
            (!token.is_empty()).then(|| token.to_string())
        })
        .collect()
}

fn runtime_mem_super_slim_v2_trim_dictionary_token(token: &str) -> &str {
    token
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
        })
        .trim_end_matches(['.', ',', ';', '!', '?'])
}

fn runtime_mem_super_slim_v2_token_dictionary_terms(token: &str) -> Vec<String> {
    let mut terms = Vec::new();
    if runtime_mem_super_slim_v2_contains_dictionary_ref_marker(token) {
        return terms;
    }
    if let Some(term) = runtime_mem_super_slim_v2_temp_path_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    for term in runtime_mem_super_slim_v2_url_terms(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    if let Some(term) = runtime_mem_super_slim_v2_identity_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    if let Some(term) = runtime_mem_super_slim_v2_stack_prefix_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    terms
}

fn runtime_mem_super_slim_v2_temp_path_term(token: &str) -> Option<String> {
    let token = token.trim_end_matches(':');
    if ![
        "/tmp/",
        "/private/tmp/",
        "/var/tmp/",
        "/var/folders/",
        "/run/user/",
        "/dev/shm/",
    ]
    .iter()
    .any(|prefix| token.starts_with(prefix))
    {
        return None;
    }
    runtime_mem_super_slim_v2_path_prefix_term(token).or_else(|| {
        (token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| token.to_string())
    })
}

fn runtime_mem_super_slim_v2_path_prefix_term(token: &str) -> Option<String> {
    let slash_index = token.rfind('/')?;
    if slash_index == 0 {
        return None;
    }
    let prefix = &token[..=slash_index];
    (prefix.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| prefix.to_string())
}

fn runtime_mem_super_slim_v2_url_terms(token: &str) -> Vec<String> {
    if !token.starts_with("https://") && !token.starts_with("http://") {
        return Vec::new();
    }
    let mut terms = Vec::new();
    let token = token.trim_end_matches('/');
    if token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        runtime_mem_push_dictionary_term(&mut terms, token.to_string());
    }
    let scheme_end = token.find("://").map(|index| index + 3).unwrap_or_default();
    if let Some(path_start) = token[scheme_end..].find('/') {
        let origin_end = scheme_end + path_start;
        let origin = &token[..origin_end];
        if origin.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
            runtime_mem_push_dictionary_term(&mut terms, origin.to_string());
        }
        if let Some(prefix) = runtime_mem_super_slim_v2_path_prefix_term(token) {
            runtime_mem_push_dictionary_term(&mut terms, prefix);
        }
    }
    terms
}

fn runtime_mem_super_slim_v2_identity_term(token: &str) -> Option<String> {
    let token = token
        .trim_end_matches(':')
        .trim_start_matches("profile=")
        .trim_start_matches("account=")
        .trim_start_matches("ref=")
        .trim_start_matches("branch=");
    if token.len() < RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        return None;
    }
    let lower = token.to_ascii_lowercase();
    let profile_like = [
        "profile-",
        "profile_",
        "prodex-profile-",
        "codex-profile-",
        "account-",
        "account_",
        "acct-",
        "acct_",
        "org-",
        "org_",
        "proj-",
        "proj_",
        "refs/heads/",
        "refs/remotes/",
        "refs/tags/",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let branch_like = token.contains('/')
        && [
            "origin/",
            "upstream/",
            "feature/",
            "bugfix/",
            "hotfix/",
            "release/",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
    let git_hash = token.len() >= 12 && token.chars().all(|ch| ch.is_ascii_hexdigit());
    (profile_like || branch_like || git_hash).then(|| token.to_string())
}

fn runtime_mem_super_slim_v2_stack_prefix_term(token: &str) -> Option<String> {
    if token.matches("::").count() < 2 {
        return None;
    }
    let prefix_end = token.rfind("::")? + 2;
    let prefix = &token[..prefix_end];
    (prefix.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| prefix.to_string())
}

fn runtime_mem_super_slim_v2_package_terms(value: &str) -> Vec<String> {
    let tokens = runtime_mem_super_slim_v2_dictionary_tokens(value);
    let mut terms = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let token = token
            .trim_start_matches("crate=")
            .trim_start_matches("package=")
            .trim_start_matches("pkg=");
        let previous = index.checked_sub(1).and_then(|index| tokens.get(index));
        let package_context = previous.is_some_and(|previous| {
            matches!(
                previous.as_str(),
                "-p" | "--package" | "--pkg" | "--crate" | "crate" | "package" | "pkg"
            )
        }) || value.contains(&format!("crate {token}"))
            || value.contains(&format!("package {token}"));
        let scoped_package = token.starts_with('@') && token.contains('/');
        let crate_like = package_context
            && token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS
            && token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '/' | '@'));
        if scoped_package || crate_like {
            runtime_mem_push_dictionary_term(&mut terms, token.to_string());
        }
    }
    terms
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

fn runtime_mem_super_slim_v2_parse_dictionary_ref(
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

fn runtime_mem_super_slim_v2_dictionary_index(value: &Value) -> Option<usize> {
    value
        .as_str()
        .and_then(|value| value.parse::<usize>().ok())
        .or_else(|| value.as_u64().and_then(|value| usize::try_from(value).ok()))
}

pub(crate) fn runtime_mem_jsonl_events_len(events: &[Value]) -> usize {
    events
        .iter()
        .map(|event| runtime_mem_json_value_len(event).saturating_add(1))
        .sum()
}

fn runtime_mem_super_slim_v2_emitted_artifact_ref(event: &Value) -> Option<String> {
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

fn runtime_mem_common_char_prefix(left: &str, right: &str) -> Option<String> {
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

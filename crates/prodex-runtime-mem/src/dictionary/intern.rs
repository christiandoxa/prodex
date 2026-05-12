use serde_json::Value;
use std::collections::HashMap;

use crate::{
    RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX,
    RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY,
    RUNTIME_MEM_SUPER_SLIM_V2_INTERN_REF_FIELD, RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD,
    RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
    runtime_mem_lookup_json_path,
};

use super::{
    RuntimeMemSuperSlimV2DictionaryEntry, RuntimeMemSuperSlimV2DictionaryMode,
    runtime_mem_common_char_prefix, runtime_mem_super_slim_v2_dictionary_index,
    runtime_mem_super_slim_v2_parse_dictionary_ref,
};

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

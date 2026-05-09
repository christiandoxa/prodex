use super::*;
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn capsule(
    id: &str,
    token_cost: usize,
    required: bool,
    project_path: Option<&str>,
    updated_at_seconds: Option<i64>,
    relevance: f32,
) -> RuntimeMemCapsuleMetadata {
    RuntimeMemCapsuleMetadata {
        id: id.to_string(),
        token_cost,
        required,
        project_path: project_path.map(PathBuf::from),
        updated_at_seconds,
        relevance,
    }
}

fn recall_capsule(
    capsule: RuntimeMemCapsuleMetadata,
    paths: &[&str],
    symbols: &[&str],
) -> RuntimeMemRecallCapsuleMetadata {
    RuntimeMemRecallCapsuleMetadata {
        capsule,
        paths: paths.iter().map(PathBuf::from).collect(),
        symbols: symbols.iter().map(|symbol| symbol.to_string()).collect(),
    }
}
#[path = "lib/capsule_selection.rs"]
mod capsule_selection;

#[path = "lib/schema_modes.rs"]
mod schema_modes;

#[path = "lib/shadow_events.rs"]
mod shadow_events;

fn test_v2_shadow_events_without_dictionary<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
    let mut ref_dedupe_state = RuntimeMemSuperSlimV2ArtifactRefDedupeState::default();
    runtime_mem_super_slim_shadow_codex_events(events)
        .iter()
        .map(runtime_mem_super_slim_v2_shadow_from_v1_shadow)
        .map(|event| ref_dedupe_state.dedupe_consecutive_event_ref(event))
        .collect()
}

fn test_v2_dictionary_events(events: &[Value]) -> Vec<&Value> {
    events
        .iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .collect()
}

fn test_expanded_non_dictionary_events(events: Vec<Value>) -> Vec<Value> {
    runtime_mem_super_slim_v2_expand_interned_events(events)
        .into_iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                != Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .collect()
}

fn schema_user_prompt_field(schema: &Value) -> Option<&Value> {
    schema_event_field(schema, "user-message", "prompt")
}

fn schema_assistant_message_field(schema: &Value) -> Option<&Value> {
    schema_event_field(schema, "assistant-message", "message")
}

fn schema_tool_response_field(schema: &Value) -> Option<&Value> {
    schema_event_field(schema, "tool-result", "toolResponse")
}

fn schema_event_field<'a>(
    schema: &'a Value,
    event_name: &str,
    field_name: &str,
) -> Option<&'a Value> {
    schema
        .get("events")?
        .as_array()?
        .iter()
        .find(|event| event.get("name").and_then(Value::as_str) == Some(event_name))?
        .get("fields")?
        .get(field_name)
}

fn resolve_schema_user_prompt(schema: &Value, entry: &Value) -> Option<String> {
    let field = schema_user_prompt_field(schema)?;
    resolve_test_field(field, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_schema_assistant_message(schema: &Value, entry: &Value) -> Option<String> {
    let field = schema_assistant_message_field(schema)?;
    resolve_test_field(field, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_schema_tool_response(schema: &Value, entry: &Value) -> Option<String> {
    let field = schema_tool_response_field(schema)?;
    resolve_test_field(field, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_schema_event_string(
    schema: &Value,
    event_name: &str,
    field_name: &str,
    entry: &Value,
) -> Option<String> {
    let field = schema_event_field(schema, event_name, field_name)?;
    resolve_test_field(field, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_test_field(spec: &Value, entry: &Value) -> Option<Value> {
    if let Some(path) = spec.as_str() {
        return lookup_test_path(entry, path).cloned();
    }
    if let Some(value) = spec.get("value") {
        return Some(value.clone());
    }
    if let Some(path) = spec.get("path").and_then(Value::as_str) {
        return lookup_test_path(entry, path).cloned();
    }
    if let Some(coalesce) = spec.get("coalesce").and_then(Value::as_array) {
        for candidate in coalesce {
            if let Some(value) = resolve_test_field(candidate, entry)
                && !value.as_str().is_some_and(str::is_empty)
            {
                return Some(value);
            }
        }
    }
    None
}

fn lookup_test_path<'a>(entry: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = entry;
    for part in path.split('.') {
        current = lookup_test_path_part(current, part)?;
    }
    Some(current)
}

fn lookup_test_path_part<'a>(value: &'a Value, part: &str) -> Option<&'a Value> {
    let mut current = value;
    let mut rest = part;
    if let Some(bracket_index) = rest.find('[') {
        if bracket_index > 0 {
            current = current.get(&rest[..bracket_index])?;
        }
        rest = &rest[bracket_index..];
    } else {
        return current.get(rest);
    }
    while !rest.is_empty() {
        let inner = rest.strip_prefix('[')?;
        let close_index = inner.find(']')?;
        let index = inner[..close_index].parse::<usize>().ok()?;
        current = current.get(index)?;
        rest = &inner[close_index + 1..];
        if !rest.is_empty() && !rest.starts_with('[') {
            return None;
        }
    }
    Some(current)
}

use super::*;

#[path = "lib/compact_v2_runtime_memory/dictionary.rs"]
mod dictionary;
#[path = "lib/compact_v2_runtime_memory/schema_refs.rs"]
mod schema_refs;

fn v2_shadow_events_without_dictionary<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
    let mut ref_dedupe_state = RuntimeMemSuperSlimV2ArtifactRefDedupeState::default();
    runtime_mem_super_slim_shadow_codex_events(events)
        .iter()
        .map(runtime_mem_super_slim_v2_shadow_from_v1_shadow)
        .map(|event| ref_dedupe_state.dedupe_consecutive_event_ref(event))
        .collect()
}

fn v2_dictionary_events(events: &[Value]) -> Vec<&Value> {
    events
        .iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .collect()
}

fn v2_tool_use_events(events: &[Value]) -> Vec<&Value> {
    v2_events_of_type(events, RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE)
}

fn v2_tool_events_with_ids(events: &[Value]) -> Vec<&Value> {
    events
        .iter()
        .filter(|event| {
            matches!(
                runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str),
                Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE)
                    | Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE)
            )
        })
        .collect()
}

fn v2_user_events(events: &[Value]) -> Vec<&Value> {
    v2_events_of_type(events, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE)
}

fn v2_events_of_type<'a>(events: &'a [Value], event_type: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str) == Some(event_type)
        })
        .collect()
}

fn expanded_non_dictionary_events(events: Vec<Value>) -> Vec<Value> {
    runtime_mem_super_slim_v2_expand_interned_events(events)
        .into_iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                != Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .collect()
}

fn assert_v2_raw_events_schema_addressable(events: &[Value]) {
    for event in events {
        match runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str) {
            Some(RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings("prodex-v2-user-message", event, &["prompt"]);
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-assistant-message",
                    event,
                    &["message"],
                );
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-tool-use",
                    event,
                    &["toolId", "toolName", "toolInput"],
                );
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-tool-result",
                    event,
                    &["toolId", "toolResponse"],
                );
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-dictionary-entry",
                    event,
                    &[
                        "dictionary",
                        "dictionaryKey",
                        "dictionaryIndex",
                        "dictionaryMode",
                        "dictionaryValue",
                    ],
                );
            }
            _ => {}
        }
    }
}

fn assert_v2_schema_fields_are_strings(event_name: &str, event: &Value, field_names: &[&str]) {
    let fields = v2_schema_fields(event_name);
    for field_name in field_names {
        let value = resolve_v2_schema_string(&fields[*field_name], event)
            .unwrap_or_else(|| panic!("{event_name}.{field_name} should resolve: {event}"));
        assert!(
            !value.trim().is_empty(),
            "{event_name}.{field_name} should be meaningful: {event}"
        );
    }
}

fn assert_v2_compact_fields_are_strings(events: &[Value]) {
    for event in events {
        for field in [
            "i",
            "n",
            "c",
            "r",
            "s",
            "p",
            "k",
            "m",
            "v",
            RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD,
        ] {
            if let Some(value) = event.get(field) {
                assert!(value.is_string(), "{field} should stay string: {event}");
            }
        }
    }
}

fn v2_schema_fields(event_name: &str) -> Value {
    runtime_mem_super_slim_codex_schema()
        .get("events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.get("name").and_then(Value::as_str) == Some(event_name))
        })
        .and_then(|event| event.get("fields"))
        .cloned()
        .expect("v2 schema fields should exist")
}

fn resolve_v2_schema_string(spec: &Value, entry: &Value) -> Option<String> {
    resolve_v2_schema_field(spec, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_v2_schema_field(spec: &Value, entry: &Value) -> Option<Value> {
    if let Some(path) = spec.as_str() {
        return runtime_mem_lookup_json_path(entry, path).cloned();
    }
    if let Some(value) = spec.get("value") {
        return Some(value.clone());
    }
    if let Some(coalesce) = spec.get("coalesce").and_then(Value::as_array) {
        for candidate in coalesce {
            if let Some(value) = resolve_v2_schema_field(candidate, entry)
                && !value.as_str().is_some_and(str::is_empty)
            {
                return Some(value);
            }
        }
    }
    None
}

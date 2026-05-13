use super::{
    RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS, RuntimeSmartContextStaticContextObservation,
    SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX,
    SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX,
    SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY,
    SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX,
    SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX,
};
use std::collections::BTreeMap;

pub(super) fn runtime_smart_context_static_context_dup_marker(source_id: &str) -> String {
    format!("{SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX}{source_id}")
}

pub(super) fn runtime_smart_context_static_context_chunk_dup_marker(
    source_id: &str,
    content_hash: &str,
) -> String {
    format!("{SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX}{source_id} {content_hash}")
}

pub(super) fn runtime_smart_context_static_context_section_dup_marker(
    source_id: &str,
    content_hash: &str,
    heading: &str,
) -> String {
    format!(
        "{heading}\n{SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX}{source_id} {content_hash}"
    )
}

pub(super) fn runtime_smart_context_static_context_delta_marker(prompt_cache_hash: &str) -> String {
    format!("{SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX}{prompt_cache_hash}")
}

fn runtime_smart_context_static_context_delta_marker_hash(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    trimmed
        .strip_prefix(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
        .or_else(|| trimmed.strip_prefix(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY))
        .filter(|hash| hash.starts_with("scpc:") && !hash.chars().any(char::is_whitespace))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_static_context_delta_prompt_cache_hash(
    value: &serde_json::Value,
) -> Option<String> {
    let items = runtime_smart_context_static_context_items(value);
    let mut hashes = Vec::new();
    for item in items {
        hashes
            .push(runtime_smart_context_static_context_delta_marker_hash(&item.text)?.to_string());
    }
    let first = hashes.first()?;
    hashes
        .iter()
        .all(|hash| hash == first)
        .then(|| first.to_string())
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_replace_static_context_texts(
    value: &mut serde_json::Value,
    observation: &RuntimeSmartContextStaticContextObservation,
    marker: &str,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if !runtime_smart_context_static_context_item_delta_allowed(key, observation) {
            continue;
        }
        if let Some(text) = object.get(key).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            object.insert(
                key.to_string(),
                serde_json::Value::String(marker.to_string()),
            );
            replaced = replaced.saturating_add(1);
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return replaced;
    };
    for (index, item) in input.iter_mut().enumerate() {
        if runtime_smart_context_replace_static_message_text(index, item, observation, marker) {
            replaced = replaced.saturating_add(1);
        }
    }
    replaced
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_replace_static_context_duplicate_texts(
    value: &mut serde_json::Value,
    duplicate_ids: &BTreeMap<String, String>,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(marker) = duplicate_ids.get(key)
            && runtime_smart_context_replace_top_level_static_field(object, key, marker)
        {
            replaced = replaced.saturating_add(1);
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return replaced;
    };
    for (index, item) in input.iter_mut().enumerate() {
        let role = item
            .as_object()
            .and_then(|object| object.get("role"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let id = format!("input[{index}].{role}");
        if let Some(marker) = duplicate_ids.get(&id)
            && runtime_smart_context_replace_static_message_text_with_marker(item, marker)
        {
            replaced = replaced.saturating_add(1);
        }
    }
    replaced
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_replace_static_context_item_texts(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
) -> usize {
    let Some(object) = value.as_object_mut() else {
        return 0;
    };
    let mut replaced = 0usize;
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(next_text) = replacements.get(key) {
            object.insert(
                key.to_string(),
                serde_json::Value::String(next_text.to_string()),
            );
            replaced = replaced.saturating_add(1);
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return replaced;
    };
    for (index, item) in input.iter_mut().enumerate() {
        let role = item
            .as_object()
            .and_then(|object| object.get("role"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let id = format!("input[{index}].{role}");
        if let Some(next_text) = replacements.get(&id)
            && runtime_smart_context_replace_static_message_text_with_marker(item, next_text)
        {
            replaced = replaced.saturating_add(1);
        }
    }
    replaced
}

fn runtime_smart_context_replace_top_level_static_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    marker: &str,
) -> bool {
    if object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
    {
        object.insert(
            key.to_string(),
            serde_json::Value::String(marker.to_string()),
        );
        true
    } else {
        false
    }
}

fn runtime_smart_context_replace_static_message_text(
    index: usize,
    value: &mut serde_json::Value,
    observation: &RuntimeSmartContextStaticContextObservation,
    marker: &str,
) -> bool {
    if !runtime_smart_context_value_is_static_context_item(value) {
        return false;
    }
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let id = format!("input[{index}].{role}");
    if !runtime_smart_context_static_context_item_delta_allowed(&id, observation) {
        return false;
    }
    if let Some(text) = object.get("content").and_then(serde_json::Value::as_str)
        && !text.trim().is_empty()
    {
        return runtime_smart_context_replace_static_message_text_with_marker(value, marker);
    }
    if let Some(text) = object.get("input_text").and_then(serde_json::Value::as_str)
        && !text.trim().is_empty()
    {
        return runtime_smart_context_replace_static_message_text_with_marker(value, marker);
    }
    if object.get("content").is_some() {
        return runtime_smart_context_replace_static_message_text_with_marker(value, marker);
    }
    false
}

fn runtime_smart_context_replace_static_message_text_with_marker(
    value: &mut serde_json::Value,
    marker: &str,
) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    if object.get("content").is_some() {
        object.insert(
            "content".to_string(),
            serde_json::Value::String(marker.to_string()),
        );
        return true;
    }
    if object.get("input_text").is_some() {
        object.insert(
            "input_text".to_string(),
            serde_json::Value::String(marker.to_string()),
        );
        return true;
    }
    false
}

fn runtime_smart_context_static_context_item_delta_allowed(
    id: &str,
    observation: &RuntimeSmartContextStaticContextObservation,
) -> bool {
    !observation.changed || !observation.changed_item_ids.contains(id)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_static_context_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextStaticContextItem> {
    let mut items = Vec::new();
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            items.push(runtime_proxy_crate::SmartContextStaticContextItem {
                id: key.to_string(),
                text: text.to_string(),
            });
        }
    }

    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return items;
    };
    for (index, item) in input.iter().enumerate() {
        let Some(object) = item.as_object() else {
            continue;
        };
        let role = object
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !runtime_smart_context_static_role_is_prompt_prefix(role) {
            continue;
        }
        if let Some(text) = runtime_smart_context_static_message_text(item)
            && !text.trim().is_empty()
        {
            items.push(runtime_proxy_crate::SmartContextStaticContextItem {
                id: format!("input[{index}].{role}"),
                text,
            });
        }
    }
    items.sort_by(|left, right| left.id.cmp(&right.id));
    items
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_static_prompt_field_key(
    key: &str,
) -> bool {
    RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS.contains(&key)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_static_role_is_prompt_prefix(
    role: &str,
) -> bool {
    matches!(role, "system" | "developer")
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_value_is_static_context_item(
    value: &serde_json::Value,
) -> bool {
    value
        .as_object()
        .and_then(|object| object.get("role"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(runtime_smart_context_static_role_is_prompt_prefix)
}

fn runtime_smart_context_static_message_text(value: &serde_json::Value) -> Option<String> {
    let object = value.as_object()?;
    if let Some(text) = object.get("content").and_then(serde_json::Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = object.get("input_text").and_then(serde_json::Value::as_str) {
        return Some(text.to_string());
    }

    let content = object.get("content")?;
    let mut parts = Vec::new();
    runtime_smart_context_collect_static_text_parts(content, &mut parts);
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn runtime_smart_context_collect_static_text_parts(
    value: &serde_json::Value,
    parts: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => parts.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_static_text_parts(item, parts);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "input_text", "content"] {
                if let Some(item) = object.get(key) {
                    runtime_smart_context_collect_static_text_parts(item, parts);
                }
            }
        }
        _ => {}
    }
}

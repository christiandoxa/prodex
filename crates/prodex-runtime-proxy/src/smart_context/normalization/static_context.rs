use super::*;
use std::cmp::Ordering;

pub(in crate::smart_context) fn smart_context_stabilize_static_context_id(id: &str) -> String {
    id.trim().replace('\\', "/")
}

pub(in crate::smart_context) fn smart_context_static_context_item_order(
    left: &SmartContextStableStaticContextItem,
    right: &SmartContextStableStaticContextItem,
) -> Ordering {
    smart_context_static_context_item_order_key(&left.id)
        .cmp(&smart_context_static_context_item_order_key(&right.id))
        .then_with(|| left.id.cmp(&right.id))
        .then_with(|| left.content_hash.cmp(&right.content_hash))
        .then_with(|| left.byte_len.cmp(&right.byte_len))
        .then_with(|| left.canonical_text.cmp(&right.canonical_text))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SmartContextStaticContextItemOrderKey {
    group: u8,
    input_index: usize,
    role_rank: u8,
    generic_id: String,
}

fn smart_context_static_context_item_order_key(id: &str) -> SmartContextStaticContextItemOrderKey {
    match id {
        "instructions" => smart_context_static_context_order_key(0, 0, 0, ""),
        "system" => smart_context_static_context_order_key(1, 0, 0, ""),
        "developer" => smart_context_static_context_order_key(2, 0, 0, ""),
        _ => smart_context_input_static_context_order_key(id)
            .unwrap_or_else(|| smart_context_static_context_order_key(100, 0, 0, id)),
    }
}

fn smart_context_input_static_context_order_key(
    id: &str,
) -> Option<SmartContextStaticContextItemOrderKey> {
    let rest = id.strip_prefix("input[")?;
    let (index, rest) = rest.split_once("].")?;
    let input_index = index.parse::<usize>().ok()?;
    let role_rank = match rest {
        "system" => 0,
        "developer" => 1,
        _ => return None,
    };
    Some(smart_context_static_context_order_key(
        3,
        input_index,
        role_rank,
        "",
    ))
}

fn smart_context_static_context_order_key(
    group: u8,
    input_index: usize,
    role_rank: u8,
    generic_id: &str,
) -> SmartContextStaticContextItemOrderKey {
    SmartContextStaticContextItemOrderKey {
        group,
        input_index,
        role_rank,
        generic_id: generic_id.to_string(),
    }
}

pub(in crate::smart_context) fn smart_context_static_context_prompt_cache_payload(
    items: &[SmartContextStableStaticContextItem],
) -> String {
    let mut payload = String::from("prodex-smart-context-static-prompt-cache-v1\n");
    for item in items {
        payload.push_str("id-bytes:");
        payload.push_str(&item.id.len().to_string());
        payload.push('\n');
        payload.push_str(&item.id);
        payload.push('\n');
        payload.push_str("text-bytes:");
        payload.push_str(&item.byte_len.to_string());
        payload.push('\n');
        payload.push_str(&item.canonical_text);
        payload.push('\n');
    }
    payload
}

pub(in crate::smart_context) fn smart_context_static_context_noise_line(line: &str) -> bool {
    let mut value = line.trim();
    if let Some(inner) = value
        .strip_prefix("<!--")
        .and_then(|value| value.strip_suffix("-->"))
    {
        value = inner.trim();
    }

    for prefix in ["//", "#", ";"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest.trim_start();
            break;
        }
    }

    let Some((key, noise_value)) = value.split_once(':').or_else(|| value.split_once('=')) else {
        return false;
    };
    let key = smart_context_static_context_noise_key(key);
    if !smart_context_static_context_noise_key_is_volatile(&key) {
        return false;
    }

    matches!(
        key.as_str(),
        "run id" | "request id" | "trace id" | "session id"
    ) || smart_context_static_context_noise_value_looks_volatile(noise_value)
}

pub(in crate::smart_context) fn smart_context_static_context_noise_key(key: &str) -> String {
    let lower = key.trim().to_ascii_lowercase();
    let mut normalized = String::new();
    let mut previous_space = false;
    for value in lower.chars() {
        let value = match value {
            '-' | '_' => ' ',
            value => value,
        };
        if value.is_whitespace() {
            if !previous_space {
                normalized.push(' ');
                previous_space = true;
            }
        } else {
            normalized.push(value);
            previous_space = false;
        }
    }

    normalized
        .trim()
        .strip_prefix("prodex ")
        .unwrap_or(normalized.trim())
        .to_string()
}

pub(in crate::smart_context) fn smart_context_static_context_noise_key_is_volatile(
    key: &str,
) -> bool {
    matches!(
        key,
        "generated"
            | "generated at"
            | "generated on"
            | "last generated"
            | "last generated at"
            | "timestamp"
            | "current date"
            | "current time"
            | "current datetime"
            | "as of"
            | "last updated"
            | "updated at"
            | "run id"
            | "request id"
            | "trace id"
            | "session id"
    )
}

pub(in crate::smart_context) fn smart_context_static_context_noise_value_looks_volatile(
    value: &str,
) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(|value| value.is_ascii_digit()) {
        return true;
    }

    matches!(
        value.to_ascii_lowercase().as_str(),
        "now" | "today" | "yesterday" | "tomorrow"
    )
}

use super::*;

pub(super) fn runtime_smart_context_static_prompt_cache_key_from_body(
    body: &[u8],
) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let items = runtime_smart_context_static_context_items(&value);
    if items.is_empty() {
        return None;
    }
    if let Some(hash) = runtime_smart_context_legacy_static_delta_hash(&items) {
        return Some(hash);
    }
    Some(
        runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(items)
            .content_hash,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextStaticContextObservation {
    pub(super) changed: bool,
    pub(super) state_changed: bool,
    pub(super) prompt_cache_hash: Option<String>,
    pub(super) fingerprints: Vec<runtime_proxy_crate::SmartContextFingerprint>,
}

pub(super) fn runtime_smart_context_static_context_observation(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
) -> RuntimeSmartContextStaticContextObservation {
    let items = runtime_smart_context_static_context_items(value);
    let (prompt_cache_hash, fingerprints) = if items.is_empty() {
        (None, Vec::new())
    } else {
        let cache =
            runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(items);
        (
            Some(cache.content_hash),
            cache
                .items
                .into_iter()
                .map(|item| runtime_proxy_crate::SmartContextFingerprint {
                    id: item.id_hash,
                    kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
                    content_hash: item.content_hash,
                    byte_len: item.byte_len,
                })
                .collect(),
        )
    };
    let previous_fingerprints = store.static_context_fingerprints();
    let previous_prompt_cache_hash = store.static_context_prompt_cache_hash().map(str::to_string);
    let fingerprint_changed = runtime_proxy_crate::smart_context_fingerprint_delta(
        previous_fingerprints.clone(),
        fingerprints.clone(),
    )
    .iter()
    .any(|change| {
        !matches!(
            change,
            runtime_proxy_crate::SmartContextFingerprintChange::Unchanged { .. }
        )
    });
    let state_changed =
        previous_fingerprints != fingerprints || previous_prompt_cache_hash != prompt_cache_hash;
    let changed = (store.static_context_prompt_cache_hash().is_some()
        || !previous_fingerprints.is_empty())
        && (fingerprint_changed
            || store.static_context_prompt_cache_hash() != prompt_cache_hash.as_deref());

    RuntimeSmartContextStaticContextObservation {
        changed,
        state_changed,
        prompt_cache_hash,
        fingerprints,
    }
}

fn runtime_smart_context_legacy_static_delta_hash(
    items: &[runtime_proxy_crate::SmartContextStaticContextItem],
) -> Option<String> {
    let hashes = items
        .iter()
        .map(|item| {
            let text = item.text.trim();
            text.strip_prefix(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
                .or_else(|| {
                    text.strip_prefix(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY)
                })
                .filter(|hash| {
                    (hash.starts_with("scpc2:") || hash.starts_with("scpc:"))
                        && !hash.chars().any(char::is_whitespace)
                })
        })
        .collect::<Option<Vec<_>>>()?;
    let first = hashes.first()?;
    hashes
        .iter()
        .all(|hash| hash == first)
        .then(|| (*first).to_string())
}

pub(super) fn runtime_smart_context_static_context_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextStaticContextItem> {
    let mut items = RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS
        .iter()
        .filter_map(|key| {
            let text = value.get(key)?.as_str()?;
            (!text.trim().is_empty()).then(|| runtime_proxy_crate::SmartContextStaticContextItem {
                id: (*key).to_string(),
                text: text.to_string(),
            })
        })
        .collect::<Vec<_>>();

    if let Some(input) = value.get("input").and_then(serde_json::Value::as_array) {
        for (index, item) in input.iter().enumerate() {
            let Some(role) = item.get("role").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if !runtime_smart_context_static_role_is_prompt_prefix(role) {
                continue;
            }
            if let Some(text) = runtime_smart_context_static_message_text(item) {
                items.push(runtime_proxy_crate::SmartContextStaticContextItem {
                    id: format!("input[{index}].{role}"),
                    text,
                });
            }
        }
    }
    items.sort_by(|left, right| left.id.cmp(&right.id));
    items
}

pub(super) fn runtime_smart_context_static_prompt_field_key(key: &str) -> bool {
    RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS.contains(&key)
}

fn runtime_smart_context_static_role_is_prompt_prefix(role: &str) -> bool {
    matches!(role, "system" | "developer")
}

pub(super) fn runtime_smart_context_value_is_static_context_item(
    value: &serde_json::Value,
) -> bool {
    value
        .get("role")
        .and_then(serde_json::Value::as_str)
        .is_some_and(runtime_smart_context_static_role_is_prompt_prefix)
}

fn runtime_smart_context_static_message_text(value: &serde_json::Value) -> Option<String> {
    let object = value.as_object()?;
    if let Some(text) = object.get("content").and_then(serde_json::Value::as_str) {
        return (!text.trim().is_empty()).then(|| text.to_string());
    }
    if let Some(text) = object.get("input_text").and_then(serde_json::Value::as_str) {
        return (!text.trim().is_empty()).then(|| text.to_string());
    }
    let mut parts = Vec::new();
    collect_static_text_parts(object.get("content")?, &mut parts);
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn collect_static_text_parts(value: &serde_json::Value, parts: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => parts.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_static_text_parts(item, parts);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "input_text", "content"] {
                if let Some(item) = object.get(key) {
                    collect_static_text_parts(item, parts);
                }
            }
        }
        _ => {}
    }
}

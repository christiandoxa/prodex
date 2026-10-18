use super::*;

pub(super) fn runtime_smart_context_static_prompt_cache_key_from_body(
    body: &[u8],
) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let (items, legacy_hash) = runtime_smart_context_static_request_projection(&value);
    if items.is_empty() {
        return None;
    }
    if let Some(hash) = legacy_hash {
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

fn runtime_smart_context_static_request_projection(
    value: &serde_json::Value,
) -> (
    Vec<runtime_proxy_crate::SmartContextStaticContextItem>,
    Option<String>,
) {
    #[derive(serde::Deserialize)]
    struct Projection {
        items: Vec<(String, String)>,
        legacy_hash: Option<String>,
    }
    let tree = crate::json_boundary::app_json_tree(value)
        .expect("bounded static-prompt request must fit its JSON ABI");
    let raw = std::str::from_utf8(&tree.raw).expect("Serde static-prompt JSON remains UTF-8");
    let output = prodex_mojo_core::json::smart_context_static_request(&tree.nodes, raw)
        .expect("compiled Mojo static-prompt projection returned invalid output");
    let projection: Projection = serde_json::from_slice(&output)
        .expect("Mojo static-prompt projection matches its caller schema");
    (
        projection
            .items
            .into_iter()
            .map(|(id, text)| runtime_proxy_crate::SmartContextStaticContextItem { id, text })
            .collect(),
        projection.legacy_hash,
    )
}

pub(super) fn runtime_smart_context_static_context_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextStaticContextItem> {
    runtime_smart_context_static_request_projection(value).0
}

pub(super) fn runtime_smart_context_static_prompt_field_key(key: &str) -> bool {
    prodex_mojo_core::json::smart_context_static_prompt_key(key)
        .expect("compiled Mojo static-prompt field classifier returned invalid output")
}

pub(super) fn runtime_smart_context_value_is_static_context_item(
    value: &serde_json::Value,
) -> bool {
    let role = value
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    prodex_mojo_core::json::smart_context_static_prompt_role(role)
        .expect("compiled Mojo static-prompt role classifier returned invalid output")
}

#[cfg(test)]
#[path = "static_context_projection_tests.rs"]
mod tests;

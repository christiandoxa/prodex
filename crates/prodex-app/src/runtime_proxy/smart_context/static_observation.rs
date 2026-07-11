//! Smart-context static-context observation and artifact fingerprint updates.

use super::*;

pub(super) fn runtime_smart_context_observe_static_context(
    shared: &RuntimeRotationProxyShared,
    value: &serde_json::Value,
) -> RuntimeSmartContextStaticContextObservation {
    let cache = runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(
        runtime_smart_context_static_context_items(value),
    );
    if cache.items.is_empty() {
        return RuntimeSmartContextStaticContextObservation::default();
    }

    let current = cache
        .items
        .iter()
        .map(|item| runtime_proxy_crate::SmartContextFingerprint {
            id: item.id.clone(),
            kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
            content_hash: item.content_hash.clone(),
            byte_len: item.byte_len,
        })
        .collect::<Vec<_>>();

    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };
    let Ok(mut states) = states.lock() else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };
    let Some(state) = states.get_mut(&shared.log_path) else {
        return RuntimeSmartContextStaticContextObservation {
            seen_before: false,
            changed: false,
            item_count: current.len(),
            delta_count: 0,
            prompt_cache_hash: Some(cache.content_hash),
            changed_item_ids: BTreeSet::new(),
        };
    };

    let seen_before = !state.last_static_context_fingerprints.is_empty();
    let delta = if seen_before {
        runtime_proxy_crate::smart_context_fingerprint_delta(
            state.last_static_context_fingerprints.clone(),
            current.clone(),
        )
    } else {
        Vec::new()
    };
    let changed = delta
        .iter()
        .any(runtime_smart_context_fingerprint_change_is_substantive);
    let changed_item_ids =
        runtime_smart_context_substantive_static_context_changed_item_ids(&delta);
    let observation = RuntimeSmartContextStaticContextObservation {
        seen_before,
        changed,
        item_count: current.len(),
        delta_count: delta.len(),
        prompt_cache_hash: Some(cache.content_hash.clone()),
        changed_item_ids,
    };
    state.last_static_context_fingerprints = current;
    state.last_static_context_prompt_cache_hash = Some(cache.content_hash);
    state.artifacts.set_static_context_fingerprints(
        observation.prompt_cache_hash.clone(),
        state.last_static_context_fingerprints.clone(),
    );
    let save_job = state
        .artifact_path
        .clone()
        .map(|path| (path, state.artifacts.clone()));
    drop(states);
    if let Some((path, store)) = save_job {
        schedule_runtime_smart_context_artifact_save(
            shared,
            path,
            store,
            "smart_context_static_fingerprints",
        );
    }
    observation
}

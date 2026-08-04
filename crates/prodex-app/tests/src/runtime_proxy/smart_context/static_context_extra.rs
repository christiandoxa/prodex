use super::*;

include!("static_context_extra/telemetry.rs");
include!("static_context_extra/prompt_cache.rs");

#[test]
fn smart_context_static_context_observation_is_secret_safe_and_tracks_removal() {
    let secret = "static-context-secret-value";
    let first = serde_json::json!({
        "instructions": secret,
        "input": [{"role": "system", "content": "system prefix"}]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();

    let initial = runtime_smart_context_static_context_observation(&first, &store);

    assert!(!initial.changed);
    assert!(initial.state_changed);
    assert!(!format!("{initial:?}").contains(secret));
    store.set_static_context_fingerprints(
        initial.prompt_cache_hash.clone(),
        initial.fingerprints.clone(),
    );

    let unchanged = runtime_smart_context_static_context_observation(&first, &store);
    assert!(!unchanged.changed);
    assert!(!unchanged.state_changed);

    let changed = serde_json::json!({
        "instructions": "changed static context",
        "input": [{"role": "system", "content": "system prefix"}]
    });
    let changed_observation = runtime_smart_context_static_context_observation(&changed, &store);
    assert!(changed_observation.changed);
    assert!(changed_observation.state_changed);

    let removed = serde_json::json!({"input": [{"role": "user", "content": "dynamic"}]});
    let removed_observation = runtime_smart_context_static_context_observation(&removed, &store);
    assert!(removed_observation.changed);
    assert!(removed_observation.state_changed);
}

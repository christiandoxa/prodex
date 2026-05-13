#[test]
fn smart_context_static_delta_prompt_cache_key_accepts_short_and_legacy_markers() {
    let shared = smart_context_test_shared("static-marker-compat");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let short = smart_context_test_request(serde_json::json!({
        "instructions": "psc static scpc:short123"
    }));
    let legacy = smart_context_test_request(serde_json::json!({
        "instructions": "prodex static context unchanged scpc:legacy123"
    }));

    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&short, &shared, true).as_deref(),
        Some("scpc:short123")
    );
    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&legacy, &shared, true).as_deref(),
        Some("scpc:legacy123")
    );
}

#[test]
fn smart_context_prompt_cache_key_prefers_explicit_request_key() {
    let shared = smart_context_test_shared("prompt-cache-explicit");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = smart_context_test_request(serde_json::json!({
        "prompt_cache_key": " explicit-cache-key ",
        "instructions": "Static instructions"
    }));

    let key = runtime_smart_context_effective_prompt_cache_key(&request, &shared, true);

    assert_eq!(key.as_deref(), Some("explicit-cache-key"));
}

#[test]
fn smart_context_prompt_cache_key_is_stable_for_same_static_context() {
    let shared = smart_context_test_shared("prompt-cache-stable");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let first = smart_context_test_request(serde_json::json!({
        "instructions": "Generated at: 2026-05-04T01:02:03Z\nUse repo rules",
        "input": [{"type": "message", "content": "first user message"}]
    }));
    let second = smart_context_test_request(serde_json::json!({
        "instructions": "Generated at: 2027-06-07T08:09:10Z\nUse repo rules",
        "input": [{"type": "message", "content": "different user message"}]
    }));

    let first_key = runtime_smart_context_effective_prompt_cache_key(&first, &shared, true);
    let second_key = runtime_smart_context_effective_prompt_cache_key(&second, &shared, true);

    assert!(
        first_key
            .as_deref()
            .is_some_and(|key| key.starts_with("scpc:"))
    );
    assert_eq!(first_key, second_key);
}

#[test]
fn smart_context_prompt_cache_key_absent_when_disabled_or_no_static_context() {
    let disabled = smart_context_test_shared("prompt-cache-disabled");
    register_runtime_smart_context_proxy_state(&disabled.log_path, false, None, None);
    let static_request = smart_context_test_request(serde_json::json!({
        "instructions": "Static instructions"
    }));
    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&static_request, &disabled, true),
        None
    );

    let enabled = smart_context_test_shared("prompt-cache-no-static");
    register_runtime_smart_context_proxy_state(&enabled.log_path, true, None, None);
    let dynamic_only_request = smart_context_test_request(serde_json::json!({
        "input": [{"type": "message", "content": "user-only context"}]
    }));
    assert_eq!(
        runtime_smart_context_effective_prompt_cache_key(&dynamic_only_request, &enabled, true),
        None
    );
}

#[test]
fn smart_context_prompt_cache_key_derivation_does_not_mutate_upstream_payload() {
    let shared = smart_context_test_shared("prompt-cache-payload");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = smart_context_test_request(serde_json::json!({
        "instructions": "Static instructions",
        "input": [{"type": "message", "content": "hello"}]
    }));
    let before = request.body.clone();

    let key = runtime_smart_context_effective_prompt_cache_key(&request, &shared, true);
    let prepared =
        prepare_runtime_smart_context_http_body(88, &request, &shared, RuntimeRouteKind::Responses);

    assert!(key.as_deref().is_some_and(|key| key.starts_with("scpc:")));
    assert_eq!(request.body, before);
    assert_eq!(prepared.as_ref(), before.as_slice());
    let upstream = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert!(upstream.get("prompt_cache_key").is_none());
}

#[test]
fn smart_context_static_context_items_have_stable_id_order() {
    let value = serde_json::json!({
        "developer": "dev rules",
        "instructions": "root instructions",
        "input": [
            {"type": "message", "role": "developer", "content": "input dev"},
            {"type": "message", "role": "system", "content": "input system"}
        ],
        "system": "system prompt"
    });

    let ids = runtime_smart_context_static_context_items(&value)
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "developer",
            "input[0].developer",
            "input[1].system",
            "instructions",
            "system"
        ]
    );
}

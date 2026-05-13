#[test]
fn smart_context_delta_replaces_unchanged_fresh_static_context_with_marker() {
    let shared = smart_context_test_shared("static-context-delta");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let instructions = format!(
        "Use repo rules.\nKeep account affinity.\n{}",
        "Static instruction line. ".repeat(80)
    );
    let input_system = format!(
        "Input system prefix stays stable.\n{}",
        "Static system line. ".repeat(80)
    );
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "system", "content": input_system.as_str()},
            {"role": "user", "content": "first fresh request"}
        ]
    }));
    let second = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "system", "content": input_system.as_str()},
            {"role": "user", "content": "second fresh request"}
        ]
    }));

    let first_prepared =
        prepare_runtime_smart_context_http_body(90, &first, &shared, RuntimeRouteKind::Responses);
    assert!(
        !String::from_utf8_lossy(first_prepared.as_ref())
            .contains(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
    );

    let second_prepared =
        prepare_runtime_smart_context_http_body(91, &second, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = second_prepared else {
        panic!("expected static context delta body");
    };
    let text = String::from_utf8(body.clone()).unwrap();
    assert!(text.contains("psc static scpc:"));
    assert!(!text.contains("prodex static context unchanged scpc:"));
    assert!(!text.contains(&instructions));
    assert!(!text.contains(&input_system));
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some("second fresh request")
    );
}

#[test]
fn smart_context_delta_preserves_exact_static_context() {
    let shared = smart_context_test_shared("static-context-delta-exact");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let instructions = "Use repo rules.\nKeep exact static content.";
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions,
        "input": [{"role": "user", "content": "first fresh request"}]
    }));
    let exact = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: vec![("x-prodex-smart-context".to_string(), "exact".to_string())],
        body: serde_json::to_vec(&serde_json::json!({
            "instructions": instructions,
            "input": [{"role": "user", "content": "exact request"}]
        }))
        .unwrap(),
    };

    let _ =
        prepare_runtime_smart_context_http_body(92, &first, &shared, RuntimeRouteKind::Responses);
    let prepared =
        prepare_runtime_smart_context_http_body(93, &exact, &shared, RuntimeRouteKind::Responses);

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(value["instructions"].as_str(), Some(instructions));
    assert!(
        !String::from_utf8_lossy(prepared.as_ref())
            .contains(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
    );
}

#[test]
fn smart_context_delta_preserves_changed_static_context() {
    let shared = smart_context_test_shared("static-context-delta-changed");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let stable_system = format!("Stable system prefix\n{}", "stable ".repeat(80));
    let first = smart_context_test_request(serde_json::json!({
        "instructions": "Use repo rules.\nKeep account affinity.",
        "input": [
            {"role": "system", "content": stable_system.as_str()},
            {"role": "user", "content": "first fresh request"}
        ]
    }));
    let changed = smart_context_test_request(serde_json::json!({
        "instructions": "Use repo rules.\nAllow account rotation.",
        "input": [
            {"role": "system", "content": stable_system.as_str()},
            {"role": "user", "content": "changed fresh request"}
        ]
    }));

    let _ =
        prepare_runtime_smart_context_http_body(94, &first, &shared, RuntimeRouteKind::Responses);
    let prepared =
        prepare_runtime_smart_context_http_body(95, &changed, &shared, RuntimeRouteKind::Responses);

    let value = serde_json::from_slice::<serde_json::Value>(prepared.as_ref()).unwrap();
    assert_eq!(
        value["instructions"].as_str(),
        Some("Use repo rules.\nAllow account rotation.")
    );
    let text = String::from_utf8_lossy(prepared.as_ref());
    assert!(text.contains("psc static scpc:"));
    assert!(!text.contains(stable_system.as_str()));
}

#[test]
fn smart_context_persists_static_fingerprints_but_does_not_delta_on_fresh_start() {
    let first_shared = smart_context_test_shared("static-persist-first");
    let artifact_path = first_shared
        .log_path
        .with_file_name("static-persist-artifacts.json");
    let _ = std::fs::remove_file(&artifact_path);
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let instructions = format!("Persistent static context\n{}", "stable rule ".repeat(120));
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "first request"}]
    }));

    let _ = prepare_runtime_smart_context_http_body(
        96,
        &first,
        &first_shared,
        RuntimeRouteKind::Responses,
    );
    let loaded = RuntimeSmartContextArtifactStore::load_from_path(&artifact_path);
    assert!(
        loaded
            .static_context_prompt_cache_hash()
            .is_some_and(|hash| hash.starts_with("scpc:"))
    );
    assert_eq!(loaded.static_context_fingerprints().len(), 1);

    let fresh_shared = smart_context_test_shared("static-persist-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let fresh_first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "fresh request"}]
    }));
    let fresh_prepared = prepare_runtime_smart_context_http_body(
        97,
        &fresh_first,
        &fresh_shared,
        RuntimeRouteKind::Responses,
    );
    let fresh_text = String::from_utf8_lossy(fresh_prepared.as_ref());
    assert!(fresh_text.contains("Persistent static context"));
    assert!(!fresh_text.contains(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX));

    let fresh_second = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "fresh second request"}]
    }));
    let second_prepared = prepare_runtime_smart_context_http_body(
        98,
        &fresh_second,
        &fresh_shared,
        RuntimeRouteKind::Responses,
    );
    assert!(String::from_utf8_lossy(second_prepared.as_ref()).contains("psc static scpc:"));

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
}

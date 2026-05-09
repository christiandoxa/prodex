use super::*;

#[test]
fn smart_context_static_context_cross_field_dedupe_keeps_one_exact_copy() {
    let repeated = "Use repo rules exactly.\n".repeat(80);
    let mut value = serde_json::json!({
        "instructions": repeated.as_str(),
        "system": repeated.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_cross_field_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(repeated.as_str()));
    assert_eq!(
        value["system"].as_str(),
        Some("psc static dup instructions")
    );
    assert_eq!(value["input"][0]["content"].as_str(), Some("do work"));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_chunk_dedupe_replaces_repeated_chunk_only() {
    let shared_chunk = (0..80)
        .map(|index| format!("Shared policy sentence number {index} stays semantically identical."))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(shared_chunk.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES);
    let instructions = format!("Primary intro.\n\n{shared_chunk}\n\nPrimary tail.");
    let system = format!("System intro.\n\n{shared_chunk}\n\nSystem tail.");
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "system": system.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_chunk_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(instructions.as_str()));
    let system_after = value["system"].as_str().unwrap();
    assert!(system_after.contains("System intro."));
    assert!(system_after.contains("System tail."));
    assert!(system_after.contains(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX));
    assert!(!system_after.contains(&shared_chunk));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_section_dedupe_replaces_later_identical_heading_section() {
    let body = (0..80)
        .map(|index| format!("Shared section rule {index} remains exact."))
        .collect::<Vec<_>>()
        .join("\n");
    let repeated_section = format!("## Runtime Proxy\n{body}\n");
    assert!(repeated_section.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES);
    let instructions = format!(
        "# One\nunique intro\n\n{repeated_section}\n## Other\nunique tail\n\n{repeated_section}"
    );
    let sections = runtime_smart_context_static_context_heading_sections(&instructions);
    assert_eq!(sections.len(), 2);
    assert_eq!(
        instructions[sections[0].start..sections[0].end].trim(),
        instructions[sections[1].start..sections[1].end].trim()
    );
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    let after = value["instructions"].as_str().unwrap();
    assert!(after.contains(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX));
    assert_eq!(after.matches("## Runtime Proxy").count(), 2);
    assert_eq!(
        after
            .matches("Shared section rule 79 remains exact.")
            .count(),
        1
    );
    assert!(after.contains("## Other"));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_section_dedupe_respects_require_exact() {
    let body = (0..80)
        .map(|index| format!("Shared exact section rule {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let instructions = format!("## Same\n{body}\n## Same\n{body}");
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::RequireExact,
            reasons: Vec::new(),
        },
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(instructions.as_str()));
    assert_eq!(stats.static_context_deltas, 0);
}

#[test]
fn smart_context_persisted_static_section_fingerprints_dedupe_fresh_start() {
    let first_shared = smart_context_test_shared("static-section-persist-first");
    let artifact_path = first_shared
        .log_path
        .with_file_name("static-section-persist-artifacts.json");
    let calibration_path = runtime_smart_context_token_calibration_path(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let body = (0..90)
        .map(|index| format!("Stable section guidance {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let instructions = format!("# Stable Section\n{body}\n");
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "first request"}]
    }));

    let first_prepared = prepare_runtime_smart_context_http_body(
        120,
        &first,
        &first_shared,
        RuntimeRouteKind::Responses,
    );
    let first_value = serde_json::from_slice::<serde_json::Value>(first_prepared.as_ref()).unwrap();
    assert_eq!(
        first_value["instructions"].as_str(),
        Some(instructions.as_str())
    );
    assert!(
        std::fs::read_to_string(&calibration_path)
            .unwrap()
            .contains("static_section_fingerprints")
    );

    let fresh_shared = smart_context_test_shared("static-section-persist-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let fresh = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "fresh request"}]
    }));

    let fresh_prepared = prepare_runtime_smart_context_http_body(
        121,
        &fresh,
        &fresh_shared,
        RuntimeRouteKind::Responses,
    );
    let fresh_text = String::from_utf8_lossy(fresh_prepared.as_ref());
    assert!(fresh_text.contains(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX));
    assert!(!fresh_text.contains("Stable section guidance 89."));
    assert!(prodex_context::critical_signal_self_check(&instructions, &fresh_text).passed());

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&calibration_path));
}

#[test]
fn smart_context_rewrite_telemetry_ring_records_bytes_tokens_and_fallback() {
    let shared = smart_context_test_shared("rewrite-telemetry");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let budget = runtime_smart_context_budget(RuntimeSmartContextBudgetInput {
        shared: &shared,
        body: br#"{"input":"test"}"#,
        route_kind: RuntimeRouteKind::Responses,
        transport: RuntimeSmartContextTransport::Http,
        profile_name: Some("main"),
        exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        missing_rehydrate_refs: Vec::new(),
        static_context_changed: false,
    });

    for index in 0..(SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT + 2) {
        runtime_smart_context_log(RuntimeSmartContextLogInput {
            request_id: index as u64,
            shared: &shared,
            route_kind: RuntimeRouteKind::Responses,
            transport: RuntimeSmartContextTransport::Http,
            tier: "minimal",
            decision: "self_check_passthrough",
            reasons: "-",
            body_bytes_before: 400 + index,
            body_bytes_after: 300,
            stats: RuntimeSmartContextTransformStats::default(),
            budget: &budget,
            self_check: "critical_signal_loss",
        });
    }

    with_runtime_smart_context_proxy_state(&shared, |state| {
        assert_eq!(
            state.rewrite_telemetry_history.len(),
            SMART_CONTEXT_REWRITE_TELEMETRY_HISTORY_LIMIT
        );
        let first = state.rewrite_telemetry_history.first().unwrap();
        assert_eq!(first.body_bytes_before, 402);
        let last = state.rewrite_telemetry_history.last().unwrap();
        assert_eq!(
            last.estimated_tokens_before,
            runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(
                last.body_bytes_before
            )
        );
        assert_eq!(last.rewrite_kind, "self_check_passthrough");
        assert_eq!(last.status, "critical_signal_loss");
        assert_eq!(
            last.fallback_reason.as_deref(),
            Some("critical_signal_loss")
        );
    })
    .unwrap();

    let log_text = std::fs::read_to_string(&shared.log_path).unwrap();
    assert!(log_text.contains("estimated_tokens_before="));
    assert!(log_text.contains("rewrite_kind=self_check_passthrough"));
    assert!(log_text.contains("fallback_reason=critical_signal_loss"));
}

#[test]
fn smart_context_regression_fallback_exact_on_quality_risk() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        tool_call_args_condensed: 0,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
        repo_state_facts: 0,
    };
    let before = br#"{"input":[{"content":"error: failed\nsrc/main.rs:10:5"}]}"#;
    let after = br#"{"input":[{"content":"summary"}]}"#;
    let regression = runtime_smart_context_regression_self_check(
        before,
        after,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
    );
    let critical = runtime_smart_context_critical_signal_self_check(before, after);

    assert_eq!(
        runtime_smart_context_fallback_exact_reason(&regression, critical, &stats),
        Some("critical_signal_loss")
    );
}

#[test]
fn smart_context_cross_turn_duplicate_uses_artifact_plan_and_exact_guard() {
    let repeated = "cross turn blob ".repeat(120);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &repeated).unwrap();
    let mut value = serde_json::json!({
        "input": [{"type": "message", "content": repeated}]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_dedupe_input_text(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    let content = value["input"][0]["content"].as_str().unwrap();
    assert!(content.contains(&runtime_smart_context_artifact_ref(&artifact.id)));
    assert_eq!(
        content,
        format!(
            "[psc rep {} b={}]",
            runtime_smart_context_artifact_ref(&artifact.id),
            repeated.len()
        )
    );
    assert!(!content.contains(" h="));
    assert_eq!(stats.cross_turn_duplicate_texts, 1);

    let mut exact_value = serde_json::json!({
        "input": [{"type": "message", "content": "cross turn blob ".repeat(120)}]
    });
    let mut exact_stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_dedupe_input_text(
        &mut exact_value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput {
                exact_mode: true,
                ..runtime_proxy_crate::SmartContextExactnessInput::default()
            },
        ),
        &mut exact_stats,
    );

    assert_eq!(
        exact_value["input"][0]["content"],
        "cross turn blob ".repeat(120)
    );
    assert_eq!(exact_stats.cross_turn_duplicate_texts, 0);
}

#[test]
fn smart_context_auto_rehydrate_plan_defers_over_budget_refs() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, &"large artifact ".repeat(400))
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{"type": "message", "content": format!("need prodex-artifact:{}", artifact.id)}]
    });
    let plan = runtime_smart_context_auto_rehydrate_plan(
        &value,
        &store,
        1,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
    );
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value_with_plan(&mut value, &store, &plan, &mut stats);

    assert!(matches!(
        plan.actions.first(),
        Some(runtime_proxy_crate::SmartContextRehydrateAction::Defer { .. })
    ));
    assert_eq!(stats.rehydrated_refs, 0);
    assert!(
        value["input"][0]["content"]
            .as_str()
            .unwrap()
            .contains("prodex-artifact:")
    );
}

#[test]
fn smart_context_static_context_fingerprint_drives_exact_policy_on_real_change() {
    let shared = smart_context_test_shared("static-context");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let first = serde_json::json!({
        "instructions": "Generated at: 2026-05-04T01:02:03Z\nKeep affinity\n"
    });
    let volatile_only = serde_json::json!({
        "instructions": "Generated at: 2027-01-02T03:04:05Z\nKeep affinity\n"
    });
    let changed = serde_json::json!({
        "instructions": "Generated at: 2027-01-02T03:04:05Z\nAllow rotation\n"
    });

    let first_observation = runtime_smart_context_observe_static_context(&shared, &first);
    let volatile_observation =
        runtime_smart_context_observe_static_context(&shared, &volatile_only);
    let changed_observation = runtime_smart_context_observe_static_context(&shared, &changed);
    assert!(!first_observation.changed);
    assert_eq!(first_observation.item_count, 1);
    assert!(!volatile_observation.changed);
    assert_eq!(volatile_observation.delta_count, 1);
    assert!(changed_observation.changed);
    assert_eq!(
        changed_observation.changed_item_ids,
        BTreeSet::from(["instructions".to_string()])
    );
}

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

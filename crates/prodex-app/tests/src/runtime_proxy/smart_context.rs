use super::*;
use std::borrow::Cow;

#[test]
fn smart_context_condenses_tool_output_with_artifact_ref() {
    let original_output = (0..500)
        .map(|index| format!("line {index}: repeated command output"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": original_output
        }]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        4 * 1024,
        &mut stats,
    );

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("prodex-sc artifact"));
    assert!(output.contains("prodex-artifact:sc:"));
    assert!(output.contains(&format!("bytes={}", original_output.len())));
    assert!(output.contains(&format!(
        "hash={}",
        runtime_proxy_crate::smart_context_hash_text(&original_output)
    )));
    assert!(output.contains("rehydrate: use prodex-artifact:"));
    assert!(output.contains("#Lstart-Lend"));
    assert!(!output.contains("artifact_id:"));
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_uses_command_metadata_hint_for_tool_output_compaction() {
    let original_output = std::iter::once("src/lib.rs:42:needle once".to_string())
        .chain((0..120).map(|index| format!("filler line {index}: no match here")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "exec_command",
                "arguments": "{\"cmd\":\"rg needle src\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": original_output
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        7,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &mut stats,
    );

    let output = value["input"][1]["output"].as_str().unwrap();
    assert!(output.contains("prodex-sc artifact"));
    assert!(output.contains("search summary: 1 matches across 1 files"));
    assert!(output.contains("src/lib.rs (1 matches):"));
    assert_eq!(stats.tool_outputs_condensed, 1);
}

#[test]
fn smart_context_rehydrates_known_artifact_refs() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, "exact artifact text").unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("need prodex-artifact:{}", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], "need exact artifact text");
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_rehydrate_preserves_static_prompt_prefix() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, "exact artifact text").unwrap();
    let static_ref = format!("keep prodex-artifact:{}", artifact.id);
    let mut value = serde_json::json!({
        "instructions": static_ref,
        "system": format!("system prodex-artifact:{}", artifact.id),
        "developer": format!("developer prodex-artifact:{}", artifact.id),
        "input": [
            {
                "role": "system",
                "content": format!("input system prodex-artifact:{}", artifact.id),
            },
            {
                "role": "developer",
                "content": format!("input developer prodex-artifact:{}", artifact.id),
            },
            {
                "type": "message",
                "content": format!("need prodex-artifact:{}", artifact.id),
            }
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["instructions"].as_str(), Some(static_ref.as_str()));
    assert_eq!(
        value["system"].as_str(),
        Some(format!("system prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["developer"].as_str(),
        Some(format!("developer prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(format!("input system prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some(format!("input developer prodex-artifact:{}", artifact.id).as_str())
    );
    assert_eq!(
        value["input"][2]["content"].as_str(),
        Some("need exact artifact text")
    );
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_rehydrates_artifact_line_ranges() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "message",
            "content": format!("need prodex-artifact:{}#L2-L3", artifact.id)
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_rehydrate_value(&mut value, &store, &mut stats);

    assert_eq!(value["input"][0]["content"], "need line two\nline three");
    assert_eq!(stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_dedupes_repeated_input_text() {
    let repeated = "same ".repeat(300);
    let mut value = serde_json::json!({
        "input": [
            {"type": "message", "content": repeated},
            {"type": "message", "content": repeated}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    let store = RuntimeSmartContextArtifactStore::default();

    runtime_smart_context_dedupe_input_text(
        &mut value,
        &store,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert!(
        value["input"][1]["content"]
            .as_str()
            .unwrap()
            .contains("prodex smart context duplicate")
    );
    assert_eq!(stats.duplicate_texts, 1);
}

#[test]
fn smart_context_dedupe_preserves_static_prompt_prefix() {
    let repeated = "static prompt prefix ".repeat(120);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &repeated).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {"role": "system", "content": repeated},
            {"role": "developer", "content": repeated},
            {"type": "message", "content": repeated}
        ]
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

    assert_eq!(
        value["input"][0]["content"].as_str(),
        Some(repeated.as_str())
    );
    assert_eq!(
        value["input"][1]["content"].as_str(),
        Some(repeated.as_str())
    );
    assert!(
        value["input"][2]["content"]
            .as_str()
            .unwrap()
            .contains(&format!("prodex-artifact:{}", artifact.id))
    );
    assert_eq!(stats.duplicate_texts, 0);
    assert_eq!(stats.cross_turn_duplicate_texts, 1);
}

#[test]
fn smart_context_budget_uses_runtime_token_usage_observation() {
    let shared = smart_context_test_shared("budget");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );

    let budget = runtime_smart_context_budget(
        &shared,
        32,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal
    );
    assert_eq!(budget.observed_context_tokens, Some(32_000));
    assert_eq!(budget.token_usage_source, "runtime_usage");
    assert_eq!(budget.model_context_window_tokens, 32_000);
    assert_eq!(budget.model_context_window_source, "fallback");
}

#[test]
fn smart_context_budget_uses_configured_model_context_window() {
    let shared = smart_context_test_shared("budget-custom-window");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );

    let budget = runtime_smart_context_budget(
        &shared,
        32,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        budget.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact
    );
    assert_eq!(budget.model_context_window_tokens, 64_000);
    assert_eq!(budget.model_context_window_source, "launch_config");
    assert_eq!(budget.observed_context_tokens, Some(32_000));
}

#[test]
fn smart_context_budget_expands_large_preview_after_recent_safe_rewrite() {
    let shared = smart_context_test_shared("budget-recent-safe");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(64_000), None);
    observe_runtime_smart_context_token_usage(
        &shared,
        RuntimeTokenUsage {
            input_tokens: 48_000,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
        },
    );

    let before = runtime_smart_context_budget(
        &shared,
        32,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );
    observe_runtime_smart_context_rewrite_safety(
        &shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: runtime_proxy_crate::SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
        },
    );
    let after = runtime_smart_context_budget(
        &shared,
        32,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        false,
    );

    assert_eq!(
        before.tier,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large
    );
    assert_eq!(before.policy.max_inline_tool_output_bytes, 32 * 1024);
    assert_eq!(after.policy.max_inline_tool_output_bytes, 64 * 1024);
    assert!(
        after.policy.reasons.contains(
            &runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe
        )
    );
}

#[test]
fn smart_context_tool_preview_lines_follow_budget_tier_and_limit() {
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
            1024,
        ),
        Some(8)
    );
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed,
            8 * 1024,
        ),
        Some(32)
    );
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Large,
            64 * 1024,
        ),
        Some(240)
    );
    assert_eq!(
        runtime_smart_context_tool_preview_max_lines(
            runtime_proxy_crate::SmartContextTokenBudgetTier::Exact,
            usize::MAX,
        ),
        None
    );
}

#[test]
fn smart_context_prepare_rewrites_when_savings_and_critical_signals_preserved() {
    let shared = smart_context_test_shared("rewrite-savings");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let output = std::iter::once("error: failed at src/main.rs:10:5".to_string())
        .chain((0..500).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": output
        }]
    }));
    let before_len = request.body.len();

    let rewritten =
        prepare_runtime_smart_context_http_body(42, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = rewritten else {
        panic!("expected rewritten body");
    };
    assert!(body.len() < before_len);
    let text = String::from_utf8(body).unwrap();
    assert!(text.contains("prodex-artifact:sc:"));
    assert!(text.contains("error: failed at src/main.rs:10:5"));
}

#[test]
fn smart_context_prepare_rewrite_preserves_static_prompt_prefix_text() {
    let shared = smart_context_test_shared("rewrite-static-prefix");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    smart_context_observe_minimal_budget(&shared);
    let instructions = "Generated at: 2026-05-04T01:02:03Z\nKeep exact static prefix.  ";
    let system = "System prefix line one.\n\nSystem prefix line two.  ";
    let developer = "Developer prefix stays exact.\nUse repo rules.  ";
    let input_system = "Input system prefix\nwith blank lines.\n\nDo not rewrite.  ";
    let output = std::iter::once("error: failed at src/main.rs:10:5".to_string())
        .chain((0..500).map(|index| format!("line {index}: noisy build output")))
        .collect::<Vec<_>>()
        .join("\n");
    let request = smart_context_test_request(serde_json::json!({
        "instructions": instructions,
        "system": system,
        "developer": developer,
        "input": [
            {
                "role": "system",
                "content": input_system,
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": output,
            }
        ]
    }));

    let rewritten =
        prepare_runtime_smart_context_http_body(42, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = rewritten else {
        panic!("expected rewritten body");
    };
    let value = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(value["instructions"].as_str(), Some(instructions));
    assert_eq!(value["system"].as_str(), Some(system));
    assert_eq!(value["developer"].as_str(), Some(developer));
    assert_eq!(value["input"][0]["content"].as_str(), Some(input_system));
    assert!(
        value["input"][1]["output"]
            .as_str()
            .unwrap()
            .contains("prodex-artifact:sc:")
    );
}

#[test]
fn smart_context_regression_fallback_exact_on_quality_risk() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
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
    assert!(content.contains(&format!("prodex-artifact:{}", artifact.id)));
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
    let budget = runtime_smart_context_budget(
        &shared,
        32,
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
        changed_observation.changed,
    );

    assert!(!first_observation.changed);
    assert_eq!(first_observation.item_count, 1);
    assert!(!volatile_observation.changed);
    assert_eq!(volatile_observation.delta_count, 1);
    assert!(changed_observation.changed);
    assert_eq!(
        budget.policy.mode,
        runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough
    );
    assert!(
        budget
            .policy
            .reasons
            .contains(&runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged)
    );
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

#[test]
fn smart_context_reuses_existing_tool_output_artifact_with_short_ref() {
    let output = "repeat tool output ".repeat(200);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, &output).unwrap();
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_repeat",
            "output": output
        }]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_tool_outputs(
        &mut value,
        &mut store,
        2,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        256,
        &mut stats,
    );

    let rewritten = value["input"][0]["output"].as_str().unwrap();
    assert_eq!(rewritten, format!("prodex-artifact:{}", artifact.id));
    assert!(!rewritten.contains("repeat tool output repeat tool output"));
    assert_eq!(stats.artifacts_stored, 0);
    assert_eq!(stats.repeat_tool_output_refs, 1);
    assert_eq!(stats.tool_outputs_condensed, 1);

    let mut rehydrate_stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_rehydrate_value(&mut value, &store, &mut rehydrate_stats);
    assert_eq!(value["input"][0]["output"].as_str(), Some(output.as_str()));
    assert_eq!(rehydrate_stats.rehydrated_refs, 1);
}

#[test]
fn smart_context_compaction_appends_missing_critical_exact_ranges() {
    let original = "\
line 1 noisy
error: hidden failure
src/main.rs:22:5
line 4 noisy
";
    let compacted = "summary without failure".to_string();

    let repaired = runtime_smart_context_append_missing_critical_ranges(original, compacted, 8);

    assert!(repaired.contains("critical exact ranges:"));
    assert!(repaired.contains("L1-L4:"));
    assert!(repaired.contains("error: hidden failure"));
    assert!(repaired.contains("src/main.rs:22:5"));
    assert!(prodex_context::critical_signal_self_check(original, &repaired).passed());
}

#[test]
fn smart_context_surgical_rehydrate_adds_lost_critical_ranges() {
    let artifact_text = std::iter::once("setup".to_string())
        .chain(std::iter::once("error: hidden failure".to_string()))
        .chain(std::iter::once("src/main.rs:22:5".to_string()))
        .chain((0..200).map(|index| format!("noise line {index}")))
        .collect::<Vec<_>>()
        .join("\n");
    let shared = smart_context_test_shared("surgical-critical");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let artifact = with_runtime_smart_context_artifacts(&shared, |store| {
        store.insert_text(1, &artifact_text).unwrap()
    })
    .unwrap();
    let original = serde_json::to_vec(&serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": artifact_text
        }]
    }))
    .unwrap();
    let value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!("prodex-artifact:{}\nsummary without failure", artifact.id)
        }]
    });
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
    };

    let (body, repaired_stats) = runtime_smart_context_try_surgical_rehydrate_critical_ranges(
        &value,
        &shared,
        &original,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &[],
        &stats,
    )
    .expect("lost critical lines should be surgically rehydrated");

    let text = String::from_utf8(body).unwrap();
    assert!(text.contains("critical exact ranges"));
    assert!(text.contains(&format!("prodex-artifact:{}#L1-L4", artifact.id)));
    assert!(text.contains("error: hidden failure"));
    assert!(text.contains("src/main.rs:22:5"));
    assert!(repaired_stats.rehydrated_refs > stats.rehydrated_refs);
    assert!(
        prodex_context::critical_signal_self_check(&String::from_utf8_lossy(&original), &text)
            .passed()
    );
}

#[test]
fn smart_context_surgical_rehydrate_prefers_artifact_line_index() {
    let artifact_text = "\
setup
error: hidden indexed failure
src/main.rs:22:5
noise";
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store.insert_text(1, artifact_text).unwrap();
    let line_index = store
        .line_index(&artifact.id)
        .expect("inserted artifact should have line index");

    let (appendix, range_count) = runtime_smart_context_missing_critical_range_appendix(
        &artifact.id,
        "fallback text without indexed critical signals",
        Some(line_index),
        &format!("prodex-artifact:{}\nsummary without failure", artifact.id),
    )
    .expect("indexed critical range should be rehydrated");

    assert_eq!(range_count, 1);
    assert!(appendix.contains("critical exact ranges:"));
    assert!(appendix.contains(&format!("prodex-artifact:{}#L1-L4", artifact.id)));
    assert!(appendix.contains("error: hidden indexed failure"));
    assert!(appendix.contains("src/main.rs:22:5"));
}

#[test]
fn smart_context_surgical_rehydrate_falls_back_for_legacy_unindexed_artifact() {
    let artifact_text = "\
setup
error: legacy failure
src/main.rs:22:5
noise";
    let artifact_id = runtime_proxy_crate::smart_context_hash_text(artifact_text);

    let (appendix, range_count) = runtime_smart_context_missing_critical_range_appendix(
        &artifact_id,
        artifact_text,
        None,
        &format!("prodex-artifact:{artifact_id}\nsummary without failure"),
    )
    .expect("legacy artifact should still rehydrate by rescanning");

    assert_eq!(range_count, 1);
    assert!(appendix.contains(&format!("prodex-artifact:{artifact_id}#L1-L4")));
    assert!(appendix.contains("error: legacy failure"));
    assert!(appendix.contains("src/main.rs:22:5"));
}

#[test]
fn smart_context_minifies_structural_json_without_touching_strings() {
    let body = br#"{
      "input": [
        {
          "type": "message",
          "content": "keep  spaces\ninside string"
        }
      ]
    }"#;
    let value = serde_json::from_slice::<serde_json::Value>(body).unwrap();

    let minified = runtime_smart_context_minified_json_body(&value, body).unwrap();
    let text = String::from_utf8(minified).unwrap();

    assert!(text.len() < body.len());
    assert!(text.contains("keep  spaces\\ninside string"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        value
    );
}

#[test]
fn smart_context_prepare_minifies_exact_json_without_changing_payload() {
    let shared = smart_context_test_shared("prepare-minify-exact");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: vec![("x-prodex-smart-context".to_string(), "exact".to_string())],
        body: br#"{
          "input": [
            {
              "type": "message",
              "content": "keep  spaces\ninside string"
            }
          ]
        }"#
        .to_vec(),
    };
    let before = serde_json::from_slice::<serde_json::Value>(&request.body).unwrap();

    let prepared =
        prepare_runtime_smart_context_http_body(77, &request, &shared, RuntimeRouteKind::Responses);

    let Cow::Owned(body) = prepared else {
        panic!("expected minified body");
    };
    let after = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert!(body.len() < request.body.len());
    assert_eq!(after, before);
    assert_eq!(
        after["input"][0]["content"].as_str(),
        Some("keep  spaces\ninside string")
    );
}

#[test]
fn smart_context_prepare_passes_invalid_json_unchanged() {
    let shared = smart_context_test_shared("prepare-invalid-json");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: b"{ invalid\n".to_vec(),
    };

    let prepared =
        prepare_runtime_smart_context_http_body(78, &request, &shared, RuntimeRouteKind::Responses);

    assert!(matches!(&prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
}

#[test]
fn smart_context_self_check_passes_through_growth_without_rehydrate() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
    };

    assert_eq!(
        runtime_smart_context_rewrite_self_check(100, 101, &stats),
        "growth"
    );
    assert!(runtime_smart_context_should_pass_through_after_self_check(
        100, 101, &stats
    ));
}

fn smart_context_test_request(body: serde_json::Value) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: serde_json::to_vec(&body).unwrap(),
    }
}

fn smart_context_observe_minimal_budget(shared: &RuntimeRotationProxyShared) {
    observe_runtime_smart_context_token_usage(
        shared,
        RuntimeTokenUsage {
            input_tokens: 24_000,
            cached_input_tokens: 0,
            output_tokens: 7_000,
            reasoning_tokens: 1_000,
        },
    );
}

fn smart_context_test_shared(name: &str) -> RuntimeRotationProxyShared {
    static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);
    let unique = NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed);
    let root = env::temp_dir().join(format!(
        "prodex-smart-context-{name}-{}-{unique}",
        std::process::id()
    ));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    };

    RuntimeRotationProxyShared {
        upstream_no_proxy: false,
        async_client: reqwest::Client::new(),
        async_runtime: Arc::new(
            TokioRuntimeBuilder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
        ),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths,
            state: AppState::default(),
            upstream_base_url: "http://127.0.0.1".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        })),
        log_path: env::temp_dir().join(format!(
            "prodex-smart-context-{name}-{}-{unique}.log",
            std::process::id()
        )),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
            responses: 8,
            compact: 8,
            websocket: 8,
            standard: 8,
        }),
    }
}

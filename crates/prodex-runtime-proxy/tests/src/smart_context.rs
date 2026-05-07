use super::*;
use std::borrow::Cow;

#[path = "smart_context/golden.rs"]
mod golden;

#[test]
fn structural_minify_json_body_removes_json_whitespace_only() {
    let body = br#"{
        "message": " keep  spaces \n and { braces } ",
        "array": [ 1, true, { "nested": " x y " } ]
    }"#;

    let minified = smart_context_structural_minify_json_body(body);
    let before = serde_json::from_slice::<serde_json::Value>(body).unwrap();
    let after = serde_json::from_slice::<serde_json::Value>(minified.as_ref()).unwrap();

    assert!(matches!(&minified, Cow::Owned(_)));
    assert!(minified.len() < body.len());
    assert_eq!(after, before);
    assert_eq!(
        after["message"].as_str(),
        Some(" keep  spaces \n and { braces } ")
    );
    assert_eq!(
        minified.as_ref(),
        serde_json::to_vec(&before).unwrap().as_slice()
    );
}

#[test]
fn structural_minify_json_body_passes_invalid_json_unchanged() {
    let body = b"{ invalid\n";

    let minified = smart_context_structural_minify_json_body(body);

    assert!(matches!(&minified, Cow::Borrowed(_)));
    assert_eq!(minified.as_ref(), body);
}

#[test]
fn model_name_helpers_extract_full_or_prefix_json_and_reject_invalid_names() {
    let full = br#"{"model":" gpt-5.5 ","input":[]}"#;
    let padded = format!(
        r#"{{"model":"gpt-5.5-mini","input":"{}"}}"#,
        "x".repeat(8 * 1024)
    );

    assert_eq!(
        smart_context_model_name_from_body(full),
        Some("gpt-5.5".to_string())
    );
    assert_eq!(
        smart_context_model_name_from_body(padded.as_bytes()),
        Some("gpt-5.5-mini".to_string())
    );
    assert_eq!(smart_context_normalized_model_name(Some(" \n ")), None);
    assert_eq!(
        smart_context_normalized_model_name(Some("bad\u{0007}model")),
        None
    );
}

#[test]
fn exactness_guard_blocks_context_affinity_and_missing_rehydrate() {
    let guard = smart_context_exactness_guard(SmartContextExactnessInput {
        previous_response_id: Some("resp_1".to_string()),
        turn_state: Some("turn_1".to_string()),
        missing_rehydrate_refs: vec!["artifact-a".to_string()],
        ..SmartContextExactnessInput::default()
    });

    assert_eq!(guard.decision, SmartContextExactnessDecision::RequireExact);
    assert_eq!(
        guard.reasons,
        vec![
            SmartContextExactnessReason::PreviousResponseAffinity,
            SmartContextExactnessReason::TurnStateAffinity,
            SmartContextExactnessReason::RehydrateRequired,
        ]
    );
}

#[test]
fn condenser_uses_artifact_only_when_hash_matches() {
    let text = "0123456789abcdef".to_string();
    let matching_artifact = SmartContextArtifactRef {
        id: "artifact-a".to_string(),
        byte_len: text.len(),
        content_hash: smart_context_hash_text(&text),
    };
    let stale_artifact = SmartContextArtifactRef {
        id: "artifact-b".to_string(),
        byte_len: text.len(),
        content_hash: smart_context_hash_text("old"),
    };

    let condensed = smart_context_condense_tool_outputs(
        [
            SmartContextToolOutput {
                call_id: "call-a".to_string(),
                text: text.clone(),
                artifact: Some(matching_artifact.clone()),
            },
            SmartContextToolOutput {
                call_id: "call-b".to_string(),
                text: text.clone(),
                artifact: Some(stale_artifact),
            },
        ],
        8,
    );

    assert_eq!(
        condensed[0],
        SmartContextCondensedToolOutput::ArtifactBacked {
            call_id: "call-a".to_string(),
            artifact: matching_artifact,
            content_hash: smart_context_hash_text(&text),
            summary: "01234567".to_string(),
        }
    );
    assert!(matches!(
        condensed[1],
        SmartContextCondensedToolOutput::Inline { .. }
    ));
}

#[test]
fn artifact_marker_uses_short_app_format_and_preserves_rehydrate_metadata() {
    let artifact = SmartContextArtifactRef {
        id: "sc:0123456789abcdef".to_string(),
        byte_len: 12_345,
        content_hash: "sc:fedcba9876543210".to_string(),
    };
    let compacted = "first compacted line\nlast compacted line";

    let marker = smart_context_artifact_marker(&artifact, compacted);
    let old_reusable = format!(
        "prodex-sc artifact prodex-artifact:{} bytes={} hash={}; rehydrate: use prodex-artifact:{} or prodex-artifact:{}#Lstart-Lend",
        artifact.id, artifact.byte_len, artifact.content_hash, artifact.id, artifact.id
    );
    let old_labeled = format!(
        "# prodex smart context artifact\nartifact_id: prodex-artifact:{}\noriginal_bytes: {}\ncontent_hash: {}\nrehydrate: automatic when exact content is referenced; use prodex-artifact:{}#Lstart-Lend for exact line range\n\n{}",
        artifact.id, artifact.byte_len, artifact.content_hash, artifact.id, compacted
    );

    let first_line = marker.lines().next().unwrap();
    assert_eq!(
        first_line,
        "psc art psc:0123456789abcdef b=12345 lines=#Lx-Ly"
    );
    assert!(first_line.len() < old_reusable.len());
    assert!(marker.len() < old_labeled.len());
    assert_eq!(marker.matches("prodex-artifact:").count(), 0);
    assert!(marker.contains("b=12345"));
    assert!(!marker.contains("h=sc:fedcba9876543210"));
    assert!(marker.contains("psc:0123456789abcdef"));
    assert!(marker.contains("lines=#Lx-Ly"));
    assert!(marker.ends_with(compacted));
    assert!(!marker.contains("artifact_id:"));
    assert!(!marker.contains("original_bytes:"));
    assert!(!marker.contains("content_hash:"));
}

#[test]
fn artifact_reference_marker_uses_short_repeat_format_with_exact_ref_fields() {
    let artifact = SmartContextArtifactRef {
        id: "sc:0123456789abcdef".to_string(),
        byte_len: 456,
        content_hash: "sc:fedcba9876543210".to_string(),
    };

    let marker = smart_context_artifact_reference_marker(&artifact);
    let old_reusable = format!(
        "prodex-sc repeat prodex-artifact:{} bytes={} hash={}; rehydrate: use prodex-artifact:{} or prodex-artifact:{}#Lstart-Lend",
        artifact.id, artifact.byte_len, artifact.content_hash, artifact.id, artifact.id
    );

    assert_eq!(marker, "psc rep psc:0123456789abcdef b=456");
    assert!(marker.len() < old_reusable.len());
    assert_eq!(marker.matches("prodex-artifact:").count(), 0);
    assert!(marker.contains("psc:0123456789abcdef"));
    assert!(!marker.contains("h=sc:fedcba9876543210"));
    assert!(marker.contains("b=456"));
    assert!(!marker.contains("artifact_id:"));
    assert!(!marker.contains("original_bytes:"));
    assert!(!marker.contains("content_hash:"));
}

#[test]
fn artifact_short_ref_helpers_match_psc_line_refs() {
    assert_eq!(
        smart_context_short_artifact_ref("sc:0123456789abcdef"),
        "psc:0123456789abcdef"
    );
    assert_eq!(
        smart_context_short_artifact_ref("custom-id"),
        "psc:custom-id"
    );
    assert_eq!(
        smart_context_short_artifact_line_ref("sc:0123456789abcdef", 2, 4),
        "psc:0123456789abcdef#L2-L4"
    );
}

#[test]
fn conversation_dedupe_keeps_first_hash_ref() {
    let deduped = smart_context_conversation_dedupe([
        SmartContextConversationItem {
            id: "a".to_string(),
            text: "same".to_string(),
        },
        SmartContextConversationItem {
            id: "b".to_string(),
            text: "different".to_string(),
        },
        SmartContextConversationItem {
            id: "c".to_string(),
            text: "same".to_string(),
        },
    ]);

    assert_eq!(
        deduped[2],
        SmartContextDedupeItem::Duplicate {
            id: "c".to_string(),
            ref_id: "a".to_string(),
            content_hash: smart_context_normalized_command_output_hash_text("same"),
        }
    );
}

#[test]
fn volatile_command_output_normalizer_stabilizes_hash_only() {
    let first = "\x1b[32mFinished\x1b[0m at 2026-05-04T01:02:03Z in 1.23s /tmp/prodex-a/run-123 1/10 10% request_id=123e4567-e89b-12d3-a456-426614174000\n";
    let second = "\x1b[31mFinished\x1b[0m at 2026-05-05T09:08:07Z in 12345ms /tmp/prodex-b-long/run-999999 10/100 100% request_id=123e4567-e89b-12d3-a456-426614174999\n";

    let normalized = smart_context_normalize_volatile_command_output(first);

    assert_eq!(
        normalized.as_ref(),
        "Finished at <timestamp> in <duration> <tmp-path> <progress> <progress> request_id=<id>\n"
    );
    assert_eq!(
        smart_context_normalized_command_output_hash_text(first),
        smart_context_normalized_command_output_hash_text(second)
    );
    assert_ne!(
        smart_context_hash_text(first),
        smart_context_hash_text(second)
    );
}

#[test]
fn command_output_cache_matches_outputs_that_only_differ_by_volatile_values() {
    let previous_text = "Finished at 2026-05-04T01:02:03Z in 1.23s /tmp/prodex-a/run-123 1/10 10% request_id=123e4567-e89b-12d3-a456-426614174000\n".repeat(20);
    let current_text = "Finished at 2026-05-05T09:08:07Z in 12345ms /tmp/prodex-b-long/run-999999 10/100 100% request_id=123e4567-e89b-12d3-a456-426614174999\n".repeat(20);
    let previous = smart_context_command_output_cache_record("cmd-a", &previous_text);

    let rewrite = smart_context_command_output_cache_rewrite(SmartContextCommandOutputCacheInput {
        id: "cmd-b".to_string(),
        text: current_text.clone(),
        previous_records: vec![previous.clone()],
        min_replacement_bytes: 1024,
    });

    assert_ne!(previous_text, current_text);
    assert_ne!(previous.byte_len, current_text.len());
    assert_eq!(rewrite.record.byte_len, current_text.len());
    assert_eq!(rewrite.record.content_hash, previous.content_hash);
    assert!(matches!(
        rewrite.action,
        SmartContextCommandOutputCacheAction::ReplaceWithUnchangedSummary {
            ref_id,
            saved_tokens,
            critical_signal_count: 0,
        } if ref_id == "cmd-a" && saved_tokens > 0
    ));
    assert_ne!(rewrite.output, current_text);
    assert!(rewrite.output.contains("vn-repeat omitted"));
}

#[test]
fn conversation_dedupe_uses_volatile_normalized_decision_hash() {
    let first = "ok at 2026-05-04T01:02:03Z in 1.23s /tmp/prodex-a/run-123\n".repeat(20);
    let second = "ok at 2026-05-05T09:08:07Z in 987ms /tmp/prodex-b/run-999\n".repeat(20);

    let deduped = smart_context_conversation_dedupe([
        SmartContextConversationItem {
            id: "first".to_string(),
            text: first.clone(),
        },
        SmartContextConversationItem {
            id: "second".to_string(),
            text: second.clone(),
        },
    ]);

    let decision_hash = smart_context_normalized_command_output_hash_text(&first);
    assert_eq!(
        decision_hash,
        smart_context_normalized_command_output_hash_text(&second)
    );
    assert_eq!(
        deduped[1],
        SmartContextDedupeItem::Duplicate {
            id: "second".to_string(),
            ref_id: "first".to_string(),
            content_hash: decision_hash,
        }
    );
}

#[test]
fn volatile_normalization_does_not_change_exact_artifact_hashing() {
    let first = "line at 2026-05-04T01:02:03Z in 1.23s /tmp/prodex-a";
    let second = "line at 2026-05-05T09:08:07Z in 987ms /tmp/prodex-b";
    let artifact = SmartContextArtifactRef {
        id: "artifact-a".to_string(),
        byte_len: first.len(),
        content_hash: smart_context_hash_text(first),
    };

    assert_eq!(
        smart_context_normalized_command_output_hash_text(first),
        smart_context_normalized_command_output_hash_text(second)
    );
    assert_ne!(
        smart_context_hash_text(first),
        smart_context_hash_text(second)
    );
    assert!(smart_context_artifact_line_range(&artifact, second, 1, 1).is_none());
}

#[test]
fn cross_turn_duplicate_ref_plan_replaces_only_artifact_backed_large_text() {
    let repeated = "large repeated cross-turn payload".to_string();
    let small = "small".to_string();
    let missing = "large duplicate without artifact".to_string();
    let mismatch = "large duplicate with stale artifact length".to_string();
    let repeated_artifact = SmartContextArtifactRef {
        id: "artifact-repeat".to_string(),
        byte_len: repeated.len(),
        content_hash: smart_context_hash_text(&repeated),
    };

    let plan = smart_context_cross_turn_duplicate_ref_plan(
        [
            SmartContextConversationItem {
                id: "repeat".to_string(),
                text: repeated.clone(),
            },
            SmartContextConversationItem {
                id: "small".to_string(),
                text: small.clone(),
            },
            SmartContextConversationItem {
                id: "missing".to_string(),
                text: missing.clone(),
            },
            SmartContextConversationItem {
                id: "mismatch".to_string(),
                text: mismatch.clone(),
            },
        ],
        [
            repeated_artifact.clone(),
            SmartContextArtifactRef {
                id: "artifact-small".to_string(),
                byte_len: small.len(),
                content_hash: smart_context_hash_text(&small),
            },
            SmartContextArtifactRef {
                id: "artifact-stale-len".to_string(),
                byte_len: mismatch.len() + 1,
                content_hash: smart_context_hash_text(&mismatch),
            },
        ],
        16,
        &smart_context_exactness_guard(SmartContextExactnessInput::default()),
    );

    assert_eq!(plan.replaced_items, 1);
    assert_eq!(plan.replaced_bytes, repeated.len());
    assert_eq!(
        plan.actions[0],
        SmartContextCrossTurnDuplicateRefAction::ReplaceWithArtifactRef {
            id: "repeat".to_string(),
            artifact: repeated_artifact,
            content_hash: smart_context_hash_text(&repeated),
            byte_len: repeated.len(),
        }
    );
    assert_eq!(
        plan.actions[1],
        SmartContextCrossTurnDuplicateRefAction::Keep {
            id: "small".to_string(),
            content_hash: smart_context_hash_text(&small),
            byte_len: small.len(),
            reason: SmartContextCrossTurnDuplicateKeepReason::BelowMinByteThreshold,
        }
    );
    assert_eq!(
        plan.actions[2],
        SmartContextCrossTurnDuplicateRefAction::Keep {
            id: "missing".to_string(),
            content_hash: smart_context_hash_text(&missing),
            byte_len: missing.len(),
            reason: SmartContextCrossTurnDuplicateKeepReason::MissingArtifact,
        }
    );
    assert_eq!(
        plan.actions[3],
        SmartContextCrossTurnDuplicateRefAction::Keep {
            id: "mismatch".to_string(),
            content_hash: smart_context_hash_text(&mismatch),
            byte_len: mismatch.len(),
            reason: SmartContextCrossTurnDuplicateKeepReason::MissingArtifact,
        }
    );
}

#[test]
fn cross_turn_duplicate_ref_plan_keeps_when_exactness_required() {
    let text = "large repeated payload with artifact".to_string();
    let guard = smart_context_exactness_guard(SmartContextExactnessInput {
        exact_mode: true,
        ..SmartContextExactnessInput::default()
    });

    let plan = smart_context_cross_turn_duplicate_ref_plan(
        [SmartContextConversationItem {
            id: "repeat".to_string(),
            text: text.clone(),
        }],
        [SmartContextArtifactRef {
            id: "artifact-repeat".to_string(),
            byte_len: text.len(),
            content_hash: smart_context_hash_text(&text),
        }],
        1,
        &guard,
    );

    assert_eq!(plan.replaced_items, 0);
    assert_eq!(plan.replaced_bytes, 0);
    assert_eq!(
        plan.actions,
        vec![SmartContextCrossTurnDuplicateRefAction::Keep {
            id: "repeat".to_string(),
            content_hash: smart_context_hash_text(&text),
            byte_len: text.len(),
            reason: SmartContextCrossTurnDuplicateKeepReason::ExactnessRequired,
        }]
    );
}

#[test]
fn command_output_cache_replaces_exact_repeated_large_output_with_stable_summary() {
    let output = "running test suite: ok\n".repeat(400);
    let previous = smart_context_command_output_cache_record("cmd-a", &output);

    let rewrite = smart_context_command_output_cache_rewrite(SmartContextCommandOutputCacheInput {
        id: "cmd-b".to_string(),
        text: output.clone(),
        previous_records: vec![previous.clone()],
        min_replacement_bytes: 1024,
    });

    assert!(matches!(
        rewrite.action,
        SmartContextCommandOutputCacheAction::ReplaceWithUnchangedSummary {
            ref_id,
            saved_tokens,
            critical_signal_count: 0,
        } if ref_id == "cmd-a" && saved_tokens > 0
    ));
    assert_ne!(rewrite.output, output);
    assert!(rewrite.output.len() < output.len());
    assert!(rewrite.output.contains("psc co same"));
    assert!(rewrite.output.contains("id=cmd-b"));
    assert!(rewrite.output.contains("ref=cmd-a"));
    assert!(rewrite.output.contains(&previous.content_hash));
    assert!(rewrite.output.contains(&format!("b={}", previous.byte_len)));
    assert!(
        rewrite
            .output
            .contains(&format!("tok={}", previous.estimated_tokens))
    );
}

#[test]
fn command_output_cache_keeps_changed_output_exact_with_delta_summary() {
    let previous_text = "compile warning: old\n".repeat(300);
    let changed_text = format!(
        "{}compile warning: new\n",
        "compile warning: old\n".repeat(299)
    );
    let previous = smart_context_command_output_cache_record("cargo-test", &previous_text);

    let rewrite = smart_context_command_output_cache_rewrite(SmartContextCommandOutputCacheInput {
        id: "cargo-test".to_string(),
        text: changed_text.clone(),
        previous_records: vec![previous.clone()],
        min_replacement_bytes: 1024,
    });

    assert_eq!(rewrite.output, changed_text);
    match rewrite.action {
        SmartContextCommandOutputCacheAction::KeepExact {
            reason: SmartContextCommandOutputCacheKeepReason::ChangedSincePreviousOutput,
            summary: Some(summary),
        } => {
            assert!(summary.contains("psc co delta"));
            assert!(summary.contains("exact kept"));
            assert!(summary.contains(&previous.content_hash));
            assert!(summary.contains(&rewrite.record.content_hash));
        }
        other => panic!("unexpected action: {other:?}"),
    }
}

#[test]
fn command_output_cache_keeps_small_repeated_output_exact() {
    let output = "error: small but exact\n".to_string();
    let previous = smart_context_command_output_cache_record("cmd-a", &output);

    let rewrite = smart_context_command_output_cache_rewrite(SmartContextCommandOutputCacheInput {
        id: "cmd-b".to_string(),
        text: output.clone(),
        previous_records: vec![previous],
        min_replacement_bytes: 1024,
    });

    assert_eq!(rewrite.output, output);
    assert_eq!(
        rewrite.action,
        SmartContextCommandOutputCacheAction::KeepExact {
            reason: SmartContextCommandOutputCacheKeepReason::BelowMinByteThreshold,
            summary: None,
        }
    );
}

#[test]
fn command_output_cache_unchanged_summary_preserves_critical_signal_samples() {
    let output = format!(
        "{}error: build failed in crates/prodex-runtime-proxy/src/smart_context.rs\nthread 'main' panicked at assertion\n",
        "ok\n".repeat(1500)
    );
    let previous = smart_context_command_output_cache_record("cargo-test", &output);

    let rewrite = smart_context_command_output_cache_rewrite(SmartContextCommandOutputCacheInput {
        id: "cargo-test-repeat".to_string(),
        text: output.clone(),
        previous_records: vec![previous.clone()],
        min_replacement_bytes: 1024,
    });

    assert!(matches!(
        rewrite.action,
        SmartContextCommandOutputCacheAction::ReplaceWithUnchangedSummary {
            ref_id,
            critical_signal_count: 2,
            ..
        } if ref_id == "cargo-test"
    ));
    assert!(rewrite.output.len() < output.len());
    assert!(rewrite.output.contains("psc co crit n=2"));
    assert!(rewrite.output.contains("error: build failed"));
    assert!(rewrite.output.contains("panicked at assertion"));
    assert!(rewrite.output.contains(&previous.content_hash));
}

#[test]
fn memory_capsule_selection_prioritizes_required_then_relevance() {
    let selected = smart_context_select_memory_capsules(
        [
            SmartContextMemoryCapsule {
                id: "optional-low".to_string(),
                token_cost: 40,
                relevance: 0.1,
                required: false,
            },
            SmartContextMemoryCapsule {
                id: "required".to_string(),
                token_cost: 60,
                relevance: 0.0,
                required: true,
            },
            SmartContextMemoryCapsule {
                id: "optional-high".to_string(),
                token_cost: 30,
                relevance: 0.9,
                required: false,
            },
        ],
        90,
    );

    assert_eq!(selected.selected_ids, vec!["required", "optional-high"]);
    assert_eq!(selected.omitted_ids, vec!["optional-low"]);
    assert_eq!(selected.used_tokens, 90);
}

#[test]
fn memory_capsule_budget_bridge_scales_by_policy_mode_and_allows_safe_exact() {
    let minimal = smart_context_memory_capsule_policy_for_available_tokens(1_500);
    let condensed = smart_context_memory_capsule_policy_for_available_tokens(6_000);
    let large = smart_context_memory_capsule_policy_for_available_tokens(12_000);
    let exact = smart_context_memory_capsule_policy_for_available_tokens(40_000);

    assert_eq!(
        smart_context_memory_capsule_token_budget(&minimal.0, &minimal.1),
        SMART_CONTEXT_MEMORY_CAPSULE_MINIMAL_TOKEN_BUDGET
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget(&condensed.0, &condensed.1),
        SMART_CONTEXT_MEMORY_CAPSULE_CONDENSED_TOKEN_BUDGET
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget(&large.0, &large.1),
        SMART_CONTEXT_MEMORY_CAPSULE_LARGE_TOKEN_BUDGET
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget(&exact.0, &exact.1),
        usize::MAX
    );
}

#[test]
fn memory_capsule_budget_bridge_does_not_unbound_unsafe_exact_policy() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: None,
            reserved_output_tokens: 8_000,
            current_input_tokens: 12_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: accounting.clone(),
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(policy.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(
        smart_context_memory_capsule_token_budget(&accounting, &policy),
        0
    );
}

#[test]
fn memory_capsule_selection_keeps_scanning_after_oversized_required_capsule() {
    let minimal = smart_context_memory_capsule_policy_for_available_tokens(1_500);
    let selected = smart_context_select_memory_capsules_for_policy(
        [
            SmartContextMemoryCapsule {
                id: "required-a-too-large".to_string(),
                token_cost: 300,
                relevance: 0.0,
                required: true,
            },
            SmartContextMemoryCapsule {
                id: "required-z-small".to_string(),
                token_cost: 100,
                relevance: 0.0,
                required: true,
            },
            SmartContextMemoryCapsule {
                id: "optional-high".to_string(),
                token_cost: 100,
                relevance: 0.9,
                required: false,
            },
            SmartContextMemoryCapsule {
                id: "optional-low".to_string(),
                token_cost: 100,
                relevance: 0.1,
                required: false,
            },
        ],
        &minimal.0,
        &minimal.1,
    );

    assert_eq!(
        selected.selected_ids,
        vec!["required-z-small", "optional-high"]
    );
    assert_eq!(
        selected.omitted_ids,
        vec!["required-a-too-large", "optional-low"]
    );
    assert_eq!(selected.used_tokens, 200);
}

#[test]
fn rehydrate_plan_respects_artifacts_tier_and_budget() {
    let plan = smart_context_auto_rehydrate_plan(
        [
            SmartContextRehydrateRef {
                id: "required".to_string(),
                token_cost: 70,
                required: true,
            },
            SmartContextRehydrateRef {
                id: "missing".to_string(),
                token_cost: 10,
                required: false,
            },
            SmartContextRehydrateRef {
                id: "optional".to_string(),
                token_cost: 10,
                required: false,
            },
        ],
        ["required".to_string(), "optional".to_string()],
        80,
        SmartContextTokenBudgetTier::Minimal,
    );

    assert_eq!(
        plan.actions,
        vec![
            SmartContextRehydrateAction::Rehydrate {
                id: "required".to_string(),
                token_cost: 70,
            },
            SmartContextRehydrateAction::Defer {
                id: "missing".to_string(),
                reason: SmartContextRehydrateDeferReason::MissingArtifact,
            },
            SmartContextRehydrateAction::Defer {
                id: "optional".to_string(),
                reason: SmartContextRehydrateDeferReason::MinimalBudgetTier,
            },
        ]
    );
    assert_eq!(plan.used_tokens, 70);
}

#[test]
fn token_budget_tiers_are_stable_boundaries() {
    assert_eq!(
        smart_context_token_budget_tier(1_999),
        SmartContextTokenBudgetTier::Minimal
    );
    assert_eq!(
        smart_context_token_budget_tier(2_000),
        SmartContextTokenBudgetTier::Condensed
    );
    assert_eq!(
        smart_context_token_budget_tier(8_000),
        SmartContextTokenBudgetTier::Large
    );
    assert_eq!(
        smart_context_token_budget_tier(16_000),
        SmartContextTokenBudgetTier::Exact
    );
}

#[test]
fn observed_token_accounting_uses_real_usage_and_available_window() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 40_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: vec![
                RuntimeTokenUsage {
                    input_tokens: 20_000,
                    cached_input_tokens: 5_000,
                    output_tokens: 1_000,
                    reasoning_tokens: 400,
                },
                RuntimeTokenUsage {
                    input_tokens: 42_000,
                    cached_input_tokens: 10_000,
                    output_tokens: 2_000,
                    reasoning_tokens: 700,
                },
            ],
        });

    assert_eq!(accounting.observed_turns, 2);
    assert_eq!(accounting.observed_input_tokens, 62_000);
    assert_eq!(accounting.observed_cached_input_tokens, 15_000);
    assert_eq!(accounting.observed_uncached_input_tokens, 47_000);
    assert_eq!(accounting.observed_output_tokens, 3_000);
    assert_eq!(accounting.observed_reasoning_tokens, 1_100);
    assert_eq!(accounting.observed_total_tokens, 65_000);
    assert_eq!(accounting.observed_context_tokens, 66_100);
    assert_eq!(accounting.last_input_tokens, 42_000);
    assert_eq!(accounting.last_accounted_input_tokens, 42_000);
    assert_eq!(accounting.last_observed_context_tokens, 44_700);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::ObservedHistory
    );
    assert_eq!(accounting.effective_input_tokens, 42_000);
    assert_eq!(accounting.available_context_tokens, Some(78_000));
    assert!(accounting.accounting_risks.is_empty());
    assert_eq!(
        smart_context_token_budget_tier_from_accounting(&accounting),
        SmartContextTokenBudgetTier::Exact
    );
}

#[test]
fn observed_token_accounting_uses_cached_only_history_as_input_fallback() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(32_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: vec![RuntimeTokenUsage {
                cached_input_tokens: 16_000,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert_eq!(accounting.last_input_tokens, 0);
    assert_eq!(accounting.last_accounted_input_tokens, 16_000);
    assert_eq!(accounting.last_observed_context_tokens, 16_000);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::ObservedHistory
    );
    assert_eq!(accounting.effective_input_tokens, 16_000);
    assert_eq!(accounting.available_context_tokens, Some(12_000));
    assert!(accounting.accounting_risks.is_empty());
}

#[test]
fn observed_token_accounting_uses_body_estimate_when_tokens_unknown() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 80_001,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    assert_eq!(
        smart_context_estimate_tokens_from_body_bytes(80_001),
        20_001
    );
    assert_eq!(accounting.estimated_current_request_tokens, 20_001);
    assert_eq!(accounting.current_request_accounted_tokens, 20_001);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    );
    assert_eq!(accounting.effective_input_tokens, 20_001);
    assert_eq!(accounting.available_context_tokens, Some(39_999));
    assert!(smart_context_accounting_safe_for_adaptive_policy(
        &accounting
    ));
}

#[test]
fn observed_token_accounting_accepts_content_aware_body_estimate() {
    let body = "word ".repeat(100);
    let estimate = smart_context_estimate_tokens_from_body(body.as_bytes());

    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(32_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: body.len(),
            current_request_estimated_tokens: Some(estimate),
            observed_usage: Vec::new(),
        });

    assert!(estimate > 0);
    assert!(estimate < smart_context_estimate_tokens_from_body_bytes(body.len()));
    assert_eq!(accounting.estimated_current_request_tokens, estimate);
    assert_eq!(accounting.current_request_accounted_tokens, estimate);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    );
}

#[test]
fn observed_token_accounting_calibrates_body_estimate_from_recent_usage_with_floor() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 80_000,
            current_request_estimated_tokens: Some(20_000),
            observed_usage: vec![RuntimeTokenUsage {
                input_tokens: 8_000,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert_eq!(accounting.estimated_current_request_tokens, 10_064);
    assert_eq!(accounting.current_request_accounted_tokens, 10_064);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    );
    assert_eq!(accounting.available_context_tokens, Some(49_936));
}

#[test]
fn observed_token_accounting_with_calibration_preserves_unbucketed_behavior() {
    let input = SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(64_000),
        reserved_output_tokens: 4_000,
        current_input_tokens: 0,
        current_request_body_bytes: 80_000,
        current_request_estimated_tokens: Some(20_000),
        observed_usage: vec![RuntimeTokenUsage {
            input_tokens: 8_000,
            ..RuntimeTokenUsage::default()
        }],
    };

    let legacy = smart_context_observed_token_accounting(input.clone());
    let explicit = smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: input,
            calibration_bucket_key: None,
            calibration_samples: Vec::new(),
        },
    );

    assert_eq!(explicit, legacy);
    assert_eq!(explicit.estimated_current_request_tokens, 10_064);
}

#[test]
fn observed_token_accounting_calibrates_separately_by_bucket() {
    let responses_bucket = SmartContextTokenCalibrationBucketKey {
        route: Some("responses".to_string()),
        model: Some("gpt-5".to_string()),
        profile: Some("alpha".to_string()),
        transport: Some("http".to_string()),
    };
    let compact_bucket = SmartContextTokenCalibrationBucketKey {
        route: Some("compact".to_string()),
        model: Some("gpt-5".to_string()),
        profile: Some("alpha".to_string()),
        transport: Some("http".to_string()),
    };
    let samples = vec![
        SmartContextTokenCalibrationSample {
            bucket_key: Some(compact_bucket.clone()),
            usage: RuntimeTokenUsage {
                input_tokens: 30_000,
                ..RuntimeTokenUsage::default()
            },
        },
        SmartContextTokenCalibrationSample {
            bucket_key: Some(responses_bucket.clone()),
            usage: RuntimeTokenUsage {
                input_tokens: 8_000,
                ..RuntimeTokenUsage::default()
            },
        },
    ];

    let responses = smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(64_000),
                reserved_output_tokens: 4_000,
                current_input_tokens: 0,
                current_request_body_bytes: 80_000,
                current_request_estimated_tokens: Some(20_000),
                observed_usage: Vec::new(),
            },
            calibration_bucket_key: Some(responses_bucket),
            calibration_samples: samples.clone(),
        },
    );
    let compact = smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(64_000),
                reserved_output_tokens: 4_000,
                current_input_tokens: 0,
                current_request_body_bytes: 80_000,
                current_request_estimated_tokens: Some(20_000),
                observed_usage: Vec::new(),
            },
            calibration_bucket_key: Some(compact_bucket),
            calibration_samples: samples,
        },
    );

    assert_eq!(responses.estimated_current_request_tokens, 10_064);
    assert_eq!(compact.estimated_current_request_tokens, 20_000);
}

#[test]
fn observed_token_accounting_prefers_exact_bucket_before_calibration_fallbacks() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some("gpt-5.4"),
        Some("alpha"),
        Some("http"),
    );
    let samples = vec![
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("gpt-5.4"),
                Some("beta"),
                Some("websocket"),
            )),
            16_000,
        ),
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("other-model"),
                Some("alpha"),
                Some("http"),
            )),
            17_000,
        ),
        smart_context_test_calibration_sample(None, 18_000),
        smart_context_test_calibration_sample(Some(target.clone()), 8_000),
    ];

    let estimate = smart_context_test_calibrated_bucket_estimate(target, samples);

    assert_eq!(estimate, 10_064);
}

#[test]
fn observed_token_accounting_falls_back_to_model_family_before_profile_route() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some(" GPT_5.4-2026-05-01 "),
        Some("alpha"),
        Some("http"),
    );
    let samples = vec![
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("other-model"),
                Some("alpha"),
                Some("websocket"),
            )),
            16_000,
        ),
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("compact"),
                Some("gpt-5.4"),
                Some("beta"),
                Some("websocket"),
            )),
            12_000,
        ),
    ];

    let estimate = smart_context_test_calibrated_bucket_estimate(target, samples);

    assert_eq!(estimate, 13_564);
}

#[test]
fn observed_token_accounting_falls_back_to_profile_route_before_route_transport() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some("gpt-5.2"),
        Some("alpha"),
        Some("http"),
    );
    let samples = vec![
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("other-model"),
                Some("beta"),
                Some("http"),
            )),
            17_000,
        ),
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("gpt-5.4"),
                Some("alpha"),
                Some("websocket"),
            )),
            12_000,
        ),
    ];

    let estimate = smart_context_test_calibrated_bucket_estimate(target, samples);

    assert_eq!(estimate, 13_564);
}

#[test]
fn observed_token_accounting_falls_back_to_route_transport_and_global_compatible_samples() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some("gpt-5.2"),
        Some("alpha"),
        Some("http"),
    );
    let route_transport_samples = vec![smart_context_test_calibration_sample(
        Some(smart_context_test_calibration_bucket(
            Some("responses"),
            Some("other-model"),
            Some("beta"),
            Some("http"),
        )),
        12_000,
    )];
    let global_samples = vec![smart_context_test_calibration_sample(None, 14_000)];

    let route_transport_estimate =
        smart_context_test_calibrated_bucket_estimate(target.clone(), route_transport_samples);
    let global_estimate = smart_context_test_calibrated_bucket_estimate(target, global_samples);

    assert_eq!(route_transport_estimate, 13_564);
    assert_eq!(global_estimate, 15_814);
}

fn smart_context_test_calibration_bucket(
    route: Option<&str>,
    model: Option<&str>,
    profile: Option<&str>,
    transport: Option<&str>,
) -> SmartContextTokenCalibrationBucketKey {
    SmartContextTokenCalibrationBucketKey {
        route: route.map(str::to_string),
        model: model.map(str::to_string),
        profile: profile.map(str::to_string),
        transport: transport.map(str::to_string),
    }
}

fn smart_context_test_calibration_sample(
    bucket_key: Option<SmartContextTokenCalibrationBucketKey>,
    input_tokens: u64,
) -> SmartContextTokenCalibrationSample {
    SmartContextTokenCalibrationSample {
        bucket_key,
        usage: RuntimeTokenUsage {
            input_tokens,
            ..RuntimeTokenUsage::default()
        },
    }
}

fn smart_context_test_calibrated_bucket_estimate(
    bucket_key: SmartContextTokenCalibrationBucketKey,
    samples: Vec<SmartContextTokenCalibrationSample>,
) -> u64 {
    smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(64_000),
                reserved_output_tokens: 4_000,
                current_input_tokens: 0,
                current_request_body_bytes: 80_000,
                current_request_estimated_tokens: Some(20_000),
                observed_usage: Vec::new(),
            },
            calibration_bucket_key: Some(bucket_key),
            calibration_samples: samples,
        },
    )
    .estimated_current_request_tokens
}

#[test]
fn observed_token_accounting_uses_recent_high_water_mark_for_calibration_safety() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 80_000,
            current_request_estimated_tokens: Some(20_000),
            observed_usage: vec![
                RuntimeTokenUsage {
                    input_tokens: 18_000,
                    ..RuntimeTokenUsage::default()
                },
                RuntimeTokenUsage {
                    input_tokens: 8_000,
                    ..RuntimeTokenUsage::default()
                },
            ],
        });

    assert_eq!(
        accounting.estimated_current_request_tokens, 20_000,
        "recent high-water usage should prevent unsafe over-shrink"
    );
    assert_eq!(accounting.available_context_tokens, Some(40_000));
}

#[test]
fn observed_token_accounting_does_not_inflate_small_body_from_history_calibration() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 48_000,
            current_request_body_bytes: b"small current request body payload".len(),
            current_request_estimated_tokens: Some(smart_context_estimate_tokens_from_body(
                b"small current request body payload",
            )),
            observed_usage: vec![RuntimeTokenUsage {
                input_tokens: 48_000,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert!(
        accounting.estimated_current_request_tokens < 100,
        "history calibration must not inflate a small current body into prior context size"
    );
    assert_eq!(accounting.current_request_accounted_tokens, 48_000);
    assert_eq!(accounting.available_context_tokens, Some(12_000));
}

#[test]
fn observed_token_accounting_uses_larger_history_to_avoid_underbudget() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 10_000,
            current_request_body_bytes: 20_000,
            current_request_estimated_tokens: None,
            observed_usage: vec![RuntimeTokenUsage {
                input_tokens: 100_000,
                output_tokens: 2_000,
                reasoning_tokens: 500,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert_eq!(accounting.current_request_accounted_tokens, 10_000);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::ObservedHistory
    );
    assert_eq!(accounting.effective_input_tokens, 100_000);
    assert_eq!(accounting.available_context_tokens, Some(20_000));
}

#[test]
fn artifact_line_range_refs_are_hash_checked_and_exact() {
    let text = "one\ntwo\nthree\nfour";
    let artifact = SmartContextArtifactRef {
        id: "artifact-lines".to_string(),
        byte_len: text.len(),
        content_hash: smart_context_hash_text(text),
    };

    let range = smart_context_artifact_line_range(&artifact, text, 2, 3).unwrap();

    assert_eq!(range.excerpt, "two\nthree");
    assert_eq!(range.reference.artifact_id, "artifact-lines");
    assert_eq!(
        range.reference.artifact_content_hash,
        smart_context_hash_text(text)
    );
    assert_eq!(range.reference.start_line, 2);
    assert_eq!(range.reference.end_line, 3);
    assert_eq!(
        range.reference.excerpt_hash,
        smart_context_hash_text("two\nthree")
    );
    assert_eq!(range.reference.excerpt_byte_len, "two\nthree".len());

    let stale_artifact = SmartContextArtifactRef {
        id: "artifact-lines".to_string(),
        byte_len: text.len(),
        content_hash: smart_context_hash_text("old"),
    };
    assert!(smart_context_artifact_line_range(&stale_artifact, text, 2, 3).is_none());
    assert!(smart_context_extract_line_range(text, 0, 1).is_none());
    assert!(smart_context_extract_line_range(text, 3, 2).is_none());
}

#[test]
fn fingerprint_delta_tracks_static_context_across_turns() {
    let previous = smart_context_fingerprints([
        SmartContextFingerprintInput {
            id: "AGENTS.md".to_string(),
            kind: SmartContextFingerprintKind::StaticContext,
            text: "rules-v1".to_string(),
        },
        SmartContextFingerprintInput {
            id: "turn-a".to_string(),
            kind: SmartContextFingerprintKind::ConversationTurn,
            text: "same".to_string(),
        },
        SmartContextFingerprintInput {
            id: "old-tool".to_string(),
            kind: SmartContextFingerprintKind::ToolOutput,
            text: "gone".to_string(),
        },
    ]);
    let current = smart_context_fingerprints([
        SmartContextFingerprintInput {
            id: "AGENTS.md".to_string(),
            kind: SmartContextFingerprintKind::StaticContext,
            text: "rules-v2".to_string(),
        },
        SmartContextFingerprintInput {
            id: "turn-a".to_string(),
            kind: SmartContextFingerprintKind::ConversationTurn,
            text: "same".to_string(),
        },
        SmartContextFingerprintInput {
            id: "new-artifact".to_string(),
            kind: SmartContextFingerprintKind::Artifact,
            text: "fresh".to_string(),
        },
    ]);

    let delta = smart_context_fingerprint_delta(previous, current);

    assert!(matches!(
        &delta[0],
        SmartContextFingerprintChange::Changed { before, after }
            if before.id == "AGENTS.md"
                && after.id == "AGENTS.md"
                && before.content_hash != after.content_hash
    ));
    assert!(matches!(
        &delta[1],
        SmartContextFingerprintChange::Unchanged { fingerprint }
            if fingerprint.id == "turn-a"
    ));
    assert!(matches!(
        &delta[2],
        SmartContextFingerprintChange::Removed { fingerprint }
            if fingerprint.id == "old-tool"
    ));
    assert!(matches!(
        &delta[3],
        SmartContextFingerprintChange::Added { fingerprint }
            if fingerprint.id == "new-artifact"
    ));
}

#[test]
fn static_context_prompt_cache_fingerprint_is_input_order_stable() {
    let left = smart_context_static_context_prompt_cache_fingerprint([
        SmartContextStaticContextItem {
            id: "README.md".to_string(),
            text: "usage\n".to_string(),
        },
        SmartContextStaticContextItem {
            id: "AGENTS.md".to_string(),
            text: "rules\n".to_string(),
        },
    ]);
    let right = smart_context_static_context_prompt_cache_fingerprint([
        SmartContextStaticContextItem {
            id: " AGENTS.md ".to_string(),
            text: "rules".to_string(),
        },
        SmartContextStaticContextItem {
            id: "README.md".to_string(),
            text: "usage".to_string(),
        },
    ]);

    assert_eq!(left, right);
    assert_eq!(left.items[0].id, "AGENTS.md");
    assert_eq!(left.items[1].id, "README.md");
    assert!(left.content_hash.starts_with("scpc:"));
}

#[test]
fn static_context_prompt_cache_fingerprint_uses_prompt_prefix_order() {
    let fingerprint = smart_context_static_context_prompt_cache_fingerprint([
        SmartContextStaticContextItem {
            id: "input[10].developer".to_string(),
            text: "developer ten".to_string(),
        },
        SmartContextStaticContextItem {
            id: "README.md".to_string(),
            text: "usage".to_string(),
        },
        SmartContextStaticContextItem {
            id: "developer".to_string(),
            text: "developer top".to_string(),
        },
        SmartContextStaticContextItem {
            id: "input[2].system".to_string(),
            text: "system two".to_string(),
        },
        SmartContextStaticContextItem {
            id: "system".to_string(),
            text: "system top".to_string(),
        },
        SmartContextStaticContextItem {
            id: "instructions".to_string(),
            text: "instructions top".to_string(),
        },
        SmartContextStaticContextItem {
            id: "input[2].developer".to_string(),
            text: "developer two".to_string(),
        },
    ]);

    assert_eq!(
        fingerprint
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "instructions",
            "system",
            "developer",
            "input[2].system",
            "input[2].developer",
            "input[10].developer",
            "README.md",
        ]
    );
}

#[test]
fn static_context_stabilizer_ignores_timestamp_noise() {
    let first = smart_context_static_context_prompt_cache_fingerprint([
        SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: "\r\nGenerated at: 2026-05-04T01:02:03Z\r\nRules  \r\n<!-- prodex current_date: 2026-05-04 -->\r\nKeep affinity\r\n"
                .to_string(),
        },
    ]);
    let second = smart_context_static_context_prompt_cache_fingerprint([
        SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: "Generated at: 2027-01-02T03:04:05Z\nRules\n<!-- prodex current_date: 2027-01-02 -->\nKeep affinity\n"
                .to_string(),
        },
    ]);

    assert_eq!(first.content_hash, second.content_hash);
    assert_eq!(first.items[0].canonical_text, "Rules\nKeep affinity");
    assert_eq!(
        first.items[0].content_hash,
        smart_context_hash_text("Rules\nKeep affinity")
    );
}

#[test]
fn static_context_prompt_cache_normalizes_inline_volatile_noise() {
    let first_text = "Trace request_id=123e4567-e89b-12d3-a456-426614174000 session_id=sess_alpha_123456789 path=/tmp/prodex-a/run-123 at 2026-05-04T01:02:03Z\nRule: Keep profile affinity\n";
    let second_text = "Trace request_id=123e4567-e89b-12d3-a456-426614174999 session_id=sess_beta_999999999 path=/tmp/prodex-b/run-999 at 2026-05-05T09:08:07Z\nRule: Keep profile affinity\n";

    let first_canonical = smart_context_stabilize_static_context_text(first_text);
    let second_canonical = smart_context_stabilize_static_context_text(second_text);
    let first =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: first_text.to_string(),
        }]);
    let second =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: second_text.to_string(),
        }]);

    assert_eq!(
        first_canonical,
        "Trace request_id=<id> session_id=<id> path=<tmp-path> at <timestamp>\nRule: Keep profile affinity"
    );
    assert_eq!(first_canonical, second_canonical);
    assert_eq!(first.content_hash, second.content_hash);
    assert_eq!(first.items[0].canonical_text, first_canonical);
}

#[test]
fn static_context_prompt_cache_still_changes_on_substantive_text_with_volatile_noise() {
    let before =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: "Trace request_id=123e4567-e89b-12d3-a456-426614174000 path=/tmp/prodex-a/run-123 at 2026-05-04T01:02:03Z\nRule: Keep profile affinity\n".to_string(),
        }]);
    let after =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: "Trace request_id=123e4567-e89b-12d3-a456-426614174999 path=/tmp/prodex-b/run-999 at 2026-05-05T09:08:07Z\nRule: Allow mid-stream rotation\n".to_string(),
        }]);

    assert_ne!(before.content_hash, after.content_hash);
    assert_ne!(
        before.items[0].canonical_text,
        after.items[0].canonical_text
    );
    assert!(
        before.items[0]
            .canonical_text
            .contains("Keep profile affinity")
    );
    assert!(
        after.items[0]
            .canonical_text
            .contains("Allow mid-stream rotation")
    );
}

#[test]
fn static_context_prompt_cache_fingerprint_changes_on_substantive_text() {
    let before =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "AGENTS.md".to_string(),
            text: "Generated at: 2026-05-04T01:02:03Z\nPreserve affinity\n".to_string(),
        }]);
    let after =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "AGENTS.md".to_string(),
            text: "Generated at: 2027-01-02T03:04:05Z\nAllow rotation\n".to_string(),
        }]);

    assert_ne!(before.content_hash, after.content_hash);
    assert_ne!(before.items[0].content_hash, after.items[0].content_hash);
}

#[test]
fn adaptive_budget_policy_prefers_safe_exact_when_required() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 125_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let guard = smart_context_exactness_guard(SmartContextExactnessInput {
        previous_response_id: Some("resp-owned".to_string()),
        ..SmartContextExactnessInput::default()
    });

    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: guard,
        accounting,
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(policy.tier, SmartContextTokenBudgetTier::Minimal);
    assert_eq!(policy.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(
        policy.reasons,
        vec![SmartContextBudgetPolicyReason::ExactnessRequired]
    );
}

#[test]
fn adaptive_budget_policy_uses_condensed_and_minimal_modes_by_real_budget() {
    let condensed =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 114_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let minimal =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 119_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    let condensed_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: condensed,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });
    let minimal_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: minimal,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });

    assert_eq!(
        condensed_policy.tier,
        SmartContextTokenBudgetTier::Condensed
    );
    assert_eq!(
        condensed_policy.mode,
        SmartContextBudgetMode::ArtifactCondensed
    );
    assert_eq!(condensed_policy.max_inline_tool_output_bytes, 8 * 1024);
    assert_eq!(
        condensed_policy.reasons,
        vec![SmartContextBudgetPolicyReason::TightBudget]
    );
    assert_eq!(minimal_policy.tier, SmartContextTokenBudgetTier::Minimal);
    assert_eq!(minimal_policy.mode, SmartContextBudgetMode::MinimalRefsOnly);
    assert_eq!(minimal_policy.max_inline_bytes, 1024);
    assert_eq!(minimal_policy.max_inline_tool_output_bytes, 1024);
}

#[test]
fn adaptive_budget_policy_caps_rehydrate_budget_to_available_tokens() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(20_000),
            reserved_output_tokens: 0,
            current_input_tokens: 11_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting,
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(policy.tier, SmartContextTokenBudgetTier::Large);
    assert_eq!(policy.max_inline_bytes, 32 * 1024);
    assert_eq!(policy.max_inline_tool_output_bytes, 32 * 1024);
    assert_eq!(policy.max_rehydrate_tokens, 9_000);
}

#[test]
fn adaptive_budget_policy_falls_back_exact_when_accounting_unknown_or_unsafe() {
    let unknown =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: None,
            reserved_output_tokens: 8_000,
            current_input_tokens: 12_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let unsafe_accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(8_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 1,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    let unknown_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: unknown,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });
    let unsafe_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: unsafe_accounting,
            recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });

    assert_eq!(
        unknown_policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(
        unknown_policy.reasons,
        vec![SmartContextBudgetPolicyReason::UnknownTokenWindow]
    );
    assert_eq!(unsafe_policy.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(
        unsafe_policy.reasons,
        vec![SmartContextBudgetPolicyReason::UnsafeAccounting]
    );
    assert_eq!(unsafe_policy.max_inline_bytes, usize::MAX);
}

#[test]
fn adaptive_budget_policy_expands_preview_only_after_recent_safe_savings() {
    let large = smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(64_000),
        reserved_output_tokens: 4_096,
        current_input_tokens: 48_000,
        current_request_body_bytes: 0,
        current_request_estimated_tokens: None,
        observed_usage: Vec::new(),
    });
    let exact = smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(128_000),
        reserved_output_tokens: 4_096,
        current_input_tokens: 32_000,
        current_request_body_bytes: 0,
        current_request_estimated_tokens: None,
        observed_usage: Vec::new(),
    });
    let safe = SmartContextRecentRewriteSafety {
        safe_rewrites: 2,
        fallback_rewrites: 0,
        saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
    };
    let mixed = SmartContextRecentRewriteSafety {
        safe_rewrites: 2,
        fallback_rewrites: 1,
        saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
    };

    let large_safe = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: large.clone(),
        recent_rewrite_safety: safe,
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });
    let large_mixed = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: large,
        recent_rewrite_safety: mixed,
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });
    let exact_safe = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: exact,
        recent_rewrite_safety: safe,
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(large_safe.tier, SmartContextTokenBudgetTier::Large);
    assert_eq!(large_safe.max_inline_tool_output_bytes, 64 * 1024);
    assert!(
        large_safe
            .reasons
            .contains(&SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe)
    );
    assert_eq!(
        large_mixed.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert!(
        !large_mixed
            .reasons
            .contains(&SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe)
    );
    assert_eq!(exact_safe.tier, SmartContextTokenBudgetTier::Exact);
    assert_eq!(exact_safe.mode, SmartContextBudgetMode::ExactPassThrough);
    assert_eq!(exact_safe.max_inline_tool_output_bytes, usize::MAX);
    assert!(
        !exact_safe
            .reasons
            .contains(&SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe)
    );
}

#[test]
fn recent_rewrite_safety_requires_savings_without_fallbacks() {
    assert!(!smart_context_recent_rewrite_safety_allows_larger_preview(
        &SmartContextRecentRewriteSafety {
            safe_rewrites: 2,
            fallback_rewrites: 0,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2 - 1,
        }
    ));
    assert!(!smart_context_recent_rewrite_safety_allows_larger_preview(
        &SmartContextRecentRewriteSafety {
            safe_rewrites: 1,
            fallback_rewrites: 1,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
        }
    ));
    assert!(smart_context_recent_rewrite_safety_allows_larger_preview(
        &SmartContextRecentRewriteSafety {
            safe_rewrites: 1,
            fallback_rewrites: 0,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
        }
    ));
}

#[test]
fn rewrite_telemetry_budget_decision_relaxes_after_safe_savings() {
    let decision =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
                smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            ],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    assert_eq!(decision, SmartContextRewriteBudgetDecision::Relax);
}

#[test]
fn rewrite_telemetry_budget_decision_tightens_after_fallback_or_weak_savings() {
    let fallback =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![SmartContextRewriteTelemetrySample {
                fallback: true,
                ..smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000)
            }],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });
    let weak =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![
                smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400),
                smart_context_test_rewrite_telemetry_sample(8_000, 7_200, 2_000, 1_900),
            ],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    assert_eq!(fallback, SmartContextRewriteBudgetDecision::Tighten);
    assert_eq!(weak, SmartContextRewriteBudgetDecision::Tighten);
}

#[test]
fn rewrite_telemetry_budget_decision_keeps_neutral_for_moderate_safe_savings() {
    let decision =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: vec![
                smart_context_test_rewrite_telemetry_sample(10_000, 7_500, 2_500, 2_100),
                smart_context_test_rewrite_telemetry_sample(8_000, 6_000, 2_000, 1_650),
            ],
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    assert_eq!(decision, SmartContextRewriteBudgetDecision::NoChange);
}

#[test]
fn per_transform_rewrite_safety_scores_categories_independently() {
    let samples = vec![
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::CommandOutputCache,
            smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::CommandOutputCache,
            smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::StaticContextPromptCache,
            smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_450),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::StaticContextPromptCache,
            smart_context_test_rewrite_telemetry_sample(8_000, 7_200, 2_000, 1_950),
        ),
        smart_context_test_transform_rewrite_telemetry_sample(
            SmartContextTransformCategory::CrossTurnDuplicateRef,
            SmartContextRewriteTelemetrySample {
                fallback: true,
                ..smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000)
            },
        ),
    ];
    let global =
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            telemetry_samples: samples.iter().map(|sample| sample.sample).collect(),
            ..SmartContextRewriteTelemetryBudgetInput::default()
        });

    let scores = smart_context_per_transform_rewrite_safety_scores(
        SmartContextPerTransformRewriteSafetyInput {
            recent_rewrite_safety: Vec::new(),
            telemetry_samples: samples,
        },
    );
    let command = smart_context_test_transform_score(
        &scores,
        SmartContextTransformCategory::CommandOutputCache,
    );
    let static_context = smart_context_test_transform_score(
        &scores,
        SmartContextTransformCategory::StaticContextPromptCache,
    );
    let cross_turn = smart_context_test_transform_score(
        &scores,
        SmartContextTransformCategory::CrossTurnDuplicateRef,
    );

    assert_eq!(global, SmartContextRewriteBudgetDecision::Tighten);
    assert_eq!(command.decision, SmartContextRewriteBudgetDecision::Relax);
    assert_eq!(command.safety_score, 100);
    assert_eq!(
        command.reasons,
        vec![SmartContextTransformRewriteSafetyReason::StableSavings]
    );
    assert_eq!(
        static_context.decision,
        SmartContextRewriteBudgetDecision::Tighten
    );
    assert_eq!(
        static_context.reasons,
        vec![SmartContextTransformRewriteSafetyReason::WeakSavings]
    );
    assert_eq!(
        cross_turn.decision,
        SmartContextRewriteBudgetDecision::Tighten
    );
    assert_eq!(cross_turn.fallback_samples, 1);
    assert_eq!(
        cross_turn.reasons,
        vec![SmartContextTransformRewriteSafetyReason::FallbackObserved]
    );
}

#[test]
fn rewrite_budget_application_respects_bounds() {
    let policy = SmartContextAdaptiveBudgetPolicy {
        tier: SmartContextTokenBudgetTier::Condensed,
        mode: SmartContextBudgetMode::ArtifactCondensed,
        max_inline_bytes: 300,
        max_inline_tool_output_bytes: 300,
        max_rehydrate_tokens: 2,
        reasons: vec![SmartContextBudgetPolicyReason::TightBudget],
    };

    let tightened = smart_context_apply_rewrite_budget_decision(
        policy.clone(),
        SmartContextRewriteBudgetDecision::Tighten,
        Some(10),
    );
    let relaxed = smart_context_apply_rewrite_budget_decision(
        policy,
        SmartContextRewriteBudgetDecision::Relax,
        Some(2),
    );

    assert_eq!(tightened.max_inline_tool_output_bytes, 270);
    assert_eq!(tightened.max_inline_bytes, 270);
    assert_eq!(tightened.max_rehydrate_tokens, 1);
    assert_eq!(relaxed.max_inline_tool_output_bytes, 375);
    assert_eq!(relaxed.max_inline_bytes, 375);
    assert_eq!(relaxed.max_rehydrate_tokens, 2);
}

#[test]
fn learned_rewrite_policy_relaxes_for_matching_route_model_profile_samples() {
    let bucket = smart_context_test_rewrite_bucket("responses", "GPT_5", "alpha");
    let learned = smart_context_test_learned_rewrite_policy(
        Some(bucket.clone()),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket.clone(),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket.clone(),
                smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "beta"),
                SmartContextRewriteTelemetrySample {
                    fallback: true,
                    ..smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400)
                },
            ),
        ],
    );

    assert_eq!(learned.decision, SmartContextRewriteBudgetDecision::Relax);
    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::LearnedRelax]
    );
    assert_eq!(learned.matching_telemetry_samples, 2);
    assert_eq!(learned.policy.mode, SmartContextBudgetMode::LargeLossless);
    assert_eq!(learned.policy.max_inline_tool_output_bytes, 64 * 1024);
}

#[test]
fn learned_rewrite_policy_tightens_for_confident_weak_matching_savings() {
    let bucket = smart_context_test_rewrite_bucket("responses", "gpt-5", "alpha");
    let learned = smart_context_test_learned_rewrite_policy(
        Some(bucket.clone()),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket.clone(),
                smart_context_test_rewrite_telemetry_sample(10_000, 9_000, 2_500, 2_400),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket,
                smart_context_test_rewrite_telemetry_sample(8_000, 7_200, 2_000, 1_900),
            ),
        ],
    );

    assert_eq!(learned.decision, SmartContextRewriteBudgetDecision::Tighten);
    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::LearnedTighten]
    );
    assert_eq!(learned.policy.mode, SmartContextBudgetMode::LargeLossless);
    assert_eq!(
        learned.policy.max_inline_tool_output_bytes,
        (32 * 1024) * 9 / 10
    );
    assert!(learned.policy.max_rehydrate_tokens < 12_000);
}

#[test]
fn learned_rewrite_policy_exact_passes_through_without_bucket_confidence() {
    let learned = smart_context_test_learned_rewrite_policy(
        Some(smart_context_test_rewrite_bucket(
            "responses",
            "gpt-5",
            "alpha",
        )),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "beta"),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
            SmartContextBucketedRewriteTelemetrySample {
                bucket_key: None,
                sample: smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            },
        ],
    );

    assert_eq!(
        learned.decision,
        SmartContextRewriteBudgetDecision::NoChange
    );
    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::MissingBucketConfidence]
    );
    assert_eq!(learned.matching_telemetry_samples, 0);
    assert_eq!(
        learned.policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(learned.policy.max_inline_tool_output_bytes, usize::MAX);
}

#[test]
fn learned_rewrite_policy_exact_passes_through_on_unsafe_matching_sample() {
    let bucket = smart_context_test_rewrite_bucket("compact", "gpt-5.2", "alpha");
    let learned = smart_context_test_learned_rewrite_policy(
        Some(bucket.clone()),
        SmartContextRecentRewriteSafety::default(),
        vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                bucket,
                SmartContextRewriteTelemetrySample {
                    fallback: true,
                    ..smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000)
                },
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("compact", "gpt-5.2", "beta"),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
        ],
    );

    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::UnsafeRewriteSample]
    );
    assert_eq!(learned.matching_telemetry_samples, 1);
    assert_eq!(
        learned.policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(learned.policy.max_inline_bytes, usize::MAX);
}

#[test]
fn learned_rewrite_policy_can_use_recent_safety_when_bucket_samples_are_absent() {
    let learned = smart_context_test_learned_rewrite_policy(
        Some(smart_context_test_rewrite_bucket(
            "responses",
            "gpt-5",
            "alpha",
        )),
        SmartContextRecentRewriteSafety {
            safe_rewrites: 2,
            fallback_rewrites: 0,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
        },
        Vec::new(),
    );

    assert_eq!(learned.decision, SmartContextRewriteBudgetDecision::Relax);
    assert_eq!(
        learned.reasons,
        vec![
            SmartContextLearnedRewritePolicyReason::RecentSafetyFallback,
            SmartContextLearnedRewritePolicyReason::LearnedRelax,
        ]
    );
    assert_eq!(learned.matching_telemetry_samples, 0);
    assert_eq!(learned.safety_samples, 2);
    assert_eq!(learned.policy.max_inline_tool_output_bytes, 64 * 1024);
}

#[test]
fn learned_rewrite_policy_keeps_base_exact_when_exactness_required() {
    let accounting = smart_context_test_large_budget_accounting();
    let learned = smart_context_learned_rewrite_policy(SmartContextLearnedRewritePolicyInput {
        adaptive_policy_input: SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput {
                previous_response_id: Some("resp-owned".to_string()),
                ..SmartContextExactnessInput::default()
            }),
            accounting,
            recent_rewrite_safety: SmartContextRecentRewriteSafety {
                safe_rewrites: 2,
                fallback_rewrites: 0,
                saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS * 2,
            },
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        },
        bucket_key: Some(smart_context_test_rewrite_bucket(
            "responses",
            "gpt-5",
            "alpha",
        )),
        telemetry_samples: vec![
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "alpha"),
                smart_context_test_rewrite_telemetry_sample(10_000, 4_000, 2_500, 1_000),
            ),
            smart_context_test_bucketed_rewrite_telemetry_sample(
                smart_context_test_rewrite_bucket("responses", "gpt-5", "alpha"),
                smart_context_test_rewrite_telemetry_sample(8_000, 3_200, 2_000, 800),
            ),
        ],
    });

    assert_eq!(
        learned.reasons,
        vec![SmartContextLearnedRewritePolicyReason::BasePolicyExactPassThrough]
    );
    assert_eq!(
        learned.policy.mode,
        SmartContextBudgetMode::ExactPassThrough
    );
    assert_eq!(
        learned.policy.reasons,
        vec![SmartContextBudgetPolicyReason::ExactnessRequired]
    );
}

fn smart_context_test_learned_rewrite_policy(
    bucket_key: Option<SmartContextRewritePolicyBucketKey>,
    recent_rewrite_safety: SmartContextRecentRewriteSafety,
    telemetry_samples: Vec<SmartContextBucketedRewriteTelemetrySample>,
) -> SmartContextLearnedRewritePolicy {
    smart_context_learned_rewrite_policy(SmartContextLearnedRewritePolicyInput {
        adaptive_policy_input: SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: smart_context_test_large_budget_accounting(),
            recent_rewrite_safety,
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        },
        bucket_key,
        telemetry_samples,
    })
}

fn smart_context_test_large_budget_accounting() -> SmartContextObservedTokenAccounting {
    smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(64_000),
        reserved_output_tokens: 4_096,
        current_input_tokens: 48_000,
        current_request_body_bytes: 0,
        current_request_estimated_tokens: None,
        observed_usage: Vec::new(),
    })
}

fn smart_context_test_rewrite_bucket(
    route: &str,
    model: &str,
    profile: &str,
) -> SmartContextRewritePolicyBucketKey {
    SmartContextRewritePolicyBucketKey {
        route: Some(route.to_string()),
        model: Some(model.to_string()),
        profile: Some(profile.to_string()),
    }
}

fn smart_context_test_bucketed_rewrite_telemetry_sample(
    bucket_key: SmartContextRewritePolicyBucketKey,
    sample: SmartContextRewriteTelemetrySample,
) -> SmartContextBucketedRewriteTelemetrySample {
    SmartContextBucketedRewriteTelemetrySample {
        bucket_key: Some(bucket_key),
        sample,
    }
}

fn smart_context_test_transform_rewrite_telemetry_sample(
    category: SmartContextTransformCategory,
    sample: SmartContextRewriteTelemetrySample,
) -> SmartContextTransformRewriteTelemetrySample {
    SmartContextTransformRewriteTelemetrySample { category, sample }
}

fn smart_context_test_transform_score(
    scores: &[SmartContextTransformRewriteSafetyScore],
    category: SmartContextTransformCategory,
) -> &SmartContextTransformRewriteSafetyScore {
    scores
        .iter()
        .find(|score| score.category == category)
        .unwrap_or_else(|| panic!("missing transform score: {category:?}"))
}

fn smart_context_test_rewrite_telemetry_sample(
    body_bytes_before: usize,
    body_bytes_after: usize,
    estimated_tokens_before: u64,
    estimated_tokens_after: u64,
) -> SmartContextRewriteTelemetrySample {
    SmartContextRewriteTelemetrySample {
        body_bytes_before,
        body_bytes_after,
        estimated_tokens_before,
        estimated_tokens_after,
        safe: true,
        fallback: false,
    }
}

#[test]
fn regression_self_check_falls_back_on_quality_risks() {
    let check = smart_context_regression_self_check(SmartContextRegressionSelfCheckInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput {
            turn_state: Some("turn".to_string()),
            ..SmartContextExactnessInput::default()
        }),
        before_hash: smart_context_hash_text("before"),
        after_hash: smart_context_hash_text("after"),
        before_estimated_tokens: 100,
        after_estimated_tokens: 100,
        before_critical_signal_count: 3,
        after_critical_signal_count: 2,
        missing_rehydrate_refs: vec!["artifact-missing".to_string()],
    });

    assert_eq!(
        check.decision,
        SmartContextRegressionSelfCheckDecision::FallbackExact
    );
    assert_eq!(
        check.reasons,
        vec![
            SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged,
            SmartContextRegressionSelfCheckReason::TokenBudgetDidNotImprove,
            SmartContextRegressionSelfCheckReason::CriticalSignalDropped,
            SmartContextRegressionSelfCheckReason::MissingRehydrateRefs,
        ]
    );
    assert_eq!(check.saved_tokens, 0);
}

#[test]
fn regression_self_check_passes_when_condense_saves_and_signals_preserved() {
    let check = smart_context_regression_self_check(SmartContextRegressionSelfCheckInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        before_hash: smart_context_hash_text("long before"),
        after_hash: smart_context_hash_text("short after"),
        before_estimated_tokens: 400,
        after_estimated_tokens: 100,
        before_critical_signal_count: 2,
        after_critical_signal_count: 2,
        missing_rehydrate_refs: Vec::new(),
    });

    assert_eq!(
        check.decision,
        SmartContextRegressionSelfCheckDecision::Pass
    );
    assert!(check.reasons.is_empty());
    assert_eq!(check.saved_tokens, 300);
}

fn smart_context_memory_capsule_policy_for_available_tokens(
    available_tokens: u64,
) -> (
    SmartContextObservedTokenAccounting,
    SmartContextAdaptiveBudgetPolicy,
) {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 120_000u64.saturating_sub(available_tokens),
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });
    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
        accounting: accounting.clone(),
        recent_rewrite_safety: SmartContextRecentRewriteSafety::default(),
        static_context_changed: false,
        missing_rehydrate_refs: Vec::new(),
    });

    (accounting, policy)
}

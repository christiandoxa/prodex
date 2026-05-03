use super::*;

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
            content_hash: smart_context_hash_text("same"),
        }
    );
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
    assert_eq!(accounting.last_input_tokens, 42_000);
    assert_eq!(accounting.effective_input_tokens, 42_000);
    assert_eq!(accounting.available_context_tokens, Some(78_000));
    assert_eq!(
        smart_context_token_budget_tier_from_accounting(&accounting),
        SmartContextTokenBudgetTier::Exact
    );
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
fn adaptive_budget_policy_prefers_safe_exact_when_required() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 125_000,
            observed_usage: Vec::new(),
        });
    let guard = smart_context_exactness_guard(SmartContextExactnessInput {
        previous_response_id: Some("resp-owned".to_string()),
        ..SmartContextExactnessInput::default()
    });

    let policy = smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
        exactness_guard: guard,
        accounting,
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
            observed_usage: Vec::new(),
        });
    let minimal =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 119_000,
            observed_usage: Vec::new(),
        });

    let condensed_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: condensed,
            static_context_changed: false,
            missing_rehydrate_refs: Vec::new(),
        });
    let minimal_policy =
        smart_context_adaptive_budget_policy(SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard: smart_context_exactness_guard(SmartContextExactnessInput::default()),
            accounting: minimal,
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
    assert_eq!(minimal_policy.max_inline_tool_output_bytes, 2 * 1024);
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

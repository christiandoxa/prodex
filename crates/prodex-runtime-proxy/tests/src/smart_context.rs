use super::*;
use std::borrow::Cow;

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
fn artifact_marker_is_short_and_preserves_rehydrate_metadata() {
    let artifact = SmartContextArtifactRef {
        id: "sc:0123456789abcdef".to_string(),
        byte_len: 12_345,
        content_hash: "sc:fedcba9876543210".to_string(),
    };
    let compacted = "first compacted line\nlast compacted line";

    let marker = smart_context_artifact_marker(&artifact, compacted);
    let old_verbose = format!(
        "# prodex smart context artifact\nartifact_id: prodex-artifact:{}\noriginal_bytes: {}\ncontent_hash: {}\nrehydrate: automatic when exact content is referenced; use prodex-artifact:{}#Lstart-Lend for exact line range\n\n{}",
        artifact.id, artifact.byte_len, artifact.content_hash, artifact.id, compacted
    );

    assert!(marker.len() < old_verbose.len());
    assert!(marker.starts_with("prodex-sc artifact prodex-artifact:sc:0123456789abcdef"));
    assert!(marker.contains("bytes=12345"));
    assert!(marker.contains("hash=sc:fedcba9876543210"));
    assert!(marker.contains(
        "rehydrate: use prodex-artifact:sc:0123456789abcdef or prodex-artifact:sc:0123456789abcdef#Lstart-Lend"
    ));
    assert!(marker.ends_with(compacted));
    assert!(!marker.contains("artifact_id:"));
    assert!(!marker.contains("original_bytes:"));
    assert!(!marker.contains("content_hash:"));
}

#[test]
fn artifact_reference_marker_has_same_exact_ref_fields() {
    let artifact = SmartContextArtifactRef {
        id: "sc:0123456789abcdef".to_string(),
        byte_len: 456,
        content_hash: "sc:fedcba9876543210".to_string(),
    };

    let marker = smart_context_artifact_reference_marker(&artifact);

    assert_eq!(
        marker,
        "prodex-sc repeat prodex-artifact:sc:0123456789abcdef bytes=456 hash=sc:fedcba9876543210; rehydrate: use prodex-artifact:sc:0123456789abcdef or prodex-artifact:sc:0123456789abcdef#Lstart-Lend"
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
            content_hash: smart_context_hash_text("same"),
        }
    );
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
            current_request_body_bytes: 0,
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
fn observed_token_accounting_uses_larger_history_to_avoid_underbudget() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 10_000,
            current_request_body_bytes: 20_000,
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
            observed_usage: Vec::new(),
        });
    let minimal =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 119_000,
            current_request_body_bytes: 0,
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
            observed_usage: Vec::new(),
        });
    let unsafe_accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(8_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 1,
            current_request_body_bytes: 0,
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
        observed_usage: Vec::new(),
    });
    let exact = smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(128_000),
        reserved_output_tokens: 4_096,
        current_input_tokens: 32_000,
        current_request_body_bytes: 0,
        observed_usage: Vec::new(),
    });
    let safe = SmartContextRecentRewriteSafety {
        safe_rewrites: 2,
        fallback_rewrites: 0,
        saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS,
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
    assert_eq!(large_mixed.max_inline_tool_output_bytes, 32 * 1024);
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
            safe_rewrites: 1,
            fallback_rewrites: 0,
            saved_tokens: SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS - 1,
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

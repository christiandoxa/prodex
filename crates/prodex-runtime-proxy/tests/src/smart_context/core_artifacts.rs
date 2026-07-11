use super::*;

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
fn structural_minify_json_body_fuzzes_malformed_json_without_rewrite() {
    for body in [
        b"".as_slice(),
        b"{",
        b"[",
        b"{\"input\":",
        b"{\"model\":\"gpt-5\",",
        b"{\"input\":[{\"type\":\"function_call_output\",\"output\":\"unterminated}",
        b"\xff\xfe{\"model\":\"gpt-5\"}",
        b"{\"model\":\"bad\x07model\"}",
    ] {
        let minified = smart_context_structural_minify_json_body(body);
        assert_eq!(minified.as_ref(), body);
        let _ = smart_context_model_name_from_body(body);
    }
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
fn exactness_guard_blocks_context_affinity_but_not_missing_rehydrate() {
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
fn volatile_normalizer_handles_unicode_near_uuid_width_without_panicking() {
    let text = "You are a senior engineer’s request handler";

    let normalized = smart_context_normalize_volatile_static_context(text);

    assert_eq!(normalized.as_ref(), text);
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

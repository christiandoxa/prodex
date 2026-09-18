use super::*;

#[test]
fn correctness_hash_uses_a_versioned_sha256_digest() {
    let hash = smart_context_hash_text("correctness-critical artifact");

    assert!(hash.starts_with("sc2:"), "unexpected digest scheme: {hash}");
    assert_eq!(hash.len(), 4 + 64);
    assert!(
        hash[4..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
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
fn model_name_helpers_find_model_after_bounded_prefix() {
    let body = format!(
        r#"{{"input":"{}","model":"gpt-5.3-codex-spark"}}"#,
        "x".repeat(8 * 1024)
    );

    assert_eq!(
        smart_context_model_name_from_body(body.as_bytes()),
        Some("gpt-5.3-codex-spark".to_string())
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
    assert_ne!(artifact.content_hash, smart_context_hash_text(second));
}

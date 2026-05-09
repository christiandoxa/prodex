use super::*;

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

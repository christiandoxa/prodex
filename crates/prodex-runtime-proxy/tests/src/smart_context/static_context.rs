use super::*;

#[test]
fn fingerprint_delta_tracks_static_context_across_turns() {
    let previous = [
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
    ]
    .into_iter()
    .map(smart_context_fingerprint)
    .collect::<Vec<_>>();
    let current = [
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
    ]
    .into_iter()
    .map(smart_context_fingerprint)
    .collect::<Vec<_>>();

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
    assert_eq!(left.items.len(), 2);
    assert_eq!(left.items[0].id_hash, smart_context_hash_text("AGENTS.md"));
    assert_eq!(left.items[1].id_hash, smart_context_hash_text("README.md"));
    assert!(left.content_hash.starts_with("scpc2:"));
    assert!(!format!("{left:?}").contains("usage"));
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
            .map(|item| item.content_hash.as_str())
            .collect::<Vec<_>>(),
        vec![
            smart_context_hash_text("instructions top"),
            smart_context_hash_text("system top"),
            smart_context_hash_text("developer top"),
            smart_context_hash_text("system two"),
            smart_context_hash_text("developer two"),
            smart_context_hash_text("developer ten"),
            smart_context_hash_text("usage"),
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
    assert_eq!(first.items[0].byte_len, "Rules\nKeep affinity".len());
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
    assert_eq!(
        first.items[0].content_hash,
        smart_context_hash_text(&first_canonical)
    );
}

#[test]
fn static_context_prompt_cache_still_changes_on_substantive_text_with_volatile_noise() {
    let before_text = "Trace request_id=123e4567-e89b-12d3-a456-426614174000 path=/tmp/prodex-a/run-123 at 2026-05-04T01:02:03Z\nRule: Keep profile affinity\n";
    let after_text = "Trace request_id=123e4567-e89b-12d3-a456-426614174999 path=/tmp/prodex-b/run-999 at 2026-05-05T09:08:07Z\nRule: Allow mid-stream rotation\n";
    let before_canonical = smart_context_stabilize_static_context_text(before_text);
    let after_canonical = smart_context_stabilize_static_context_text(after_text);
    let before =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: before_text.to_string(),
        }]);
    let after =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: "prodex-context".to_string(),
            text: after_text.to_string(),
        }]);

    assert_ne!(before.content_hash, after.content_hash);
    assert_ne!(before.items[0].content_hash, after.items[0].content_hash);
    assert!(before_canonical.contains("Keep profile affinity"));
    assert!(after_canonical.contains("Allow mid-stream rotation"));
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
fn static_context_fingerprint_is_bounded_and_secret_safe() {
    let secret = "prompt-secret-".repeat(
        SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEM_BYTES / "prompt-secret-".len() + 1,
    );
    let secret_id = "prompt-id-secret-".repeat(32);
    let exact = smart_context_static_context_prompt_cache_fingerprint(
        (0..SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS).map(|index| {
            SmartContextStaticContextItem {
                id: format!("item-{index}"),
                text: "stable".to_string(),
            }
        }),
    );
    let bounded = smart_context_static_context_prompt_cache_fingerprint(
        (0..=SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS).map(|index| {
            SmartContextStaticContextItem {
                id: format!("item-{index}"),
                text: if index == 0 {
                    secret.clone()
                } else {
                    "stable".to_string()
                },
            }
        }),
    );

    assert_eq!(
        exact.items.len(),
        SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS
    );
    assert!(!exact.truncated);
    assert_eq!(
        bounded.items.len(),
        SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS
    );
    assert_eq!(
        bounded.item_count,
        SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS + 1
    );
    assert!(bounded.truncated);
    assert!(!format!("{bounded:?}").contains(&secret));

    let id_bounded =
        smart_context_static_context_prompt_cache_fingerprint([SmartContextStaticContextItem {
            id: secret_id.clone(),
            text: "stable".to_string(),
        }]);
    assert!(!format!("{id_bounded:?}").contains(&secret_id));

    let mut large = (0..SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS + 32)
        .map(|index| SmartContextStaticContextItem {
            id: format!("ordered-{index:03}"),
            text: format!("value-{index}"),
        })
        .collect::<Vec<_>>();
    let forward = smart_context_static_context_prompt_cache_fingerprint(large.clone());
    large.reverse();
    let reversed = smart_context_static_context_prompt_cache_fingerprint(large);
    assert_eq!(forward, reversed);
}

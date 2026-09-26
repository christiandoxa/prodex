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
fn fingerprint_delta_uses_last_duplicate_and_stable_kind_order() {
    fn fingerprint(
        id: &str,
        kind: SmartContextFingerprintKind,
        content_hash: &str,
    ) -> SmartContextFingerprint {
        SmartContextFingerprint {
            id: id.to_string(),
            kind,
            content_hash: content_hash.to_string(),
            byte_len: content_hash.len(),
        }
    }

    let previous = vec![
        fingerprint(
            "gone",
            SmartContextFingerprintKind::MemoryCapsule,
            "removed",
        ),
        fingerprint("same", SmartContextFingerprintKind::Artifact, "discarded"),
        fingerprint("tool", SmartContextFingerprintKind::ToolOutput, "before"),
        fingerprint("root", SmartContextFingerprintKind::StaticContext, ""),
        fingerprint("same", SmartContextFingerprintKind::Artifact, "shared"),
        fingerprint("b", SmartContextFingerprintKind::Artifact, "shared"),
    ];
    let current = vec![
        fingerprint(
            "same",
            SmartContextFingerprintKind::Artifact,
            "discarded-current",
        ),
        fingerprint("same", SmartContextFingerprintKind::Artifact, "shared"),
        fingerprint("tool", SmartContextFingerprintKind::ToolOutput, "after"),
        fingerprint("turn", SmartContextFingerprintKind::ConversationTurn, "new"),
        fingerprint("b", SmartContextFingerprintKind::Artifact, "shared"),
        fingerprint("root", SmartContextFingerprintKind::StaticContext, ""),
    ];

    assert_eq!(
        smart_context_fingerprint_delta(previous, current),
        vec![
            SmartContextFingerprintChange::Unchanged {
                fingerprint: fingerprint("root", SmartContextFingerprintKind::StaticContext, ""),
            },
            SmartContextFingerprintChange::Added {
                fingerprint: fingerprint(
                    "turn",
                    SmartContextFingerprintKind::ConversationTurn,
                    "new",
                ),
            },
            SmartContextFingerprintChange::Changed {
                before: fingerprint("tool", SmartContextFingerprintKind::ToolOutput, "before"),
                after: fingerprint("tool", SmartContextFingerprintKind::ToolOutput, "after"),
            },
            SmartContextFingerprintChange::Unchanged {
                fingerprint: fingerprint("b", SmartContextFingerprintKind::Artifact, "shared"),
            },
            SmartContextFingerprintChange::Unchanged {
                fingerprint: fingerprint("same", SmartContextFingerprintKind::Artifact, "shared"),
            },
            SmartContextFingerprintChange::Removed {
                fingerprint: fingerprint(
                    "gone",
                    SmartContextFingerprintKind::MemoryCapsule,
                    "removed",
                ),
            },
        ]
    );
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
fn static_context_prompt_cache_payload_format_is_stable() {
    let payload =
        smart_context_static_context_prompt_cache_payload(&[SmartContextStableStaticContextItem {
            id: "AGENTS.md".to_string(),
            canonical_text: "rules\n".to_string(),
            content_hash: "ignored-by-payload".to_string(),
            byte_len: 6,
        }]);

    assert_eq!(
        payload,
        "prodex-smart-context-static-prompt-cache-v1\nid-bytes:9\nAGENTS.md\ntext-bytes:6\nrules\n\n"
    );
}

#[test]
fn static_context_noise_classifier_matches_expected_values() {
    let cases = [
        ("generated: 2026-05-04", true),
        (" Generated-at : today ", true),
        ("generated_on: yesterday", true),
        ("<!-- prodex_current_date: 2026-05-04 -->", true),
        ("\u{00a0}generated\u{00a0}: 2026-05-04", true),
        (
            "\u{2003}#\u{3000}generated\u{2007}:\u{2009}2026-05-04",
            true,
        ),
        (
            "\u{0085}<!--\u{1680}prodex_current_date\u{202f}: 2026-05-04 \u{3000}-->\u{2000}",
            true,
        ),
        ("\u{00a0}//\u{2009}run_id: stable", true),
        ("# last_generated_at =", true),
        ("; request_id = stable", true),
        ("last generated: stable", false),
        ("last updated: now", true),
        ("; updated-at: stable", false),
        ("run_id: stable", true),
        ("request id=stable", true),
        ("trace-id: stable", true),
        ("session_id: stable", true),
        ("current time: now", true),
        ("current_datetime: tomorrow", true),
        ("as_of: yesterday", true),
        ("timestamp: noon", false),
        ("timestamp: 2026", true),
        ("not volatile: 2026", false),
        ("Generated: stable", false),
        ("updated_at: \u{0661}", false),
        ("generated！: 2026", false),
        ("generated\u{2003}: stable", false),
        ("generated::", false),
        ("<!-- generated: 2026", false),
        ("<!-- generated: 2026 --> trailing", false),
        ("## generated: 2026", false),
        ("timestamp = 2026: stable", false),
        ("timestamp = 2026", true),
        ("\u{2028}: 2026", false),
    ];

    for (line, expected) in cases {
        assert_eq!(
            smart_context_static_context_noise_line(line),
            expected,
            "{line:?}"
        );
    }

    for whitespace in [
        '\u{0009}', '\u{000a}', '\u{000b}', '\u{000c}', '\u{000d}', ' ', '\u{0085}', '\u{00a0}',
        '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}',
        '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}',
        '\u{202f}', '\u{205f}', '\u{3000}',
    ] {
        assert!(smart_context_static_context_noise_line(&format!(
            "{whitespace}generated{whitespace}: 2026{whitespace}"
        )));
        assert!(smart_context_static_context_noise_line(&format!(
            "generated{whitespace}at: 2026"
        )));
    }

    let large = format!("generated: {}7", "x".repeat(2 * 1024 * 1024));
    assert!(smart_context_static_context_noise_line(&large));
}

#[test]
fn static_context_item_order_matches_expected_values() {
    let items = [
        ("README.md", "readme", 1, "readme"),
        ("input[10].developer", "input-10", 8, "developer ten"),
        ("developer", "developer", 9, "developer"),
        ("input[2].system", "input-2", 8, "system two"),
        ("system", "system", 6, "system"),
        ("instructions", "instructions", 12, "instructions"),
        (
            "input[2].developer",
            "input-2-developer",
            13,
            "developer two",
        ),
        ("input[02].system", "input-02", 9, "zero padded"),
        ("input[0002].system", "input-0002", 10, "more zeroes"),
        ("input[+2].system", "input-plus-2", 11, "plus index"),
        ("input[0].system", "input-0", 7, "system zero"),
        ("input[+].system", "invalid-plus", 13, "invalid plus"),
        (
            "input[+x].system",
            "invalid-plus-text",
            18,
            "invalid plus text",
        ),
        (
            "input[-2].system",
            "invalid-negative",
            17,
            "invalid negative",
        ),
        ("input[2].other", "invalid-role", 13, "invalid role"),
        ("input[2x].system", "invalid-digit", 13, "invalid digit"),
        ("éclair.md", "unicode-e", 10, "accented id"),
        ("猫.md", "unicode-cat", 7, "CJK id"),
    ]
    .map(
        |(id, content_hash, byte_len, canonical_text)| SmartContextStableStaticContextItem {
            id: id.to_string(),
            content_hash: content_hash.to_string(),
            byte_len,
            canonical_text: canonical_text.to_string(),
        },
    );
    let inputs = items
        .iter()
        .map(
            |item| prodex_mojo_core::runtime::SmartContextStaticItemPlanInput {
                id: &item.id,
                content_hash: &item.content_hash,
                canonical_text: &item.canonical_text,
                byte_len: item.byte_len as u64,
            },
        )
        .collect::<Vec<_>>();
    let expected = [
        ("instructions", "instructions"),
        ("system", "system"),
        ("developer", "developer"),
        ("input[0].system", "input-0"),
        ("input[+2].system", "input-plus-2"),
        ("input[0002].system", "input-0002"),
        ("input[02].system", "input-02"),
        ("input[2].system", "input-2"),
        ("input[2].developer", "input-2-developer"),
        ("input[10].developer", "input-10"),
        ("README.md", "readme"),
        ("input[+].system", "invalid-plus"),
        ("input[+x].system", "invalid-plus-text"),
        ("input[-2].system", "invalid-negative"),
        ("input[2].other", "invalid-role"),
        ("input[2x].system", "invalid-digit"),
        ("éclair.md", "unicode-e"),
        ("猫.md", "unicode-cat"),
    ];
    let plan = prodex_mojo_core::runtime::smart_context_static_item_plan(&inputs, items.len())
        .expect("Mojo orders valid static-context items");
    let planned = plan
        .selected_indices
        .iter()
        .map(|index| {
            (
                items[*index].id.as_str(),
                items[*index].content_hash.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(planned, expected);
}

#[test]
fn static_context_item_order_uses_hash_length_and_text_tiebreakers() {
    let items = [
        ("same.md", "hash-b", 1, "a"),
        ("same.md", "hash-a", 4, "z"),
        ("same.md", "hash-a", 3, "z"),
        ("same.md", "hash-a", 3, "a"),
    ]
    .map(
        |(id, content_hash, byte_len, canonical_text)| SmartContextStableStaticContextItem {
            id: id.to_string(),
            content_hash: content_hash.to_string(),
            byte_len,
            canonical_text: canonical_text.to_string(),
        },
    );

    let inputs = items
        .iter()
        .map(
            |item| prodex_mojo_core::runtime::SmartContextStaticItemPlanInput {
                id: &item.id,
                content_hash: &item.content_hash,
                canonical_text: &item.canonical_text,
                byte_len: item.byte_len as u64,
            },
        )
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::runtime::smart_context_static_item_plan(&inputs, items.len())
        .expect("Mojo orders items by their complete tie-breaker tuple");
    assert_eq!(
        plan.selected_indices
            .iter()
            .map(|index| (
                items[*index].content_hash.as_str(),
                items[*index].byte_len,
                items[*index].canonical_text.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("hash-a", 3, "a"),
            ("hash-a", 3, "z"),
            ("hash-a", 4, "z"),
            ("hash-b", 1, "a"),
        ]
    );
}

#[test]
fn static_context_item_order_handles_usize_extremes_and_exact_ties() {
    let item = |id: String| SmartContextStableStaticContextItem {
        id,
        content_hash: "same-hash".to_string(),
        byte_len: 9,
        canonical_text: "same text".to_string(),
    };
    let max_index = usize::MAX.to_string();
    let max_id = format!("input[{max_index}].system");
    let max_plus_id = format!("input[+{max_index}].system");
    let max_padded_id = format!("input[000{max_index}].system");
    let overflow_id = format!("input[{max_index}0].system");
    let overflow_plus_id = format!("input[+{max_index}0].system");
    let marker = item("README.md".to_string());
    let max_item = item(max_id);
    let max_plus_item = item(max_plus_id);
    let max_padded_item = item(max_padded_id);
    let overflow_item = item(overflow_id);
    let overflow_plus_item = item(overflow_plus_id);
    let invalid_plus_item = item("input[+x].system".to_string());
    let invalid_negative_item = item("input[-1].system".to_string());
    let invalid_role_item = item("input[2].other".to_string());

    for (earlier, later) in [
        (&max_item, &marker),
        (&max_plus_item, &max_item),
        (&max_padded_item, &max_item),
        (&marker, &overflow_item),
        (&marker, &overflow_plus_item),
        (&marker, &invalid_plus_item),
        (&marker, &invalid_negative_item),
        (&marker, &invalid_role_item),
    ] {
        let inputs = [earlier, later].map(|item| {
            prodex_mojo_core::runtime::SmartContextStaticItemPlanInput {
                id: &item.id,
                content_hash: &item.content_hash,
                canonical_text: &item.canonical_text,
                byte_len: item.byte_len as u64,
            }
        });
        let plan = prodex_mojo_core::runtime::smart_context_static_item_plan(&inputs, 2)
            .expect("Mojo orders the usize-boundary pair");
        assert_eq!(plan.selected_indices, [0, 1]);
    }

    let exact_twin = marker.clone();
    let tie_inputs = [&marker, &exact_twin].map(|item| {
        prodex_mojo_core::runtime::SmartContextStaticItemPlanInput {
            id: &item.id,
            content_hash: &item.content_hash,
            canonical_text: &item.canonical_text,
            byte_len: item.byte_len as u64,
        }
    });
    let tie_plan = prodex_mojo_core::runtime::smart_context_static_item_plan(&tie_inputs, 2)
        .expect("Mojo preserves exact ties");
    assert_eq!(tie_plan.selected_indices, [0, 1]);
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

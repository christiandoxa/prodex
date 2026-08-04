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

#[test]
fn static_context_heading_sections_preserve_offsets_and_ordinals() {
    let intro = "intro\n";
    let first_body = "alpha ".repeat(90);
    let second_body = "beta ".repeat(110);
    let text = format!("{intro}# First\r\n{first_body}\n## Second\n{second_body}");

    let sections = smart_context_static_context_heading_sections(&text);

    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].heading, "# First");
    assert_eq!(sections[0].start, intro.len());
    assert_eq!(sections[0].ordinal, 0);
    assert_eq!(
        smart_context_static_heading_section_body(&text, &sections[0]),
        Some(&text[sections[0].start..sections[0].end])
    );
    assert_eq!(sections[1].heading, "## Second");
    assert_eq!(sections[1].ordinal, 1);
    assert_eq!(sections[0].end, sections[1].start);
}

#[test]
fn static_context_heading_sections_ignore_short_and_invalid_sections() {
    let short_body = "short";
    let valid_body = "rule ".repeat(110);
    let text = format!("# Too Short\n{short_body}\nnot a heading\n# Valid\n{valid_body}");

    let sections = smart_context_static_context_heading_sections(&text);

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].heading, "# Valid");
    assert_eq!(sections[0].ordinal, 1);
    assert!(
        smart_context_static_heading_section_body(&text, &sections[0])
            .unwrap()
            .contains(&valid_body)
    );
}

#[test]
fn static_context_heading_section_body_rejects_invalid_ranges() {
    let text = "é\n# Heading\n".to_string() + &"body ".repeat(110);

    assert!(
        smart_context_static_heading_section_body(
            &text,
            &SmartContextStaticHeadingSection {
                heading: "# Heading".to_string(),
                start: 1,
                end: 2,
                ordinal: 0,
            },
        )
        .is_none()
    );
    assert!(
        smart_context_static_heading_section_body(
            &text,
            &SmartContextStaticHeadingSection {
                heading: "# Heading".to_string(),
                start: text.len(),
                end: text.len(),
                ordinal: 0,
            },
        )
        .is_none()
    );
}

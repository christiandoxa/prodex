use super::*;

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

    assert!(repaired.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(repaired.contains("L1-L4:"));
    assert!(repaired.contains("error: hidden failure"));
    assert!(repaired.contains("src/main.rs:22:5"));
    assert!(prodex_context::critical_signal_self_check(original, &repaired).passed());
}

#[test]
fn smart_context_exact_appendices_dedupe_duplicate_range_bodies_when_shorter() {
    let duplicate = "error: repeated failure\nsrc/lib.rs:10:5\n".repeat(12);
    let ranges = vec![
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L1-L24".to_string(),
            body: duplicate.clone(),
        },
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L49-L72".to_string(),
            body: duplicate.clone(),
        },
    ];
    let naive = format!(
        "{SMART_CONTEXT_LABEL_CRITICAL_EXACT}\npsc:abc#L1-L24\n{duplicate}\npsc:abc#L49-L72\n{duplicate}"
    );

    let (crit, count) =
        runtime_smart_context_render_exact_appendix(SMART_CONTEXT_LABEL_CRITICAL_EXACT, ranges)
            .unwrap();
    let (sem, sem_count) = runtime_smart_context_render_exact_appendix(
        SMART_CONTEXT_LABEL_SEMANTIC_EXACT,
        vec![
            RuntimeSmartContextExactAppendixRange {
                reference: "psc:abc#L1-L24".to_string(),
                body: duplicate.clone(),
            },
            RuntimeSmartContextExactAppendixRange {
                reference: "psc:abc#L49-L72".to_string(),
                body: duplicate.clone(),
            },
        ],
    )
    .unwrap();

    assert_eq!(count, 2);
    assert_eq!(sem_count, 2);
    assert!(crit.len() < naive.len());
    assert_eq!(crit.match_indices(&duplicate).count(), 1);
    assert!(crit.contains("[psc exdup h="));
    assert!(crit.contains(&format!("b={}", duplicate.len())));
    assert!(crit.contains("refs=psc:abc#L1-L24"));
    assert!(sem.contains(SMART_CONTEXT_LABEL_SEMANTIC_EXACT));
    assert_eq!(sem.match_indices(&duplicate).count(), 1);
}

#[test]
fn smart_context_exact_appendices_merge_adjacent_line_ranges() {
    let ranges = vec![
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L10-L12".to_string(),
            body: "error: first\nsrc/lib.rs:10:5\ncontext".to_string(),
        },
        RuntimeSmartContextExactAppendixRange {
            reference: "psc:abc#L13-L14".to_string(),
            body: "panic: second\nsrc/lib.rs:14:5".to_string(),
        },
    ];

    let (appendix, count) =
        runtime_smart_context_render_exact_appendix(SMART_CONTEXT_LABEL_CRITICAL_EXACT, ranges)
            .unwrap();

    assert_eq!(count, 2);
    assert!(appendix.contains("psc:abc#L10-L14"));
    assert!(appendix.contains("error: first"));
    assert!(appendix.contains("panic: second"));
    assert_eq!(appendix.matches("psc:abc#L").count(), 1);
}

#[test]
fn smart_context_exact_ref_lists_emit_compact_multi_ranges_when_shorter() {
    let refs = vec![
        "psc:abc#L1-L4".to_string(),
        "psc:abc#L9-L12".to_string(),
        "psc:abc#L20-L24".to_string(),
    ];

    let compact = runtime_smart_context_compact_line_refs_if_shorter(&refs);
    let parsed = runtime_smart_context_parse_non_alias_artifact_reference(&compact).unwrap();

    assert_eq!(compact, "psc:abc#L1-L4,L9-L12,L20-L24");
    assert_eq!(parsed.line_ranges.len(), 3);
    assert!(compact.len() < refs.join(",").len());
}

#[test]
fn smart_context_scored_exact_appendix_keeps_high_signal_and_refs_overflow() {
    let ranges = (1usize..=14)
        .map(|index| RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref("sc:abc", index, index),
            body: if index == 13 {
                "error[E0425]: cannot find value\nsrc/lib.rs:13:5".to_string()
            } else {
                format!("context line {index}")
            },
        })
        .collect::<Vec<_>>();

    let (appendix, count) = runtime_smart_context_render_scored_exact_appendix(
        SMART_CONTEXT_LABEL_SEMANTIC_EXACT,
        ranges,
        4,
        runtime_smart_context_critical_exact_appendix_score,
    )
    .unwrap();

    assert_eq!(count, 4);
    assert!(appendix.contains("error[E0425]"));
    assert!(appendix.contains("refs: psc:abc#"));
    assert!(appendix.contains(",L"));
    assert!(appendix.matches("context line ").count() <= 4);
}

#[test]
fn smart_context_exact_range_label_parser_accepts_legacy_critical_label() {
    let legacy = "old summary\n\ncritical exact ranges:\nL1-L1:\nerror: legacy";
    let v1 = "old summary\n\ncrit exact:\nL2-L2:\nerror: v1";

    let body = runtime_smart_context_labeled_section_body(
        legacy,
        &[
            SMART_CONTEXT_LABEL_CRITICAL_EXACT,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_V1,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY,
        ],
    );
    let v1_body = runtime_smart_context_labeled_section_body(
        v1,
        &[
            SMART_CONTEXT_LABEL_CRITICAL_EXACT,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_V1,
            SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY,
        ],
    );

    assert_eq!(body, Some("L1-L1:\nerror: legacy"));
    assert_eq!(v1_body, Some("L2-L2:\nerror: v1"));
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
        tool_call_args_condensed: 0,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
        repo_state_facts: 0,
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
    assert!(text.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(text.contains(&runtime_smart_context_artifact_line_ref(&artifact.id, 1, 4)));
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
    assert!(appendix.contains(SMART_CONTEXT_LABEL_CRITICAL_EXACT));
    assert!(appendix.contains(&runtime_smart_context_artifact_line_ref(&artifact.id, 1, 4)));
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
    assert!(appendix.contains(&runtime_smart_context_artifact_line_ref(&artifact_id, 1, 4)));
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
fn smart_context_prepare_passes_too_deep_json_unchanged_without_panic_fallback() {
    let shared = smart_context_test_shared("prepare-too-deep-json");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(32_000), None);
    let mut nested = serde_json::Value::String("leaf".to_string());
    for _ in 0..RUNTIME_SMART_CONTEXT_MAX_JSON_DEPTH {
        nested = serde_json::json!({ "nested": nested });
    }
    let body = serde_json::json!({
        "model": "gpt-5.5",
        "input": [{
            "type": "message",
            "role": "user",
            "content": "keep exact"
        }],
        "metadata": nested
    })
    .to_string();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/v1/responses".to_string(),
        headers: Vec::new(),
        body: body.into_bytes(),
    };

    let prepared =
        prepare_runtime_smart_context_http_body(79, &request, &shared, RuntimeRouteKind::Responses);

    assert!(matches!(&prepared, Cow::Borrowed(_)));
    assert_eq!(prepared.as_ref(), request.body.as_slice());
    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(log.contains("decision=unsupported_json_shape"));
    assert!(log.contains("reasons=json_depth_limit"));
    assert!(!log.contains("smart_context_panic"));
}

#[test]
fn smart_context_json_shape_guard_rejects_excessive_node_count_iteratively() {
    let value = serde_json::Value::Array(vec![
        serde_json::Value::Null;
        RUNTIME_SMART_CONTEXT_MAX_JSON_NODES + 1
    ]);

    assert_eq!(
        runtime_smart_context_unsupported_json_shape_reason(&value),
        Some("json_node_limit")
    );
}

#[test]
fn smart_context_static_section_body_rejects_non_char_boundary_offsets_without_panic() {
    let text = "## Résumé café\n".repeat(90);
    let bad_start = text.find('é').unwrap() + 1;
    let section = RuntimeSmartContextStaticHeadingSection {
        heading: "## Résumé café".to_string(),
        start: bad_start,
        end: text.len(),
        ordinal: 0,
    };

    assert_eq!(
        runtime_smart_context_static_heading_section_body(&text, &section),
        None
    );
}

#[test]
fn smart_context_self_check_passes_through_growth_without_rehydrate() {
    let stats = RuntimeSmartContextTransformStats {
        artifacts_stored: 1,
        tool_outputs_condensed: 1,
        tool_call_args_condensed: 0,
        duplicate_texts: 0,
        cross_turn_duplicate_texts: 0,
        repeat_tool_output_refs: 0,
        blob_outputs_condensed: 0,
        rehydrated_refs: 0,
        static_context_deltas: 0,
        repo_state_facts: 0,
    };

    assert_eq!(
        runtime_smart_context_rewrite_self_check(100, 101, &stats),
        "growth"
    );
    assert!(runtime_smart_context_should_pass_through_after_self_check(
        100, 101, &stats
    ));
}

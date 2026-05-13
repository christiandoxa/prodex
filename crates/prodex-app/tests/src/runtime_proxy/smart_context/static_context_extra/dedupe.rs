#[test]
fn smart_context_static_context_cross_field_dedupe_keeps_one_exact_copy() {
    let repeated = "Use repo rules exactly.\n".repeat(80);
    let mut value = serde_json::json!({
        "instructions": repeated.as_str(),
        "system": repeated.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_cross_field_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(repeated.as_str()));
    assert_eq!(
        value["system"].as_str(),
        Some("psc static dup instructions")
    );
    assert_eq!(value["input"][0]["content"].as_str(), Some("do work"));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_chunk_dedupe_replaces_repeated_chunk_only() {
    let shared_chunk = (0..80)
        .map(|index| format!("Shared policy sentence number {index} stays semantically identical."))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(shared_chunk.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES);
    let instructions = format!("Primary intro.\n\n{shared_chunk}\n\nPrimary tail.");
    let system = format!("System intro.\n\n{shared_chunk}\n\nSystem tail.");
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "system": system.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_chunk_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(instructions.as_str()));
    let system_after = value["system"].as_str().unwrap();
    assert!(system_after.contains("System intro."));
    assert!(system_after.contains("System tail."));
    assert!(system_after.contains(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX));
    assert!(!system_after.contains(&shared_chunk));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_section_dedupe_replaces_later_identical_heading_section() {
    let body = (0..80)
        .map(|index| format!("Shared section rule {index} remains exact."))
        .collect::<Vec<_>>()
        .join("\n");
    let repeated_section = format!("## Runtime Proxy\n{body}\n");
    assert!(repeated_section.len() >= SMART_CONTEXT_STATIC_CONTEXT_CHUNK_MIN_BYTES);
    let instructions = format!(
        "# One\nunique intro\n\n{repeated_section}\n## Other\nunique tail\n\n{repeated_section}"
    );
    let sections = runtime_smart_context_static_context_heading_sections(&instructions);
    assert_eq!(sections.len(), 2);
    assert_eq!(
        instructions[sections[0].start..sections[0].end].trim(),
        instructions[sections[1].start..sections[1].end].trim()
    );
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        &mut stats,
    );

    let after = value["instructions"].as_str().unwrap();
    assert!(after.contains(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX));
    assert_eq!(after.matches("## Runtime Proxy").count(), 2);
    assert_eq!(
        after
            .matches("Shared section rule 79 remains exact.")
            .count(),
        1
    );
    assert!(after.contains("## Other"));
    assert_eq!(stats.static_context_deltas, 1);
}

#[test]
fn smart_context_static_context_section_dedupe_respects_require_exact() {
    let body = (0..80)
        .map(|index| format!("Shared exact section rule {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let instructions = format!("## Same\n{body}\n## Same\n{body}");
    let mut value = serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [
            {"role": "user", "content": "do work"}
        ]
    });
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_apply_static_context_section_dedupe(
        &mut value,
        &runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::RequireExact,
            reasons: Vec::new(),
        },
        &mut stats,
    );

    assert_eq!(value["instructions"].as_str(), Some(instructions.as_str()));
    assert_eq!(stats.static_context_deltas, 0);
}

#[test]
fn smart_context_persisted_static_section_fingerprints_dedupe_fresh_start() {
    let first_shared = smart_context_test_shared("static-section-persist-first");
    let artifact_path = first_shared
        .log_path
        .with_file_name("static-section-persist-artifacts.json");
    let calibration_path = runtime_smart_context_token_calibration_path(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let body = (0..90)
        .map(|index| format!("Stable section guidance {index}."))
        .collect::<Vec<_>>()
        .join("\n");
    let instructions = format!("# Stable Section\n{body}\n");
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let first = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "first request"}]
    }));

    let first_prepared = prepare_runtime_smart_context_http_body(
        120,
        &first,
        &first_shared,
        RuntimeRouteKind::Responses,
    );
    let first_value = serde_json::from_slice::<serde_json::Value>(first_prepared.as_ref()).unwrap();
    assert_eq!(
        first_value["instructions"].as_str(),
        Some(instructions.as_str())
    );
    assert!(
        std::fs::read_to_string(&calibration_path)
            .unwrap()
            .contains("static_section_fingerprints")
    );

    let fresh_shared = smart_context_test_shared("static-section-persist-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let fresh = smart_context_test_request(serde_json::json!({
        "instructions": instructions.as_str(),
        "input": [{"role": "user", "content": "fresh request"}]
    }));

    let fresh_prepared = prepare_runtime_smart_context_http_body(
        121,
        &fresh,
        &fresh_shared,
        RuntimeRouteKind::Responses,
    );
    let fresh_text = String::from_utf8_lossy(fresh_prepared.as_ref());
    assert!(fresh_text.contains(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX));
    assert!(!fresh_text.contains("Stable section guidance 89."));
    assert!(prodex_context::critical_signal_self_check(&instructions, &fresh_text).passed());

    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&calibration_path));
}

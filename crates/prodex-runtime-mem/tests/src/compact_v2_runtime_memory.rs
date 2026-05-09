use super::*;

#[test]
fn super_slim_v2_omits_tool_name_and_input_when_both_match_schema_defaults() {
    let shadow = runtime_mem_super_slim_v2_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "custom_tool_call",
            "call_id": "call-default",
            "name": "tool",
            "action": "tool call"
        }
    }));

    assert_eq!(shadow["t"].as_str(), Some("pm2:tu"));
    assert_eq!(shadow.get("n"), None);
    assert_eq!(shadow.get("c"), None);

    let fields = v2_schema_fields("prodex-v2-tool-use");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &shadow).as_deref(),
        Some("tool")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &shadow).as_deref(),
        Some("tool call")
    );
}

#[test]
fn super_slim_v2_omits_tool_input_when_it_duplicates_tool_name() {
    let shadow = runtime_mem_super_slim_v2_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call",
            "call_id": "call-dup",
            "name": "web_search"
        }
    }));

    assert_eq!(shadow["t"].as_str(), Some("pm2:tu"));
    assert_eq!(shadow["n"].as_str(), Some("web_search"));
    assert_eq!(shadow.get("c"), None);

    let fields = v2_schema_fields("prodex-v2-tool-use");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &shadow).as_deref(),
        Some("web_search")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &shadow).as_deref(),
        Some("web_search")
    );
}

#[test]
fn super_slim_v2_omits_default_tool_name_when_input_preserves_reader_output() {
    let shadow = runtime_mem_super_slim_v2_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "custom_tool_call",
            "call_id": "call-tool",
            "name": "tool",
            "action": "run diagnostics"
        }
    }));

    assert_eq!(shadow["t"].as_str(), Some("pm2:tu"));
    assert_eq!(shadow.get("n"), None);
    assert_eq!(shadow["c"].as_str(), Some("run diagnostics"));

    let fields = v2_schema_fields("prodex-v2-tool-use");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &shadow).as_deref(),
        Some("tool")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &shadow).as_deref(),
        Some("run diagnostics")
    );
}

#[test]
fn super_slim_v2_shadow_events_mark_consecutive_duplicate_artifact_refs() {
    let artifact_ref = "psc:repeat-ref";
    let events = [
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": "first artifact-backed prompt",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command_output",
                "call_id": "call-repeat",
                "output": "same artifact-backed output",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": "third consecutive artifact-backed prompt",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert_eq!(shadows[0]["r"].as_str(), Some(artifact_ref));
    assert_eq!(
        shadows[0].get(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD),
        None
    );
    assert_eq!(shadows[1].get("r"), None);
    assert_eq!(
        shadows[1][RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD].as_str(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );
    assert_eq!(shadows[2].get("r"), None);
    assert_eq!(
        shadows[2][RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD].as_str(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );

    let fields = v2_schema_fields("prodex-v2-tool-result");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolResponse"], &shadows[1]).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );

    let legacy_tool_response = serde_json::json!({
        "coalesce": ["s", "r", { "value": RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED }]
    });
    assert_eq!(
        resolve_v2_schema_string(&legacy_tool_response, &shadows[1]).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED)
    );
}

#[test]
fn super_slim_v2_shadow_events_do_not_mark_non_consecutive_refs() {
    let artifact_ref = "psc:not-consecutive";
    let events = [
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": "first artifact-backed prompt",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": "assistant event breaks adjacency",
                "summary": "assistant summary"
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command_output",
                "call_id": "call-later",
                "output": "same ref after non-ref event",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert_eq!(shadows[0]["r"].as_str(), Some(artifact_ref));
    assert_eq!(shadows[2]["r"].as_str(), Some(artifact_ref));
    assert_eq!(
        shadows[2].get(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD),
        None
    );
}

#[test]
fn super_slim_v2_schema_reads_previous_ref_marker_and_legacy_full_refs() {
    let marker_event = serde_json::json!({
        "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
        RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD: RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER
    });
    let legacy_marker_event = serde_json::json!({
        "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
        RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD: RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER_LEGACY
    });
    let legacy_event = serde_json::json!({
        "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
        "r": "psc:legacy-full-ref"
    });
    let fields = v2_schema_fields("prodex-v2-user-message");

    assert_eq!(
        resolve_v2_schema_string(&fields["prompt"], &marker_event).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["prompt"], &legacy_marker_event).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER_LEGACY)
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["prompt"], &legacy_event).as_deref(),
        Some("psc:legacy-full-ref")
    );
    assert!(runtime_mem_event_has_super_slim_prompt_reference(
        &marker_event
    ));
    assert!(runtime_mem_event_has_super_slim_prompt_reference(
        &legacy_marker_event
    ));
}

#[test]
fn super_slim_v2_schema_still_reads_legacy_tool_name_and_input_fields() {
    let legacy = serde_json::json!({
        "t": "pm2:tu",
        "i": "call-legacy",
        "n": "exec_command",
        "c": "cargo test -q"
    });
    let fields = v2_schema_fields("prodex-v2-tool-use");

    assert_eq!(
        resolve_v2_schema_string(&fields["toolId"], &legacy).as_deref(),
        Some("call-legacy")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &legacy).as_deref(),
        Some("exec_command")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &legacy).as_deref(),
        Some("cargo test -q")
    );
}

#[test]
fn super_slim_v2_interns_repeated_tool_names_when_smaller() {
    let tool_name = "very_long_custom_repo_tool_name_for_runtime_mem_schema_native_dictionary";
    let events = (0..8)
        .map(|index| {
            serde_json::json!({
            "payload": {
                "type": "custom_tool_call",
                "call_id": format!("call-tool-name-{index}"),
                "name": tool_name,
                "action": format!("action {index}")
            }
            })
        })
        .collect::<Vec<_>>();

    let base_shadows = v2_shadow_events_without_dictionary(events.iter());
    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert!(runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows));
    assert!(v2_dictionary_events(&shadows).iter().any(|event| {
        event.get("k").and_then(Value::as_str) == Some("n")
            && event.get("m").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT)
            && event.get("v").and_then(Value::as_str) == Some(tool_name)
    }));
    assert!(v2_tool_use_events(&shadows).iter().any(|event| {
        event
            .get("n")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("ss:d:n#"))
    }));
    assert_v2_raw_events_schema_addressable(&shadows);
    assert_v2_compact_fields_are_strings(&shadows);

    let expanded = expanded_non_dictionary_events(shadows.clone());
    let fields = v2_schema_fields("prodex-v2-tool-use");
    for event in v2_tool_use_events(&expanded) {
        assert_eq!(
            resolve_v2_schema_string(&fields["toolName"], event).as_deref(),
            Some(tool_name)
        );
    }
}

#[test]
fn super_slim_v2_interns_command_and_repo_path_prefixes_when_smaller() {
    let command_prefix =
        "cargo test -q -p prodex-runtime-mem --lib compact_v2_runtime_memory_tests::";
    let repo_prefix = "/workspace/prodex/crates/prodex-runtime-mem/src/runtime/schema/native/";
    let mut events = Vec::new();
    for name in [
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
    ] {
        events.push(serde_json::json!({
            "payload": {
                "type": "exec_command",
                "call_id": format!("call-cmd-{name}"),
                "command": format!("{command_prefix}{name}")
            }
        }));
    }
    for name in [
        "lib.rs",
        "tests.rs",
        "schema.rs",
        "dictionary.rs",
        "prefix.rs",
        "call_id.rs",
        "shadow.rs",
        "expand.rs",
    ] {
        events.push(serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": format!("path ref {name}"),
                "metadata": {
                    "artifact_ref": format!("{repo_prefix}{name}")
                }
            }
        }));
    }

    let base_shadows = v2_shadow_events_without_dictionary(events.iter());
    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert!(runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows));
    assert!(v2_dictionary_events(&shadows).iter().any(|event| {
        event.get("k").and_then(Value::as_str) == Some("c")
            && event.get("m").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX)
    }));
    assert!(v2_dictionary_events(&shadows).iter().any(|event| {
        event.get("k").and_then(Value::as_str) == Some("r")
            && event.get("m").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX)
    }));
    assert!(v2_tool_use_events(&shadows).iter().any(|event| {
        event
            .get("c")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("ss:d:c#"))
    }));
    assert!(v2_user_events(&shadows).iter().any(|event| {
        event
            .get("r")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("ss:d:r#"))
    }));
    assert_v2_raw_events_schema_addressable(&shadows);
    assert_v2_compact_fields_are_strings(&shadows);

    let expanded = expanded_non_dictionary_events(shadows);
    let tool_fields = v2_schema_fields("prodex-v2-tool-use");
    let user_fields = v2_schema_fields("prodex-v2-user-message");
    let tool_inputs = v2_tool_use_events(&expanded)
        .iter()
        .filter_map(|event| resolve_v2_schema_string(&tool_fields["toolInput"], event))
        .collect::<Vec<_>>();
    let user_prompts = v2_user_events(&expanded)
        .iter()
        .filter_map(|event| resolve_v2_schema_string(&user_fields["prompt"], event))
        .collect::<Vec<_>>();
    assert!(tool_inputs.contains(&format!("{command_prefix}theta")));
    assert!(user_prompts.contains(&format!("{repo_prefix}expand.rs")));
}

#[test]
fn super_slim_v2_interns_call_id_prefixes_when_smaller() {
    let call_id_prefix = "call_01HF97R8Y9_prodex_runtime_mem_schema_native_dictionary_";
    let events = [
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
    ]
    .into_iter()
    .map(|suffix| {
        serde_json::json!({
            "payload": {
                "type": "function_call",
                "call_id": format!("{call_id_prefix}{suffix}"),
                "name": "tool"
            }
        })
    })
    .collect::<Vec<_>>();

    let base_shadows = v2_shadow_events_without_dictionary(events.iter());
    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert!(runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows));
    assert!(v2_dictionary_events(&shadows).iter().any(|event| {
        event.get("k").and_then(Value::as_str) == Some("i")
            && event.get("m").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX)
    }));
    assert!(v2_tool_use_events(&shadows).iter().any(|event| {
        event
            .get("i")
            .and_then(Value::as_str)
            .is_some_and(|value| value.starts_with("ss:d:i#"))
    }));
    assert_v2_raw_events_schema_addressable(&shadows);
    assert_v2_compact_fields_are_strings(&shadows);

    let expanded = expanded_non_dictionary_events(shadows);
    let fields = v2_schema_fields("prodex-v2-tool-use");
    let tool_ids = v2_tool_use_events(&expanded)
        .iter()
        .filter_map(|event| resolve_v2_schema_string(&fields["toolId"], event))
        .collect::<Vec<_>>();
    assert!(tool_ids.contains(&format!("{call_id_prefix}theta")));
}

#[test]
fn super_slim_v2_interns_exact_repeated_tool_ids_when_smaller() {
    let call_id = "call_exact_prodex_runtime_mem_dictionary_repeated_identifier_0123456789";
    let mut events = Vec::new();
    for index in 0..6 {
        events.push(serde_json::json!({
            "payload": {
                "type": "function_call",
                "call_id": call_id,
                "name": "tool",
                "arguments": format!("input {index}")
            }
        }));
        events.push(serde_json::json!({
            "payload": {
                "type": "function_call_output",
                "call_id": call_id,
                "output": format!("output {index}")
            }
        }));
    }

    let base_shadows = v2_shadow_events_without_dictionary(events.iter());
    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert!(runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows));
    assert!(v2_dictionary_events(&shadows).iter().any(|event| {
        event.get("k").and_then(Value::as_str) == Some("i")
            && event.get("m").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT)
            && event.get("v").and_then(Value::as_str) == Some(call_id)
    }));
    assert_v2_raw_events_schema_addressable(&shadows);
    assert_v2_compact_fields_are_strings(&shadows);

    let expanded = expanded_non_dictionary_events(shadows);
    for event in v2_tool_events_with_ids(&expanded) {
        assert_eq!(event.get("i").and_then(Value::as_str), Some(call_id));
    }
}

#[test]
fn super_slim_v2_interns_exact_repeated_summaries_when_smaller() {
    let summary = "user: exact repeated runtime memory summary retained through expansion";
    let events = (0..8)
        .map(|index| {
            serde_json::json!({
                "payload": {
                    "type": "user_message",
                    "id": format!("user-{index}"),
                    "message": format!("full user prompt {index} {}", "detail ".repeat(40)),
                    "metadata": {
                        "prompt_summary": summary
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    let base_shadows = v2_shadow_events_without_dictionary(events.iter());
    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert!(runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows));
    assert!(v2_dictionary_events(&shadows).iter().any(|event| {
        event.get("k").and_then(Value::as_str) == Some("s")
            && event.get("m").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT)
            && event.get("v").and_then(Value::as_str) == Some(summary)
    }));
    assert_v2_raw_events_schema_addressable(&shadows);
    assert_v2_compact_fields_are_strings(&shadows);

    let expanded = expanded_non_dictionary_events(shadows);
    let fields = v2_schema_fields("prodex-v2-user-message");
    for event in v2_user_events(&expanded) {
        assert_eq!(
            resolve_v2_schema_string(&fields["prompt"], event).as_deref(),
            Some(summary)
        );
    }
}

#[test]
fn super_slim_v2_dictionary_skips_compaction_when_not_smaller() {
    let events = [
        serde_json::json!({
            "payload": {
                "type": "custom_tool_call",
                "call_id": "call-a",
                "name": "sh",
                "action": "a"
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "custom_tool_call",
                "call_id": "call-b",
                "name": "sh",
                "action": "b"
            }
        }),
    ];

    let base_shadows = v2_shadow_events_without_dictionary(events.iter());
    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert_eq!(shadows, base_shadows);
    assert!(v2_dictionary_events(&shadows).is_empty());
    assert_v2_raw_events_schema_addressable(&shadows);
}

#[test]
fn super_slim_v2_dictionary_candidate_savings_matches_applied_jsonl_delta() {
    let summary = "user: repeated summary for candidate savings";
    let events = (0..6)
        .map(|index| {
            serde_json::json!({
                "payload": {
                    "type": "user_message",
                    "message": format!("full user prompt {index} {}", "detail ".repeat(16)),
                    "metadata": {
                        "prompt_summary": summary
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    let base_shadows = v2_shadow_events_without_dictionary(events.iter());
    let candidate = runtime_mem_super_slim_v2_dictionary_candidates(&base_shadows)
        .into_iter()
        .find(|candidate| {
            candidate.field == "s"
                && candidate.mode == RuntimeMemSuperSlimV2DictionaryMode::Exact
                && candidate.value == summary
        })
        .expect("expected repeated summary dictionary candidate");

    let estimated = runtime_mem_super_slim_v2_candidate_savings(&base_shadows, &candidate)
        .expect("candidate should shrink JSONL");
    let compacted =
        runtime_mem_super_slim_v2_apply_dictionary_candidate(base_shadows.clone(), &candidate);
    let actual =
        runtime_mem_jsonl_events_len(&base_shadows) - runtime_mem_jsonl_events_len(&compacted);

    assert_eq!(estimated, actual);
}

#[test]
fn super_slim_v2_intern_expansion_preserves_legacy_explicit_events() {
    let legacy = serde_json::json!({
        "t": RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE,
        "i": "call-legacy-explicit",
        "n": "exec_command",
        "c": "cargo test -q -p prodex-runtime-mem --lib"
    });

    let expanded = runtime_mem_super_slim_v2_expand_interned_events([legacy]);
    let fields = v2_schema_fields("prodex-v2-tool-use");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolId"], &expanded[0]).as_deref(),
        Some("call-legacy-explicit")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &expanded[0]).as_deref(),
        Some("exec_command")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &expanded[0]).as_deref(),
        Some("cargo test -q -p prodex-runtime-mem --lib")
    );
}

#[test]
fn super_slim_v2_intern_expansion_accepts_legacy_dictionary_refs() {
    let events = [
        serde_json::json!({
            "t": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
            "k": "n",
            "i": "0",
            "m": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY,
            "v": "legacy_tool"
        }),
        serde_json::json!({
            "t": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
            "k": "c",
            "i": "0",
            "m": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY,
            "v": "legacy inline command"
        }),
        serde_json::json!({
            "t": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
            "k": "r",
            "i": "0",
            "m": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX_LEGACY,
            "v": "psc:legacy-prefix-"
        }),
        serde_json::json!({
            "t": RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE,
            "i": "call-legacy-dict",
            "n": "ss:dict:n#0",
            "c": "run {ss:dict:c#0}"
        }),
        serde_json::json!({
            "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
            "r": "ss:dict:r#0+tail"
        }),
    ];

    let expanded = runtime_mem_super_slim_v2_expand_interned_events(events);
    let tool_fields = v2_schema_fields("prodex-v2-tool-use");
    let user_fields = v2_schema_fields("prodex-v2-user-message");

    assert_eq!(
        resolve_v2_schema_string(&tool_fields["toolName"], &expanded[0]).as_deref(),
        Some("legacy_tool")
    );
    assert_eq!(
        resolve_v2_schema_string(&tool_fields["toolInput"], &expanded[0]).as_deref(),
        Some("run legacy inline command")
    );
    assert_eq!(
        resolve_v2_schema_string(&user_fields["prompt"], &expanded[1]).as_deref(),
        Some("psc:legacy-prefix-tail")
    );
}

fn v2_shadow_events_without_dictionary<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
    let mut ref_dedupe_state = RuntimeMemSuperSlimV2ArtifactRefDedupeState::default();
    runtime_mem_super_slim_shadow_codex_events(events)
        .iter()
        .map(runtime_mem_super_slim_v2_shadow_from_v1_shadow)
        .map(|event| ref_dedupe_state.dedupe_consecutive_event_ref(event))
        .collect()
}

fn v2_dictionary_events(events: &[Value]) -> Vec<&Value> {
    events
        .iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .collect()
}

fn v2_tool_use_events(events: &[Value]) -> Vec<&Value> {
    v2_events_of_type(events, RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE)
}

fn v2_tool_events_with_ids(events: &[Value]) -> Vec<&Value> {
    events
        .iter()
        .filter(|event| {
            matches!(
                runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str),
                Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE)
                    | Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE)
            )
        })
        .collect()
}

fn v2_user_events(events: &[Value]) -> Vec<&Value> {
    v2_events_of_type(events, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE)
}

fn v2_events_of_type<'a>(events: &'a [Value], event_type: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str) == Some(event_type)
        })
        .collect()
}

fn expanded_non_dictionary_events(events: Vec<Value>) -> Vec<Value> {
    runtime_mem_super_slim_v2_expand_interned_events(events)
        .into_iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                != Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .collect()
}

fn assert_v2_raw_events_schema_addressable(events: &[Value]) {
    for event in events {
        match runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str) {
            Some(RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings("prodex-v2-user-message", event, &["prompt"]);
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-assistant-message",
                    event,
                    &["message"],
                );
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-tool-use",
                    event,
                    &["toolId", "toolName", "toolInput"],
                );
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-tool-result",
                    event,
                    &["toolId", "toolResponse"],
                );
            }
            Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE) => {
                assert_v2_schema_fields_are_strings(
                    "prodex-v2-dictionary-entry",
                    event,
                    &[
                        "dictionary",
                        "dictionaryKey",
                        "dictionaryIndex",
                        "dictionaryMode",
                        "dictionaryValue",
                    ],
                );
            }
            _ => {}
        }
    }
}

fn assert_v2_schema_fields_are_strings(event_name: &str, event: &Value, field_names: &[&str]) {
    let fields = v2_schema_fields(event_name);
    for field_name in field_names {
        let value = resolve_v2_schema_string(&fields[*field_name], event)
            .unwrap_or_else(|| panic!("{event_name}.{field_name} should resolve: {event}"));
        assert!(
            !value.trim().is_empty(),
            "{event_name}.{field_name} should be meaningful: {event}"
        );
    }
}

fn assert_v2_compact_fields_are_strings(events: &[Value]) {
    for event in events {
        for field in [
            "i",
            "n",
            "c",
            "r",
            "s",
            "p",
            "k",
            "m",
            "v",
            RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD,
        ] {
            if let Some(value) = event.get(field) {
                assert!(value.is_string(), "{field} should stay string: {event}");
            }
        }
    }
}

fn v2_schema_fields(event_name: &str) -> Value {
    runtime_mem_super_slim_codex_schema()
        .get("events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.get("name").and_then(Value::as_str) == Some(event_name))
        })
        .and_then(|event| event.get("fields"))
        .cloned()
        .expect("v2 schema fields should exist")
}

fn resolve_v2_schema_string(spec: &Value, entry: &Value) -> Option<String> {
    resolve_v2_schema_field(spec, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_v2_schema_field(spec: &Value, entry: &Value) -> Option<Value> {
    if let Some(path) = spec.as_str() {
        return runtime_mem_lookup_json_path(entry, path).cloned();
    }
    if let Some(value) = spec.get("value") {
        return Some(value.clone());
    }
    if let Some(coalesce) = spec.get("coalesce").and_then(Value::as_array) {
        for candidate in coalesce {
            if let Some(value) = resolve_v2_schema_field(candidate, entry)
                && !value.as_str().is_some_and(str::is_empty)
            {
                return Some(value);
            }
        }
    }
    None
}

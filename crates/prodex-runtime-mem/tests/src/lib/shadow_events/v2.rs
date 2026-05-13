use super::*;

#[test]
fn super_slim_v2_shadow_events_are_short_and_schema_addressable() {
    let user_prompt = "Implement concise memory bridge\n".to_string() + &"detail ".repeat(120);
    let tool_output = "cargo test passed\n".to_string() + &"ok ".repeat(120);
    let events = [
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": user_prompt,
                "metadata": {
                    "artifact_ref": "p:0123456789abcdef"
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": "Long assistant body",
                "summary": "assistant concise summary"
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command",
                "call_id": "call-1",
                "command": "cargo test -q"
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command_output",
                "call_id": "call-1",
                "output": tool_output,
                "metadata": {
                    "artifact_ref": "psc:fedcba9876543210"
                }
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());
    assert_eq!(shadows.len(), 4);
    assert_eq!(shadows[0]["t"].as_str(), Some("pm2:u"));
    assert_eq!(shadows[1]["t"].as_str(), Some("pm2:a"));
    assert_eq!(shadows[2]["t"].as_str(), Some("pm2:tu"));
    assert_eq!(shadows[3]["t"].as_str(), Some("pm2:tr"));
    assert_eq!(shadows[0]["r"].as_str(), Some("p:0123456789abcdef"));
    assert_eq!(shadows[3]["r"].as_str(), Some("psc:fedcba9876543210"));
    assert_eq!(shadows[0].get("s"), None);
    assert_eq!(shadows[3].get("s"), None);
    assert!(runtime_mem_event_has_super_slim_prompt_reference(
        &shadows[0]
    ));
    assert!(!shadows[0].to_string().contains("detail detail detail"));
    assert!(!shadows[3].to_string().contains("ok ok ok"));

    let schema_text = runtime_mem_super_slim_codex_schema().to_string();
    assert!(schema_text.contains("prodex-v2-user-message"));
    assert!(schema_text.contains("prodex-v2-tool-result"));
}

#[test]
fn super_slim_v2_keeps_artifact_backed_summary_when_critical() {
    let event = serde_json::json!({
        "payload": {
            "type": "exec_command_output",
            "call_id": "call-err",
            "summary": "tool: error[E0425]: cannot find value",
            "metadata": {
                "artifact_ref": "psc:fedcba9876543210"
            },
            "output": "full output omitted"
        }
    });

    let shadow = runtime_mem_super_slim_v2_shadow_codex_event(&event);

    assert_eq!(shadow["t"].as_str(), Some("pm2:tr"));
    assert_eq!(shadow["r"].as_str(), Some("psc:fedcba9876543210"));
    assert_eq!(
        shadow["s"].as_str(),
        Some("tool: error[E0425]: cannot find value")
    );
}

#[test]
fn super_slim_v2_elides_old_tool_output_into_fact_index_summary() {
    let old_output = [
        "$ cargo test -q -p prodex-runtime-mem",
        "error[E0425]: cannot find value `missing` in this scope",
        " --> crates/prodex-runtime-mem/src/lib.rs:42:9",
        "stored artifact p:old-tool-output",
        &"repeated compiler context ".repeat(80),
    ]
    .join("\n");
    let recent_output =
        "error[E0308]: mismatched types\n --> crates/prodex-runtime-mem/src/lib.rs:99:5\n"
            .to_string()
            + &"recent failure detail ".repeat(60);
    let mut events = vec![
        serde_json::json!({
            "payload": {
                "type": "exec_command",
                "call_id": "call-old",
                "command": "cargo test -q -p prodex-runtime-mem"
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command_output",
                "call_id": "call-old",
                "output": old_output
            }
        }),
    ];
    for index in 0..8 {
        events.push(serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": format!("progress marker {index}")
            }
        }));
    }
    events.push(serde_json::json!({
        "payload": {
            "type": "exec_command",
            "call_id": "call-recent",
            "command": "cargo check -q"
        }
    }));
    events.push(serde_json::json!({
        "payload": {
            "type": "exec_command_output",
            "call_id": "call-recent",
            "output": recent_output
        }
    }));

    let shadows = runtime_mem_super_slim_v2_expand_interned_events(
        runtime_mem_super_slim_v2_shadow_codex_events(events.iter()),
    );
    let old_result = shadows
        .iter()
        .find(|event| {
            event.get("t").and_then(Value::as_str) == Some("pm2:tr")
                && event.get("i").and_then(Value::as_str) == Some("call-old")
        })
        .expect("old tool result should exist");
    let old_summary = old_result
        .get("s")
        .and_then(Value::as_str)
        .expect("old tool result should keep fact summary");

    assert!(old_summary.starts_with("mem ledger: kind=tool"));
    assert!(old_summary.contains("cargo test -q -p prodex-runtime-mem"));
    assert!(old_summary.contains("crates/prodex-runtime-mem/src/lib.rs"));
    assert!(old_summary.contains("error[E0425]"));
    assert!(old_summary.contains("p:old-tool-output"));
    assert!(!old_summary.contains("repeated compiler context repeated compiler context"));
    assert_eq!(
        old_result.get("r").and_then(Value::as_str),
        Some("p:old-tool-output")
    );

    let recent_result = shadows
        .iter()
        .find(|event| {
            event.get("t").and_then(Value::as_str) == Some("pm2:tr")
                && event.get("i").and_then(Value::as_str) == Some("call-recent")
        })
        .expect("recent tool result should exist");
    let recent_summary = recent_result
        .get("s")
        .and_then(Value::as_str)
        .expect("recent failure summary should stay explicit");
    assert!(recent_summary.starts_with("tool: error[E0308]"));
    assert!(!recent_summary.starts_with("mem ledger:"));
}

#[test]
fn super_slim_v2_elides_old_assistant_output_but_preserves_final_decision() {
    let old_assistant = "Implemented cache probe in crates/prodex-runtime-mem/src/lib.rs\n"
        .to_string()
        + "$ cargo test -q -p prodex-runtime-mem\n"
        + "Changed summary writer for repeated outputs\n"
        + &"verbose implementation notes ".repeat(80);
    let final_decision = "Final decision: keep prodex s launch behavior unchanged.\n".to_string()
        + &"rationale detail ".repeat(80);
    let mut events = vec![
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": old_assistant
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": final_decision
            }
        }),
    ];
    for index in 0..9 {
        events.push(serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": format!("recent progress marker {index}")
            }
        }));
    }

    let shadows = runtime_mem_super_slim_v2_expand_interned_events(
        runtime_mem_super_slim_v2_shadow_codex_events(events.iter()),
    );
    let old_summary = shadows[0]
        .get("s")
        .and_then(Value::as_str)
        .expect("old assistant should have summary");
    let final_summary = shadows[1]
        .get("s")
        .and_then(Value::as_str)
        .expect("final decision should have summary");

    assert!(old_summary.starts_with("mem ledger: kind=assistant"));
    assert!(old_summary.contains("crates/prodex-runtime-mem/src/lib.rs"));
    assert!(old_summary.contains("cargo test -q -p prodex-runtime-mem"));
    assert!(old_summary.contains("Changed summary writer"));
    assert!(!old_summary.contains("verbose implementation notes verbose implementation notes"));

    assert!(final_summary.starts_with("mem ledger: kind=assistant"));
    assert!(final_summary.contains("decisions=[Final decision: keep prodex s launch behavior"));
    assert!(!final_summary.contains("decision rationale detail decision rationale detail"));
}

#[test]
fn super_slim_v2_old_turns_use_task_ledger_and_keep_recent_turns_rich() {
    let old_prompt = "Task: Implement token-efficient task ledger for older turns\n".to_string()
        + "Touch crates/prodex-runtime-mem/src/lib.rs and crates/prodex-runtime-mem/tests/src/lib.rs\n"
        + &"older prompt detail ".repeat(80);
    let old_tool_output = [
        "$ cargo test -q -p prodex-runtime-mem",
        "error[E0425]: cannot find value `missing_ledger` in this scope",
        " --> crates/prodex-runtime-mem/src/lib.rs:42:9",
        &"verbose failing test context ".repeat(80),
    ]
    .join("\n");
    let old_assistant = "Decision: keep recent turns in rich summary form\n".to_string()
        + "Implemented ledger extraction in crates/prodex-runtime-mem/src/lib.rs\n"
        + &"verbose assistant implementation notes ".repeat(80);
    let recent_assistant = "Recent rich response should keep normal assistant summary\n"
        .to_string()
        + &"recent context ".repeat(80);
    let mut events = vec![
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": old_prompt.clone()
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command",
                "call_id": "call-ledger-test",
                "command": "cargo test -q -p prodex-runtime-mem"
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command_output",
                "call_id": "call-ledger-test",
                "output": old_tool_output.clone()
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": old_assistant.clone()
            }
        }),
    ];
    for index in 0..8 {
        events.push(serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": format!("recent marker {index}")
            }
        }));
    }
    events.push(serde_json::json!({
        "payload": {
            "type": "agent_message",
            "message": recent_assistant
        }
    }));

    let raw_len = runtime_mem_jsonl_events_len(&events);
    let shadows = runtime_mem_super_slim_v2_expand_interned_events(
        runtime_mem_super_slim_v2_shadow_codex_events(events.iter()),
    );
    let shadow_len = runtime_mem_jsonl_events_len(&shadows);
    let ledger_text = shadows
        .iter()
        .filter_map(|event| event.get("s").and_then(Value::as_str))
        .filter(|summary| summary.starts_with("mem ledger:"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(shadow_len < raw_len);
    assert!(
        runtime_mem_approx_token_count(&ledger_text)
            < runtime_mem_approx_token_count(
                &[
                    old_prompt.as_str(),
                    old_tool_output.as_str(),
                    old_assistant.as_str()
                ]
                .join("\n")
            )
    );
    assert!(ledger_text.contains("objective=[Implement token-efficient task ledger"));
    assert!(ledger_text.contains("files=[crates/prodex-runtime-mem/src/lib.rs"));
    assert!(ledger_text.contains("decisions=[Decision: keep recent turns"));
    assert!(ledger_text.contains("tests=[cargo test -q -p prodex-runtime-mem"));
    assert!(ledger_text.contains("open_failures=[error[E0425]"));

    let recent_summary = shadows
        .last()
        .and_then(|event| event.get("s"))
        .and_then(Value::as_str)
        .expect("recent assistant should still have a summary");
    assert!(recent_summary.starts_with("a: Recent rich response"));
    assert!(!recent_summary.starts_with("mem ledger:"));
}

#[test]
fn super_slim_v2_inline_dictionary_interns_common_runtime_strings_and_expands_exactly() {
    let temp_prefix = "/tmp/prodex-token-ledger-cache/session-alpha/build/";
    let url_prefix = "https://updates.example.com/prodex/runtime/";
    let branch = "refs/heads/feature/token-efficiency-ledger";
    let profile = "prodex-profile-alpha-token-ledger";
    let crate_name = "prodex-runtime-memory-ledger-support";
    let stack_prefix = "prodex_runtime_mem::ledger::collector::";
    let events = (0..10)
        .map(|index| {
            serde_json::json!({
                "payload": {
                    "type": "user_message",
                    "message": format!("prompt {index}"),
                    "metadata": {
                        "prompt_summary": format!(
                            "case {index} failed at {temp_prefix}{index}.log url {url_prefix}{index} branch {branch} profile={profile} package {crate_name} stack {stack_prefix}frame_{index}"
                        )
                    }
                }
            })
        })
        .collect::<Vec<_>>();

    let base_shadows = test_v2_shadow_events_without_dictionary(events.iter());
    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());
    let dictionary_values = test_v2_dictionary_events(&shadows)
        .iter()
        .filter_map(|event| event.get("v").and_then(Value::as_str))
        .collect::<Vec<_>>();

    assert!(runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows));
    assert!(dictionary_values.contains(&temp_prefix));
    assert!(
        dictionary_values.contains(&url_prefix)
            || dictionary_values.contains(&"https://updates.example.com")
    );
    assert!(dictionary_values.contains(&branch));
    assert!(dictionary_values.contains(&profile));
    assert!(dictionary_values.contains(&crate_name));
    assert!(dictionary_values.contains(&stack_prefix));
    assert!(shadows.iter().any(|event| {
        event
            .get("s")
            .and_then(Value::as_str)
            .is_some_and(|summary| summary.contains("ss:d:s#"))
    }));

    let expanded = test_expanded_non_dictionary_events(shadows);
    assert_eq!(expanded, base_shadows);
}

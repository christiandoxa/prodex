use super::*;

#[test]
fn extract_mode_keeps_slim_and_full_behavior_and_accepts_super_slim() {
    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("mem"), OsString::from("exec")]);
    assert!(mem_mode);
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) =
        runtime_mem_extract_mode_with_detail(&[OsString::from("mem-full"), OsString::from("exec")]);
    assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Full));
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&[
        OsString::from("mem"),
        OsString::from("--mem-full"),
        OsString::from("exec"),
    ]);
    assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Full));
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&[
        OsString::from("mem-super-slim"),
        OsString::from("exec"),
    ]);
    assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::SuperSlim));
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&[
        OsString::from("mem"),
        OsString::from("--mem-super-slim"),
        OsString::from("exec"),
    ]);
    assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::SuperSlim));
    assert_eq!(codex_args, vec![OsString::from("exec")]);

    let (mem_mode, codex_args) =
        runtime_mem_extract_mode(&[OsString::from("exec"), OsString::from("mem")]);
    assert!(!mem_mode);
    assert_eq!(
        codex_args,
        vec![OsString::from("exec"), OsString::from("mem")]
    );
}

#[test]
fn super_default_transcript_mode_upgrades_only_slim_to_super_slim() {
    assert_eq!(
        runtime_mem_super_default_transcript_mode(Some(RuntimeMemTranscriptMode::Slim)),
        Some(RuntimeMemTranscriptMode::SuperSlim)
    );
    assert_eq!(
        runtime_mem_super_default_transcript_mode(Some(RuntimeMemTranscriptMode::SuperSlim)),
        Some(RuntimeMemTranscriptMode::SuperSlim)
    );
    assert_eq!(
        runtime_mem_super_default_transcript_mode(Some(RuntimeMemTranscriptMode::Full)),
        Some(RuntimeMemTranscriptMode::Full)
    );
    assert_eq!(runtime_mem_super_default_transcript_mode(None), None);
}

#[test]
fn slim_and_full_schema_keep_legacy_fields_and_include_codex_129_events() {
    let slim = runtime_mem_default_codex_schema().to_string();
    assert!(slim.contains("0.4-slim"));
    assert!(slim.contains("\"prompt\":\"payload.message\""));
    assert!(slim.contains("payload.content[0].text"));
    assert!(slim.contains("local_shell_call"));
    assert!(slim.contains("output omitted"));
    assert!(!slim.contains("\"toolResponse\":\"payload.output\""));

    let full = runtime_mem_full_codex_schema().to_string();
    assert!(full.contains("Full schema"));
    assert!(full.contains("\"prompt\":\"payload.message\""));
    assert!(full.contains("payload.content[0].text"));
    assert!(full.contains("local_shell_call"));
    assert!(full.contains("\"toolResponse\":\"payload.output\""));
    assert!(full.contains("\"message\":\"payload.message\""));
}

#[test]
fn codex_129_response_item_schema_resolves_messages_and_local_shell_calls() {
    let response_user = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [
                { "type": "input_text", "text": "new Codex prompt" }
            ]
        }
    });
    let response_assistant = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "output_text", "text": "new Codex answer" }
            ]
        }
    });
    let local_shell = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "local_shell_call",
            "call_id": "call-shell",
            "action": { "command": "cargo test -q -p prodex-runtime-mem" }
        }
    });

    let slim_schema = runtime_mem_default_codex_schema();
    assert_eq!(
        resolve_schema_event_string(
            &slim_schema,
            "response-user-message",
            "prompt",
            &response_user
        )
        .as_deref(),
        Some("new Codex prompt")
    );
    assert_eq!(
        resolve_schema_event_string(
            &slim_schema,
            "response-assistant-message",
            "message",
            &response_assistant
        )
        .as_deref(),
        Some("new Codex answer")
    );
    assert_eq!(
        resolve_schema_event_string(&slim_schema, "tool-use", "toolInput", &local_shell).as_deref(),
        Some("cargo test -q -p prodex-runtime-mem")
    );

    let full_schema = runtime_mem_full_codex_schema();
    assert_eq!(
        resolve_schema_event_string(
            &full_schema,
            "response-user-message",
            "prompt",
            &response_user
        )
        .as_deref(),
        Some("new Codex prompt")
    );

    let super_slim_schema = runtime_mem_super_slim_codex_schema();
    assert_eq!(
        resolve_schema_event_string(
            &super_slim_schema,
            "response-user-message",
            "prompt",
            &response_user
        )
        .as_deref(),
        Some("ss:omit=prompt")
    );
    let summarized_user = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [
                { "type": "input_text", "text": "raw prompt should not be recalled by super-slim" }
            ],
            "metadata": {
                "prompt_summary": "short Codex 0.129 prompt summary"
            }
        }
    });
    assert_eq!(
        resolve_schema_event_string(
            &super_slim_schema,
            "response-user-message",
            "prompt",
            &summarized_user
        )
        .as_deref(),
        Some("short Codex 0.129 prompt summary")
    );
    let super_slim_prompt_field =
        schema_event_field(&super_slim_schema, "response-user-message", "prompt")
            .expect("super-slim response user prompt field should exist")
            .to_string();
    assert!(
        !super_slim_prompt_field.contains("payload.content"),
        "super-slim schema must not recall raw Codex 0.129 prompt content"
    );
}

#[test]
fn super_slim_schema_prefers_prompt_summary_or_refs_and_omits_plain_prompt_body() {
    let large_prompt = "repeat ".repeat(8_000);
    let slim_prompt = resolve_schema_user_prompt(
        &runtime_mem_default_codex_schema(),
        &serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": large_prompt
            }
        }),
    )
    .expect("slim prompt should resolve");
    let super_slim_schema = runtime_mem_super_slim_codex_schema();
    let super_slim_prompt = resolve_schema_user_prompt(
        &super_slim_schema,
        &serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": large_prompt
            }
        }),
    )
    .expect("super-slim prompt should resolve");

    assert_eq!(slim_prompt, large_prompt);
    assert_eq!(super_slim_prompt, "ss:omit=prompt");

    let summarized = resolve_schema_user_prompt(
        &super_slim_schema,
        &serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": large_prompt,
                "metadata": {
                    "prompt_summary": "Task summary and artifact prodex://artifact/prompt-1"
                }
            }
        }),
    )
    .expect("super-slim summary prompt should resolve");
    assert_eq!(
        summarized,
        "Task summary and artifact prodex://artifact/prompt-1"
    );
    let artifact_ref = resolve_schema_user_prompt(
        &super_slim_schema,
        &serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": large_prompt,
                "metadata": {
                    "artifact_ref": "prodex-artifact:sc:prompt-1"
                }
            }
        }),
    )
    .expect("super-slim artifact ref prompt should resolve");
    assert_eq!(artifact_ref, "prodex-artifact:sc:prompt-1");

    let prompt_field = schema_user_prompt_field(&super_slim_schema)
        .expect("super-slim schema should define prompt field")
        .to_string();
    assert!(prompt_field.contains("metadata.prompt_summary"));
    assert!(prompt_field.contains("artifact"));
    assert!(
        !prompt_field.contains("payload.message"),
        "super-slim schema must not recall full prompt bodies without shadow summaries"
    );
}

#[test]
fn transcript_watch_super_slim_schema_is_shadow_safe_and_full_mode_remains_full() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-runtime-mem-watch-test-{}-{stamp}",
        std::process::id()
    ));
    let config_path = root.join("claude-mem/transcript-watch.json");
    let sessions_root = root.join("codex/sessions");
    fs::create_dir_all(&sessions_root).expect("sessions root should exist");

    ensure_runtime_mem_codex_watch_for_sessions_root_with_mode(
        &config_path,
        &sessions_root,
        RuntimeMemTranscriptMode::SuperSlim,
    )
    .expect("super-slim watch should write");
    let super_slim_config: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("config should read"))
            .expect("config should parse");
    let super_slim_schema = &super_slim_config["schemas"]["codex"];
    let super_slim_schema_text = super_slim_schema.to_string();
    assert_eq!(
        super_slim_schema["version"].as_str(),
        Some("0.7-super-slim-v2")
    );
    assert!(super_slim_schema_text.contains("pm2:u"));
    assert!(super_slim_schema_text.contains("prompt_summary"));
    assert!(!super_slim_schema_text.contains("payload.message"));
    assert!(!super_slim_schema_text.contains("payload.content"));
    assert!(!super_slim_schema_text.contains("payload.output"));

    ensure_runtime_mem_codex_watch_for_sessions_root_with_mode(
        &config_path,
        &sessions_root,
        RuntimeMemTranscriptMode::Full,
    )
    .expect("full watch should write");
    let full_config: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("config should read"))
            .expect("config should parse");
    let full_schema_text = full_config["schemas"]["codex"].to_string();
    assert!(full_schema_text.contains("\"prompt\":\"payload.message\""));
    assert!(full_schema_text.contains("\"toolResponse\":\"payload.output\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn safe_auto_schema_mode_uses_super_slim_only_with_prompt_summary_or_artifact_ref() {
    let plain_prompt = serde_json::json!({
        "payload": {
            "type": "user_message",
            "message": "long prompt without safe summary"
        }
    });
    let prompt_summary = serde_json::json!({
        "payload": {
            "type": "user_message",
            "message": "long prompt",
            "metadata": {
                "prompt_summary": "short prompt summary"
            }
        }
    });
    let artifact_ref = serde_json::json!({
        "payload": {
            "type": "user_message",
            "message": "long prompt",
            "metadata": {
                "artifact_ref": "prodex-artifact:sc:abc"
            }
        }
    });
    let marker_text = serde_json::json!({
        "payload": {
            "type": "user_message",
            "message": "use prodex-artifact:sc:def"
        }
    });
    let generic_summary = serde_json::json!({
        "payload": {
            "type": "user_message",
            "message": "long prompt",
            "metadata": {
                "summary": "not a prompt_summary field"
            }
        }
    });

    assert_eq!(
        runtime_mem_safe_auto_codex_schema_mode_for_event(
            RuntimeMemTranscriptMode::Slim,
            &plain_prompt,
        ),
        RuntimeMemTranscriptMode::Slim
    );
    assert_eq!(
        runtime_mem_safe_auto_codex_schema_mode_for_event(
            RuntimeMemTranscriptMode::SuperSlim,
            &plain_prompt,
        ),
        RuntimeMemTranscriptMode::Slim
    );
    assert_eq!(
        runtime_mem_safe_auto_codex_schema_mode_for_event(
            RuntimeMemTranscriptMode::Slim,
            &prompt_summary,
        ),
        RuntimeMemTranscriptMode::SuperSlim
    );
    assert_eq!(
        runtime_mem_safe_auto_codex_schema_mode_for_event(
            RuntimeMemTranscriptMode::Slim,
            &artifact_ref,
        ),
        RuntimeMemTranscriptMode::SuperSlim
    );
    assert_eq!(
        runtime_mem_safe_auto_codex_schema_mode_for_event(
            RuntimeMemTranscriptMode::Slim,
            &marker_text,
        ),
        RuntimeMemTranscriptMode::SuperSlim
    );
    assert_eq!(
        runtime_mem_safe_auto_codex_schema_mode_for_event(
            RuntimeMemTranscriptMode::Slim,
            &generic_summary,
        ),
        RuntimeMemTranscriptMode::Slim
    );
    assert_eq!(
        runtime_mem_safe_auto_codex_schema_mode_for_event(
            RuntimeMemTranscriptMode::Full,
            &prompt_summary,
        ),
        RuntimeMemTranscriptMode::Full
    );
}

#[test]
fn safe_auto_schema_policy_and_schema_helper_are_ready_for_app_integration() {
    let event = serde_json::json!({
        "payload": {
            "type": "user_message",
            "metadata": {
                "artifact_id": "prompt-artifact-1"
            }
        }
    });

    assert!(runtime_mem_event_has_super_slim_prompt_reference(&event));
    assert_eq!(
        runtime_mem_select_codex_schema_mode_for_event(
            RuntimeMemSchemaSelectionPolicy::SafeSuperSlimCandidate {
                fallback_mode: RuntimeMemTranscriptMode::Slim,
            },
            &event,
        ),
        RuntimeMemTranscriptMode::SuperSlim
    );
    assert_eq!(
        runtime_mem_select_codex_schema_mode_for_event(
            RuntimeMemSchemaSelectionPolicy::Explicit(RuntimeMemTranscriptMode::Slim),
            &event,
        ),
        RuntimeMemTranscriptMode::Slim
    );

    let schema =
        runtime_mem_codex_schema_for_safe_auto_event(RuntimeMemTranscriptMode::Slim, &event);
    assert_eq!(
        schema.get("version").and_then(Value::as_str),
        Some("0.7-super-slim-v2")
    );
}

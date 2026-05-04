use super::*;
use serde_json::Value;
use std::ffi::OsString;
use std::path::PathBuf;

fn capsule(
    id: &str,
    token_cost: usize,
    required: bool,
    project_path: Option<&str>,
    updated_at_seconds: Option<i64>,
    relevance: f32,
) -> RuntimeMemCapsuleMetadata {
    RuntimeMemCapsuleMetadata {
        id: id.to_string(),
        token_cost,
        required,
        project_path: project_path.map(PathBuf::from),
        updated_at_seconds,
        relevance,
    }
}

#[test]
fn classifies_required_project_local_recent_and_optional_capsules() {
    let context = RuntimeMemCapsuleSelectionContext {
        token_budget: 100,
        project_root: Some(PathBuf::from("/work/prodex")),
        now_seconds: Some(1_000),
        recent_window_seconds: 60,
    };

    assert_eq!(
        runtime_mem_classify_capsule(
            &capsule("required", 10, true, Some("/elsewhere"), Some(0), 0.0),
            &context,
        ),
        RuntimeMemCapsulePriority::Required
    );
    assert_eq!(
        runtime_mem_classify_capsule(
            &capsule(
                "project-local",
                10,
                false,
                Some("/work/prodex/src/lib.rs"),
                Some(0),
                0.0,
            ),
            &context,
        ),
        RuntimeMemCapsulePriority::ProjectLocal
    );
    assert_eq!(
        runtime_mem_classify_capsule(
            &capsule("recent", 10, false, Some("/elsewhere"), Some(980), 0.0),
            &context,
        ),
        RuntimeMemCapsulePriority::Recent
    );
    assert_eq!(
        runtime_mem_classify_capsule(
            &capsule("optional", 10, false, Some("/elsewhere"), Some(100), 0.0),
            &context,
        ),
        RuntimeMemCapsulePriority::Optional
    );
}

#[test]
fn selection_uses_required_project_local_recent_optional_priority_before_relevance() {
    let selection = runtime_mem_select_capsules(
        [
            capsule("optional-high", 20, false, None, Some(100), 1.0),
            capsule("recent-low", 20, false, None, Some(995), 0.1),
            capsule(
                "project-low",
                30,
                false,
                Some("/repo/prodex/crates/mem"),
                Some(100),
                0.1,
            ),
            capsule("required", 10, true, None, Some(100), 0.0),
        ],
        RuntimeMemCapsuleSelectionContext {
            token_budget: 60,
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
    );

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["required", "project-low", "recent-low"]
    );
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.priority)
            .collect::<Vec<_>>(),
        vec![
            RuntimeMemCapsulePriority::Required,
            RuntimeMemCapsulePriority::ProjectLocal,
            RuntimeMemCapsulePriority::Recent,
        ]
    );
    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional-high"]
    );
    assert_eq!(selection.used_tokens, 60);
}

#[test]
fn selection_keeps_scanning_after_oversized_higher_priority_capsule() {
    let selection = runtime_mem_select_capsules(
        [
            capsule("required-too-large", 90, true, None, None, 0.0),
            capsule("project-local", 30, false, Some("/repo/prodex"), None, 0.0),
            capsule("recent", 15, false, None, Some(1_000), 0.0),
        ],
        RuntimeMemCapsuleSelectionContext {
            token_budget: 45,
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
    );

    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["required-too-large"]
    );
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["project-local", "recent"]
    );
    assert_eq!(selection.used_tokens, 45);
}

#[test]
fn selection_sorts_same_priority_by_relevance_recency_cost_then_id() {
    let selection = runtime_mem_select_capsules(
        [
            capsule("c", 20, false, None, Some(990), 0.8),
            capsule("b", 10, false, None, Some(990), 0.8),
            capsule("a", 10, false, None, Some(990), 0.8),
            capsule("newer", 30, false, None, Some(995), 0.8),
            capsule("best", 30, false, None, Some(980), 0.9),
        ],
        RuntimeMemCapsuleSelectionContext {
            token_budget: 100,
            project_root: None,
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
    );

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["best", "newer", "a", "b", "c"]
    );
}

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
fn slim_and_full_schema_outputs_stay_unchanged() {
    let slim = runtime_mem_default_codex_schema().to_string();
    assert!(slim.contains("0.4-slim"));
    assert!(slim.contains("\"prompt\":\"payload.message\""));
    assert!(slim.contains("output omitted"));
    assert!(!slim.contains("\"toolResponse\":\"payload.output\""));

    let full = runtime_mem_full_codex_schema().to_string();
    assert!(full.contains("Full schema"));
    assert!(full.contains("\"prompt\":\"payload.message\""));
    assert!(full.contains("\"toolResponse\":\"payload.output\""));
    assert!(full.contains("\"message\":\"payload.message\""));
}

#[test]
fn super_slim_schema_prefers_prompt_summary_or_refs_and_falls_back_to_prompt_body() {
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
    assert_eq!(super_slim_prompt, large_prompt);

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
    assert!(prompt_field.contains("payload.message"));
    assert!(
        prompt_field.find("metadata.prompt_summary") < prompt_field.find("payload.message"),
        "safe summary/ref candidates must be tried before full prompt fallback"
    );
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
        Some("0.6-super-slim")
    );
}

fn schema_user_prompt_field(schema: &Value) -> Option<&Value> {
    schema
        .get("events")?
        .as_array()?
        .iter()
        .find(|event| event.get("name").and_then(Value::as_str) == Some("user-message"))?
        .get("fields")?
        .get("prompt")
}

fn resolve_schema_user_prompt(schema: &Value, entry: &Value) -> Option<String> {
    let field = schema_user_prompt_field(schema)?;
    resolve_test_field(field, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_test_field(spec: &Value, entry: &Value) -> Option<Value> {
    if let Some(path) = spec.as_str() {
        return lookup_test_path(entry, path).cloned();
    }
    if let Some(value) = spec.get("value") {
        return Some(value.clone());
    }
    if let Some(path) = spec.get("path").and_then(Value::as_str) {
        return lookup_test_path(entry, path).cloned();
    }
    if let Some(coalesce) = spec.get("coalesce").and_then(Value::as_array) {
        for candidate in coalesce {
            if let Some(value) = resolve_test_field(candidate, entry)
                && !value.as_str().is_some_and(str::is_empty)
            {
                return Some(value);
            }
        }
    }
    None
}

fn lookup_test_path<'a>(entry: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = entry;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

use super::*;
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn recall_capsule(
    capsule: RuntimeMemCapsuleMetadata,
    paths: &[&str],
    symbols: &[&str],
) -> RuntimeMemRecallCapsuleMetadata {
    RuntimeMemRecallCapsuleMetadata {
        capsule,
        paths: paths.iter().map(PathBuf::from).collect(),
        symbols: symbols.iter().map(|symbol| symbol.to_string()).collect(),
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
fn auto_capsule_budget_tiers_support_default_and_super_modes() {
    assert_eq!(
        runtime_mem_capsule_budget_tier(1_999),
        RuntimeMemCapsuleBudgetTier::Minimal
    );
    assert_eq!(
        runtime_mem_capsule_budget_tier(2_000),
        RuntimeMemCapsuleBudgetTier::Condensed
    );
    assert_eq!(
        runtime_mem_capsule_budget_tier(8_000),
        RuntimeMemCapsuleBudgetTier::Large
    );
    assert_eq!(
        runtime_mem_capsule_budget_tier(16_000),
        RuntimeMemCapsuleBudgetTier::Exact
    );

    assert_eq!(
        runtime_mem_capsule_token_budget_for_tier(
            RuntimeMemCapsuleBudgetMode::Default,
            RuntimeMemCapsuleBudgetTier::Minimal,
        ),
        RUNTIME_MEM_DEFAULT_CAPSULE_MINIMAL_TOKEN_BUDGET
    );
    assert_eq!(
        runtime_mem_capsule_token_budget_for_tier(
            RuntimeMemCapsuleBudgetMode::Default,
            RuntimeMemCapsuleBudgetTier::Condensed,
        ),
        RUNTIME_MEM_DEFAULT_CAPSULE_CONDENSED_TOKEN_BUDGET
    );
    assert_eq!(
        runtime_mem_capsule_token_budget_for_tier(
            RuntimeMemCapsuleBudgetMode::Default,
            RuntimeMemCapsuleBudgetTier::Large,
        ),
        RUNTIME_MEM_DEFAULT_CAPSULE_LARGE_TOKEN_BUDGET
    );
    assert_eq!(
        runtime_mem_capsule_token_budget_for_tier(
            RuntimeMemCapsuleBudgetMode::Default,
            RuntimeMemCapsuleBudgetTier::Exact,
        ),
        RUNTIME_MEM_DEFAULT_CAPSULE_LARGE_TOKEN_BUDGET
    );
    assert_eq!(
        runtime_mem_capsule_token_budget_for_tier(
            RuntimeMemCapsuleBudgetMode::Super,
            RuntimeMemCapsuleBudgetTier::Minimal,
        ),
        RUNTIME_MEM_SUPER_CAPSULE_MINIMAL_TOKEN_BUDGET
    );
    assert_eq!(
        runtime_mem_capsule_token_budget_for_tier(
            RuntimeMemCapsuleBudgetMode::Super,
            RuntimeMemCapsuleBudgetTier::Condensed,
        ),
        RUNTIME_MEM_SUPER_CAPSULE_CONDENSED_TOKEN_BUDGET
    );
    assert_eq!(
        runtime_mem_capsule_token_budget_for_tier(
            RuntimeMemCapsuleBudgetMode::Super,
            RuntimeMemCapsuleBudgetTier::Large,
        ),
        RUNTIME_MEM_SUPER_CAPSULE_LARGE_TOKEN_BUDGET
    );
    assert_eq!(
        runtime_mem_capsule_token_budget(RuntimeMemCapsuleBudget::Explicit(777)),
        777
    );
}

#[test]
fn auto_capsule_selection_super_budget_admits_more_than_default_budget() {
    let capsules = || {
        [
            capsule("optional", 300, false, None, Some(100), 1.0),
            capsule("recent", 400, false, None, Some(995), 0.1),
            capsule(
                "project",
                300,
                false,
                Some("/repo/prodex/src"),
                Some(100),
                0.1,
            ),
            capsule("required", 200, true, None, None, 0.0),
        ]
    };

    let default_selection = runtime_mem_select_capsules_auto(
        capsules(),
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Tier {
                available_tokens: 6_000,
                mode: RuntimeMemCapsuleBudgetMode::Default,
            },
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
    );
    let super_selection = runtime_mem_select_capsules_auto(
        capsules(),
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Tier {
                available_tokens: 6_000,
                mode: RuntimeMemCapsuleBudgetMode::Super,
            },
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
    );

    assert_eq!(
        default_selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["required", "project"]
    );
    assert_eq!(
        default_selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["recent", "optional"]
    );
    assert_eq!(
        default_selection.token_budget,
        RUNTIME_MEM_DEFAULT_CAPSULE_CONDENSED_TOKEN_BUDGET
    );
    assert_eq!(default_selection.used_tokens, 500);

    assert_eq!(
        super_selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["required", "project", "recent"]
    );
    assert_eq!(
        super_selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional"]
    );
    assert_eq!(
        super_selection.token_budget,
        RUNTIME_MEM_SUPER_CAPSULE_CONDENSED_TOKEN_BUDGET
    );
    assert_eq!(super_selection.used_tokens, 900);
}

#[test]
fn auto_capsule_selection_prioritizes_project_local_and_recent_over_optional_relevance() {
    let selection = runtime_mem_select_capsules_auto(
        [
            capsule("optional-high", 20, false, None, Some(100), 1.0),
            capsule("recent-low", 20, false, Some("/other"), Some(995), 0.1),
            capsule(
                "project-low",
                20,
                false,
                Some("/repo/prodex/crates/runtime-mem"),
                Some(100),
                0.1,
            ),
            capsule("required", 20, true, None, None, 0.0),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(60),
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
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional-high"]
    );
}

#[test]
fn recall_diet_prioritizes_path_intent_matches_before_unrelated_optional() {
    let selection = runtime_mem_select_capsules_for_recall_diet(
        [
            recall_capsule(
                capsule("optional-unrelated-high", 20, false, None, Some(1_000), 1.0),
                &[],
                &[],
            ),
            recall_capsule(
                capsule("path-match", 30, false, None, Some(900), 0.1),
                &["crates/prodex-runtime-mem/src/lib.rs"],
                &[],
            ),
            recall_capsule(
                capsule(
                    "project-path-match",
                    30,
                    false,
                    Some("/repo/prodex/crates"),
                    Some(800),
                    0.1,
                ),
                &[],
                &[],
            ),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(60),
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
        RuntimeMemRecallIntent {
            paths: vec![PathBuf::from("crates/prodex-runtime-mem/src/lib.rs")],
            symbols: Vec::new(),
        },
    );

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["project-path-match", "path-match"]
    );
    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional-unrelated-high"]
    );
}

#[test]
fn recall_diet_prioritizes_symbol_intent_matches_before_unrelated_optional() {
    let selection = runtime_mem_select_capsules_for_recall_diet(
        [
            recall_capsule(
                capsule("optional-unrelated-high", 20, false, None, Some(1_000), 1.0),
                &[],
                &["other::Thing"],
            ),
            recall_capsule(
                capsule("symbol-match", 30, false, None, Some(900), 0.1),
                &[],
                &["prodex_runtime_mem::RuntimeMemCapsuleMetadata"],
            ),
            recall_capsule(
                capsule("recent-symbol-match", 30, false, None, Some(995), 0.1),
                &[],
                &["RuntimeMemRecallIntent"],
            ),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(60),
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
        RuntimeMemRecallIntent {
            paths: Vec::new(),
            symbols: vec![
                "RuntimeMemCapsuleMetadata".to_string(),
                "prodex_runtime_mem::RuntimeMemRecallIntent".to_string(),
            ],
        },
    );

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["recent-symbol-match", "symbol-match"]
    );
    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional-unrelated-high"]
    );
}

#[test]
fn recall_diet_oversized_required_does_not_block_smaller_useful_capsules() {
    let selection = runtime_mem_select_capsules_for_recall_diet(
        [
            recall_capsule(
                capsule("required-too-large", 90, true, None, None, 0.0),
                &[],
                &[],
            ),
            recall_capsule(
                capsule(
                    "path-match",
                    25,
                    false,
                    Some("/repo/prodex/crates/prodex-runtime-mem"),
                    Some(900),
                    0.1,
                ),
                &["crates/prodex-runtime-mem/src/lib.rs"],
                &[],
            ),
            recall_capsule(
                capsule("symbol-match", 20, false, None, Some(900), 0.1),
                &[],
                &["RuntimeMemRecallIntent"],
            ),
            recall_capsule(
                capsule("optional-unrelated", 20, false, None, Some(1_000), 1.0),
                &[],
                &[],
            ),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(45),
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
        RuntimeMemRecallIntent {
            paths: vec![PathBuf::from("crates/prodex-runtime-mem/src/lib.rs")],
            symbols: vec!["prodex_runtime_mem::RuntimeMemRecallIntent".to_string()],
        },
    );

    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["required-too-large", "optional-unrelated"]
    );
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["path-match", "symbol-match"]
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
fn recall_content_dedupe_keeps_required_copy_and_replaces_optional_exact_duplicates() {
    let content = "critical memory line\n".repeat(30);
    let entries = runtime_mem_dedupe_recall_content([
        RuntimeMemRecallDedupeItem {
            id: "required-main".to_string(),
            content: content.clone(),
            required: true,
            artifact_ref: None,
        },
        RuntimeMemRecallDedupeItem {
            id: "optional-dup".to_string(),
            content: content.clone(),
            required: false,
            artifact_ref: None,
        },
    ]);

    assert_eq!(entries[0].replacement, None);
    assert_eq!(entries[0].content, content);
    assert_eq!(
        entries[1].reason,
        Some(RuntimeMemRecallDedupeReason::Duplicate {
            original_id: "required-main".to_string()
        })
    );
    let replacement = entries[1]
        .replacement
        .as_deref()
        .expect("optional duplicate should be replaced");
    assert!(replacement.contains("mem dup: original=required-main"));
    assert!(replacement.contains("h=sc:"));
    assert!(replacement.contains("b="));
    assert!(
        !replacement.contains("critical memory line"),
        "duplicate replacement must not semantic-summarize content"
    );
}

#[test]
fn recall_content_dedupe_replaces_optional_prodex_artifact_content_with_ref() {
    let content = "large artifact-backed memory\n".repeat(40);
    let entries = runtime_mem_dedupe_recall_content([
        RuntimeMemRecallDedupeItem {
            id: "artifact-backed".to_string(),
            content: content.clone(),
            required: false,
            artifact_ref: Some("prodex-artifact:sc:abc123".to_string()),
        },
        RuntimeMemRecallDedupeItem {
            id: "required-artifact-backed".to_string(),
            content,
            required: true,
            artifact_ref: Some("prodex-artifact:sc:def456".to_string()),
        },
    ]);

    assert_eq!(
        entries[0].reason,
        Some(RuntimeMemRecallDedupeReason::ArtifactRef {
            artifact_ref: "prodex-artifact:sc:abc123".to_string()
        })
    );
    assert_eq!(entries[1].replacement, None);
    let replacement = entries[0]
        .replacement
        .as_deref()
        .expect("artifact-backed optional content should be replaced");
    assert!(replacement.starts_with("prodex-artifact:sc:abc123"));
    assert!(replacement.contains("[mem art;"));
    assert!(replacement.contains("h=sc:"));
    assert!(replacement.contains("b="));
    assert!(
        !replacement.contains("large artifact-backed memory"),
        "artifact replacement must stay reference-only"
    );
}

#[test]
fn prodex_artifact_ref_helpers_accept_short_and_alias_refs() {
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref("psc:0123456789abcdef"),
        Some("psc:0123456789abcdef".to_string())
    );
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref("p:0123456789abcdef"),
        Some("p:0123456789abcdef".to_string())
    );
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref("prodex-artifact:sc:legacy"),
        Some("prodex-artifact:sc:legacy".to_string())
    );
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref(
            "need @0 first\npsc aliases @0=psc:fedcba9876543210"
        ),
        Some("psc:fedcba9876543210".to_string())
    );

    let aliases_across_values =
        serde_json::json!(["need @0", "psc aliases @0=psc:0123456789abcdef"]);
    assert_eq!(
        runtime_mem_extract_artifact_marker(&aliases_across_values),
        Some("psc:0123456789abcdef".to_string())
    );
    assert!(runtime_mem_value_contains_artifact_marker(
        &serde_json::json!({
            "payload": {
                "message": "refer to psc:0123456789abcdef"
            }
        })
    ));
    assert!(runtime_mem_value_contains_artifact_marker(
        &serde_json::json!({
            "payload": {
                "message": "refer to p:0123456789abcdef"
            }
        })
    ));
    assert!(runtime_mem_value_contains_artifact_marker(
        &aliases_across_values
    ));
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

#[test]
fn super_slim_v2_shadow_events_are_short_and_schema_addressable() {
    let user_prompt = "Implement concise memory bridge\n".to_string() + &"detail ".repeat(120);
    let tool_output = "cargo test passed\n".to_string() + &"ok ".repeat(120);
    let events = vec![
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
fn super_slim_shadow_user_prompt_stores_summary_counts_and_ref_not_full_prompt() {
    let prompt = "\n\nImplement shadow transcript helpers\n".to_string() + &"detail ".repeat(400);
    let event = serde_json::json!({
        "payload": {
            "type": "user_message",
            "message": prompt,
            "metadata": {
                "artifact_ref": "prodex-artifact:prompt-123"
            }
        }
    });

    let shadow = runtime_mem_super_slim_shadow_codex_event(&event);
    let summary = lookup_test_path(&shadow, "payload.prompt_summary")
        .and_then(Value::as_str)
        .expect("shadow prompt summary should exist");
    let shadow_message = lookup_test_path(&shadow, "payload.message")
        .and_then(Value::as_str)
        .expect("shadow prompt body should exist");

    assert!(summary.starts_with("u: Implement shadow transcript helpers"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("ref=prodex-artifact:prompt-123"));
    assert!(summary.contains("omit=prompt"));
    assert!(!summary.contains("tok~="));
    assert!(!summary.contains("prompt omitted"));
    assert_eq!(shadow_message, "ss:omit");
    assert_ne!(
        lookup_test_path(&event, "payload.message").and_then(Value::as_str),
        Some(shadow_message)
    );
    assert_eq!(
        resolve_schema_user_prompt(&runtime_mem_super_slim_codex_schema(), &shadow).as_deref(),
        Some(summary)
    );
}

#[test]
fn super_slim_shadow_assistant_message_uses_short_summary() {
    let message =
        "Completed helper implementation\n".to_string() + &"verbose explanation ".repeat(300);
    let shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "agent_message",
            "message": message
        }
    }));
    let summary = lookup_test_path(&shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("assistant summary should exist");

    assert!(summary.starts_with("a: Completed helper implementation"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("omit=message"));
    assert_eq!(
        lookup_test_path(&shadow, "payload.message").and_then(Value::as_str),
        Some("ss:omit")
    );
    assert_eq!(
        resolve_schema_assistant_message(&runtime_mem_super_slim_codex_schema(), &shadow)
            .as_deref(),
        Some(summary)
    );
}

#[test]
fn super_slim_shadow_referenced_artifact_uses_shorter_prefix_than_plain_summary() {
    let tail = "TAIL_AFTER_SHORT_REF_CAP";
    let line = format!(
        "{}{tail}",
        "x".repeat(RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT + 8)
    );
    assert!(line.chars().count() < RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT);

    let plain_shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call_output",
            "call_id": "plain-call",
            "output": line
        }
    }));
    let artifact_shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call_output",
            "call_id": "artifact-call",
            "output": line,
            "metadata": {
                "artifact_ref": "prodex-artifact:sc:short-prefix"
            }
        }
    }));

    let plain_summary = lookup_test_path(&plain_shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("plain summary should exist");
    let artifact_summary = lookup_test_path(&artifact_shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("artifact summary should exist");

    assert!(plain_summary.contains(tail));
    assert!(!artifact_summary.contains(tail));
    assert!(artifact_summary.contains("ref=prodex-artifact:sc:short-prefix"));
    assert!(artifact_summary.contains("omit=output"));
}

#[test]
fn super_slim_shadow_tool_output_stores_summary_and_ref() {
    let output = "\nfirst useful output line\n".to_string() + &"artifact data ".repeat(500);
    let shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call_output",
            "call_id": "call-1",
            "output": output,
            "artifact": {
                "ref": "prodex://artifact/tool-456"
            }
        }
    }));
    let summary = lookup_test_path(&shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("tool output summary should exist");

    assert!(summary.starts_with("tool: first useful output line"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("omit=output"));
    assert!(summary.contains("ref=prodex://artifact/tool-456"));
    assert_eq!(
        lookup_test_path(&shadow, "payload.metadata.artifact_ref").and_then(Value::as_str),
        Some("prodex://artifact/tool-456")
    );
    assert_eq!(
        lookup_test_path(&shadow, "payload.output").and_then(Value::as_str),
        Some("ss:omit")
    );
    assert_eq!(
        resolve_schema_tool_response(&runtime_mem_super_slim_codex_schema(), &shadow).as_deref(),
        Some(summary)
    );
}

#[test]
fn super_slim_shadow_falls_back_to_local_summary_when_no_summary_or_ref_exists() {
    let shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "custom_tool_call_output",
            "call_id": "call-2",
            "output": "\n\nplain output only\nsecond line has more detail"
        }
    }));
    let summary = lookup_test_path(&shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("fallback summary should exist");

    assert!(summary.starts_with("tool: plain output only"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("omit=output"));
    assert!(!summary.contains("ref="));
    assert_eq!(
        resolve_schema_tool_response(&runtime_mem_super_slim_codex_schema(), &shadow).as_deref(),
        Some(summary)
    );
}

#[test]
fn super_slim_shadow_events_replaces_later_exact_duplicate_without_semantic_summary() {
    let message = "Important but repeated answer\n".to_string() + &"detail ".repeat(300);
    let events = [
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": message
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": message
            }
        }),
    ];

    let single_shadow = runtime_mem_super_slim_shadow_codex_event(&events[0]);
    let shadows = runtime_mem_super_slim_shadow_codex_events(events.iter());
    let first_summary = lookup_test_path(&shadows[0], "payload.summary")
        .and_then(Value::as_str)
        .expect("first summary should exist");
    let duplicate_summary = lookup_test_path(&shadows[1], "payload.summary")
        .and_then(Value::as_str)
        .expect("duplicate summary should exist");

    assert_eq!(shadows[0], single_shadow);
    assert!(first_summary.starts_with("a: Important but repeated answer"));
    assert!(duplicate_summary.starts_with("mem dup: original=event[0]"));
    assert!(duplicate_summary.contains("h=sc:"));
    assert!(!duplicate_summary.contains("Important but repeated answer"));
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.message").and_then(Value::as_str),
        Some("ss:omit")
    );
}

#[test]
fn super_slim_shadow_events_use_artifact_ref_for_later_exact_duplicate() {
    let output = "large artifact-backed output\n".to_string() + &"payload ".repeat(300);
    let events = [
        serde_json::json!({
            "payload": {
                "type": "function_call_output",
                "call_id": "call-original",
                "output": output,
                "metadata": {
                    "artifact_ref": "prodex-artifact:sc:tool-output"
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "function_call_output",
                "call_id": "call-dup",
                "output": output
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_shadow_codex_events(events.iter());
    let duplicate_summary = lookup_test_path(&shadows[1], "payload.summary")
        .and_then(Value::as_str)
        .expect("artifact duplicate summary should exist");

    assert!(duplicate_summary.starts_with("prodex-artifact:sc:tool-output"));
    assert!(duplicate_summary.contains("[mem art;"));
    assert!(duplicate_summary.contains("h=sc:"));
    assert!(!duplicate_summary.contains("large artifact-backed output"));
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.metadata.artifact_ref").and_then(Value::as_str),
        Some("prodex-artifact:sc:tool-output")
    );
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.metadata.summary").and_then(Value::as_str),
        Some(duplicate_summary)
    );
}

#[test]
fn super_slim_shadow_events_replaces_later_exact_duplicate_assistant_summary() {
    let summary = "Repeated assistant summary from upstream; same exact text.";
    let events = [
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "id": "assistant-summary-1",
                "message": "first full assistant message",
                "summary": summary
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "id": "assistant-summary-2",
                "message": "different full assistant message",
                "summary": summary
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_shadow_codex_events(events.iter());
    let first_summary = lookup_test_path(&shadows[0], "payload.summary")
        .and_then(Value::as_str)
        .expect("first assistant summary should exist");
    let duplicate_summary = lookup_test_path(&shadows[1], "payload.summary")
        .and_then(Value::as_str)
        .expect("duplicate assistant summary should exist");

    assert_eq!(first_summary, summary);
    assert!(duplicate_summary.starts_with("mem dup: original=assistant-summary-1"));
    assert!(duplicate_summary.contains("h=sc:"));
    assert!(duplicate_summary.contains("b="));
    assert!(!duplicate_summary.contains(summary));
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.message").and_then(Value::as_str),
        Some("ss:omit")
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

fn schema_assistant_message_field(schema: &Value) -> Option<&Value> {
    schema
        .get("events")?
        .as_array()?
        .iter()
        .find(|event| event.get("name").and_then(Value::as_str) == Some("assistant-message"))?
        .get("fields")?
        .get("message")
}

fn schema_tool_response_field(schema: &Value) -> Option<&Value> {
    schema
        .get("events")?
        .as_array()?
        .iter()
        .find(|event| event.get("name").and_then(Value::as_str) == Some("tool-result"))?
        .get("fields")?
        .get("toolResponse")
}

fn resolve_schema_user_prompt(schema: &Value, entry: &Value) -> Option<String> {
    let field = schema_user_prompt_field(schema)?;
    resolve_test_field(field, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_schema_assistant_message(schema: &Value, entry: &Value) -> Option<String> {
    let field = schema_assistant_message_field(schema)?;
    resolve_test_field(field, entry).and_then(|value| match value {
        Value::String(value) => Some(value),
        _ => None,
    })
}

fn resolve_schema_tool_response(schema: &Value, entry: &Value) -> Option<String> {
    let field = schema_tool_response_field(schema)?;
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

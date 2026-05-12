use super::*;

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
            prompt: None,
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
            prompt: None,
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
            prompt: None,
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
fn recall_diet_uses_prompt_intent_and_skips_unmatched_optional_fill() {
    let selection = runtime_mem_select_capsules_for_recall_diet(
        [
            recall_capsule(capsule("required", 10, true, None, None, 0.0), &[], &[]),
            recall_capsule(
                capsule(
                    "project-local",
                    10,
                    false,
                    Some("/repo/prodex/README.md"),
                    None,
                    0.0,
                ),
                &[],
                &[],
            ),
            recall_capsule(capsule("recent", 10, false, None, Some(995), 0.0), &[], &[]),
            recall_capsule(
                capsule("prompt-path-match", 20, false, None, Some(700), 0.1),
                &["crates/prodex-runtime-mem/src/lib.rs"],
                &[],
            ),
            recall_capsule(
                capsule("prompt-symbol-match", 20, false, None, Some(700), 0.1),
                &[],
                &["prodex_runtime_mem::RuntimeMemRecallIntent"],
            ),
            recall_capsule(
                capsule("optional-unmatched-high", 20, false, None, Some(700), 1.0),
                &[],
                &["OtherThing"],
            ),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(100),
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
        RuntimeMemRecallIntent::from_prompt(
            "Update crates/prodex-runtime-mem/src/lib.rs around RuntimeMemRecallIntent.",
        ),
    );

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "required",
            "project-local",
            "prompt-path-match",
            "prompt-symbol-match",
            "recent"
        ]
    );
    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional-unmatched-high"]
    );
    assert_eq!(selection.used_tokens, 70);
    assert_eq!(selection.token_budget, 100);
}

#[test]
fn recall_diet_preserves_project_local_before_optional_prompt_match_when_budget_tight() {
    let selection = runtime_mem_select_capsules_for_recall_diet(
        [
            recall_capsule(capsule("required", 10, true, None, None, 0.0), &[], &[]),
            recall_capsule(
                capsule(
                    "project-local",
                    20,
                    false,
                    Some("/repo/prodex/AGENTS.md"),
                    Some(700),
                    0.0,
                ),
                &[],
                &[],
            ),
            recall_capsule(
                capsule("optional-prompt-match", 20, false, None, Some(995), 1.0),
                &["src/hot.rs"],
                &[],
            ),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(30),
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
        RuntimeMemRecallIntent::from_prompt("Fix src/hot.rs"),
    );

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["required", "project-local"]
    );
    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional-prompt-match"]
    );
}

#[test]
fn recall_diet_empty_intent_uses_exact_base_selection_fallback() {
    let context = RuntimeMemCapsuleSelectionContext {
        token_budget: 40,
        project_root: Some(PathBuf::from("/repo/prodex")),
        now_seconds: Some(1_000),
        recent_window_seconds: 60,
    };
    let capsules = vec![
        recall_capsule(
            capsule("dup", 10, false, None, Some(995), 0.5),
            &["src/hot.rs"],
            &["HotThing"],
        ),
        recall_capsule(
            capsule("dup", 10, false, None, Some(990), 0.4),
            &["src/other.rs"],
            &["OtherThing"],
        ),
        recall_capsule(
            capsule(
                "project",
                30,
                false,
                Some("/repo/prodex/src/lib.rs"),
                Some(500),
                0.0,
            ),
            &[],
            &[],
        ),
    ];

    let recall_selection = runtime_mem_select_capsules_with_recall_intent(
        capsules.clone(),
        context.clone(),
        RuntimeMemRecallIntent {
            prompt: Some("   ".to_string()),
            paths: Vec::new(),
            symbols: Vec::new(),
        },
    );
    let base_selection = runtime_mem_select_capsules(
        capsules.into_iter().map(|candidate| candidate.capsule),
        context,
    );

    assert_eq!(recall_selection, base_selection);
}

#[test]
fn recall_diet_dedupes_duplicate_capsule_ids_by_prompt_intent_and_recency() {
    let selection = runtime_mem_select_capsules_for_recall_diet(
        [
            recall_capsule(
                capsule("dup", 20, false, None, Some(700), 1.0),
                &["src/old.rs"],
                &[],
            ),
            recall_capsule(
                capsule("dup", 20, false, None, Some(995), 0.1),
                &["src/hot.rs"],
                &[],
            ),
            recall_capsule(
                capsule("unrelated", 20, false, None, Some(900), 0.9),
                &[],
                &[],
            ),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(40),
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
        RuntimeMemRecallIntent::from_prompt("Fix src/hot.rs"),
    );

    assert_eq!(selection.selected.len(), 1);
    assert_eq!(selection.selected[0].id, "dup");
    assert_eq!(
        selection.selected[0].priority,
        RuntimeMemCapsulePriority::Recent
    );
    assert!(
        selection
            .omitted
            .iter()
            .all(|entry| entry.id.as_str() != "dup")
    );
}

#[test]
fn recall_diet_extracts_prompt_paths_with_line_locators_and_symbols_with_calls() {
    let selection = runtime_mem_select_capsules_for_recall_diet(
        [
            recall_capsule(
                capsule("path-line-match", 20, false, None, Some(700), 0.1),
                &["crates/prodex-runtime-mem/src/lib.rs"],
                &[],
            ),
            recall_capsule(
                capsule("symbol-call-match", 20, false, None, Some(700), 0.1),
                &[],
                &["prodex_runtime_mem::RuntimeMemRecallIntent::from_prompt"],
            ),
            recall_capsule(
                capsule("optional-unmatched", 20, false, None, Some(995), 1.0),
                &[],
                &[],
            ),
        ],
        RuntimeMemAutoCapsuleSelectionContext {
            budget: RuntimeMemCapsuleBudget::Explicit(40),
            project_root: Some(PathBuf::from("/repo/prodex")),
            now_seconds: Some(1_000),
            recent_window_seconds: 60,
        },
        RuntimeMemRecallIntent::from_prompt(
            "Fix crates/prodex-runtime-mem/src/lib.rs:3172 via RuntimeMemRecallIntent::from_prompt().",
        ),
    );

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["path-line-match", "symbol-call-match"]
    );
    assert_eq!(
        selection
            .omitted
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["optional-unmatched"]
    );
}

use super::*;

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

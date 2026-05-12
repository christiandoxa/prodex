use super::*;

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

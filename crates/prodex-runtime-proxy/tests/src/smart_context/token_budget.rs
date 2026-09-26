use super::*;

fn accounting(
    available_context_tokens: Option<u64>,
    accounting_safe: bool,
) -> SmartContextObservedTokenAccounting {
    SmartContextObservedTokenAccounting {
        model_context_window_tokens: None,
        observed_turns: 0,
        observed_input_tokens: 0,
        observed_cached_input_tokens: 0,
        observed_uncached_input_tokens: 0,
        observed_output_tokens: 0,
        observed_reasoning_tokens: 0,
        observed_total_tokens: 0,
        observed_context_tokens: 0,
        last_input_tokens: 0,
        last_accounted_input_tokens: 0,
        last_observed_context_tokens: 0,
        current_request_body_bytes: 0,
        estimated_current_request_tokens: 0,
        current_request_accounted_tokens: 0,
        effective_input_tokens: 0,
        effective_input_source: SmartContextTokenAccountingSource::Unknown,
        reserved_output_tokens: 0,
        available_context_tokens,
        accounting_risks: if accounting_safe {
            Vec::new()
        } else {
            vec![SmartContextTokenAccountingRisk::UnknownCurrentRequestAccounting]
        },
        pressure: SmartContextPressureSnapshot {
            model_context_window_tokens: None,
            reserved_output_tokens: 0,
            effective_usable_context_tokens: None,
            effective_used_tokens: 0,
            pressure_basis_points: None,
            pressure_band: SmartContextPressureBand::Unknown,
            absolute_safety_floor_tokens: 0,
            available_context_tokens,
            estimator_confidence: SmartContextEstimatorConfidence::Low,
        },
    }
}

fn policy(
    mode: SmartContextBudgetMode,
    tier: SmartContextTokenBudgetTier,
    reasons: Vec<SmartContextBudgetPolicyReason>,
    max_rehydrate_tokens: u64,
) -> SmartContextAdaptiveBudgetPolicy {
    SmartContextAdaptiveBudgetPolicy {
        tier,
        mode,
        max_inline_bytes: 0,
        max_inline_tool_output_bytes: 0,
        max_rehydrate_tokens,
        reasons,
    }
}

#[test]
fn budget_tier_uses_mojo_thresholds() {
    for (tokens, expected) in [
        (0, SmartContextTokenBudgetTier::Minimal),
        (1_999, SmartContextTokenBudgetTier::Minimal),
        (2_000, SmartContextTokenBudgetTier::Condensed),
        (7_999, SmartContextTokenBudgetTier::Condensed),
        (8_000, SmartContextTokenBudgetTier::Large),
        (15_999, SmartContextTokenBudgetTier::Large),
        (16_000, SmartContextTokenBudgetTier::Exact),
        (u64::MAX, SmartContextTokenBudgetTier::Exact),
    ] {
        assert_eq!(smart_context_u64_budget_tier(tokens), expected);
        if let Ok(tokens) = usize::try_from(tokens) {
            assert_eq!(smart_context_token_budget_tier(tokens), expected);
        }
    }
    assert_eq!(
        smart_context_token_budget_tier(usize::MAX),
        SmartContextTokenBudgetTier::Exact
    );
}

#[test]
fn memory_capsule_budget_keeps_expected_caps_and_safety_gates() {
    let safe_accounting = accounting(Some(10_000), true);
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &safe_accounting,
            &policy(
                SmartContextBudgetMode::ExactPassThrough,
                SmartContextTokenBudgetTier::Exact,
                vec![SmartContextBudgetPolicyReason::PlentyOfBudget],
                0,
            ),
        ),
        usize::MAX
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &safe_accounting,
            &policy(
                SmartContextBudgetMode::ExactPassThrough,
                SmartContextTokenBudgetTier::Condensed,
                vec![SmartContextBudgetPolicyReason::ModerateBudget],
                10_000,
            ),
        ),
        1_024
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &safe_accounting,
            &policy(
                SmartContextBudgetMode::LargeLossless,
                SmartContextTokenBudgetTier::Minimal,
                vec![SmartContextBudgetPolicyReason::TightBudget],
                2_048,
            ),
        ),
        2_048
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &safe_accounting,
            &policy(
                SmartContextBudgetMode::ArtifactCondensed,
                SmartContextTokenBudgetTier::Exact,
                vec![SmartContextBudgetPolicyReason::ModerateBudget],
                u64::MAX,
            ),
        ),
        1_024
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &safe_accounting,
            &policy(
                SmartContextBudgetMode::MinimalRefsOnly,
                SmartContextTokenBudgetTier::Minimal,
                vec![SmartContextBudgetPolicyReason::CriticalBudget],
                u64::MAX,
            ),
        ),
        256
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &safe_accounting,
            &policy(
                SmartContextBudgetMode::LargeLossless,
                SmartContextTokenBudgetTier::Large,
                vec![SmartContextBudgetPolicyReason::ExactnessRequired],
                u64::MAX,
            ),
        ),
        0
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &accounting(None, true),
            &policy(
                SmartContextBudgetMode::LargeLossless,
                SmartContextTokenBudgetTier::Large,
                vec![SmartContextBudgetPolicyReason::ModerateBudget],
                u64::MAX,
            ),
        ),
        0
    );
    assert_eq!(
        smart_context_memory_capsule_token_budget_impl(
            &accounting(Some(10_000), false),
            &policy(
                SmartContextBudgetMode::LargeLossless,
                SmartContextTokenBudgetTier::Large,
                vec![SmartContextBudgetPolicyReason::ModerateBudget],
                u64::MAX,
            ),
        ),
        0
    );
}

#[test]
fn capsule_selection_preserves_unicode_order_and_avoids_cost_overflow() {
    let selected = smart_context_select_memory_capsules_impl(
        [
            SmartContextMemoryCapsule {
                id: "optional-猫".to_string(),
                token_cost: 1,
                relevance: 0.8,
                required: false,
            },
            SmartContextMemoryCapsule {
                id: "required-b".to_string(),
                token_cost: 2,
                relevance: 0.0,
                required: true,
            },
            SmartContextMemoryCapsule {
                id: "optional-é".to_string(),
                token_cost: 1,
                relevance: 0.8,
                required: false,
            },
            SmartContextMemoryCapsule {
                id: "required-a".to_string(),
                token_cost: usize::MAX - 1,
                relevance: 0.0,
                required: true,
            },
        ],
        usize::MAX,
    );
    assert_eq!(
        selected,
        SmartContextMemoryCapsuleSelection {
            selected_ids: vec!["required-a".to_string(), "optional-é".to_string()],
            omitted_ids: vec!["required-b".to_string(), "optional-猫".to_string()],
            used_tokens: usize::MAX,
        }
    );
    for (token_budget, expected) in [
        (0, Vec::<&str>::new()),
        (1, vec!["optional-é"]),
        (2, vec!["optional-é", "optional-猫"]),
    ] {
        assert_eq!(
            smart_context_select_memory_capsules_impl(
                [
                    SmartContextMemoryCapsule {
                        id: "optional-猫".to_string(),
                        token_cost: 1,
                        relevance: 0.8,
                        required: false,
                    },
                    SmartContextMemoryCapsule {
                        id: "optional-é".to_string(),
                        token_cost: 1,
                        relevance: 0.8,
                        required: false,
                    },
                ],
                token_budget,
            )
            .selected_ids,
            expected
        );
    }
}

#[test]
fn mojo_budget_and_capsule_abis_keep_input_validation() {
    for (mode, tier) in [(-1, 0), (4, 0), (0, -1), (0, 4)] {
        assert_eq!(
            prodex_mojo_core::rich::smart_context_memory_capsule_token_budget(
                None, mode, tier, 0, 0, true
            ),
            Err(prodex_mojo_core::MojoError::InvalidInput)
        );
    }

    let inputs = vec![
        prodex_mojo_core::rich::SmartContextCapsuleInput {
            token_cost: 0,
            required: false,
        };
        65_536
    ];
    assert_eq!(
        prodex_mojo_core::rich::plan_smart_context_capsules(&inputs, 0)
            .unwrap()
            .selected
            .len(),
        65_536
    );
    let mut too_many = inputs;
    too_many.push(prodex_mojo_core::rich::SmartContextCapsuleInput {
        token_cost: 0,
        required: false,
    });
    assert_eq!(
        prodex_mojo_core::rich::plan_smart_context_capsules(&too_many, 0),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}

#[test]
fn capsule_selection_batches_more_than_65536_items_without_losing_budget() {
    let capsules = (0..65_537)
        .map(|index| SmartContextMemoryCapsule {
            id: format!("{index:05}"),
            token_cost: 1,
            relevance: 0.0,
            required: false,
        })
        .collect::<Vec<_>>();
    let result = smart_context_select_memory_capsules_impl(capsules, 65_537);
    assert_eq!(result.selected_ids.len(), 65_537);
    assert_eq!(
        result.selected_ids.last().map(String::as_str),
        Some("65536")
    );
    assert!(result.omitted_ids.is_empty());
    assert_eq!(result.used_tokens, 65_537);
}

#[test]
fn mojo_u64_budget_conversion_saturates_at_usize_max() {
    assert_eq!(smart_context_u64_saturating_usize(0), 0);
    assert_eq!(smart_context_u64_saturating_usize(u64::MAX), usize::MAX);
}

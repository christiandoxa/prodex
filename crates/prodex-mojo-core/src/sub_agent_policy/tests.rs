use super::*;

#[test]
fn sub_agent_policy_kernel_matches_public_cli_contract() {
    assert_eq!(
        parse_concurrency(" default ").unwrap().unwrap(),
        ParsedConcurrency {
            value: 4,
            source: ConcurrencySourcePlan::Default,
        }
    );
    assert_eq!(
        parse_concurrency("8").unwrap().unwrap(),
        ParsedConcurrency {
            value: 8,
            source: ConcurrencySourcePlan::Preset,
        }
    );
    assert_eq!(
        parse_concurrency("23").unwrap().unwrap(),
        ParsedConcurrency {
            value: 23,
            source: ConcurrencySourcePlan::Custom,
        }
    );
    assert_eq!(
        parse_concurrency("1e2").unwrap(),
        Err(ConcurrencyParseViolation::Syntax)
    );
    assert_eq!(
        parse_concurrency("999999999999999999999").unwrap(),
        Err(ConcurrencyParseViolation::Overflow)
    );
    assert_eq!(
        parse_concurrency("65").unwrap(),
        Err(ConcurrencyParseViolation::OutOfRange)
    );
    assert!(concurrency_valid(64).unwrap());
    assert!(!concurrency_valid(0).unwrap());

    assert_eq!(
        reasoning_effort(" XHIGH ").unwrap(),
        Some(ReasoningEffortPlan::XHigh)
    );
    assert_eq!(reasoning_effort("extreme").unwrap(), None);
    assert!(model_nonempty(" 模型/β-🦀 ").unwrap());
    assert!(!model_nonempty(" \t\u{3000} ").unwrap());
    assert_eq!(provider_url_violation(true, true).unwrap(), None);
    assert_eq!(
        provider_url_violation(true, false).unwrap(),
        Some(ProviderUrlViolation::LocalRequiresUrl)
    );
    assert_eq!(
        provider_url_violation(false, true).unwrap(),
        Some(ProviderUrlViolation::NonLocalRejectsUrl)
    );
    assert_eq!(
        child_spec_scalar_violation("PRODEX_SUB_AGENT", 65_536).unwrap(),
        None
    );
    assert_eq!(
        child_spec_scalar_violation("bad", 65_536).unwrap(),
        Some(ChildSpecScalarViolation::InvalidRecursionMarker)
    );
    assert_eq!(
        child_spec_scalar_violation("PRODEX_SUB_AGENT", 0).unwrap(),
        Some(ChildSpecScalarViolation::InvalidTaskSize)
    );
    assert_eq!(
        prompt_step_plan(false, true, false, false, false).unwrap(),
        SubAgentPromptStepPlan {
            provider: true,
            local_url: true,
            model: true,
            reasoning_effort: true,
            max_concurrency: true,
        }
    );
    assert_eq!(
        prompt_step_plan(true, false, false, true, true).unwrap(),
        SubAgentPromptStepPlan {
            provider: false,
            local_url: false,
            model: false,
            reasoning_effort: false,
            max_concurrency: true,
        }
    );
    assert_eq!(
        child_argv_plan(false, false, true, 2, true, true).unwrap(),
        vec![
            ChildArgvAction::Super,
            ChildArgvAction::NoSubAgent,
            ChildArgvAction::Presidio,
            ChildArgvAction::RequireTool,
            ChildArgvAction::RequireTool,
            ChildArgvAction::NamedProvider,
            ChildArgvAction::Model,
            ChildArgvAction::Effort,
            ChildArgvAction::Exec,
            ChildArgvAction::Task,
        ]
    );
    assert_eq!(
        child_argv_plan(true, false, false, 0, false, false).unwrap(),
        vec![
            ChildArgvAction::Super,
            ChildArgvAction::NoSubAgent,
            ChildArgvAction::NoPresidio,
            ChildArgvAction::OpenAiProvider,
            ChildArgvAction::Exec,
            ChildArgvAction::Task,
        ]
    );
}

#[test]
fn app_policy_plans_cover_slots_catalogs_recursion_and_child_results() {
    assert_eq!(
        catalog_status(false, true).unwrap(),
        CatalogStatusPlan::Degraded
    );
    assert_eq!(
        catalog_status(false, false).unwrap(),
        CatalogStatusPlan::NoDynamicCatalog
    );
    assert_eq!(
        catalog_status(true, false).unwrap(),
        CatalogStatusPlan::Available
    );
    assert_eq!(
        catalog_entry_plan(
            [Some("  "), Some(" model "), None, None, None],
            false,
            false,
            Some("LiSt"),
        )
        .unwrap(),
        CatalogEntryPlan {
            model_id: Some((1, 1, 6)),
            set_id: true,
            selectable: true,
        }
    );
    assert_eq!(
        catalog_entry_plan(
            [Some(" canonical "), Some("lower"), None, None, None],
            false,
            false,
            None,
        )
        .unwrap(),
        CatalogEntryPlan {
            model_id: Some((0, 1, 10)),
            set_id: false,
            selectable: true,
        }
    );
    assert!(
        !catalog_entry_plan([None, None, None, None, None], true, false, Some("list"))
            .unwrap()
            .selectable
    );
    assert!(
        !catalog_entry_plan([None, None, None, None, None], false, true, Some("list"))
            .unwrap()
            .selectable
    );
    assert!(
        !catalog_entry_plan([None, None, None, None, None], false, false, Some(" list "))
            .unwrap()
            .selectable
    );
    assert!(use_all_effort_suggestions(false).unwrap());
    assert!(!use_all_effort_suggestions(true).unwrap());

    assert_eq!(
        recursion_decision(false, false).unwrap(),
        RecursionDecision::Allowed
    );
    assert_eq!(
        recursion_decision(true, false).unwrap(),
        RecursionDecision::Disabled
    );
    assert_eq!(
        recursion_decision(true, true).unwrap(),
        RecursionDecision::InternalLauncher
    );

    assert_eq!(
        slot_plan_step(2, 0, false).unwrap(),
        SlotPlanStep::Candidate { index: 0 }
    );
    assert_eq!(
        slot_plan_step(2, 1, false).unwrap(),
        SlotPlanStep::Candidate { index: 1 }
    );
    assert_eq!(
        slot_plan_step(2, 2, false).unwrap(),
        SlotPlanStep::LimitReached { exit_code: 75 }
    );
    assert_eq!(
        slot_plan_step(2, 0, true).unwrap(),
        SlotPlanStep::Retire { index: 2 }
    );
    assert_eq!(
        slot_plan_step(2, 61, true).unwrap(),
        SlotPlanStep::Retire { index: 63 }
    );
    assert_eq!(
        slot_plan_step(2, 62, true).unwrap(),
        SlotPlanStep::Ensure { index: 0 }
    );
    assert_eq!(
        slot_plan_step(2, 63, true).unwrap(),
        SlotPlanStep::Ensure { index: 1 }
    );
    assert_eq!(slot_plan_step(2, 64, true).unwrap(), SlotPlanStep::Complete);
    assert!(matches!(
        slot_plan_step(0, 0, false),
        Err(MojoError::InvalidInput)
    ));
    assert_eq!(
        slot_lock_error_action(false, true, false).unwrap(),
        SlotLockErrorAction::TryNext
    );
    assert_eq!(
        slot_lock_error_action(true, false, true).unwrap(),
        SlotLockErrorAction::BlockResize
    );
    assert_eq!(
        slot_lock_error_action(false, false, false).unwrap(),
        SlotLockErrorAction::Propagate
    );

    assert_eq!(
        child_outcome(true, true, true, false).unwrap(),
        ChildOutcomeAction::Cancelled {
            exit_code: 130,
            output_incomplete: true,
        }
    );
    assert_eq!(
        child_outcome(false, false, true, false).unwrap(),
        ChildOutcomeAction::ChildFailed {
            output_incomplete: true,
        }
    );
    assert_eq!(
        child_outcome(false, true, true, true).unwrap(),
        ChildOutcomeAction::OutputIncomplete
    );
    assert_eq!(
        child_outcome(false, true, false, false).unwrap(),
        ChildOutcomeAction::NoOutput
    );
    assert_eq!(
        child_outcome(false, true, false, true).unwrap(),
        ChildOutcomeAction::Success
    );

    assert_eq!(
        config_validation_plan(
            Some(" \t"),
            ConfigReasoningState::Unsupported,
            ConfigUrlState::Invalid,
            true,
        )
        .unwrap(),
        ConfigValidationAction::ModelNonempty
    );
    assert_eq!(
        config_validation_plan(
            Some("model"),
            ConfigReasoningState::Unsupported,
            ConfigUrlState::Invalid,
            true,
        )
        .unwrap(),
        ConfigValidationAction::UnsupportedReasoning
    );
    assert_eq!(
        config_validation_plan(
            Some("model"),
            ConfigReasoningState::Valid,
            ConfigUrlState::Invalid,
            true,
        )
        .unwrap(),
        ConfigValidationAction::InvalidUrl
    );
    assert_eq!(
        config_validation_plan(
            Some("model"),
            ConfigReasoningState::Absent,
            ConfigUrlState::Absent,
            true,
        )
        .unwrap(),
        ConfigValidationAction::LocalRequiresUrl
    );
    assert_eq!(
        config_validation_plan(
            None,
            ConfigReasoningState::Absent,
            ConfigUrlState::Absent,
            false,
        )
        .unwrap(),
        ConfigValidationAction::Valid
    );
}

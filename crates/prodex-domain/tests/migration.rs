use prodex_domain::{
    MigrationCompatibilityError, MigrationCompatibilityErrorStatus, MigrationCompatibilityWindow,
    MigrationExecutionMode, MigrationPlan, MigrationPlanError, MigrationPlanErrorStatus,
    MigrationStep, MigrationStepKind, MigrationStepState, MigrationVersion, MigrationVersionError,
    MigrationVersionErrorStatus, plan_migration_compatibility_error_response,
    plan_migration_plan_error_response, plan_migration_version_error_response,
    validate_expand_contract_order, validate_migration_compatibility,
};

#[test]
fn migration_version_rejects_empty_values() {
    assert_eq!(MigrationVersion::new(""), Err(MigrationVersionError::Empty));
    assert_eq!(
        MigrationVersion::new(" "),
        Err(MigrationVersionError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        MigrationVersion::new("20260701 create tenants"),
        Err(MigrationVersionError::InvalidCharacter {
            index: 8,
            character: ' ',
        })
    );
    assert_eq!(
        MigrationVersion::new(" 202607010001_create_tenants"),
        Err(MigrationVersionError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        MigrationVersion::new("20260701\ncreate_tenants"),
        Err(MigrationVersionError::InvalidCharacter {
            index: 8,
            character: '\n',
        })
    );
    assert_eq!(
        MigrationVersion::new("20260701-é"),
        Err(MigrationVersionError::InvalidCharacter {
            index: 9,
            character: 'é',
        })
    );
}

#[test]
fn migration_version_error_response_is_stable_and_redacted() {
    let response = plan_migration_version_error_response(&MigrationVersionError::Empty);

    assert_eq!(
        MigrationVersionError::Empty.to_string(),
        "migration version is invalid"
    );
    assert!(!MigrationVersionError::Empty.to_string().contains("empty"));
    assert_eq!(response.status, MigrationVersionErrorStatus::BadRequest);
    assert_eq!(response.code, "migration_version_invalid");
    assert_eq!(response.message, "migration version is invalid");
    assert!(!response.message.contains("empty"));

    let invalid = MigrationVersionError::InvalidCharacter {
        index: 8,
        character: '\n',
    };
    assert_eq!(invalid.to_string(), "migration version is invalid");
    assert!(!invalid.to_string().contains("8"));
    assert!(!invalid.to_string().contains("\\n"));
    let rendered = format!("{invalid:?}");
    assert!(!rendered.contains("8"));
    assert!(!rendered.contains("\\n"));
    assert!(rendered.contains("\"<redacted>\""));
    let invalid_response = plan_migration_version_error_response(&invalid);
    assert_eq!(invalid_response, response);
    assert!(!invalid_response.message.contains("8"));
    assert!(!invalid_response.message.contains("\\n"));
}

#[test]
fn migration_version_debug_output_is_stable_and_redacted() {
    let version = MigrationVersion::new("202607010001_secret_migration").unwrap();

    let rendered = format!("{version:?}");
    assert!(!rendered.contains("202607010001_secret_migration"));
    assert_eq!(rendered, "MigrationVersion(\"<redacted>\")");
}

#[test]
fn migration_plan_lists_pending_versions_in_order() {
    let first = MigrationVersion::new("202607010001_create_tenants").unwrap();
    let second = MigrationVersion::new("202607010002_create_keys").unwrap();
    let plan = MigrationPlan::new(
        MigrationExecutionMode::DedicatedMigrator,
        Some("migrator-1"),
        vec![
            MigrationStep::new(first.clone(), "create tenants", MigrationStepState::Applied),
            MigrationStep::new(second.clone(), "create keys", MigrationStepState::Pending),
        ],
    );

    assert_eq!(plan.pending_versions(), vec![&second]);
}

#[test]
fn migration_plan_debug_output_is_stable_and_redacted() {
    let plan = MigrationPlan::new(
        MigrationExecutionMode::DedicatedMigrator,
        Some("migrator-secret-owner"),
        vec![MigrationStep::new(
            MigrationVersion::new("202607010001_secret_migration").unwrap(),
            "copy tenant secrets into new table",
            MigrationStepState::Pending,
        )],
    );
    let step = &plan.steps[0];

    let rendered = format!("{step:?} {plan:?}");
    for sensitive in [
        "202607010001_secret_migration",
        "copy tenant secrets",
        "migrator-secret-owner",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "migration debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("description: \"<redacted>\""));
    assert!(rendered.contains("lock_owner: Some(\"<redacted>\")"));
    assert!(rendered.contains("step_count: 1"));
}

#[test]
fn request_serving_runtime_cannot_execute_migrations() {
    let plan = MigrationPlan::new(
        MigrationExecutionMode::RequestServingRuntime,
        Some("runtime-1"),
        vec![],
    );

    assert_eq!(
        plan.can_execute(),
        Err(MigrationPlanError::RequestServingExecutionForbidden)
    );
}

#[test]
fn dedicated_migrator_requires_lock_owner() {
    let plan = MigrationPlan::new(
        MigrationExecutionMode::DedicatedMigrator,
        None::<String>,
        vec![],
    );

    assert_eq!(
        plan.can_execute(),
        Err(MigrationPlanError::MigratorLockRequired)
    );
}

#[test]
fn failed_step_blocks_execution_until_resolved() {
    let plan = MigrationPlan::new(
        MigrationExecutionMode::DedicatedMigrator,
        Some("migrator-1"),
        vec![MigrationStep::new(
            MigrationVersion::new("202607010001_create_tenants").unwrap(),
            "create tenants",
            MigrationStepState::Failed,
        )],
    );

    assert_eq!(
        plan.can_execute(),
        Err(MigrationPlanError::FailedStepPresent)
    );
}

#[test]
fn dedicated_locked_plan_without_failed_steps_can_execute() {
    let plan = MigrationPlan::new(
        MigrationExecutionMode::DedicatedMigrator,
        Some("migrator-1"),
        vec![MigrationStep::new(
            MigrationVersion::new("202607010001_create_tenants").unwrap(),
            "create tenants",
            MigrationStepState::Pending,
        )],
    );

    assert_eq!(plan.can_execute(), Ok(()));
}

#[test]
fn expand_contract_order_requires_expand_before_backfill_verify_or_contract() {
    let expand = MigrationStep::new(
        MigrationVersion::new("202607010001_expand").unwrap(),
        "add nullable columns",
        MigrationStepState::Pending,
    )
    .with_kind(MigrationStepKind::Expand);
    let backfill = MigrationStep::new(
        MigrationVersion::new("202607010002_backfill").unwrap(),
        "backfill data",
        MigrationStepState::Pending,
    )
    .with_kind(MigrationStepKind::Backfill);
    let verify = MigrationStep::new(
        MigrationVersion::new("202607010003_verify").unwrap(),
        "verify data",
        MigrationStepState::Pending,
    )
    .with_kind(MigrationStepKind::Verify);
    let contract = MigrationStep::new(
        MigrationVersion::new("202607010004_contract").unwrap(),
        "drop legacy column",
        MigrationStepState::Pending,
    )
    .with_kind(MigrationStepKind::Contract);

    assert_eq!(
        validate_expand_contract_order(std::slice::from_ref(&backfill)),
        Err(MigrationPlanError::BackfillBeforeExpand)
    );
    assert_eq!(
        validate_expand_contract_order(&[expand.clone(), verify.clone()]),
        Err(MigrationPlanError::VerifyBeforeBackfill)
    );
    assert_eq!(
        validate_expand_contract_order(std::slice::from_ref(&contract)),
        Err(MigrationPlanError::ContractBeforeExpand)
    );
    assert_eq!(
        validate_expand_contract_order(&[expand, backfill, verify, contract]),
        Ok(())
    );
}

#[test]
fn migration_plan_error_responses_are_stable_and_redacted() {
    assert_eq!(
        MigrationPlanError::RequestServingExecutionForbidden.to_string(),
        "migration execution is not allowed from request-serving runtime"
    );
    assert_eq!(
        MigrationPlanError::MigratorLockRequired.to_string(),
        "migration lock is required"
    );
    assert_eq!(
        MigrationPlanError::FailedStepPresent.to_string(),
        "migration plan has unresolved failed steps"
    );
    assert_eq!(
        MigrationPlanError::VerifyBeforeBackfill.to_string(),
        "migration step order is invalid"
    );
    assert!(
        !MigrationPlanError::MigratorLockRequired
            .to_string()
            .contains("owner")
    );
    assert!(
        !MigrationPlanError::VerifyBeforeBackfill
            .to_string()
            .contains("verify")
    );
    assert!(
        !MigrationPlanError::VerifyBeforeBackfill
            .to_string()
            .contains("backfill")
    );

    let request_path =
        plan_migration_plan_error_response(&MigrationPlanError::RequestServingExecutionForbidden);

    assert_eq!(
        request_path.status,
        MigrationPlanErrorStatus::InvalidConfiguration
    );
    assert_eq!(request_path.code, "migration_request_path_forbidden");
    assert_eq!(
        request_path.message,
        "migration execution is not allowed from request-serving runtime"
    );

    let lock = plan_migration_plan_error_response(&MigrationPlanError::MigratorLockRequired);
    assert_eq!(lock.status, MigrationPlanErrorStatus::Conflict);
    assert_eq!(lock.code, "migration_lock_required");
    assert_eq!(lock.message, "migration lock is required");
    assert!(!lock.message.contains("owner"));

    let failed = plan_migration_plan_error_response(&MigrationPlanError::FailedStepPresent);
    assert_eq!(failed.status, MigrationPlanErrorStatus::Conflict);
    assert_eq!(failed.code, "migration_failed_step_present");
    assert_eq!(failed.message, "migration plan has unresolved failed steps");

    let order = plan_migration_plan_error_response(&MigrationPlanError::VerifyBeforeBackfill);
    assert_eq!(order.status, MigrationPlanErrorStatus::InvalidConfiguration);
    assert_eq!(order.code, "migration_order_invalid");
    assert_eq!(order.message, "migration step order is invalid");
    assert!(!order.message.contains("Verify"));
    assert!(!order.message.contains("Backfill"));
}

#[test]
fn compatibility_window_requires_at_least_two_previous_versions_and_checks_source() {
    let target = MigrationVersion::new("0.220.0").unwrap();
    assert_eq!(
        MigrationCompatibilityWindow::new(
            target.clone(),
            vec![MigrationVersion::new("0.219.0").unwrap()]
        )
        .unwrap_err(),
        MigrationCompatibilityError::RequiresAtLeastTwoPreviousVersions
    );

    let window = MigrationCompatibilityWindow::new(
        target,
        vec![
            MigrationVersion::new("0.218.0").unwrap(),
            MigrationVersion::new("0.219.0").unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(
        validate_migration_compatibility(&MigrationVersion::new("0.219.0").unwrap(), &window),
        Ok(())
    );
    assert_eq!(
        validate_migration_compatibility(&MigrationVersion::new("0.217.0").unwrap(), &window),
        Err(MigrationCompatibilityError::UnsupportedSourceVersion)
    );

    let rendered = format!("{window:?}");
    for sensitive in ["0.220.0", "0.218.0", "0.219.0"] {
        assert!(
            !rendered.contains(sensitive),
            "migration compatibility debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("compatible_from_count: 2"));
}

#[test]
fn migration_compatibility_error_responses_are_stable_and_redacted() {
    let invalid_window = plan_migration_compatibility_error_response(
        &MigrationCompatibilityError::RequiresAtLeastTwoPreviousVersions,
    );

    assert_eq!(
        MigrationCompatibilityError::RequiresAtLeastTwoPreviousVersions.to_string(),
        "migration compatibility window is invalid"
    );
    assert!(
        !MigrationCompatibilityError::RequiresAtLeastTwoPreviousVersions
            .to_string()
            .contains("two")
    );
    assert_eq!(
        invalid_window.status,
        MigrationCompatibilityErrorStatus::InvalidConfiguration
    );
    assert_eq!(
        invalid_window.code,
        "migration_compatibility_window_invalid"
    );
    assert_eq!(
        invalid_window.message,
        "migration compatibility window is invalid"
    );
    assert!(!invalid_window.message.contains("two"));

    let unsupported = plan_migration_compatibility_error_response(
        &MigrationCompatibilityError::UnsupportedSourceVersion,
    );
    assert_eq!(
        MigrationCompatibilityError::UnsupportedSourceVersion.to_string(),
        "migration source version is unsupported"
    );
    assert!(
        !MigrationCompatibilityError::UnsupportedSourceVersion
            .to_string()
            .contains("window")
    );
    assert_eq!(
        unsupported.status,
        MigrationCompatibilityErrorStatus::Conflict
    );
    assert_eq!(unsupported.code, "migration_source_version_unsupported");
    assert_eq!(
        unsupported.message,
        "migration source version is unsupported"
    );
    assert!(!unsupported.message.contains("0."));
}

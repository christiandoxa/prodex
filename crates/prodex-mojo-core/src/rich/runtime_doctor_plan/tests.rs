use super::*;

#[test]
fn input_contracts_are_validated_by_mojo() {
    let plan = RuntimeDoctorPlanInput {
        operation: RUNTIME_DOCTOR_PLAN_OP_POLICY_SUGGESTIONS + 1,
        ..RuntimeDoctorPlanInput::default()
    };
    assert!(matches!(
        runtime_doctor_plan(plan),
        Err(MojoError::InvalidInput)
    ));

    let mut summary = RuntimeDoctorSummaryPlanInput::default();
    summary.marker_counts[RUNTIME_DOCTOR_SUMMARY_MARKER_COUNT - 1] =
        RUNTIME_DOCTOR_PLAN_MAX_COUNT + 1;
    assert!(matches!(
        runtime_doctor_summary_plan(summary),
        Err(MojoError::InvalidInput)
    ));

    let state = RuntimeDoctorStatePlanInput {
        operation: RUNTIME_DOCTOR_STATE_OP_CIRCUIT + 1,
        ..RuntimeDoctorStatePlanInput::default()
    };
    assert!(matches!(
        runtime_doctor_state_plan(state),
        Err(MojoError::InvalidInput)
    ));

    let route = RuntimeDoctorRoutePlanInput {
        health_decay_seconds: 0,
        ..RuntimeDoctorRoutePlanInput::default()
    };
    assert!(matches!(
        runtime_doctor_route_plan(route),
        Err(MojoError::InvalidInput)
    ));
}

#[test]
fn plan_self_test_passes() {
    assert!(runtime_doctor_plan_self_test());
}

#[test]
fn report_plan_preserves_command_precedence_and_profile_order() {
    let install = runtime_doctor_report_plan(RuntimeDoctorReportPlanInput {
        operation: RUNTIME_DOCTOR_REPORT_OP_COMMAND,
        install: 1,
        ..RuntimeDoctorReportPlanInput::default()
    })
    .unwrap();
    assert_eq!(install.mode, RUNTIME_DOCTOR_REPORT_MODE_INSTALL_ONLY);
    assert_eq!(install.include_install, 1);

    let install_with_repair = runtime_doctor_report_plan(RuntimeDoctorReportPlanInput {
        operation: RUNTIME_DOCTOR_REPORT_OP_COMMAND,
        install: 1,
        repair_session_index: 1,
        ..RuntimeDoctorReportPlanInput::default()
    })
    .unwrap();
    assert_eq!(install_with_repair.mode, RUNTIME_DOCTOR_REPORT_MODE_HUMAN);
    assert_eq!(install_with_repair.include_install, 1);

    let runtime_json = runtime_doctor_report_plan(RuntimeDoctorReportPlanInput {
        operation: RUNTIME_DOCTOR_REPORT_OP_COMMAND,
        install: 1,
        runtime: 1,
        json: 1,
        suggest_policy: 1,
        runtime_config_valid: 1,
        ..RuntimeDoctorReportPlanInput::default()
    })
    .unwrap();
    assert_eq!(runtime_json.mode, RUNTIME_DOCTOR_REPORT_MODE_RUNTIME_JSON);
    assert_eq!(runtime_json.include_install, 1);
    assert_eq!(runtime_json.include_suggestions, 1);

    let invalid_config = runtime_doctor_report_plan(RuntimeDoctorReportPlanInput {
        operation: RUNTIME_DOCTOR_REPORT_OP_COMMAND,
        runtime: 1,
        suggest_policy: 1,
        runtime_config_valid: 0,
        ..RuntimeDoctorReportPlanInput::default()
    })
    .unwrap();
    assert_eq!(invalid_config.include_suggestions, 0);

    let profile = runtime_doctor_report_plan(RuntimeDoctorReportPlanInput {
        operation: RUNTIME_DOCTOR_REPORT_OP_PROFILE,
        provider_kind: RUNTIME_DOCTOR_REPORT_PROVIDER_GEMINI,
        ..RuntimeDoctorReportPlanInput::default()
    })
    .unwrap();
    assert_eq!(profile.field_count, 10);
    assert_eq!(profile.fields[0], RUNTIME_DOCTOR_REPORT_FIELD_CURRENT);
    assert_eq!(profile.fields[9], RUNTIME_DOCTOR_REPORT_FIELD_MIGRATION);
}

#[test]
fn summary_and_state_plan_self_tests_pass() {
    assert!(runtime_doctor_summary_plan_self_test());
    assert!(runtime_doctor_state_plan_self_test());
}

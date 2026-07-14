use prodex_application::{
    ApplicationInspectionRequest, ApplicationInspectionSource, ApplicationObligationContext,
    ApplicationObligationDisposition, ApplicationObligationMode, ApplicationObligationViolation,
    ApplicationResponseTransport, plan_application_obligation_execution,
    plan_application_request_inspection,
};
use prodex_domain::{
    CapabilitySet, DataClassification, DataModality, DetectorRevisionId, EnvironmentContext,
    FindingKind, GovernanceObligation, InspectionCoverage, InspectionLimits, InspectionReasonCode,
    ModelCapability, NetworkZone, PolicyDecision, PolicyEffect, PolicyRevisionId, PolicySelector,
    SessionPolicyContext,
};

fn decision(obligations: Vec<GovernanceObligation>) -> PolicyDecision {
    PolicyDecision {
        effect: PolicyEffect::Allow,
        obligations,
        reason_codes: Vec::new(),
        policy_revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
    }
}

fn attribute_restriction_plan(
    mode: ApplicationObligationMode,
    obligations: Vec<GovernanceObligation>,
    requested_model: Option<&str>,
    requested_modalities: &[DataModality],
) -> prodex_application::ApplicationObligationExecutionPlan {
    let capabilities = CapabilitySet::new(Vec::new());
    plan_application_obligation_execution(
        &decision(obligations),
        ApplicationObligationContext {
            mode,
            classification: DataClassification::Internal,
            inspection_coverage: InspectionCoverage::Full,
            detected_findings: &[],
            masked_findings: &[],
            requested_capabilities: &capabilities,
            requested_model,
            requested_tools: Some(&[]),
            requested_modalities,
            estimated_input_tokens: 1,
            estimated_context_tokens: 1,
            requested_output_tokens: None,
            session: SessionPolicyContext {
                age_seconds: 0,
                idle_seconds: 0,
                revoked: false,
                mfa_satisfied: false,
                retained_classification: DataClassification::Internal,
            },
            environment: EnvironmentContext {
                network_zone: NetworkZone::Unknown,
                authentication_strength: 1,
                mfa_satisfied: false,
            },
            response_transport: ApplicationResponseTransport::Unary,
            response_inspection_coverage: InspectionCoverage::Unsupported,
        },
    )
}

#[test]
fn model_allow_set_rejects_missing_model_metadata_in_enforcing_modes() {
    for mode in [
        ApplicationObligationMode::Enforce,
        ApplicationObligationMode::BankEnforce,
    ] {
        let plan = attribute_restriction_plan(
            mode,
            vec![GovernanceObligation::AllowModel(
                PolicySelector::new("model-a").unwrap(),
            )],
            None,
            &[DataModality::Text],
        );
        assert_eq!(plan.disposition, ApplicationObligationDisposition::Reject);
        assert_eq!(
            plan.violations,
            vec![ApplicationObligationViolation::ModelNotAllowed]
        );
        assert_eq!(plan.violations[0].code(), "model_not_allowed");
    }
}

#[test]
fn text_only_modality_allow_set_rejects_audio_requests() {
    let plan = attribute_restriction_plan(
        ApplicationObligationMode::Enforce,
        vec![GovernanceObligation::AllowModality(DataModality::Text)],
        Some("model-a"),
        &[DataModality::Text, DataModality::Audio],
    );
    assert_eq!(plan.disposition, ApplicationObligationDisposition::Reject);
    assert_eq!(
        plan.violations,
        vec![ApplicationObligationViolation::ModalityNotAllowed]
    );
}

#[test]
fn allow_tool_obligations_form_one_allow_set() {
    let capabilities = CapabilitySet::new(vec![ModelCapability::Tools]);
    let requested_tools = ["lookup", "summarize"];
    let plan = plan_application_obligation_execution(
        &decision(vec![
            GovernanceObligation::AllowTool(PolicySelector::new("lookup").unwrap()),
            GovernanceObligation::AllowTool(PolicySelector::new("summarize").unwrap()),
        ]),
        ApplicationObligationContext {
            mode: ApplicationObligationMode::Enforce,
            classification: DataClassification::Internal,
            inspection_coverage: InspectionCoverage::Full,
            detected_findings: &[],
            masked_findings: &[],
            requested_capabilities: &capabilities,
            requested_model: None,
            requested_tools: Some(&requested_tools),
            requested_modalities: &[DataModality::Text],
            estimated_input_tokens: 1,
            estimated_context_tokens: 1,
            requested_output_tokens: None,
            session: SessionPolicyContext {
                age_seconds: 0,
                idle_seconds: 0,
                revoked: false,
                mfa_satisfied: false,
                retained_classification: DataClassification::Internal,
            },
            environment: EnvironmentContext {
                network_zone: NetworkZone::Unknown,
                authentication_strength: 1,
                mfa_satisfied: false,
            },
            response_transport: ApplicationResponseTransport::Unary,
            response_inspection_coverage: InspectionCoverage::Unsupported,
        },
    );

    assert!(plan.violations.is_empty());
    assert_eq!(plan.disposition, ApplicationObligationDisposition::Proceed);
}

#[test]
fn mask_obligation_requires_explicit_masking_evidence() {
    let decision = decision(vec![GovernanceObligation::MaskFinding(
        FindingKind::EmailAddress,
    )]);
    let capabilities = CapabilitySet::new(Vec::new());
    for (masked_findings, expected) in [
        (&[][..], ApplicationObligationDisposition::Reject),
        (
            &[FindingKind::EmailAddress][..],
            ApplicationObligationDisposition::Proceed,
        ),
    ] {
        let plan = plan_application_obligation_execution(
            &decision,
            ApplicationObligationContext {
                mode: ApplicationObligationMode::Enforce,
                classification: DataClassification::Confidential,
                inspection_coverage: InspectionCoverage::Full,
                detected_findings: &[FindingKind::EmailAddress],
                masked_findings,
                requested_capabilities: &capabilities,
                requested_model: None,
                requested_tools: None,
                requested_modalities: &[DataModality::Text],
                estimated_input_tokens: 1,
                estimated_context_tokens: 1,
                requested_output_tokens: None,
                session: SessionPolicyContext {
                    age_seconds: 0,
                    idle_seconds: 0,
                    revoked: false,
                    mfa_satisfied: false,
                    retained_classification: DataClassification::Confidential,
                },
                environment: EnvironmentContext {
                    network_zone: NetworkZone::Unknown,
                    authentication_strength: 1,
                    mfa_satisfied: false,
                },
                response_transport: ApplicationResponseTransport::Unary,
                response_inspection_coverage: InspectionCoverage::Unsupported,
            },
        );
        assert_eq!(plan.disposition, expected);
    }
}

fn plan(
    mode: ApplicationObligationMode,
    classification: DataClassification,
    coverage: InspectionCoverage,
    transport: ApplicationResponseTransport,
) -> prodex_application::ApplicationObligationExecutionPlan {
    plan_application_obligation_execution(
        &decision(vec![GovernanceObligation::RequireResponseInspection]),
        ApplicationObligationContext {
            mode,
            classification,
            inspection_coverage: coverage,
            detected_findings: &[],
            masked_findings: &[],
            requested_capabilities: &CapabilitySet::new(vec![ModelCapability::Streaming]),
            requested_model: Some("model-a"),
            requested_tools: Some(&[]),
            requested_modalities: &[DataModality::Text],
            estimated_input_tokens: 10,
            estimated_context_tokens: 10,
            requested_output_tokens: Some(10),
            session: SessionPolicyContext {
                age_seconds: 5,
                idle_seconds: 2,
                revoked: false,
                mfa_satisfied: true,
                retained_classification: classification,
            },
            environment: EnvironmentContext {
                network_zone: NetworkZone::Local,
                authentication_strength: 2,
                mfa_satisfied: true,
            },
            response_transport: transport,
            response_inspection_coverage: coverage,
        },
    )
}

#[test]
fn obligation_matrix_preserves_classification_and_observe_enforce_semantics() {
    let classifications = [
        DataClassification::Public,
        DataClassification::Internal,
        DataClassification::Confidential,
        DataClassification::Restricted,
    ];
    let coverages = [
        InspectionCoverage::Full,
        InspectionCoverage::Partial,
        InspectionCoverage::Unsupported,
    ];
    let transports = [
        ApplicationResponseTransport::Unary,
        ApplicationResponseTransport::ServerSentEvents,
        ApplicationResponseTransport::WebSocket,
    ];

    for classification in classifications {
        for coverage in coverages {
            for transport in transports {
                let observed = plan(
                    ApplicationObligationMode::Observe,
                    classification,
                    coverage,
                    transport,
                );
                let enforced = plan(
                    ApplicationObligationMode::Enforce,
                    classification,
                    coverage,
                    transport,
                );
                assert_eq!(observed.classification, classification);
                assert_eq!(observed.response.transport, transport);
                assert_eq!(
                    observed.disposition,
                    ApplicationObligationDisposition::Proceed
                );
                assert_eq!(
                    enforced.disposition,
                    if coverage == InspectionCoverage::Unsupported {
                        ApplicationObligationDisposition::Reject
                    } else {
                        ApplicationObligationDisposition::Proceed
                    }
                );
            }
        }
    }
}

#[test]
fn bank_response_inspection_requires_full_coverage() {
    for coverage in [
        InspectionCoverage::Full,
        InspectionCoverage::Partial,
        InspectionCoverage::Unsupported,
    ] {
        let bank = plan(
            ApplicationObligationMode::BankEnforce,
            DataClassification::Restricted,
            coverage,
            ApplicationResponseTransport::ServerSentEvents,
        );
        assert_eq!(
            bank.disposition,
            if coverage == InspectionCoverage::Full {
                ApplicationObligationDisposition::Proceed
            } else {
                ApplicationObligationDisposition::Reject
            }
        );
    }
}

#[test]
fn inspection_failure_mode_matrix_preserves_shadow_and_fail_closed_semantics() {
    for (reason, coverage) in [
        ("modality.image.unsupported", InspectionCoverage::Partial),
        ("modality.audio.unsupported", InspectionCoverage::Partial),
        ("modality.file.unsupported", InspectionCoverage::Partial),
        ("presidio.timeout", InspectionCoverage::Unsupported),
        ("presidio.unavailable", InspectionCoverage::Unsupported),
        ("presidio.malformed", InspectionCoverage::Unsupported),
        ("content.non_utf8", InspectionCoverage::Unsupported),
    ] {
        let inspection = plan_application_request_inspection(ApplicationInspectionRequest {
            sources: vec![ApplicationInspectionSource {
                coverage,
                findings: Vec::new(),
                masked_findings: Vec::new(),
                tags: Vec::new(),
                reason_codes: vec![InspectionReasonCode::new(reason).unwrap()],
            }],
            default_classification: DataClassification::Restricted,
            trusted_label: None,
            prior_classification: None,
            detector_revision: DetectorRevisionId::new("failure-matrix-v1").unwrap(),
            limits: InspectionLimits::default(),
        })
        .unwrap();
        assert_eq!(inspection.result.coverage(), coverage, "reason={reason}");
        assert_ne!(
            inspection.result.coverage(),
            InspectionCoverage::Full,
            "reason={reason}"
        );
        assert_eq!(inspection.result.reason_codes()[0].as_str(), reason);

        let observed = plan(
            ApplicationObligationMode::Observe,
            inspection.result.classification(),
            coverage,
            ApplicationResponseTransport::ServerSentEvents,
        );
        assert_eq!(
            observed.disposition,
            ApplicationObligationDisposition::Proceed
        );
        assert!(!observed.response.enforce);
        assert_eq!(observed.response.inspection_coverage, coverage);

        let enforced = plan(
            ApplicationObligationMode::Enforce,
            inspection.result.classification(),
            coverage,
            ApplicationResponseTransport::ServerSentEvents,
        );
        assert!(enforced.response.enforce);
        assert_eq!(
            enforced.disposition,
            if coverage == InspectionCoverage::Unsupported {
                ApplicationObligationDisposition::Reject
            } else {
                ApplicationObligationDisposition::Proceed
            },
            "reason={reason}"
        );

        let bank = plan(
            ApplicationObligationMode::BankEnforce,
            inspection.result.classification(),
            coverage,
            ApplicationResponseTransport::ServerSentEvents,
        );
        assert_eq!(bank.disposition, ApplicationObligationDisposition::Reject);
        assert!(bank.response.require_full_inspection);
    }
}

#[test]
fn request_and_session_obligations_return_stable_typed_violations() {
    let capabilities = CapabilitySet::new(vec![ModelCapability::Tools]);
    let plan = plan_application_obligation_execution(
        &decision(vec![
            GovernanceObligation::DisableTools,
            GovernanceObligation::MaxInputTokens(4),
            GovernanceObligation::SessionIdleTimeoutSeconds(3),
            GovernanceObligation::RequireMfa,
        ]),
        ApplicationObligationContext {
            mode: ApplicationObligationMode::Enforce,
            classification: DataClassification::Confidential,
            inspection_coverage: InspectionCoverage::Full,
            detected_findings: &[],
            masked_findings: &[],
            requested_capabilities: &capabilities,
            requested_model: None,
            requested_tools: None,
            requested_modalities: &[DataModality::Text],
            estimated_input_tokens: 5,
            estimated_context_tokens: 5,
            requested_output_tokens: None,
            session: SessionPolicyContext {
                age_seconds: 10,
                idle_seconds: 4,
                revoked: false,
                mfa_satisfied: false,
                retained_classification: DataClassification::Confidential,
            },
            environment: EnvironmentContext {
                network_zone: NetworkZone::Unknown,
                authentication_strength: 1,
                mfa_satisfied: false,
            },
            response_transport: ApplicationResponseTransport::Unary,
            response_inspection_coverage: InspectionCoverage::Unsupported,
        },
    );

    assert_eq!(plan.disposition, ApplicationObligationDisposition::Reject);
    assert_eq!(
        plan.violations,
        vec![
            ApplicationObligationViolation::ToolsDisabled,
            ApplicationObligationViolation::InputTokenLimitExceeded,
            ApplicationObligationViolation::SessionIdleTimeout,
            ApplicationObligationViolation::MfaRequired,
        ]
    );
    assert_eq!(
        plan.violations
            .iter()
            .map(|violation| violation.code())
            .collect::<Vec<_>>(),
        vec![
            "tools_disabled",
            "input_token_limit_exceeded",
            "session_idle_timeout",
            "mfa_required",
        ]
    );
}

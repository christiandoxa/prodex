use prodex_application::{
    ApplicationObligationContext, ApplicationObligationDisposition, ApplicationObligationMode,
    ApplicationObligationViolation, ApplicationResponseTransport,
    plan_application_obligation_execution,
};
use prodex_domain::{
    CapabilitySet, DataClassification, DataModality, EnvironmentContext, FindingKind,
    GovernanceObligation, InspectionCoverage, ModelCapability, NetworkZone, PolicyDecision,
    PolicyEffect, PolicyRevisionId, PolicySelector, SessionPolicyContext,
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

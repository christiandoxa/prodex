use super::*;

use prodex_domain::{
    CapabilitySet, DataClassification, DataModality, EnvironmentContext, FindingKind,
    GovernanceObligation, InspectionCoverage, ModelCapability, NetworkZone, PolicyDecision,
    PolicyEffect, PolicyRevisionId, PolicySelector, SessionPolicyContext,
};

fn decision() -> PolicyDecision {
    PolicyDecision {
        effect: PolicyEffect::RequireApproval,
        obligations: vec![
            GovernanceObligation::MaskFinding(FindingKind::EmailAddress),
            GovernanceObligation::DisableTools,
            GovernanceObligation::AllowTool(PolicySelector::new("lookup").unwrap()),
            GovernanceObligation::AllowModel(PolicySelector::new("model-a").unwrap()),
            GovernanceObligation::AllowModality(DataModality::Text),
            GovernanceObligation::MaxInputTokens(10),
            GovernanceObligation::MaxOutputTokens(5),
            GovernanceObligation::MaxContextTokens(20),
            GovernanceObligation::RequireResponseInspection,
            GovernanceObligation::SessionIdleTimeoutSeconds(3),
            GovernanceObligation::SessionAbsoluteTimeoutSeconds(10),
            GovernanceObligation::MinimumAuthenticationStrength(2),
            GovernanceObligation::RequireReauthentication,
            GovernanceObligation::RequireMfa,
            GovernanceObligation::RequireHumanApproval,
        ],
        reason_codes: Vec::new(),
        policy_revision: PolicyRevisionId::new(),
        valid_until_unix_ms: u64::MAX,
    }
}

fn context<'a>(
    mode: ApplicationObligationMode,
    capabilities: &'a CapabilitySet,
    tools: Option<&'a [&'a str]>,
    response_coverage: InspectionCoverage,
) -> ApplicationObligationContext<'a> {
    ApplicationObligationContext {
        mode,
        classification: DataClassification::Restricted,
        inspection_coverage: InspectionCoverage::Full,
        detected_findings: &[FindingKind::EmailAddress],
        masked_findings: &[],
        requested_capabilities: capabilities,
        requested_model: Some("model-a"),
        requested_tools: tools,
        requested_modalities: &[DataModality::Text],
        estimated_input_tokens: 11,
        estimated_context_tokens: 21,
        requested_output_tokens: Some(6),
        session: SessionPolicyContext {
            age_seconds: 11,
            idle_seconds: 4,
            revoked: true,
            mfa_satisfied: false,
            retained_classification: DataClassification::Restricted,
        },
        environment: EnvironmentContext {
            network_zone: NetworkZone::Unknown,
            authentication_strength: 1,
            mfa_satisfied: true,
            reauthentication_satisfied: false,
        },
        response_transport: ApplicationResponseTransport::ServerSentEvents,
        response_inspection_coverage: response_coverage,
    }
}

#[test]
fn mojo_obligation_plan_matches_rust_oracle_and_preserves_observe_mode() {
    let capabilities = CapabilitySet::new(vec![ModelCapability::Streaming, ModelCapability::Tools]);
    let tools = ["lookup"];
    let decision = decision();

    let expected = super::plan_application_obligation_execution_rust(
        &decision,
        context(
            ApplicationObligationMode::BankEnforce,
            &capabilities,
            Some(&tools),
            InspectionCoverage::Partial,
        ),
    );
    let actual = super::plan_application_obligation_execution(
        &decision,
        context(
            ApplicationObligationMode::BankEnforce,
            &capabilities,
            Some(&tools),
            InspectionCoverage::Partial,
        ),
    );
    assert_eq!(actual, expected);

    let expected = super::plan_application_obligation_execution_rust(
        &decision,
        context(
            ApplicationObligationMode::Observe,
            &capabilities,
            None,
            InspectionCoverage::Unsupported,
        ),
    );
    let actual = super::plan_application_obligation_execution(
        &decision,
        context(
            ApplicationObligationMode::Observe,
            &capabilities,
            None,
            InspectionCoverage::Unsupported,
        ),
    );
    assert_eq!(actual, expected);
    assert_eq!(
        actual.disposition,
        ApplicationObligationDisposition::Proceed
    );
}

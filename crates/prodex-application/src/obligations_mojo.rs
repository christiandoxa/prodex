use super::*;

use prodex_domain::{
    DataModality, FindingKind, GovernanceObligation, ModelCapability, PolicyEffect,
};

pub(super) fn plan(
    decision: &PolicyDecision,
    context: ApplicationObligationContext<'_>,
) -> ApplicationObligationExecutionPlan {
    let obligations = decision
        .obligations
        .iter()
        .map(normalize_obligation)
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::rich::plan_application_obligations(
        prodex_mojo_core::rich::ApplicationObligationInput {
            mode: obligation_mode(context.mode),
            effect: policy_effect(decision.effect),
            obligations: &obligations,
            classification: data_classification(context.classification),
            inspection_coverage: inspection_coverage(context.inspection_coverage),
            detected_findings_mask: finding_mask(context.detected_findings),
            masked_findings_mask: finding_mask(context.masked_findings),
            requested_capabilities_mask: capability_mask(context.requested_capabilities),
            requested_model: context.requested_model,
            requested_tools: context.requested_tools,
            requested_modalities_mask: modality_mask(context.requested_modalities),
            estimated_input_tokens: context.estimated_input_tokens,
            estimated_context_tokens: context.estimated_context_tokens,
            requested_output_tokens: context.requested_output_tokens,
            session_age_seconds: context.session.age_seconds,
            session_idle_seconds: context.session.idle_seconds,
            session_revoked: context.session.revoked,
            session_mfa_satisfied: context.session.mfa_satisfied,
            authentication_strength: context.environment.authentication_strength,
            environment_mfa_satisfied: context.environment.mfa_satisfied,
            reauthentication_satisfied: context.environment.reauthentication_satisfied,
            response_transport: response_transport(context.response_transport),
            response_inspection_coverage: inspection_coverage(context.response_inspection_coverage),
        },
    )
    .expect("Mojo application obligation planner returned invalid output");

    ApplicationObligationExecutionPlan {
        classification: context.classification,
        request: ApplicationRequestObligationPlan {
            mask_findings: plan
                .mask_findings
                .into_iter()
                .map(finding_kind_from_tag)
                .collect(),
            disable_tools: plan.disable_tools,
            maximum_input_tokens: plan.maximum_input_tokens,
            maximum_context_tokens: plan.maximum_context_tokens,
        },
        response: ApplicationResponseObligationPlan {
            enforce: plan.enforce,
            inspection_required: plan.inspection_required,
            require_full_inspection: plan.require_full_inspection,
            inspection_coverage: context.response_inspection_coverage,
            maximum_output_tokens: plan.maximum_output_tokens,
            transport: context.response_transport,
        },
        violations: plan
            .violation_tags
            .into_iter()
            .map(violation_from_tag)
            .collect(),
        disposition: match plan.disposition {
            0 => ApplicationObligationDisposition::Proceed,
            1 => ApplicationObligationDisposition::Reject,
            _ => unreachable!("validated Mojo disposition tag"),
        },
    }
}

fn normalize_obligation(
    obligation: &GovernanceObligation,
) -> prodex_mojo_core::rich::ApplicationObligationRecord<'_> {
    use prodex_mojo_core::rich as mojo;

    let (kind, value, selector) = match obligation {
        GovernanceObligation::MaskFinding(finding) => (
            mojo::APPLICATION_OBLIGATION_MASK_FINDING,
            u64::from(finding_kind_to_tag(*finding)),
            None,
        ),
        GovernanceObligation::DisableTools => (mojo::APPLICATION_OBLIGATION_DISABLE_TOOLS, 0, None),
        GovernanceObligation::AllowTool(selector) => (
            mojo::APPLICATION_OBLIGATION_ALLOW_TOOL,
            0,
            Some(selector.as_str()),
        ),
        GovernanceObligation::AllowModel(selector) => (
            mojo::APPLICATION_OBLIGATION_ALLOW_MODEL,
            0,
            Some(selector.as_str()),
        ),
        GovernanceObligation::AllowModality(modality) => (
            mojo::APPLICATION_OBLIGATION_ALLOW_MODALITY,
            u64::from(modality_to_tag(*modality)),
            None,
        ),
        GovernanceObligation::MaxInputTokens(limit) => (
            mojo::APPLICATION_OBLIGATION_MAX_INPUT_TOKENS,
            u64::from(*limit),
            None,
        ),
        GovernanceObligation::MaxOutputTokens(limit) => (
            mojo::APPLICATION_OBLIGATION_MAX_OUTPUT_TOKENS,
            u64::from(*limit),
            None,
        ),
        GovernanceObligation::MaxContextTokens(limit) => (
            mojo::APPLICATION_OBLIGATION_MAX_CONTEXT_TOKENS,
            u64::from(*limit),
            None,
        ),
        GovernanceObligation::RequireResponseInspection => (
            mojo::APPLICATION_OBLIGATION_REQUIRE_RESPONSE_INSPECTION,
            0,
            None,
        ),
        GovernanceObligation::SessionIdleTimeoutSeconds(limit) => (
            mojo::APPLICATION_OBLIGATION_SESSION_IDLE_TIMEOUT,
            u64::from(*limit),
            None,
        ),
        GovernanceObligation::SessionAbsoluteTimeoutSeconds(limit) => (
            mojo::APPLICATION_OBLIGATION_SESSION_ABSOLUTE_TIMEOUT,
            u64::from(*limit),
            None,
        ),
        GovernanceObligation::MinimumAuthenticationStrength(strength) => (
            mojo::APPLICATION_OBLIGATION_MIN_AUTHENTICATION_STRENGTH,
            u64::from(*strength),
            None,
        ),
        GovernanceObligation::RequireReauthentication => (
            mojo::APPLICATION_OBLIGATION_REQUIRE_REAUTHENTICATION,
            0,
            None,
        ),
        GovernanceObligation::RequireMfa => (mojo::APPLICATION_OBLIGATION_REQUIRE_MFA, 0, None),
        GovernanceObligation::RequireHumanApproval => {
            (mojo::APPLICATION_OBLIGATION_REQUIRE_HUMAN_APPROVAL, 0, None)
        }
        GovernanceObligation::MinimumProviderTrust(_)
        | GovernanceObligation::AllowProvider(_)
        | GovernanceObligation::DenyProvider(_)
        | GovernanceObligation::RequireLocalExecution
        | GovernanceObligation::ProhibitRetention
        | GovernanceObligation::ProhibitTrainingUse
        | GovernanceObligation::RequireRegion(_)
        | GovernanceObligation::AuditDetail(_)
        | GovernanceObligation::RetentionSeconds(_)
        | GovernanceObligation::DenyFallbackOutsideEligibility => {
            (mojo::APPLICATION_OBLIGATION_OTHER, 0, None)
        }
    };
    prodex_mojo_core::rich::ApplicationObligationRecord {
        kind,
        value,
        selector,
    }
}

fn obligation_mode(mode: ApplicationObligationMode) -> u8 {
    match mode {
        ApplicationObligationMode::Observe => 0,
        ApplicationObligationMode::Enforce => 1,
        ApplicationObligationMode::BankEnforce => 2,
    }
}

fn policy_effect(effect: PolicyEffect) -> u8 {
    match effect {
        PolicyEffect::Allow => 0,
        PolicyEffect::RequireApproval => 1,
        PolicyEffect::Deny => 2,
    }
}

fn data_classification(classification: DataClassification) -> u8 {
    match classification {
        DataClassification::Public => 0,
        DataClassification::Internal => 1,
        DataClassification::Confidential => 2,
        DataClassification::Restricted => 3,
    }
}

fn inspection_coverage(coverage: InspectionCoverage) -> u8 {
    match coverage {
        InspectionCoverage::Full => 0,
        InspectionCoverage::Partial => 1,
        InspectionCoverage::Unsupported => 2,
    }
}

fn response_transport(transport: ApplicationResponseTransport) -> u8 {
    match transport {
        ApplicationResponseTransport::Unary => 0,
        ApplicationResponseTransport::ServerSentEvents => 1,
        ApplicationResponseTransport::WebSocket => 2,
    }
}

fn finding_kind_to_tag(finding: FindingKind) -> u8 {
    match finding {
        FindingKind::EmailAddress => 0,
        FindingKind::PhoneNumber => 1,
        FindingKind::PersonName => 2,
        FindingKind::PhysicalAddress => 3,
        FindingKind::GovernmentId => 4,
        FindingKind::FinancialAccount => 5,
        FindingKind::PaymentCard => 6,
        FindingKind::AccessToken => 7,
        FindingKind::ApiKey => 8,
        FindingKind::PrivateKey => 9,
        FindingKind::Password => 10,
        FindingKind::TenantSensitive => 11,
    }
}

fn finding_kind_from_tag(tag: u8) -> FindingKind {
    FindingKind::ALL
        .get(usize::from(tag))
        .copied()
        .expect("validated Mojo finding tag")
}

fn modality_to_tag(modality: DataModality) -> u8 {
    match modality {
        DataModality::Text => 0,
        DataModality::Image => 1,
        DataModality::Audio => 2,
        DataModality::Video => 3,
        DataModality::File => 4,
    }
}

fn modality_mask(modalities: &[DataModality]) -> u8 {
    modalities.iter().fold(0, |mask, modality| {
        mask | (1_u8 << modality_to_tag(*modality))
    })
}

fn capability_mask(capabilities: &CapabilitySet) -> u8 {
    capabilities.as_slice().iter().fold(0, |mask, capability| {
        mask | match capability {
            ModelCapability::ResponsesApi => 1 << 0,
            ModelCapability::Streaming => 1 << 1,
            ModelCapability::Tools => 1 << 2,
            ModelCapability::Vision => 1 << 3,
            ModelCapability::JsonMode => 1 << 4,
            ModelCapability::RemoteCompact => 1 << 5,
            ModelCapability::WebSocket => 1 << 6,
        }
    })
}

fn finding_mask(findings: &[FindingKind]) -> u16 {
    findings.iter().fold(0, |mask, finding| {
        mask | (1_u16 << finding_kind_to_tag(*finding))
    })
}

fn violation_from_tag(tag: u8) -> ApplicationObligationViolation {
    match tag {
        0 => ApplicationObligationViolation::PolicyDenied,
        1 => ApplicationObligationViolation::ApprovalRequired,
        2 => ApplicationObligationViolation::RequiredMaskMissing,
        3 => ApplicationObligationViolation::ToolsDisabled,
        4 => ApplicationObligationViolation::ToolNotAllowed,
        5 => ApplicationObligationViolation::ToolMetadataUnsupported,
        6 => ApplicationObligationViolation::ModelNotAllowed,
        7 => ApplicationObligationViolation::ModalityNotAllowed,
        8 => ApplicationObligationViolation::InputTokenLimitExceeded,
        9 => ApplicationObligationViolation::OutputTokenLimitExceeded,
        10 => ApplicationObligationViolation::ContextTokenLimitExceeded,
        11 => ApplicationObligationViolation::ResponseInspectionUnsupported,
        12 => ApplicationObligationViolation::ResponseInspectionIncomplete,
        13 => ApplicationObligationViolation::SessionRevoked,
        14 => ApplicationObligationViolation::SessionIdleTimeout,
        15 => ApplicationObligationViolation::SessionAbsoluteTimeout,
        16 => ApplicationObligationViolation::AuthenticationStrengthRequired,
        17 => ApplicationObligationViolation::ReauthenticationRequired,
        18 => ApplicationObligationViolation::MfaRequired,
        _ => unreachable!("validated Mojo violation tag"),
    }
}

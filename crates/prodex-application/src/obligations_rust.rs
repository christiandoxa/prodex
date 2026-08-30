use super::*;

use prodex_domain::{GovernanceObligation, ModelCapability, PolicyEffect};

#[derive(Default)]
struct ApplicationObligationAccumulator<'a> {
    mask_findings: Vec<FindingKind>,
    disable_tools: bool,
    maximum_input_tokens: Option<u32>,
    maximum_output_tokens: Option<u32>,
    maximum_context_tokens: Option<u32>,
    inspection_required: bool,
    allowed_tools: Vec<&'a str>,
    allowed_models: Vec<&'a str>,
    allowed_modalities: Vec<DataModality>,
}

pub(super) fn plan(
    decision: &PolicyDecision,
    context: ApplicationObligationContext<'_>,
) -> ApplicationObligationExecutionPlan {
    let mut accumulator = ApplicationObligationAccumulator::default();
    let mut violations = Vec::new();

    match decision.effect {
        PolicyEffect::Allow => {}
        PolicyEffect::Deny => violations.push(ApplicationObligationViolation::PolicyDenied),
        PolicyEffect::RequireApproval => {
            violations.push(ApplicationObligationViolation::ApprovalRequired);
        }
    }

    if context.session.revoked {
        violations.push(ApplicationObligationViolation::SessionRevoked);
    }

    for obligation in &decision.obligations {
        apply_request_obligation(obligation, &context, &mut accumulator, &mut violations);
        apply_security_obligation(obligation, &context, &mut accumulator, &mut violations);
    }

    validate_allowed_tools(&accumulator, &context, &mut violations);
    validate_allowed_model(&accumulator, &context, &mut violations);
    validate_allowed_modalities(&accumulator, &context, &mut violations);

    let ApplicationObligationAccumulator {
        mut mask_findings,
        disable_tools,
        maximum_input_tokens,
        maximum_output_tokens,
        maximum_context_tokens,
        inspection_required,
        ..
    } = accumulator;
    mask_findings.sort();
    mask_findings.dedup();
    violations.sort();
    violations.dedup();
    let enforce = matches!(
        context.mode,
        ApplicationObligationMode::Enforce | ApplicationObligationMode::BankEnforce
    );
    let disposition = if enforce && !violations.is_empty() {
        ApplicationObligationDisposition::Reject
    } else {
        ApplicationObligationDisposition::Proceed
    };
    ApplicationObligationExecutionPlan {
        classification: context.classification,
        request: ApplicationRequestObligationPlan {
            mask_findings,
            disable_tools,
            maximum_input_tokens,
            maximum_context_tokens,
        },
        response: ApplicationResponseObligationPlan {
            enforce,
            inspection_required,
            require_full_inspection: inspection_required
                && context.mode == ApplicationObligationMode::BankEnforce,
            inspection_coverage: context.response_inspection_coverage,
            maximum_output_tokens,
            transport: context.response_transport,
        },
        violations,
        disposition,
    }
}

fn apply_request_obligation<'a>(
    obligation: &'a GovernanceObligation,
    context: &ApplicationObligationContext<'_>,
    accumulator: &mut ApplicationObligationAccumulator<'a>,
    violations: &mut Vec<ApplicationObligationViolation>,
) {
    match obligation {
        GovernanceObligation::MaskFinding(kind) => {
            accumulator.mask_findings.push(*kind);
            if context.detected_findings.contains(kind) && !context.masked_findings.contains(kind) {
                violations.push(ApplicationObligationViolation::RequiredMaskMissing);
            }
        }
        GovernanceObligation::DisableTools => {
            accumulator.disable_tools = true;
            if context
                .requested_capabilities
                .contains(ModelCapability::Tools)
            {
                violations.push(ApplicationObligationViolation::ToolsDisabled);
            }
        }
        GovernanceObligation::AllowTool(selector) => {
            accumulator.allowed_tools.push(selector.as_str());
        }
        GovernanceObligation::AllowModel(selector) => {
            accumulator.allowed_models.push(selector.as_str());
        }
        GovernanceObligation::AllowModality(modality) => {
            accumulator.allowed_modalities.push(*modality);
        }
        GovernanceObligation::MaxInputTokens(limit) => {
            accumulator.maximum_input_tokens =
                minimum_limit(accumulator.maximum_input_tokens, *limit);
            if context.estimated_input_tokens > *limit {
                violations.push(ApplicationObligationViolation::InputTokenLimitExceeded);
            }
        }
        GovernanceObligation::MaxOutputTokens(limit) => {
            accumulator.maximum_output_tokens =
                minimum_limit(accumulator.maximum_output_tokens, *limit);
            if context
                .requested_output_tokens
                .is_some_and(|requested| requested > *limit)
            {
                violations.push(ApplicationObligationViolation::OutputTokenLimitExceeded);
            }
        }
        GovernanceObligation::MaxContextTokens(limit) => {
            accumulator.maximum_context_tokens =
                minimum_limit(accumulator.maximum_context_tokens, *limit);
            if context.estimated_context_tokens > *limit {
                violations.push(ApplicationObligationViolation::ContextTokenLimitExceeded);
            }
        }
        _ => {}
    }
}

fn apply_security_obligation(
    obligation: &GovernanceObligation,
    context: &ApplicationObligationContext<'_>,
    accumulator: &mut ApplicationObligationAccumulator<'_>,
    violations: &mut Vec<ApplicationObligationViolation>,
) {
    match obligation {
        GovernanceObligation::RequireResponseInspection => {
            apply_response_inspection_obligation(context, accumulator, violations);
        }
        GovernanceObligation::SessionIdleTimeoutSeconds(limit) => {
            if context.session.idle_seconds > u64::from(*limit) {
                violations.push(ApplicationObligationViolation::SessionIdleTimeout);
            }
        }
        GovernanceObligation::SessionAbsoluteTimeoutSeconds(limit) => {
            if context.session.age_seconds > u64::from(*limit) {
                violations.push(ApplicationObligationViolation::SessionAbsoluteTimeout);
            }
        }
        GovernanceObligation::MinimumAuthenticationStrength(minimum) => {
            if context.environment.authentication_strength < *minimum {
                violations.push(ApplicationObligationViolation::AuthenticationStrengthRequired);
            }
        }
        GovernanceObligation::RequireReauthentication => {
            if !context.environment.reauthentication_satisfied {
                violations.push(ApplicationObligationViolation::ReauthenticationRequired);
            }
        }
        GovernanceObligation::RequireMfa => {
            if !context.session.mfa_satisfied || !context.environment.mfa_satisfied {
                violations.push(ApplicationObligationViolation::MfaRequired);
            }
        }
        GovernanceObligation::RequireHumanApproval => {
            violations.push(ApplicationObligationViolation::ApprovalRequired);
        }
        _ => {}
    }
}

fn apply_response_inspection_obligation(
    context: &ApplicationObligationContext<'_>,
    accumulator: &mut ApplicationObligationAccumulator<'_>,
    violations: &mut Vec<ApplicationObligationViolation>,
) {
    accumulator.inspection_required = true;
    if context.response_inspection_coverage == InspectionCoverage::Unsupported {
        violations.push(ApplicationObligationViolation::ResponseInspectionUnsupported);
    } else if context.mode == ApplicationObligationMode::BankEnforce
        && context.response_inspection_coverage != InspectionCoverage::Full
    {
        violations.push(ApplicationObligationViolation::ResponseInspectionIncomplete);
    }
}

fn validate_allowed_tools(
    accumulator: &ApplicationObligationAccumulator<'_>,
    context: &ApplicationObligationContext<'_>,
    violations: &mut Vec<ApplicationObligationViolation>,
) {
    if accumulator.allowed_tools.is_empty()
        || !context
            .requested_capabilities
            .contains(ModelCapability::Tools)
    {
        return;
    }
    match context.requested_tools {
        Some(tools)
            if tools.iter().any(|tool| {
                !accumulator
                    .allowed_tools
                    .iter()
                    .any(|allowed| *allowed == "*" || *allowed == *tool)
            }) =>
        {
            violations.push(ApplicationObligationViolation::ToolNotAllowed)
        }
        None => violations.push(ApplicationObligationViolation::ToolMetadataUnsupported),
        Some(_) => {}
    }
}

fn validate_allowed_model(
    accumulator: &ApplicationObligationAccumulator<'_>,
    context: &ApplicationObligationContext<'_>,
    violations: &mut Vec<ApplicationObligationViolation>,
) {
    if accumulator.allowed_models.is_empty() {
        return;
    }
    if !context.requested_model.is_some_and(|model| {
        accumulator
            .allowed_models
            .iter()
            .any(|allowed| *allowed == "*" || *allowed == model)
    }) {
        violations.push(ApplicationObligationViolation::ModelNotAllowed);
    }
}

fn validate_allowed_modalities(
    accumulator: &ApplicationObligationAccumulator<'_>,
    context: &ApplicationObligationContext<'_>,
    violations: &mut Vec<ApplicationObligationViolation>,
) {
    if !accumulator.allowed_modalities.is_empty()
        && context
            .requested_modalities
            .iter()
            .any(|requested| !accumulator.allowed_modalities.contains(requested))
    {
        violations.push(ApplicationObligationViolation::ModalityNotAllowed);
    }
}

fn minimum_limit(current: Option<u32>, candidate: u32) -> Option<u32> {
    Some(current.map_or(candidate, |current| current.min(candidate)))
}

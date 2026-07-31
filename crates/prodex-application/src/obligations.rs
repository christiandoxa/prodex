//! Typed, side-effect-free request and response obligation execution plans.

use prodex_domain::{
    CapabilitySet, DataClassification, DataModality, EnvironmentContext, FindingKind,
    GovernanceObligation, InspectionCoverage, ModelCapability, PolicyDecision, PolicyEffect,
    SessionPolicyContext,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationObligationMode {
    Observe,
    Enforce,
    BankEnforce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationResponseTransport {
    Unary,
    ServerSentEvents,
    WebSocket,
}

pub struct ApplicationObligationContext<'a> {
    pub mode: ApplicationObligationMode,
    pub classification: DataClassification,
    pub inspection_coverage: InspectionCoverage,
    pub detected_findings: &'a [FindingKind],
    pub masked_findings: &'a [FindingKind],
    pub requested_capabilities: &'a CapabilitySet,
    pub requested_model: Option<&'a str>,
    pub requested_tools: Option<&'a [&'a str]>,
    pub requested_modalities: &'a [DataModality],
    pub estimated_input_tokens: u32,
    pub estimated_context_tokens: u32,
    pub requested_output_tokens: Option<u32>,
    pub session: SessionPolicyContext,
    pub environment: EnvironmentContext,
    pub response_transport: ApplicationResponseTransport,
    pub response_inspection_coverage: InspectionCoverage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRequestObligationPlan {
    pub mask_findings: Vec<FindingKind>,
    pub disable_tools: bool,
    pub maximum_input_tokens: Option<u32>,
    pub maximum_context_tokens: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationResponseObligationPlan {
    pub enforce: bool,
    pub inspection_required: bool,
    pub require_full_inspection: bool,
    pub inspection_coverage: InspectionCoverage,
    pub maximum_output_tokens: Option<u32>,
    pub transport: ApplicationResponseTransport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApplicationObligationViolation {
    PolicyDenied,
    ApprovalRequired,
    RequiredMaskMissing,
    ToolsDisabled,
    ToolNotAllowed,
    ToolMetadataUnsupported,
    ModelNotAllowed,
    ModalityNotAllowed,
    InputTokenLimitExceeded,
    OutputTokenLimitExceeded,
    ContextTokenLimitExceeded,
    ResponseInspectionUnsupported,
    ResponseInspectionIncomplete,
    SessionRevoked,
    SessionIdleTimeout,
    SessionAbsoluteTimeout,
    AuthenticationStrengthRequired,
    ReauthenticationRequired,
    MfaRequired,
}

impl ApplicationObligationViolation {
    pub const fn code(self) -> &'static str {
        match self {
            Self::PolicyDenied => "policy_denied",
            Self::ApprovalRequired => "approval_required",
            Self::RequiredMaskMissing => "required_mask_missing",
            Self::ToolsDisabled => "tools_disabled",
            Self::ToolNotAllowed => "tool_not_allowed",
            Self::ToolMetadataUnsupported => "tool_metadata_unsupported",
            Self::ModelNotAllowed => "model_not_allowed",
            Self::ModalityNotAllowed => "modality_not_allowed",
            Self::InputTokenLimitExceeded => "input_token_limit_exceeded",
            Self::OutputTokenLimitExceeded => "output_token_limit_exceeded",
            Self::ContextTokenLimitExceeded => "context_token_limit_exceeded",
            Self::ResponseInspectionUnsupported => "response_inspection_unsupported",
            Self::ResponseInspectionIncomplete => "response_inspection_incomplete",
            Self::SessionRevoked => "session_revoked",
            Self::SessionIdleTimeout => "session_idle_timeout",
            Self::SessionAbsoluteTimeout => "session_absolute_timeout",
            Self::AuthenticationStrengthRequired => "authentication_strength_required",
            Self::ReauthenticationRequired => "reauthentication_required",
            Self::MfaRequired => "mfa_required",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationObligationDisposition {
    Proceed,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationObligationExecutionPlan {
    pub classification: DataClassification,
    pub request: ApplicationRequestObligationPlan,
    pub response: ApplicationResponseObligationPlan,
    pub violations: Vec<ApplicationObligationViolation>,
    pub disposition: ApplicationObligationDisposition,
}

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

pub fn plan_application_obligation_execution(
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
            accumulator.inspection_required = true;
            if context.response_inspection_coverage == InspectionCoverage::Unsupported {
                violations.push(ApplicationObligationViolation::ResponseInspectionUnsupported);
            } else if context.mode == ApplicationObligationMode::BankEnforce
                && context.response_inspection_coverage != InspectionCoverage::Full
            {
                violations.push(ApplicationObligationViolation::ResponseInspectionIncomplete);
            }
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

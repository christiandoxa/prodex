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

pub fn plan_application_obligation_execution(
    decision: &PolicyDecision,
    context: ApplicationObligationContext<'_>,
) -> ApplicationObligationExecutionPlan {
    let mut mask_findings = Vec::new();
    let mut disable_tools = false;
    let mut maximum_input_tokens = None;
    let mut maximum_output_tokens = None;
    let mut maximum_context_tokens = None;
    let mut inspection_required = false;
    let mut allowed_tools = Vec::new();
    let mut allowed_models = Vec::new();
    let mut allowed_modalities = Vec::new();
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
        match obligation {
            GovernanceObligation::MaskFinding(kind) => {
                mask_findings.push(*kind);
                if context.detected_findings.contains(kind)
                    && !context.masked_findings.contains(kind)
                {
                    violations.push(ApplicationObligationViolation::RequiredMaskMissing);
                }
            }
            GovernanceObligation::DisableTools => {
                disable_tools = true;
                if context
                    .requested_capabilities
                    .contains(ModelCapability::Tools)
                {
                    violations.push(ApplicationObligationViolation::ToolsDisabled);
                }
            }
            GovernanceObligation::AllowTool(selector) => {
                allowed_tools.push(selector.as_str());
            }
            GovernanceObligation::AllowModel(selector) => {
                allowed_models.push(selector.as_str());
            }
            GovernanceObligation::AllowModality(modality) => {
                allowed_modalities.push(*modality);
            }
            GovernanceObligation::MaxInputTokens(limit) => {
                maximum_input_tokens = minimum_limit(maximum_input_tokens, *limit);
                if context.estimated_input_tokens > *limit {
                    violations.push(ApplicationObligationViolation::InputTokenLimitExceeded);
                }
            }
            GovernanceObligation::MaxOutputTokens(limit) => {
                maximum_output_tokens = minimum_limit(maximum_output_tokens, *limit);
                if context
                    .requested_output_tokens
                    .is_some_and(|requested| requested > *limit)
                {
                    violations.push(ApplicationObligationViolation::OutputTokenLimitExceeded);
                }
            }
            GovernanceObligation::MaxContextTokens(limit) => {
                maximum_context_tokens = minimum_limit(maximum_context_tokens, *limit);
                if context.estimated_context_tokens > *limit {
                    violations.push(ApplicationObligationViolation::ContextTokenLimitExceeded);
                }
            }
            GovernanceObligation::RequireResponseInspection => {
                inspection_required = true;
                if context.response_inspection_coverage == InspectionCoverage::Unsupported {
                    violations.push(ApplicationObligationViolation::ResponseInspectionUnsupported);
                } else if context.mode == ApplicationObligationMode::BankEnforce
                    && context.response_inspection_coverage != InspectionCoverage::Full
                {
                    violations.push(ApplicationObligationViolation::ResponseInspectionIncomplete);
                }
            }
            GovernanceObligation::SessionIdleTimeoutSeconds(limit)
                if context.session.idle_seconds > u64::from(*limit) =>
            {
                violations.push(ApplicationObligationViolation::SessionIdleTimeout);
            }
            GovernanceObligation::SessionAbsoluteTimeoutSeconds(limit)
                if context.session.age_seconds > u64::from(*limit) =>
            {
                violations.push(ApplicationObligationViolation::SessionAbsoluteTimeout);
            }
            GovernanceObligation::RequireReauthentication => {
                violations.push(ApplicationObligationViolation::ReauthenticationRequired);
            }
            GovernanceObligation::RequireMfa
                if !context.session.mfa_satisfied || !context.environment.mfa_satisfied =>
            {
                violations.push(ApplicationObligationViolation::MfaRequired);
            }
            GovernanceObligation::RequireHumanApproval => {
                violations.push(ApplicationObligationViolation::ApprovalRequired);
            }
            _ => {}
        }
    }

    if !allowed_tools.is_empty()
        && context
            .requested_capabilities
            .contains(ModelCapability::Tools)
    {
        match context.requested_tools {
            Some(tools)
                if tools.iter().any(|tool| {
                    !allowed_tools
                        .iter()
                        .any(|allowed| *allowed == "*" || *allowed == *tool)
                }) =>
            {
                violations.push(ApplicationObligationViolation::ToolNotAllowed);
            }
            None => violations.push(ApplicationObligationViolation::ToolMetadataUnsupported),
            Some(_) => {}
        }
    }
    if !allowed_models.is_empty() {
        match context.requested_model {
            Some(model)
                if allowed_models
                    .iter()
                    .any(|allowed| *allowed == "*" || *allowed == model) => {}
            Some(_) | None => violations.push(ApplicationObligationViolation::ModelNotAllowed),
        }
    }
    if !allowed_modalities.is_empty()
        && context
            .requested_modalities
            .iter()
            .any(|requested| !allowed_modalities.contains(requested))
    {
        violations.push(ApplicationObligationViolation::ModalityNotAllowed);
    }

    mask_findings.sort();
    mask_findings.dedup();
    violations.sort();
    violations.dedup();
    let disposition = if matches!(
        context.mode,
        ApplicationObligationMode::Enforce | ApplicationObligationMode::BankEnforce
    ) && !violations.is_empty()
    {
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
            enforce: matches!(
                context.mode,
                ApplicationObligationMode::Enforce | ApplicationObligationMode::BankEnforce
            ),
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

fn minimum_limit(current: Option<u32>, candidate: u32) -> Option<u32> {
    Some(current.map_or(candidate, |current| current.min(candidate)))
}

//! Typed, side-effect-free request and response obligation execution plans.

use prodex_domain::{
    CapabilitySet, DataClassification, DataModality, EnvironmentContext, FindingKind,
    InspectionCoverage, PolicyDecision, SessionPolicyContext,
};

#[cfg(feature = "mojo")]
#[path = "obligations_mojo.rs"]
mod mojo;
#[cfg(all(test, feature = "mojo"))]
#[path = "obligations_mojo_tests.rs"]
mod mojo_tests;
#[cfg(any(not(feature = "mojo"), test))]
#[path = "obligations_rust.rs"]
mod rust;

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

pub fn plan_application_obligation_execution(
    decision: &PolicyDecision,
    context: ApplicationObligationContext<'_>,
) -> ApplicationObligationExecutionPlan {
    #[cfg(feature = "mojo")]
    {
        mojo::plan(decision, context)
    }

    #[cfg(not(feature = "mojo"))]
    {
        plan_application_obligation_execution_rust(decision, context)
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn plan_application_obligation_execution_rust(
    decision: &PolicyDecision,
    context: ApplicationObligationContext<'_>,
) -> ApplicationObligationExecutionPlan {
    rust::plan(decision, context)
}

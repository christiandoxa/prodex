//! Bounded, side-effect-free governance policy decisions.

use std::error::Error;
use std::fmt;

use serde::Serialize;

use crate::{
    CapabilitySet, CredentialScope, ModelCapability, PolicyRevisionId, Principal, PrincipalKind,
    Role, TenantContext,
};

use super::{DataClassification, FindingKind, InspectionCoverage};

pub const MAX_GOVERNANCE_POLICY_RULES: usize = 256;
pub const MAX_POLICY_OBLIGATIONS: usize = 64;
pub const MAX_POLICY_REASON_CODES: usize = 32;
const MAX_POLICY_TOKEN_BYTES: usize = 128;

macro_rules! policy_token {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, GovernancePolicyError> {
                let value = value.into();
                if !policy_token_is_valid(&value) {
                    return Err(GovernancePolicyError::InvalidToken);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&"<redacted>")
                    .finish()
            }
        }
    };
}

policy_token!(GovernancePolicyRuleId);
policy_token!(PolicyReasonCode);
policy_token!(PolicySelector);

fn policy_token_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_POLICY_TOKEN_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/' | b'*')
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Channel {
    Cli,
    Ide,
    Api,
    Mcp,
    InternalService,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum GovernedAction {
    InvokeModel,
    UseTool,
    UploadContent,
    CompactContext,
    MutateControlPlane,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CanonicalRoute(PolicySelector);

impl CanonicalRoute {
    pub fn new(value: impl Into<String>) -> Result<Self, GovernancePolicyError> {
        PolicySelector::new(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for CanonicalRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CanonicalRoute")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum RequestRisk {
    Low,
    Elevated,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SessionPolicyContext {
    pub age_seconds: u64,
    pub idle_seconds: u64,
    pub revoked: bool,
    pub mfa_satisfied: bool,
    pub retained_classification: DataClassification,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DataPolicyContext {
    pub classification: DataClassification,
    pub inspection_coverage: InspectionCoverage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct QuotaContext {
    pub has_headroom: bool,
    pub reservation_required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum NetworkZone {
    Local,
    TrustedInternal,
    Partner,
    Public,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct EnvironmentContext {
    pub network_zone: NetworkZone,
    pub authentication_strength: u8,
    pub mfa_satisfied: bool,
}

pub struct PolicyInput<'a> {
    pub tenant: TenantContext,
    pub principal: &'a Principal,
    pub channel: Channel,
    pub credential_scope: CredentialScope,
    pub session: SessionPolicyContext,
    pub action: GovernedAction,
    pub route: &'a CanonicalRoute,
    pub data: DataPolicyContext,
    pub request_risk: RequestRisk,
    pub requested_capabilities: &'a CapabilitySet,
    pub quota: QuotaContext,
    pub environment: EnvironmentContext,
}

impl fmt::Debug for PolicyInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyInput")
            .field("tenant", &self.tenant)
            .field("principal", &self.principal)
            .field("channel", &self.channel)
            .field("credential_scope", &self.credential_scope)
            .field("session", &self.session)
            .field("action", &self.action)
            .field("route", &self.route)
            .field("data", &self.data)
            .field("request_risk", &self.request_risk)
            .field("requested_capabilities", &self.requested_capabilities)
            .field("quota", &self.quota)
            .field("environment", &self.environment)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum PolicyEffect {
    Allow,
    RequireApproval,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ProviderTrustTier {
    Standard,
    Enterprise,
    RestrictedApproved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum DataModality {
    Text,
    Image,
    Audio,
    Video,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum AuditDetailLevel {
    Minimal,
    Standard,
    Elevated,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum GovernanceObligation {
    MaskFinding(FindingKind),
    MinimumProviderTrust(ProviderTrustTier),
    AllowProvider(PolicySelector),
    DenyProvider(PolicySelector),
    RequireLocalExecution,
    ProhibitRetention,
    ProhibitTrainingUse,
    RequireRegion(PolicySelector),
    DisableTools,
    AllowTool(PolicySelector),
    AllowModel(PolicySelector),
    AllowModality(DataModality),
    MaxInputTokens(u32),
    MaxOutputTokens(u32),
    MaxContextTokens(u32),
    RequireResponseInspection,
    SessionIdleTimeoutSeconds(u32),
    SessionAbsoluteTimeoutSeconds(u32),
    RequireReauthentication,
    RequireMfa,
    AuditDetail(AuditDetailLevel),
    RequireHumanApproval,
    RetentionSeconds(u32),
    DenyFallbackOutsideEligibility,
}

impl fmt::Debug for GovernanceObligation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllowProvider(_)
            | Self::DenyProvider(_)
            | Self::RequireRegion(_)
            | Self::AllowTool(_)
            | Self::AllowModel(_) => f.write_str("GovernanceObligation(<redacted-selector>)"),
            other => fmt::Debug::fmt(&policy_obligation_safe_debug(other), f),
        }
    }
}

fn policy_obligation_safe_debug(obligation: &GovernanceObligation) -> &'static str {
    match obligation {
        GovernanceObligation::MaskFinding(_) => "mask_finding",
        GovernanceObligation::MinimumProviderTrust(_) => "minimum_provider_trust",
        GovernanceObligation::AllowProvider(_) => "allow_provider",
        GovernanceObligation::DenyProvider(_) => "deny_provider",
        GovernanceObligation::RequireLocalExecution => "require_local_execution",
        GovernanceObligation::ProhibitRetention => "prohibit_retention",
        GovernanceObligation::ProhibitTrainingUse => "prohibit_training_use",
        GovernanceObligation::RequireRegion(_) => "require_region",
        GovernanceObligation::DisableTools => "disable_tools",
        GovernanceObligation::AllowTool(_) => "allow_tool",
        GovernanceObligation::AllowModel(_) => "allow_model",
        GovernanceObligation::AllowModality(_) => "allow_modality",
        GovernanceObligation::MaxInputTokens(_) => "max_input_tokens",
        GovernanceObligation::MaxOutputTokens(_) => "max_output_tokens",
        GovernanceObligation::MaxContextTokens(_) => "max_context_tokens",
        GovernanceObligation::RequireResponseInspection => "require_response_inspection",
        GovernanceObligation::SessionIdleTimeoutSeconds(_) => "session_idle_timeout_seconds",
        GovernanceObligation::SessionAbsoluteTimeoutSeconds(_) => {
            "session_absolute_timeout_seconds"
        }
        GovernanceObligation::RequireReauthentication => "require_reauthentication",
        GovernanceObligation::RequireMfa => "require_mfa",
        GovernanceObligation::AuditDetail(_) => "audit_detail",
        GovernanceObligation::RequireHumanApproval => "require_human_approval",
        GovernanceObligation::RetentionSeconds(_) => "retention_seconds",
        GovernanceObligation::DenyFallbackOutsideEligibility => "deny_fallback_outside_eligibility",
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct PolicyRuleCondition {
    pub channel: Option<Channel>,
    pub principal_kind: Option<PrincipalKind>,
    pub minimum_role: Option<Role>,
    pub credential_scope: Option<CredentialScope>,
    pub action: Option<GovernedAction>,
    pub route: Option<CanonicalRoute>,
    pub minimum_classification: Option<DataClassification>,
    pub inspection_coverage: Option<InspectionCoverage>,
    pub minimum_request_risk: Option<RequestRisk>,
    pub network_zone: Option<NetworkZone>,
    pub maximum_session_age_seconds: Option<u64>,
    pub maximum_session_idle_seconds: Option<u64>,
    pub session_revoked: Option<bool>,
    pub session_mfa_satisfied: Option<bool>,
    pub minimum_session_retained_classification: Option<DataClassification>,
    pub minimum_authentication_strength: Option<u8>,
    pub environment_mfa_satisfied: Option<bool>,
    pub requested_capability: Option<ModelCapability>,
    pub quota_has_headroom: Option<bool>,
    pub quota_reservation_required: Option<bool>,
}

impl PolicyRuleCondition {
    fn matches(&self, input: &PolicyInput<'_>) -> bool {
        self.channel.is_none_or(|value| value == input.channel)
            && self
                .principal_kind
                .is_none_or(|value| value == input.principal.kind)
            && self
                .minimum_role
                .is_none_or(|value| input.principal.role >= value)
            && self
                .credential_scope
                .is_none_or(|value| value == input.credential_scope)
            && self.action.is_none_or(|value| value == input.action)
            && self.route.as_ref().is_none_or(|value| value == input.route)
            && self
                .minimum_classification
                .is_none_or(|value| input.data.classification >= value)
            && self
                .inspection_coverage
                .is_none_or(|value| value == input.data.inspection_coverage)
            && self
                .minimum_request_risk
                .is_none_or(|value| input.request_risk >= value)
            && self
                .network_zone
                .is_none_or(|value| value == input.environment.network_zone)
            && self
                .maximum_session_age_seconds
                .is_none_or(|value| input.session.age_seconds <= value)
            && self
                .maximum_session_idle_seconds
                .is_none_or(|value| input.session.idle_seconds <= value)
            && self
                .session_revoked
                .is_none_or(|value| value == input.session.revoked)
            && self
                .session_mfa_satisfied
                .is_none_or(|value| value == input.session.mfa_satisfied)
            && self
                .minimum_session_retained_classification
                .is_none_or(|value| input.session.retained_classification >= value)
            && self
                .minimum_authentication_strength
                .is_none_or(|value| input.environment.authentication_strength >= value)
            && self
                .environment_mfa_satisfied
                .is_none_or(|value| value == input.environment.mfa_satisfied)
            && self
                .requested_capability
                .is_none_or(|value| input.requested_capabilities.contains(value))
            && self
                .quota_has_headroom
                .is_none_or(|value| value == input.quota.has_headroom)
            && self
                .quota_reservation_required
                .is_none_or(|value| value == input.quota.reservation_required)
    }
}

impl fmt::Debug for PolicyRuleCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyRuleCondition")
            .field("channel", &self.channel)
            .field("principal_kind", &self.principal_kind)
            .field("minimum_role", &self.minimum_role)
            .field("credential_scope", &self.credential_scope)
            .field("action", &self.action)
            .field("route", &self.route.as_ref().map(|_| "<redacted>"))
            .field("minimum_classification", &self.minimum_classification)
            .field("inspection_coverage", &self.inspection_coverage)
            .field("minimum_request_risk", &self.minimum_request_risk)
            .field("network_zone", &self.network_zone)
            .field(
                "maximum_session_age_seconds",
                &self.maximum_session_age_seconds,
            )
            .field(
                "maximum_session_idle_seconds",
                &self.maximum_session_idle_seconds,
            )
            .field("session_revoked", &self.session_revoked)
            .field("session_mfa_satisfied", &self.session_mfa_satisfied)
            .field(
                "minimum_session_retained_classification",
                &self.minimum_session_retained_classification,
            )
            .field(
                "minimum_authentication_strength",
                &self.minimum_authentication_strength,
            )
            .field("environment_mfa_satisfied", &self.environment_mfa_satisfied)
            .field("requested_capability", &self.requested_capability)
            .field("quota_has_headroom", &self.quota_has_headroom)
            .field(
                "quota_reservation_required",
                &self.quota_reservation_required,
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernancePolicyRule {
    pub id: GovernancePolicyRuleId,
    pub condition: PolicyRuleCondition,
    pub effect: PolicyEffect,
    pub obligations: Vec<GovernanceObligation>,
    pub reason_code: PolicyReasonCode,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernancePolicyArtifact {
    pub revision: PolicyRevisionId,
    pub valid_until_unix_ms: u64,
    pub default_effect: PolicyEffect,
    pub rules: Vec<GovernancePolicyRule>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CompiledGovernancePolicy {
    revision: PolicyRevisionId,
    valid_until_unix_ms: u64,
    default_effect: PolicyEffect,
    rules: Vec<GovernancePolicyRule>,
}

impl CompiledGovernancePolicy {
    pub fn revision(&self) -> PolicyRevisionId {
        self.revision
    }
}

impl fmt::Debug for CompiledGovernancePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledGovernancePolicy")
            .field("revision", &"<redacted>")
            .field("valid_until_unix_ms", &"<redacted>")
            .field("default_effect", &self.default_effect)
            .field("rule_count", &self.rules.len())
            .finish()
    }
}

pub fn compile_governance_policy(
    mut artifact: GovernancePolicyArtifact,
) -> Result<CompiledGovernancePolicy, GovernancePolicyError> {
    if artifact.rules.len() > MAX_GOVERNANCE_POLICY_RULES {
        return Err(GovernancePolicyError::RuleLimitExceeded);
    }
    if artifact.valid_until_unix_ms == 0 {
        return Err(GovernancePolicyError::InvalidExpiry);
    }
    artifact.rules.sort_by(|left, right| left.id.cmp(&right.id));
    if artifact
        .rules
        .windows(2)
        .any(|rules| rules[0].id == rules[1].id)
    {
        return Err(GovernancePolicyError::DuplicateRule);
    }
    for rule in &mut artifact.rules {
        if rule.obligations.len() > MAX_POLICY_OBLIGATIONS {
            return Err(GovernancePolicyError::ObligationLimitExceeded);
        }
        if rule.effect == PolicyEffect::Deny && !rule.obligations.is_empty() {
            return Err(GovernancePolicyError::DenyRuleHasObligations);
        }
        rule.obligations.sort();
        rule.obligations.dedup();
    }
    Ok(CompiledGovernancePolicy {
        revision: artifact.revision,
        valid_until_unix_ms: artifact.valid_until_unix_ms,
        default_effect: artifact.default_effect,
        rules: artifact.rules,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub effect: PolicyEffect,
    pub obligations: Vec<GovernanceObligation>,
    pub reason_codes: Vec<PolicyReasonCode>,
    pub policy_revision: PolicyRevisionId,
    pub valid_until_unix_ms: u64,
}

impl fmt::Debug for PolicyDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyDecision")
            .field("effect", &self.effect)
            .field("obligation_count", &self.obligations.len())
            .field("reason_code_count", &self.reason_codes.len())
            .field("policy_revision", &"<redacted>")
            .field("valid_until_unix_ms", &"<redacted>")
            .finish()
    }
}

pub fn evaluate_governance_policy(
    policy: &CompiledGovernancePolicy,
    input: &PolicyInput<'_>,
) -> Result<PolicyDecision, GovernancePolicyError> {
    if input.principal.tenant_id != Some(input.tenant.tenant_id)
        || input.principal.credential_scope != input.credential_scope
    {
        return Ok(PolicyDecision {
            effect: PolicyEffect::Deny,
            obligations: Vec::new(),
            reason_codes: vec![PolicyReasonCode::new(
                "policy.missing_or_invalid_attribute",
            )?],
            policy_revision: policy.revision,
            valid_until_unix_ms: policy.valid_until_unix_ms,
        });
    }

    let mut effect = policy.default_effect;
    let mut obligations = Vec::new();
    let mut reason_codes = Vec::new();
    for rule in policy
        .rules
        .iter()
        .filter(|rule| rule.condition.matches(input))
    {
        effect = effect.max(rule.effect);
        obligations.extend(rule.obligations.iter().cloned());
        reason_codes.push(rule.reason_code.clone());
    }
    if reason_codes.is_empty() {
        reason_codes.push(PolicyReasonCode::new("policy.default")?);
    }
    obligations.sort();
    obligations.dedup();
    reason_codes.sort();
    reason_codes.dedup();
    if obligations.len() > MAX_POLICY_OBLIGATIONS {
        return Err(GovernancePolicyError::ObligationLimitExceeded);
    }
    if reason_codes.len() > MAX_POLICY_REASON_CODES {
        return Err(GovernancePolicyError::ReasonLimitExceeded);
    }
    if effect == PolicyEffect::Deny {
        obligations.clear();
    }
    Ok(PolicyDecision {
        effect,
        obligations,
        reason_codes,
        policy_revision: policy.revision,
        valid_until_unix_ms: policy.valid_until_unix_ms,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernancePolicyError {
    InvalidToken,
    InvalidExpiry,
    RuleLimitExceeded,
    DuplicateRule,
    ObligationLimitExceeded,
    DenyRuleHasObligations,
    ReasonLimitExceeded,
}

impl fmt::Display for GovernancePolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "governance policy is invalid")
    }
}

impl Error for GovernancePolicyError {}

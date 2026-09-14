//! Bounded, side-effect-free governance policy decisions.

use std::error::Error;
use std::fmt;

use serde::Serialize;

use crate::{
    CapabilitySet, CredentialScope, ModelCapability, PolicyRevisionId, Principal, PrincipalKind,
    Role, TenantContext,
};

use super::{DataClassification, FindingKind, InspectionCoverage};

mod compiled_policy;
mod principal_attributes;
pub use principal_attributes::{MAX_POLICY_PRINCIPAL_GROUPS, PrincipalPolicyAttributes};

pub const MAX_GOVERNANCE_POLICY_RULES: usize = 256;
pub const MAX_POLICY_OBLIGATIONS: usize = 64;
pub const MAX_POLICY_REASON_CODES: usize = 32;
const MAX_POLICY_REQUESTED_TOOLS: usize = 128;
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
    pub reauthentication_satisfied: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct BreakGlassPolicyContext {
    scope: PolicySelector,
    expires_at_unix_ms: u64,
}

impl BreakGlassPolicyContext {
    pub fn new(
        scope: impl Into<String>,
        expires_at_unix_ms: u64,
    ) -> Result<Self, GovernancePolicyError> {
        if expires_at_unix_ms == 0 {
            return Err(GovernancePolicyError::InvalidExpiry);
        }
        Ok(Self {
            scope: PolicySelector::new(scope)?,
            expires_at_unix_ms,
        })
    }

    fn is_valid_at(&self, evaluated_at_unix_ms: u64) -> bool {
        evaluated_at_unix_ms < self.expires_at_unix_ms
    }
}

impl fmt::Debug for BreakGlassPolicyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BreakGlassPolicyContext")
            .field("scope", &"<redacted>")
            .field("expires_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct RequestPolicyAttributes {
    requested_model: Option<PolicySelector>,
    requested_tools: Vec<PolicySelector>,
    requested_modalities: Vec<DataModality>,
    break_glass: Option<BreakGlassPolicyContext>,
    evaluated_at_unix_ms: u64,
}

impl RequestPolicyAttributes {
    pub fn new(
        requested_model: Option<&str>,
        requested_tools: &[String],
        mut requested_modalities: Vec<DataModality>,
        break_glass: Option<BreakGlassPolicyContext>,
        evaluated_at_unix_ms: u64,
    ) -> Result<Self, GovernancePolicyError> {
        if requested_tools.len() > MAX_POLICY_REQUESTED_TOOLS {
            return Err(GovernancePolicyError::AttributeLimitExceeded);
        }
        let mut requested_tools = requested_tools
            .iter()
            .map(PolicySelector::new)
            .collect::<Result<Vec<_>, _>>()?;
        requested_tools.sort();
        requested_tools.dedup();
        requested_modalities.sort();
        requested_modalities.dedup();
        Ok(Self {
            requested_model: requested_model.map(PolicySelector::new).transpose()?,
            requested_tools,
            requested_modalities,
            break_glass,
            evaluated_at_unix_ms,
        })
    }

    pub fn requested_model(&self) -> Option<&str> {
        self.requested_model.as_ref().map(PolicySelector::as_str)
    }

    pub fn requested_tools(&self) -> impl Iterator<Item = &str> {
        self.requested_tools.iter().map(PolicySelector::as_str)
    }

    pub fn requested_modalities(&self) -> &[DataModality] {
        &self.requested_modalities
    }

    fn valid_break_glass(&self) -> Option<&BreakGlassPolicyContext> {
        self.break_glass
            .as_ref()
            .filter(|grant| grant.is_valid_at(self.evaluated_at_unix_ms))
    }
}

impl fmt::Debug for RequestPolicyAttributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestPolicyAttributes")
            .field(
                "requested_model",
                &self.requested_model.as_ref().map(|_| "<redacted>"),
            )
            .field("requested_tool_count", &self.requested_tools.len())
            .field("requested_modalities", &self.requested_modalities)
            .field(
                "break_glass",
                &self.break_glass.as_ref().map(|_| "<redacted>"),
            )
            .field("evaluated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

pub struct PolicyInput<'a> {
    pub tenant: TenantContext,
    pub principal: &'a Principal,
    pub principal_attributes: &'a PrincipalPolicyAttributes,
    pub channel: Channel,
    pub credential_scope: CredentialScope,
    pub session: SessionPolicyContext,
    pub action: GovernedAction,
    pub route: &'a CanonicalRoute,
    pub data: DataPolicyContext,
    pub request_risk: RequestRisk,
    pub requested_capabilities: &'a CapabilitySet,
    pub request_attributes: &'a RequestPolicyAttributes,
    pub quota: QuotaContext,
    pub environment: EnvironmentContext,
}

impl fmt::Debug for PolicyInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyInput")
            .field("tenant", &self.tenant)
            .field("principal", &self.principal)
            .field("principal_attributes", &self.principal_attributes)
            .field("channel", &self.channel)
            .field("credential_scope", &self.credential_scope)
            .field("session", &self.session)
            .field("action", &self.action)
            .field("route", &self.route)
            .field("data", &self.data)
            .field("request_risk", &self.request_risk)
            .field("requested_capabilities", &self.requested_capabilities)
            .field("request_attributes", &self.request_attributes)
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
    MinimumAuthenticationStrength(u8),
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
        GovernanceObligation::MinimumAuthenticationStrength(_) => "minimum_authentication_strength",
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
    pub team_id: Option<PolicySelector>,
    pub project_id: Option<PolicySelector>,
    pub user_id: Option<PolicySelector>,
    pub group_id: Option<PolicySelector>,
    pub department_id: Option<PolicySelector>,
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
    pub requested_model: Option<PolicySelector>,
    pub requested_tool: Option<PolicySelector>,
    pub requested_modality: Option<DataModality>,
    pub break_glass_required: Option<bool>,
    pub break_glass_scope: Option<PolicySelector>,
    pub quota_has_headroom: Option<bool>,
    pub quota_reservation_required: Option<bool>,
}

impl PolicyRuleCondition {
    #[cfg(feature = "mojo")]
    fn matches(&self, input: &PolicyInput<'_>) -> bool {
        use prodex_mojo_core::policy::{
            GOVERNANCE_MATCH_CONTAINS, GOVERNANCE_MATCH_EXACT, GOVERNANCE_MATCH_MAXIMUM,
            GOVERNANCE_MATCH_MINIMUM,
        };

        let values = [
            (
                self.channel.map(|value| value as u64),
                Some(input.channel as u64),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.principal_kind.map(|value| value as u64),
                Some(input.principal.kind as u64),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.minimum_role.map(|value| value as u64),
                Some(input.principal.role as u64),
                GOVERNANCE_MATCH_MINIMUM,
            ),
            (
                self.credential_scope.map(|value| value as u64),
                Some(input.credential_scope as u64),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.action.map(|value| value as u64),
                Some(input.action as u64),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.minimum_classification.map(|value| value as u64),
                Some(input.data.classification as u64),
                GOVERNANCE_MATCH_MINIMUM,
            ),
            (
                self.inspection_coverage.map(|value| value as u64),
                Some(input.data.inspection_coverage as u64),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.minimum_request_risk.map(|value| value as u64),
                Some(input.request_risk as u64),
                GOVERNANCE_MATCH_MINIMUM,
            ),
            (
                self.network_zone.map(|value| value as u64),
                Some(input.environment.network_zone as u64),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.maximum_session_age_seconds,
                Some(input.session.age_seconds),
                GOVERNANCE_MATCH_MAXIMUM,
            ),
            (
                self.maximum_session_idle_seconds,
                Some(input.session.idle_seconds),
                GOVERNANCE_MATCH_MAXIMUM,
            ),
            (
                self.session_revoked.map(u64::from),
                Some(u64::from(input.session.revoked)),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.session_mfa_satisfied.map(u64::from),
                Some(u64::from(input.session.mfa_satisfied)),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.minimum_session_retained_classification
                    .map(|value| value as u64),
                Some(input.session.retained_classification as u64),
                GOVERNANCE_MATCH_MINIMUM,
            ),
            (
                self.minimum_authentication_strength.map(u64::from),
                Some(u64::from(input.environment.authentication_strength)),
                GOVERNANCE_MATCH_MINIMUM,
            ),
            (
                self.environment_mfa_satisfied.map(u64::from),
                Some(u64::from(input.environment.mfa_satisfied)),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.requested_capability.map(model_capability_bit),
                Some(model_capability_mask(input.requested_capabilities)),
                GOVERNANCE_MATCH_CONTAINS,
            ),
            (
                self.requested_modality.map(data_modality_bit),
                Some(data_modality_mask(
                    input.request_attributes.requested_modalities(),
                )),
                GOVERNANCE_MATCH_CONTAINS,
            ),
            (
                self.break_glass_required.map(u64::from),
                Some(u64::from(
                    input.request_attributes.valid_break_glass().is_some(),
                )),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.quota_has_headroom.map(u64::from),
                Some(u64::from(input.quota.has_headroom)),
                GOVERNANCE_MATCH_EXACT,
            ),
            (
                self.quota_reservation_required.map(u64::from),
                Some(u64::from(input.quota.reservation_required)),
                GOVERNANCE_MATCH_EXACT,
            ),
        ];
        let team_id = input.principal_attributes.team_id();
        let project_id = input.principal_attributes.project_id();
        let user_id = input.principal_attributes.user_id();
        let group_ids = input.principal_attributes.group_ids().collect::<Vec<_>>();
        let department_id = input.principal_attributes.department_id();
        let route = [input.route.as_str()];
        let requested_model = input.request_attributes.requested_model();
        let requested_tools = input
            .request_attributes
            .requested_tools()
            .collect::<Vec<_>>();
        let break_glass_scope = input
            .request_attributes
            .valid_break_glass()
            .map(|grant| grant.scope.as_str());
        let selectors = [
            (
                self.team_id.as_ref().map(PolicySelector::as_str),
                team_id.as_slice(),
                true,
            ),
            (
                self.project_id.as_ref().map(PolicySelector::as_str),
                project_id.as_slice(),
                true,
            ),
            (
                self.user_id.as_ref().map(PolicySelector::as_str),
                user_id.as_slice(),
                true,
            ),
            (
                self.group_id.as_ref().map(PolicySelector::as_str),
                group_ids.as_slice(),
                true,
            ),
            (
                self.department_id.as_ref().map(PolicySelector::as_str),
                department_id.as_slice(),
                true,
            ),
            (
                self.route.as_ref().map(CanonicalRoute::as_str),
                route.as_slice(),
                false,
            ),
            (
                self.requested_model.as_ref().map(PolicySelector::as_str),
                requested_model.as_slice(),
                true,
            ),
            (
                self.requested_tool.as_ref().map(PolicySelector::as_str),
                requested_tools.as_slice(),
                true,
            ),
            (
                self.break_glass_scope.as_ref().map(PolicySelector::as_str),
                break_glass_scope.as_slice(),
                true,
            ),
        ];
        prodex_mojo_core::policy::governance_policy_rule_matches(&values, &selectors)
            .expect("Mojo governance condition matcher returned invalid output")
    }

    #[cfg(not(feature = "mojo"))]
    fn matches(&self, input: &PolicyInput<'_>) -> bool {
        self.matches_rust(input)
    }

    #[cfg(any(test, not(feature = "mojo")))]
    fn matches_rust(&self, input: &PolicyInput<'_>) -> bool {
        self.channel.is_none_or(|value| value == input.channel)
            && self
                .principal_kind
                .is_none_or(|value| value == input.principal.kind)
            && self.team_id.as_ref().is_none_or(|selector| {
                input
                    .principal_attributes
                    .team_id()
                    .is_some_and(|value| selector_matches(selector, value))
            })
            && self.project_id.as_ref().is_none_or(|selector| {
                input
                    .principal_attributes
                    .project_id()
                    .is_some_and(|value| selector_matches(selector, value))
            })
            && self.user_id.as_ref().is_none_or(|selector| {
                input
                    .principal_attributes
                    .user_id()
                    .is_some_and(|value| selector_matches(selector, value))
            })
            && self.group_id.as_ref().is_none_or(|selector| {
                input
                    .principal_attributes
                    .group_ids()
                    .any(|value| selector_matches(selector, value))
            })
            && self.department_id.as_ref().is_none_or(|selector| {
                input
                    .principal_attributes
                    .department_id()
                    .is_some_and(|value| selector_matches(selector, value))
            })
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
            && self.requested_model.as_ref().is_none_or(|selector| {
                input
                    .request_attributes
                    .requested_model()
                    .is_some_and(|value| selector_matches(selector, value))
            })
            && self.requested_tool.as_ref().is_none_or(|selector| {
                input
                    .request_attributes
                    .requested_tools()
                    .any(|value| selector_matches(selector, value))
            })
            && self.requested_modality.is_none_or(|modality| {
                input
                    .request_attributes
                    .requested_modalities()
                    .contains(&modality)
            })
            && self.break_glass_required.is_none_or(|required| {
                input.request_attributes.valid_break_glass().is_some() == required
            })
            && self.break_glass_scope.as_ref().is_none_or(|selector| {
                input
                    .request_attributes
                    .valid_break_glass()
                    .is_some_and(|grant| selector_matches(selector, grant.scope.as_str()))
            })
            && self
                .quota_has_headroom
                .is_none_or(|value| value == input.quota.has_headroom)
            && self
                .quota_reservation_required
                .is_none_or(|value| value == input.quota.reservation_required)
    }
}

#[cfg(feature = "mojo")]
fn model_capability_bit(value: ModelCapability) -> u64 {
    1_u64 << value as usize
}

#[cfg(feature = "mojo")]
fn model_capability_mask(values: &CapabilitySet) -> u64 {
    values
        .as_slice()
        .iter()
        .fold(0, |mask, value| mask | model_capability_bit(*value))
}

#[cfg(feature = "mojo")]
fn data_modality_bit(value: DataModality) -> u64 {
    1_u64 << value as usize
}

#[cfg(feature = "mojo")]
fn data_modality_mask(values: &[DataModality]) -> u64 {
    values
        .iter()
        .fold(0, |mask, value| mask | data_modality_bit(*value))
}

#[cfg(feature = "mojo")]
fn selector_matches(selector: &PolicySelector, value: &str) -> bool {
    prodex_mojo_core::policy::governance_selector_matches(selector.as_str(), value)
        .expect("Mojo governance selector predicate returned invalid output")
}

#[cfg(any(test, not(feature = "mojo")))]
fn selector_matches_rust(selector: &PolicySelector, value: &str) -> bool {
    selector.as_str() == "*" || selector.as_str() == value
}

#[cfg(not(feature = "mojo"))]
fn selector_matches(selector: &PolicySelector, value: &str) -> bool {
    selector_matches_rust(selector, value)
}

impl fmt::Debug for PolicyRuleCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyRuleCondition")
            .field("channel", &self.channel)
            .field("principal_kind", &self.principal_kind)
            .field("team_id", &self.team_id.as_ref().map(|_| "<redacted>"))
            .field(
                "project_id",
                &self.project_id.as_ref().map(|_| "<redacted>"),
            )
            .field("user_id", &self.user_id.as_ref().map(|_| "<redacted>"))
            .field("group_id", &self.group_id.as_ref().map(|_| "<redacted>"))
            .field(
                "department_id",
                &self.department_id.as_ref().map(|_| "<redacted>"),
            )
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
            .field(
                "requested_model",
                &self.requested_model.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "requested_tool",
                &self.requested_tool.as_ref().map(|_| "<redacted>"),
            )
            .field("requested_modality", &self.requested_modality)
            .field("break_glass_required", &self.break_glass_required)
            .field(
                "break_glass_scope",
                &self.break_glass_scope.as_ref().map(|_| "<redacted>"),
            )
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
    validate_governance_obligation_conflicts(&artifact.rules)?;
    Ok(CompiledGovernancePolicy {
        revision: artifact.revision,
        valid_until_unix_ms: artifact.valid_until_unix_ms,
        default_effect: artifact.default_effect,
        rules: artifact.rules,
    })
}

fn validate_governance_obligation_conflicts(
    rules: &[GovernancePolicyRule],
) -> Result<(), GovernancePolicyError> {
    for (index, rule) in rules.iter().enumerate() {
        if rule
            .obligations
            .iter()
            .any(governance_obligation_bound_is_invalid)
        {
            return Err(GovernancePolicyError::ConflictingObligations);
        }
        for (left_index, left) in rule.obligations.iter().enumerate() {
            if rule.obligations[left_index + 1..]
                .iter()
                .any(|right| governance_obligations_conflict(left, right))
            {
                return Err(GovernancePolicyError::ConflictingObligations);
            }
        }
        for prior in &rules[..index] {
            if policy_rule_conditions_overlap(&rule.condition, &prior.condition)
                && rule.obligations.iter().any(|left| {
                    prior
                        .obligations
                        .iter()
                        .any(|right| governance_obligations_conflict(left, right))
                })
            {
                return Err(GovernancePolicyError::ConflictingObligations);
            }
        }
    }
    Ok(())
}

fn governance_obligation_bound_is_invalid(obligation: &GovernanceObligation) -> bool {
    matches!(
        obligation,
        GovernanceObligation::MaxInputTokens(0)
            | GovernanceObligation::MaxOutputTokens(0)
            | GovernanceObligation::MaxContextTokens(0)
            | GovernanceObligation::SessionIdleTimeoutSeconds(0)
            | GovernanceObligation::SessionAbsoluteTimeoutSeconds(0)
            | GovernanceObligation::MinimumAuthenticationStrength(0)
    )
}

fn governance_obligations_conflict(
    left: &GovernanceObligation,
    right: &GovernanceObligation,
) -> bool {
    use GovernanceObligation::{
        AllowProvider, AllowTool, DenyProvider, DisableTools, MaxContextTokens, MaxInputTokens,
        MaxOutputTokens, ProhibitRetention, RequireRegion, RetentionSeconds,
    };

    match (left, right) {
        (AllowProvider(allowed), DenyProvider(denied))
        | (DenyProvider(denied), AllowProvider(allowed)) => {
            denied.as_str() == "*" || allowed == denied
        }
        (RequireRegion(left), RequireRegion(right)) => {
            left.as_str() != "*" && right.as_str() != "*" && left != right
        }
        (ProhibitRetention, RetentionSeconds(seconds))
        | (RetentionSeconds(seconds), ProhibitRetention) => *seconds != 0,
        (DisableTools, AllowTool(_)) | (AllowTool(_), DisableTools) => true,
        (MaxInputTokens(limit), MaxContextTokens(context))
        | (MaxContextTokens(context), MaxInputTokens(limit))
        | (MaxOutputTokens(limit), MaxContextTokens(context))
        | (MaxContextTokens(context), MaxOutputTokens(limit)) => limit > context,
        _ => false,
    }
}

#[cfg(feature = "mojo")]
fn policy_rule_conditions_overlap(left: &PolicyRuleCondition, right: &PolicyRuleCondition) -> bool {
    let values = [
        (
            left.channel.map(|value| value as i64),
            right.channel.map(|value| value as i64),
        ),
        (
            left.principal_kind.map(|value| value as i64),
            right.principal_kind.map(|value| value as i64),
        ),
        (
            left.credential_scope.map(|value| value as i64),
            right.credential_scope.map(|value| value as i64),
        ),
        (
            left.action.map(|value| value as i64),
            right.action.map(|value| value as i64),
        ),
        (
            left.inspection_coverage.map(|value| value as i64),
            right.inspection_coverage.map(|value| value as i64),
        ),
        (
            left.network_zone.map(|value| value as i64),
            right.network_zone.map(|value| value as i64),
        ),
        (
            left.session_revoked.map(i64::from),
            right.session_revoked.map(i64::from),
        ),
        (
            left.session_mfa_satisfied.map(i64::from),
            right.session_mfa_satisfied.map(i64::from),
        ),
        (
            left.environment_mfa_satisfied.map(i64::from),
            right.environment_mfa_satisfied.map(i64::from),
        ),
        (
            left.requested_modality.map(|value| value as i64),
            right.requested_modality.map(|value| value as i64),
        ),
        (
            left.break_glass_required.map(i64::from),
            right.break_glass_required.map(i64::from),
        ),
        (
            left.quota_has_headroom.map(i64::from),
            right.quota_has_headroom.map(i64::from),
        ),
        (
            left.quota_reservation_required.map(i64::from),
            right.quota_reservation_required.map(i64::from),
        ),
    ];
    let exact_selectors = [(
        left.route.as_ref().map(CanonicalRoute::as_str),
        right.route.as_ref().map(CanonicalRoute::as_str),
    )];
    let wildcard_selectors = [
        policy_selector_pair(&left.team_id, &right.team_id),
        policy_selector_pair(&left.project_id, &right.project_id),
        policy_selector_pair(&left.user_id, &right.user_id),
        policy_selector_pair(&left.department_id, &right.department_id),
        policy_selector_pair(&left.requested_model, &right.requested_model),
        policy_selector_pair(&left.requested_tool, &right.requested_tool),
        policy_selector_pair(&left.break_glass_scope, &right.break_glass_scope),
    ];
    prodex_mojo_core::policy::governance_policy_conditions_overlap(
        &values,
        &exact_selectors,
        &wildcard_selectors,
    )
    .expect("Mojo governance overlap predicate returned invalid output")
}

#[cfg(feature = "mojo")]
fn policy_selector_pair<'a>(
    left: &'a Option<PolicySelector>,
    right: &'a Option<PolicySelector>,
) -> (Option<&'a str>, Option<&'a str>) {
    (
        left.as_ref().map(PolicySelector::as_str),
        right.as_ref().map(PolicySelector::as_str),
    )
}

#[cfg(any(test, not(feature = "mojo")))]
fn policy_rule_conditions_overlap_rust(
    left: &PolicyRuleCondition,
    right: &PolicyRuleCondition,
) -> bool {
    optional_policy_attributes_overlap(&left.channel, &right.channel)
        && optional_policy_attributes_overlap(&left.principal_kind, &right.principal_kind)
        && policy_selectors_overlap(&left.team_id, &right.team_id)
        && policy_selectors_overlap(&left.project_id, &right.project_id)
        && policy_selectors_overlap(&left.user_id, &right.user_id)
        && policy_selectors_overlap(&left.department_id, &right.department_id)
        && optional_policy_attributes_overlap(&left.credential_scope, &right.credential_scope)
        && optional_policy_attributes_overlap(&left.action, &right.action)
        && optional_policy_attributes_overlap(&left.route, &right.route)
        && optional_policy_attributes_overlap(&left.inspection_coverage, &right.inspection_coverage)
        && optional_policy_attributes_overlap(&left.network_zone, &right.network_zone)
        && optional_policy_attributes_overlap(&left.session_revoked, &right.session_revoked)
        && optional_policy_attributes_overlap(
            &left.session_mfa_satisfied,
            &right.session_mfa_satisfied,
        )
        && optional_policy_attributes_overlap(
            &left.environment_mfa_satisfied,
            &right.environment_mfa_satisfied,
        )
        && policy_selectors_overlap(&left.requested_model, &right.requested_model)
        && policy_selectors_overlap(&left.requested_tool, &right.requested_tool)
        && optional_policy_attributes_overlap(&left.requested_modality, &right.requested_modality)
        && optional_policy_attributes_overlap(
            &left.break_glass_required,
            &right.break_glass_required,
        )
        && policy_selectors_overlap(&left.break_glass_scope, &right.break_glass_scope)
        && optional_policy_attributes_overlap(&left.quota_has_headroom, &right.quota_has_headroom)
        && optional_policy_attributes_overlap(
            &left.quota_reservation_required,
            &right.quota_reservation_required,
        )
}

#[cfg(not(feature = "mojo"))]
fn policy_rule_conditions_overlap(left: &PolicyRuleCondition, right: &PolicyRuleCondition) -> bool {
    policy_rule_conditions_overlap_rust(left, right)
}

#[cfg(any(test, not(feature = "mojo")))]
fn policy_selectors_overlap(left: &Option<PolicySelector>, right: &Option<PolicySelector>) -> bool {
    !matches!(
        (left, right),
        (Some(left), Some(right))
            if left.as_str() != "*" && right.as_str() != "*" && left != right
    )
}

#[cfg(any(test, not(feature = "mojo")))]
fn optional_policy_attributes_overlap<T: PartialEq>(left: &Option<T>, right: &Option<T>) -> bool {
    !matches!((left, right), (Some(left), Some(right)) if left != right)
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
        || !policy_required_attributes_present(policy, input)
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

    let mut effect = PolicyEffect::Allow;
    let mut obligations = Vec::new();
    let mut reasons = Vec::new();
    for rule in policy
        .rules
        .iter()
        .filter(|rule| rule.condition.matches(input))
    {
        effect = effect.max(rule.effect);
        obligations.extend(rule.obligations.iter().cloned());
        reasons.push(rule.reason_code.clone());
    }
    if reasons.is_empty() {
        effect = policy.default_effect;
        reasons.push(PolicyReasonCode::new("policy.default")?);
    }
    obligations.sort();
    obligations.dedup();
    reasons.sort();
    reasons.dedup();
    if obligations.len() > MAX_POLICY_OBLIGATIONS {
        return Err(GovernancePolicyError::ObligationLimitExceeded);
    }
    if reasons.len() > MAX_POLICY_REASON_CODES {
        return Err(GovernancePolicyError::ReasonLimitExceeded);
    }
    if effect == PolicyEffect::Deny {
        obligations.clear();
    }
    Ok(PolicyDecision {
        effect,
        obligations,
        reason_codes: reasons,
        policy_revision: policy.revision,
        valid_until_unix_ms: policy.valid_until_unix_ms,
    })
}

#[cfg(feature = "mojo")]
fn policy_required_attributes_present(
    policy: &CompiledGovernancePolicy,
    input: &PolicyInput<'_>,
) -> bool {
    let available_mask = u64::from(input.principal_attributes.team_id().is_some())
        | u64::from(input.principal_attributes.project_id().is_some()) << 1
        | u64::from(input.principal_attributes.user_id().is_some()) << 2
        | u64::from(input.principal_attributes.group_ids().next().is_some()) << 3
        | u64::from(input.principal_attributes.department_id().is_some()) << 4
        | u64::from(input.request_attributes.requested_model().is_some()) << 5
        | u64::from(input.request_attributes.requested_tools().next().is_some()) << 6
        | u64::from(!input.request_attributes.requested_modalities().is_empty()) << 7
        | u64::from(input.request_attributes.valid_break_glass().is_some()) << 8;
    let required_masks = policy
        .rules
        .iter()
        .map(|rule| {
            u64::from(rule.condition.team_id.is_some())
                | u64::from(rule.condition.project_id.is_some()) << 1
                | u64::from(rule.condition.user_id.is_some()) << 2
                | u64::from(rule.condition.group_id.is_some()) << 3
                | u64::from(rule.condition.department_id.is_some()) << 4
                | u64::from(rule.condition.requested_model.is_some()) << 5
                | u64::from(rule.condition.requested_tool.is_some()) << 6
                | u64::from(rule.condition.requested_modality.is_some()) << 7
                | u64::from(
                    rule.condition.break_glass_scope.is_some()
                        || rule.condition.break_glass_required == Some(true),
                ) << 8
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::policy::governance_required_attributes_present(
        &required_masks,
        available_mask,
    )
    .expect("Mojo governance required-attribute predicate returned invalid output")
}

#[cfg(not(feature = "mojo"))]
fn policy_required_attributes_present(
    policy: &CompiledGovernancePolicy,
    input: &PolicyInput<'_>,
) -> bool {
    policy_required_attributes_present_rust(policy, input)
}

#[cfg(any(test, not(feature = "mojo")))]
fn policy_required_attributes_present_rust(
    policy: &CompiledGovernancePolicy,
    input: &PolicyInput<'_>,
) -> bool {
    policy.rules.iter().all(|rule| {
        (rule.condition.team_id.is_none() || input.principal_attributes.team_id().is_some())
            && (rule.condition.project_id.is_none()
                || input.principal_attributes.project_id().is_some())
            && (rule.condition.user_id.is_none() || input.principal_attributes.user_id().is_some())
            && (rule.condition.group_id.is_none()
                || input.principal_attributes.group_ids().next().is_some())
            && (rule.condition.department_id.is_none()
                || input.principal_attributes.department_id().is_some())
            && (rule.condition.requested_model.is_none()
                || input.request_attributes.requested_model().is_some())
            && (rule.condition.requested_tool.is_none()
                || input.request_attributes.requested_tools().next().is_some())
            && (rule.condition.requested_modality.is_none()
                || !input.request_attributes.requested_modalities().is_empty())
            && (rule.condition.break_glass_scope.is_none()
                || input.request_attributes.valid_break_glass().is_some())
            && (rule.condition.break_glass_required != Some(true)
                || input.request_attributes.valid_break_glass().is_some())
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
    ConflictingObligations,
    ReasonLimitExceeded,
    AttributeLimitExceeded,
}

impl fmt::Display for GovernancePolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "governance policy is invalid")
    }
}

impl Error for GovernancePolicyError {}

#[cfg(all(test, feature = "mojo"))]
mod governance_predicate_tests {
    use super::*;

    #[test]
    fn mojo_selector_predicates_match_rust_oracle() {
        for (selector, value) in [("*", "model-a"), ("model-a", "model-a"), ("model-a", "*")] {
            let selector = PolicySelector::new(selector).expect("valid test selector");
            assert_eq!(
                selector_matches(&selector, value),
                selector_matches_rust(&selector, value)
            );
        }
    }

    #[test]
    fn mojo_condition_overlap_matches_rust_oracle() {
        let wildcard = PolicySelector::new("*").expect("valid wildcard");
        let exact = PolicySelector::new("team-a").expect("valid selector");
        let cases = [
            (
                PolicyRuleCondition::default(),
                PolicyRuleCondition::default(),
            ),
            (
                PolicyRuleCondition {
                    channel: Some(Channel::Api),
                    ..PolicyRuleCondition::default()
                },
                PolicyRuleCondition {
                    channel: Some(Channel::Cli),
                    ..PolicyRuleCondition::default()
                },
            ),
            (
                PolicyRuleCondition {
                    team_id: Some(wildcard),
                    ..PolicyRuleCondition::default()
                },
                PolicyRuleCondition {
                    team_id: Some(exact),
                    ..PolicyRuleCondition::default()
                },
            ),
            (
                PolicyRuleCondition {
                    route: Some(CanonicalRoute::new("route-a").expect("valid route")),
                    ..PolicyRuleCondition::default()
                },
                PolicyRuleCondition {
                    route: Some(CanonicalRoute::new("route-b").expect("valid route")),
                    ..PolicyRuleCondition::default()
                },
            ),
        ];
        for (left, right) in cases {
            assert_eq!(
                policy_rule_conditions_overlap(&left, &right),
                policy_rule_conditions_overlap_rust(&left, &right)
            );
        }
    }
}

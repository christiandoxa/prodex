//! Bounded, side-effect-free governance policy decisions.

use std::error::Error;
use std::fmt;

use serde::Serialize;

use crate::{CapabilitySet, CredentialScope, PolicyRevisionId, Principal, TenantContext};

use super::{DataClassification, FindingKind, InspectionCoverage};

mod compiled_policy;
mod condition;
mod principal_attributes;
pub use compiled_policy::{
    CompiledGovernancePolicy, GovernancePolicyArtifact, GovernancePolicyRule,
    compile_governance_policy,
};
pub use condition::PolicyRuleCondition;
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

    let rule_matches = policy
        .rules
        .iter()
        .map(|rule| (rule.condition.matches(input), rule.effect))
        .collect::<Vec<_>>();
    let (effect, matched) = governance_policy_decision_plan(&rule_matches, policy.default_effect);
    let mut obligations = Vec::new();
    let mut reasons = Vec::new();
    for rule in policy
        .rules
        .iter()
        .zip(&matched)
        .filter_map(|(rule, matched)| matched.then_some(rule))
    {
        obligations.extend(rule.obligations.iter().cloned());
        reasons.push(rule.reason_code.clone());
    }
    if !matched.iter().any(|matched| *matched) {
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

#[cfg(any())]
fn governance_policy_decision_plan(
    rules: &[(bool, PolicyEffect)],
    default_effect: PolicyEffect,
) -> (PolicyEffect, Vec<bool>) {
    let rules = rules
        .iter()
        .map(|(matched, effect)| (*matched, *effect as i64))
        .collect::<Vec<_>>();
    let (effect, matched) =
        prodex_mojo_core::policy::governance_policy_decision_plan(&rules, default_effect as i64)
            .expect("Mojo governance policy decision plan returned invalid output");
    (policy_effect_from_i64(effect), matched)
}

fn governance_policy_decision_plan(
    rules: &[(bool, PolicyEffect)],
    default_effect: PolicyEffect,
) -> (PolicyEffect, Vec<bool>) {
    governance_policy_decision_plan_rust(rules, default_effect)
}

fn governance_policy_decision_plan_rust(
    rules: &[(bool, PolicyEffect)],
    default_effect: PolicyEffect,
) -> (PolicyEffect, Vec<bool>) {
    let matched = rules
        .iter()
        .map(|(matched, _)| *matched)
        .collect::<Vec<_>>();
    let effects = rules
        .iter()
        .filter_map(|(matched, effect)| matched.then_some(*effect))
        .collect::<Vec<_>>();
    (
        governance_policy_effect_rust(&effects, default_effect),
        matched,
    )
}

#[cfg(any())]
fn policy_effect_from_i64(effect: i64) -> PolicyEffect {
    match effect {
        0 => PolicyEffect::Allow,
        1 => PolicyEffect::RequireApproval,
        2 => PolicyEffect::Deny,
        _ => unreachable!("validated Mojo governance policy effect"),
    }
}

fn governance_policy_effect_rust(
    effects: &[PolicyEffect],
    default_effect: PolicyEffect,
) -> PolicyEffect {
    effects.iter().copied().max().unwrap_or(default_effect)
}

#[cfg(any())]
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

fn policy_required_attributes_present(
    policy: &CompiledGovernancePolicy,
    input: &PolicyInput<'_>,
) -> bool {
    policy_required_attributes_present_rust(policy, input)
}

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

#[cfg(any())]
mod governance_predicate_tests {
    use super::*;
    use super::{
        compiled_policy::{
            policy_rule_conditions_overlap, policy_rule_conditions_overlap_rust,
            validate_governance_policy_shape, validate_governance_policy_shape_rust,
        },
        condition::{selector_matches, selector_matches_rust},
    };

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

    #[test]
    fn mojo_policy_shape_matches_rust_oracle() {
        let rule = |id: &str, effect, obligations| GovernancePolicyRule {
            id: GovernancePolicyRuleId::new(id).expect("valid rule id"),
            condition: PolicyRuleCondition::default(),
            effect,
            obligations,
            reason_code: PolicyReasonCode::new("policy.test").expect("valid reason"),
        };
        let mut cases = vec![
            (1, vec![rule("a", PolicyEffect::Allow, Vec::new())]),
            (0, Vec::new()),
            (
                1,
                vec![
                    rule("a", PolicyEffect::Allow, Vec::new()),
                    rule("a", PolicyEffect::Allow, Vec::new()),
                ],
            ),
            (
                1,
                vec![rule(
                    "a",
                    PolicyEffect::Deny,
                    vec![GovernanceObligation::RequireMfa],
                )],
            ),
            (
                1,
                vec![
                    rule(
                        "a",
                        PolicyEffect::Deny,
                        vec![GovernanceObligation::RequireMfa],
                    ),
                    rule("a", PolicyEffect::Allow, Vec::new()),
                ],
            ),
            (
                1,
                vec![rule(
                    "a",
                    PolicyEffect::Allow,
                    vec![GovernanceObligation::RequireMfa; MAX_POLICY_OBLIGATIONS + 1],
                )],
            ),
        ];
        cases.push((
            1,
            (0..=MAX_GOVERNANCE_POLICY_RULES)
                .map(|index| rule(&format!("rule-{index}"), PolicyEffect::Allow, Vec::new()))
                .collect(),
        ));
        for (valid_until_unix_ms, rules) in cases {
            assert_eq!(
                validate_governance_policy_shape(valid_until_unix_ms, &rules),
                validate_governance_policy_shape_rust(valid_until_unix_ms, &rules)
            );
        }
    }

    #[test]
    fn mojo_policy_decision_plan_matches_rust_oracle() {
        for (rules, default_effect) in [
            (Vec::new(), PolicyEffect::Deny),
            (
                vec![
                    (true, PolicyEffect::Allow),
                    (false, PolicyEffect::Deny),
                    (true, PolicyEffect::RequireApproval),
                ],
                PolicyEffect::Deny,
            ),
        ] {
            assert_eq!(
                governance_policy_decision_plan(&rules, default_effect),
                governance_policy_decision_plan_rust(&rules, default_effect)
            );
        }
    }
}

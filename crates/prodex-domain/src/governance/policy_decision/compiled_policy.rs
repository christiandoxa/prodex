use std::fmt;

use crate::{GovernancePolicyRuleId, PolicyReasonCode, PolicyRevisionId};

#[cfg(feature = "mojo")]
use super::CanonicalRoute;
use super::{
    GovernanceObligation, GovernancePolicyError, PolicyEffect, PolicyRuleCondition, PolicySelector,
};
#[cfg(any(test, not(feature = "mojo")))]
use super::{MAX_GOVERNANCE_POLICY_RULES, MAX_POLICY_OBLIGATIONS};

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
    pub(super) revision: PolicyRevisionId,
    pub(super) valid_until_unix_ms: u64,
    pub(super) default_effect: PolicyEffect,
    pub(super) rules: Vec<GovernancePolicyRule>,
}

pub fn compile_governance_policy(
    mut artifact: GovernancePolicyArtifact,
) -> Result<CompiledGovernancePolicy, GovernancePolicyError> {
    artifact.rules.sort_by(|left, right| left.id.cmp(&right.id));
    validate_governance_policy_shape(artifact.valid_until_unix_ms, &artifact.rules)?;
    for rule in &mut artifact.rules {
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

#[cfg(feature = "mojo")]
pub(super) fn validate_governance_policy_shape(
    valid_until_unix_ms: u64,
    rules: &[GovernancePolicyRule],
) -> Result<(), GovernancePolicyError> {
    let rules = rules
        .iter()
        .map(|rule| (rule.id.as_str(), rule.effect as i64, rule.obligations.len()))
        .collect::<Vec<_>>();
    match prodex_mojo_core::policy::governance_policy_shape(valid_until_unix_ms, &rules)
        .expect("Mojo governance policy shape returned invalid output")
    {
        0 => Ok(()),
        1 => Err(GovernancePolicyError::InvalidExpiry),
        2 => Err(GovernancePolicyError::DuplicateRule),
        3 => Err(GovernancePolicyError::ObligationLimitExceeded),
        4 => Err(GovernancePolicyError::DenyRuleHasObligations),
        5 => Err(GovernancePolicyError::RuleLimitExceeded),
        _ => unreachable!("validated Mojo governance policy shape output"),
    }
}

#[cfg(not(feature = "mojo"))]
pub(super) fn validate_governance_policy_shape(
    valid_until_unix_ms: u64,
    rules: &[GovernancePolicyRule],
) -> Result<(), GovernancePolicyError> {
    validate_governance_policy_shape_rust(valid_until_unix_ms, rules)
}

#[cfg(any(test, not(feature = "mojo")))]
pub(super) fn validate_governance_policy_shape_rust(
    valid_until_unix_ms: u64,
    rules: &[GovernancePolicyRule],
) -> Result<(), GovernancePolicyError> {
    if rules.len() > MAX_GOVERNANCE_POLICY_RULES {
        return Err(GovernancePolicyError::RuleLimitExceeded);
    }
    if valid_until_unix_ms == 0 {
        return Err(GovernancePolicyError::InvalidExpiry);
    }
    if rules.windows(2).any(|rules| rules[0].id == rules[1].id) {
        return Err(GovernancePolicyError::DuplicateRule);
    }
    for rule in rules {
        if rule.obligations.len() > MAX_POLICY_OBLIGATIONS {
            return Err(GovernancePolicyError::ObligationLimitExceeded);
        }
        if rule.effect == PolicyEffect::Deny && !rule.obligations.is_empty() {
            return Err(GovernancePolicyError::DenyRuleHasObligations);
        }
    }
    Ok(())
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

#[cfg(feature = "mojo")]
fn governance_obligation_bound_is_invalid(obligation: &GovernanceObligation) -> bool {
    prodex_mojo_core::policy::governance_obligation_bound_is_invalid(governance_obligation_input(
        obligation,
    ))
    .expect("Mojo governance obligation bound predicate returned invalid output")
}

#[cfg(not(feature = "mojo"))]
fn governance_obligation_bound_is_invalid(obligation: &GovernanceObligation) -> bool {
    governance_obligation_bound_is_invalid_rust(obligation)
}

#[cfg(any(test, not(feature = "mojo")))]
fn governance_obligation_bound_is_invalid_rust(obligation: &GovernanceObligation) -> bool {
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

#[cfg(feature = "mojo")]
fn governance_obligations_conflict(
    left: &GovernanceObligation,
    right: &GovernanceObligation,
) -> bool {
    prodex_mojo_core::policy::governance_obligations_conflict(
        governance_obligation_input(left),
        governance_obligation_input(right),
    )
    .expect("Mojo governance obligation conflict predicate returned invalid output")
}

#[cfg(feature = "mojo")]
fn governance_obligation_input(obligation: &GovernanceObligation) -> (i64, u64, Option<&str>) {
    use prodex_mojo_core::policy::{
        GOVERNANCE_OBLIGATION_ALLOW_PROVIDER, GOVERNANCE_OBLIGATION_ALLOW_TOOL,
        GOVERNANCE_OBLIGATION_DENY_PROVIDER, GOVERNANCE_OBLIGATION_DISABLE_TOOLS,
        GOVERNANCE_OBLIGATION_MAX_CONTEXT, GOVERNANCE_OBLIGATION_MAX_INPUT,
        GOVERNANCE_OBLIGATION_MAX_OUTPUT, GOVERNANCE_OBLIGATION_MIN_AUTHENTICATION,
        GOVERNANCE_OBLIGATION_OTHER, GOVERNANCE_OBLIGATION_PROHIBIT_RETENTION,
        GOVERNANCE_OBLIGATION_REQUIRE_REGION, GOVERNANCE_OBLIGATION_RETENTION_SECONDS,
        GOVERNANCE_OBLIGATION_SESSION_ABSOLUTE, GOVERNANCE_OBLIGATION_SESSION_IDLE,
    };

    match obligation {
        GovernanceObligation::AllowProvider(selector) => (
            GOVERNANCE_OBLIGATION_ALLOW_PROVIDER,
            0,
            Some(selector.as_str()),
        ),
        GovernanceObligation::DenyProvider(selector) => (
            GOVERNANCE_OBLIGATION_DENY_PROVIDER,
            0,
            Some(selector.as_str()),
        ),
        GovernanceObligation::RequireRegion(selector) => (
            GOVERNANCE_OBLIGATION_REQUIRE_REGION,
            0,
            Some(selector.as_str()),
        ),
        GovernanceObligation::ProhibitRetention => {
            (GOVERNANCE_OBLIGATION_PROHIBIT_RETENTION, 0, None)
        }
        GovernanceObligation::RetentionSeconds(value) => (
            GOVERNANCE_OBLIGATION_RETENTION_SECONDS,
            u64::from(*value),
            None,
        ),
        GovernanceObligation::DisableTools => (GOVERNANCE_OBLIGATION_DISABLE_TOOLS, 0, None),
        GovernanceObligation::AllowTool(selector) => {
            (GOVERNANCE_OBLIGATION_ALLOW_TOOL, 0, Some(selector.as_str()))
        }
        GovernanceObligation::MaxInputTokens(value) => {
            (GOVERNANCE_OBLIGATION_MAX_INPUT, u64::from(*value), None)
        }
        GovernanceObligation::MaxOutputTokens(value) => {
            (GOVERNANCE_OBLIGATION_MAX_OUTPUT, u64::from(*value), None)
        }
        GovernanceObligation::MaxContextTokens(value) => {
            (GOVERNANCE_OBLIGATION_MAX_CONTEXT, u64::from(*value), None)
        }
        GovernanceObligation::SessionIdleTimeoutSeconds(value) => {
            (GOVERNANCE_OBLIGATION_SESSION_IDLE, u64::from(*value), None)
        }
        GovernanceObligation::SessionAbsoluteTimeoutSeconds(value) => (
            GOVERNANCE_OBLIGATION_SESSION_ABSOLUTE,
            u64::from(*value),
            None,
        ),
        GovernanceObligation::MinimumAuthenticationStrength(value) => (
            GOVERNANCE_OBLIGATION_MIN_AUTHENTICATION,
            u64::from(*value),
            None,
        ),
        GovernanceObligation::MaskFinding(_)
        | GovernanceObligation::MinimumProviderTrust(_)
        | GovernanceObligation::RequireLocalExecution
        | GovernanceObligation::ProhibitTrainingUse
        | GovernanceObligation::AllowModel(_)
        | GovernanceObligation::AllowModality(_)
        | GovernanceObligation::RequireResponseInspection
        | GovernanceObligation::RequireReauthentication
        | GovernanceObligation::RequireMfa
        | GovernanceObligation::AuditDetail(_)
        | GovernanceObligation::RequireHumanApproval
        | GovernanceObligation::DenyFallbackOutsideEligibility => {
            (GOVERNANCE_OBLIGATION_OTHER, 0, None)
        }
    }
}

#[cfg(not(feature = "mojo"))]
fn governance_obligations_conflict(
    left: &GovernanceObligation,
    right: &GovernanceObligation,
) -> bool {
    governance_obligations_conflict_rust(left, right)
}

#[cfg(any(test, not(feature = "mojo")))]
fn governance_obligations_conflict_rust(
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
pub(super) fn policy_rule_conditions_overlap(
    left: &PolicyRuleCondition,
    right: &PolicyRuleCondition,
) -> bool {
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
pub(super) fn policy_rule_conditions_overlap_rust(
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
pub(super) fn policy_rule_conditions_overlap(
    left: &PolicyRuleCondition,
    right: &PolicyRuleCondition,
) -> bool {
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

impl CompiledGovernancePolicy {
    pub fn revision(&self) -> PolicyRevisionId {
        self.revision
    }

    pub fn valid_until_unix_ms(&self) -> u64 {
        self.valid_until_unix_ms
    }

    pub fn is_valid_at(&self, now_unix_ms: u64) -> bool {
        now_unix_ms < self.valid_until_unix_ms
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

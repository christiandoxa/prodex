use std::fmt;

#[cfg(any())]
use crate::CapabilitySet;
use crate::{CredentialScope, ModelCapability, PrincipalKind, Role};

use super::{
    CanonicalRoute, Channel, DataClassification, DataModality, GovernedAction, InspectionCoverage,
    NetworkZone, PolicyInput, PolicySelector, RequestRisk,
};

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
    #[cfg(any())]
    pub(super) fn matches(&self, input: &PolicyInput<'_>) -> bool {
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

    pub(super) fn matches(&self, input: &PolicyInput<'_>) -> bool {
        self.matches_rust(input)
    }

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

#[cfg(any())]
fn model_capability_bit(value: ModelCapability) -> u64 {
    1_u64 << value as usize
}

#[cfg(any())]
fn model_capability_mask(values: &CapabilitySet) -> u64 {
    values
        .as_slice()
        .iter()
        .fold(0, |mask, value| mask | model_capability_bit(*value))
}

#[cfg(any())]
fn data_modality_bit(value: DataModality) -> u64 {
    1_u64 << value as usize
}

#[cfg(any())]
fn data_modality_mask(values: &[DataModality]) -> u64 {
    values
        .iter()
        .fold(0, |mask, value| mask | data_modality_bit(*value))
}

#[cfg(any())]
pub(super) fn selector_matches(selector: &PolicySelector, value: &str) -> bool {
    prodex_mojo_core::policy::governance_selector_matches(selector.as_str(), value)
        .expect("Mojo governance selector predicate returned invalid output")
}

pub(super) fn selector_matches_rust(selector: &PolicySelector, value: &str) -> bool {
    selector.as_str() == "*" || selector.as_str() == value
}

pub(super) fn selector_matches(selector: &PolicySelector, value: &str) -> bool {
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

use super::{
    RuntimeGovernanceDataClassification, RuntimeGovernancePolicyAction,
    RuntimeGovernancePolicyChannel, RuntimeGovernancePolicyDataModality,
    RuntimeGovernancePolicyNetworkZone, RuntimeGovernancePolicyRequestRisk,
};
use prodex_domain::{CredentialScope, InspectionCoverage, ModelCapability, PrincipalKind, Role};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGovernancePolicyRuleCondition {
    pub channel: Option<RuntimeGovernancePolicyChannel>,
    pub principal_kind: Option<PrincipalKind>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub group_id: Option<String>,
    pub department_id: Option<String>,
    pub minimum_role: Option<Role>,
    pub credential_scope: Option<CredentialScope>,
    pub action: Option<RuntimeGovernancePolicyAction>,
    pub route: Option<String>,
    pub minimum_classification: Option<RuntimeGovernanceDataClassification>,
    pub inspection_coverage: Option<InspectionCoverage>,
    pub minimum_request_risk: Option<RuntimeGovernancePolicyRequestRisk>,
    pub network_zone: Option<RuntimeGovernancePolicyNetworkZone>,
    pub maximum_session_age_seconds: Option<u64>,
    pub maximum_session_idle_seconds: Option<u64>,
    pub session_revoked: Option<bool>,
    pub session_mfa_satisfied: Option<bool>,
    pub minimum_session_retained_classification: Option<RuntimeGovernanceDataClassification>,
    pub minimum_authentication_strength: Option<u8>,
    pub environment_mfa_satisfied: Option<bool>,
    pub requested_capability: Option<ModelCapability>,
    pub requested_model: Option<String>,
    pub requested_tool: Option<String>,
    pub requested_modality: Option<RuntimeGovernancePolicyDataModality>,
    pub break_glass_required: Option<bool>,
    pub break_glass_scope: Option<String>,
    pub quota_has_headroom: Option<bool>,
    pub quota_reservation_required: Option<bool>,
}

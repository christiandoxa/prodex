use anyhow::{Context, Result};
use prodex_application::ApplicationGovernanceSnapshot;
use prodex_domain::{
    AuditDetailLevel, ClassificationRule, ClassificationRuleSet, ClassificationRuleSetChecksum,
    ClassificationRuleSetRevisionId, DataClassification, GovernanceObligation,
    GovernancePolicyArtifact, GovernancePolicyRule, GovernancePolicyRuleId, InspectionCoverage,
    PolicyEffect, PolicyReasonCode, PolicyRuleCondition, ProviderTrustTier,
    compile_classification_rule_set, compile_governance_policy,
};
use prodex_runtime_policy::{RuntimeGovernanceMode, RuntimePolicyGovernanceSettings};

pub(crate) fn runtime_governance_config(
    settings: &RuntimePolicyGovernanceSettings,
) -> prodex_config::GovernanceConfig {
    use prodex_config::{GovernanceConfig, GovernanceMode, GovernanceRolloutMode};
    use prodex_runtime_policy::RuntimeGovernanceRolloutMode;

    let mode = match settings.mode {
        RuntimeGovernanceMode::Personal => GovernanceMode::Personal,
        RuntimeGovernanceMode::EnterpriseObserve => GovernanceMode::EnterpriseObserve,
        RuntimeGovernanceMode::EnterpriseEnforce => GovernanceMode::EnterpriseEnforce,
        RuntimeGovernanceMode::BankEnforce => GovernanceMode::BankEnforce,
    };
    let rollout = |value| match value {
        RuntimeGovernanceRolloutMode::Off => GovernanceRolloutMode::Off,
        RuntimeGovernanceRolloutMode::Observe => GovernanceRolloutMode::Observe,
        RuntimeGovernanceRolloutMode::Enforce => GovernanceRolloutMode::Enforce,
    };
    GovernanceConfig {
        mode,
        inspection: rollout(settings.inspection),
        classification: rollout(settings.classification),
        policy: rollout(settings.policy),
        routing: rollout(settings.routing),
        mandatory_audit: settings.mandatory_audit,
        anonymous_data_plane: settings.anonymous_data_plane,
        raw_secret_sources: settings.raw_secret_sources,
    }
}

pub(crate) fn build_runtime_governance_snapshot(
    settings: &RuntimePolicyGovernanceSettings,
) -> Result<ApplicationGovernanceSnapshot> {
    let classification_rules = compile_classification_rule_set(ClassificationRuleSet {
        revision: ClassificationRuleSetRevisionId::new(
            settings
                .classification_revision
                .as_deref()
                .unwrap_or("builtin-classification-v1"),
        )
        .context("invalid governance classification revision")?,
        checksum: ClassificationRuleSetChecksum::new(
            settings
                .classification_checksum
                .as_deref()
                .unwrap_or("builtin-classification-v1"),
        )
        .context("invalid governance classification checksum")?,
        unsupported_coverage_floor: match settings.mode {
            RuntimeGovernanceMode::Personal => DataClassification::Internal,
            RuntimeGovernanceMode::EnterpriseObserve | RuntimeGovernanceMode::EnterpriseEnforce => {
                DataClassification::Confidential
            }
            RuntimeGovernanceMode::BankEnforce => DataClassification::Restricted,
        },
        rules: prodex_domain::FindingKind::ALL
            .into_iter()
            .map(|finding_kind| ClassificationRule {
                finding_kind,
                classification: finding_kind.minimum_classification(),
            })
            .collect(),
    })
    .context("invalid governance classification snapshot")?;
    let policy = compile_governance_policy(GovernancePolicyArtifact {
        revision: settings.policy_revision.unwrap_or_default(),
        valid_until_unix_ms: settings.policy_valid_until_unix_ms.unwrap_or(u64::MAX),
        default_effect: PolicyEffect::Allow,
        rules: runtime_governance_rules(settings.mode)?,
    })
    .context("invalid governance policy snapshot")?;
    Ok(ApplicationGovernanceSnapshot {
        classification_rules,
        policy,
    })
}

fn runtime_governance_rules(mode: RuntimeGovernanceMode) -> Result<Vec<GovernancePolicyRule>> {
    if mode == RuntimeGovernanceMode::Personal {
        return Ok(Vec::new());
    }
    let mut rules = vec![runtime_governance_rule(
        "coverage-unsupported",
        PolicyRuleCondition {
            inspection_coverage: Some(InspectionCoverage::Unsupported),
            ..runtime_governance_condition()
        },
        PolicyEffect::Deny,
        Vec::new(),
        "policy.inspection_unsupported",
    )?];
    if mode == RuntimeGovernanceMode::BankEnforce {
        rules.push(runtime_governance_rule(
            "bank-approved-provider",
            runtime_governance_condition(),
            PolicyEffect::Allow,
            vec![
                GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::Enterprise),
                GovernanceObligation::ProhibitRetention,
                GovernanceObligation::ProhibitTrainingUse,
                GovernanceObligation::RequireResponseInspection,
                GovernanceObligation::AuditDetail(AuditDetailLevel::Elevated),
                GovernanceObligation::DenyFallbackOutsideEligibility,
            ],
            "policy.bank_baseline",
        )?);
    }
    rules.push(runtime_governance_rule(
        "confidential-controls",
        PolicyRuleCondition {
            minimum_classification: Some(DataClassification::Confidential),
            ..runtime_governance_condition()
        },
        PolicyEffect::Allow,
        vec![
            GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::Enterprise),
            GovernanceObligation::ProhibitRetention,
            GovernanceObligation::ProhibitTrainingUse,
            GovernanceObligation::RequireResponseInspection,
            GovernanceObligation::DenyFallbackOutsideEligibility,
        ],
        "policy.confidential_controls",
    )?);
    rules.push(runtime_governance_rule(
        "restricted-controls",
        PolicyRuleCondition {
            minimum_classification: Some(DataClassification::Restricted),
            ..runtime_governance_condition()
        },
        PolicyEffect::Allow,
        vec![
            GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::RestrictedApproved),
            GovernanceObligation::RequireLocalExecution,
            GovernanceObligation::RequireMfa,
            GovernanceObligation::RequireReauthentication,
            GovernanceObligation::AuditDetail(AuditDetailLevel::Elevated),
        ],
        "policy.restricted_controls",
    )?);
    Ok(rules)
}

fn runtime_governance_rule(
    id: &str,
    condition: PolicyRuleCondition,
    effect: PolicyEffect,
    obligations: Vec<GovernanceObligation>,
    reason: &str,
) -> Result<GovernancePolicyRule> {
    Ok(GovernancePolicyRule {
        id: GovernancePolicyRuleId::new(id).context("invalid governance rule id")?,
        condition,
        effect,
        obligations,
        reason_code: PolicyReasonCode::new(reason)
            .context("invalid governance policy reason code")?,
    })
}

const fn runtime_governance_condition() -> PolicyRuleCondition {
    PolicyRuleCondition {
        channel: None,
        principal_kind: None,
        minimum_role: None,
        action: None,
        route: None,
        minimum_classification: None,
        inspection_coverage: None,
        minimum_request_risk: None,
        network_zone: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_snapshot_denies_unsupported_inspection() {
        let settings = RuntimePolicyGovernanceSettings {
            mode: RuntimeGovernanceMode::BankEnforce,
            ..RuntimePolicyGovernanceSettings::default()
        };
        let snapshot = build_runtime_governance_snapshot(&settings).unwrap();
        assert!(format!("{snapshot:?}").contains("ApplicationGovernanceSnapshot"));
    }
}

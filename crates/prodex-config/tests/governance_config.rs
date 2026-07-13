use prodex_config::{
    GovernanceConfig, GovernanceConfigError, GovernanceMode, GovernancePolicyFailureMode,
    GovernanceRolloutMode, GovernanceSessionConfig, GovernanceUnknownClassificationBehavior,
    validate_governance_config,
};
use prodex_domain::{DataClassification, PolicyRevisionId};

#[test]
fn governance_modes_preserve_personal_compatibility_and_bank_fail_closed_rules() {
    assert_eq!(
        validate_governance_config(GovernanceConfig::personal_compatible()).unwrap(),
        GovernanceConfig::personal_compatible()
    );

    let bank = GovernanceConfig {
        mode: GovernanceMode::BankEnforce,
        inspection: GovernanceRolloutMode::Enforce,
        classification: GovernanceRolloutMode::Enforce,
        policy: GovernanceRolloutMode::Enforce,
        routing: GovernanceRolloutMode::Enforce,
        mandatory_audit: true,
        anonymous_data_plane: false,
        raw_secret_sources: false,
        classification_default: DataClassification::Restricted,
        classification_unknown: GovernanceUnknownClassificationBehavior::Deny,
        policy_failure_mode: GovernancePolicyFailureMode::Closed,
        active_policy_revision: Some(PolicyRevisionId::default()),
        session: GovernanceSessionConfig {
            absolute_timeout_seconds: Some(3_600),
            idle_timeout_seconds: Some(900),
            max_concurrent: Some(10),
        },
        ..GovernanceConfig::personal_compatible()
    };
    assert_eq!(validate_governance_config(bank).unwrap(), bank);

    assert_eq!(
        validate_governance_config(GovernanceConfig {
            mandatory_audit: false,
            ..bank
        }),
        Err(GovernanceConfigError::EnforceAuditRequired)
    );
    assert_eq!(
        validate_governance_config(GovernanceConfig {
            inspection: GovernanceRolloutMode::Observe,
            ..bank
        }),
        Err(GovernanceConfigError::EnforceModeRequiresEnforcement)
    );

    assert_eq!(
        validate_governance_config(GovernanceConfig {
            mode: GovernanceMode::EnterpriseEnforce,
            mandatory_audit: false,
            ..bank
        }),
        Err(GovernanceConfigError::EnforceAuditRequired)
    );
}

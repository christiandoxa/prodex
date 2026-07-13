use prodex_config::{
    GovernanceConfig, GovernanceConfigError, GovernanceMode, GovernanceRolloutMode,
    validate_governance_config,
};

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
    };
    assert_eq!(validate_governance_config(bank).unwrap(), bank);

    assert_eq!(
        validate_governance_config(GovernanceConfig {
            mandatory_audit: false,
            ..bank
        }),
        Err(GovernanceConfigError::BankAuditRequired)
    );
    assert_eq!(
        validate_governance_config(GovernanceConfig {
            inspection: GovernanceRolloutMode::Observe,
            ..bank
        }),
        Err(GovernanceConfigError::EnforceModeRequiresEnforcement)
    );
}

use prodex_config::{GovernanceConfig, GovernanceMode, GovernanceRolloutMode};
use prodex_domain::DataClassification;

#[test]
fn personal_compatible_defaults_remain_stable() {
    let config = GovernanceConfig::personal_compatible();
    assert_eq!(config.mode, GovernanceMode::Personal);
    assert_eq!(config.inspection, GovernanceRolloutMode::Off);
    assert_eq!(config.classification_default, DataClassification::Internal);
    assert!(!config.mode.is_enforcing());
    assert_eq!(config.mode.as_str(), "personal");
}

#[test]
fn enforcing_modes_and_minimal_governance_fields_remain_typed() {
    for (mode, label) in [
        (GovernanceMode::EnterpriseEnforce, "enterprise_enforce"),
        (GovernanceMode::BankEnforce, "bank_enforce"),
    ] {
        let config = GovernanceConfig {
            mode,
            inspection: GovernanceRolloutMode::Enforce,
            classification_default: DataClassification::Restricted,
        };
        assert!(config.mode.is_enforcing());
        assert_eq!(config.mode.as_str(), label);
        assert_eq!(config.inspection, GovernanceRolloutMode::Enforce);
        assert_eq!(
            config.classification_default,
            DataClassification::Restricted
        );
    }
}

#[test]
fn observe_mode_is_non_enforcing() {
    let config = GovernanceConfig {
        mode: GovernanceMode::EnterpriseObserve,
        inspection: GovernanceRolloutMode::Observe,
        classification_default: DataClassification::Confidential,
    };
    assert!(!config.mode.is_enforcing());
    assert_eq!(config.mode.as_str(), "enterprise_observe");
}

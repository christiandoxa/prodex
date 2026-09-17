#![forbid(unsafe_code)]
//! Minimal governance configuration used by the core Presidio runtime.

use prodex_domain::DataClassification;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceMode {
    Personal,
    EnterpriseObserve,
    EnterpriseEnforce,
    BankEnforce,
}

impl GovernanceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::EnterpriseObserve => "enterprise_observe",
            Self::EnterpriseEnforce => "enterprise_enforce",
            Self::BankEnforce => "bank_enforce",
        }
    }

    pub const fn is_enforcing(self) -> bool {
        matches!(self, Self::EnterpriseEnforce | Self::BankEnforce)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceRolloutMode {
    Off,
    Observe,
    Enforce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GovernanceConfig {
    pub mode: GovernanceMode,
    pub inspection: GovernanceRolloutMode,
    pub classification_default: DataClassification,
}

impl GovernanceConfig {
    pub const fn personal_compatible() -> Self {
        Self {
            mode: GovernanceMode::Personal,
            inspection: GovernanceRolloutMode::Off,
            classification_default: DataClassification::Internal,
        }
    }
}

use crate::MojoError;

use super::{Operation, ProviderUrlViolation, call};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigReasoningState {
    Absent,
    Valid,
    InvalidCatalog,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigUrlState {
    Absent,
    Invalid,
    Valid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValidationAction {
    Valid,
    ModelNonempty,
    InvalidReasoningCatalog,
    UnsupportedReasoning,
    InvalidUrl,
    LocalRequiresUrl,
    NonLocalRejectsUrl,
}

pub fn provider_url_violation(
    provider_is_local: bool,
    url_present: bool,
) -> Result<Option<ProviderUrlViolation>, MojoError> {
    let scalar = i64::from(provider_is_local) | (i64::from(url_present) << 1);
    let result = call(Operation::ProviderUrlPolicy, "", scalar)?;
    match result[0] {
        0 => Ok(None),
        1 => Ok(Some(ProviderUrlViolation::LocalRequiresUrl)),
        2 => Ok(Some(ProviderUrlViolation::NonLocalRejectsUrl)),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Chooses the first sub-agent configuration error after Rust has acquired and
/// validated provider-catalog and URL facts.
pub fn config_validation_plan(
    model: Option<&str>,
    reasoning: ConfigReasoningState,
    url: ConfigUrlState,
    provider_is_local: bool,
) -> Result<ConfigValidationAction, MojoError> {
    let reasoning = match reasoning {
        ConfigReasoningState::Absent => 0,
        ConfigReasoningState::Valid => 1,
        ConfigReasoningState::InvalidCatalog => 2,
        ConfigReasoningState::Unsupported => 3,
    };
    let url = match url {
        ConfigUrlState::Absent => 0,
        ConfigUrlState::Invalid => 1,
        ConfigUrlState::Valid => 2,
    };
    let scalar = i64::from(reasoning)
        | (i64::from(url) << 2)
        | (i64::from(provider_is_local) << 4)
        | (i64::from(model.is_some()) << 5);
    let result = call(
        Operation::ConfigValidation,
        model.unwrap_or_default(),
        scalar,
    )?;
    match result[0] {
        0 => Ok(ConfigValidationAction::Valid),
        1 => Ok(ConfigValidationAction::ModelNonempty),
        2 => Ok(ConfigValidationAction::InvalidReasoningCatalog),
        3 => Ok(ConfigValidationAction::UnsupportedReasoning),
        4 => Ok(ConfigValidationAction::InvalidUrl),
        5 => Ok(ConfigValidationAction::LocalRequiresUrl),
        6 => Ok(ConfigValidationAction::NonLocalRejectsUrl),
        _ => Err(MojoError::InvalidOutput),
    }
}

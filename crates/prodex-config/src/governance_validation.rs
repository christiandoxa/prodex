use super::*;

pub fn validate_governance_config(
    config: GovernanceConfig,
) -> Result<GovernanceConfig, GovernanceConfigError> {
    if config.config_version != 1 {
        return Err(GovernanceConfigError::UnsupportedVersion);
    }
    validate_rollout_modes(&config)?;
    validate_bank_enforce_config(&config)?;
    validate_enforcing_config(&config)?;
    validate_session_bounds(config.session)?;
    Ok(config)
}

fn validate_rollout_modes(config: &GovernanceConfig) -> Result<(), GovernanceConfigError> {
    if config.mode.is_enforcing()
        && [
            config.inspection,
            config.classification,
            config.policy,
            config.routing,
        ]
        .into_iter()
        .any(|mode| mode != GovernanceRolloutMode::Enforce)
    {
        return Err(GovernanceConfigError::EnforceModeRequiresEnforcement);
    }
    if config.mode.is_enforcing() && !config.mandatory_audit {
        return Err(GovernanceConfigError::EnforceAuditRequired);
    }
    Ok(())
}

fn validate_bank_enforce_config(config: &GovernanceConfig) -> Result<(), GovernanceConfigError> {
    if config.mode != GovernanceMode::BankEnforce {
        return Ok(());
    }
    if config.anonymous_data_plane {
        return Err(GovernanceConfigError::BankIdentityRequired);
    }
    if config.raw_secret_sources {
        return Err(GovernanceConfigError::BankSecretReferenceRequired);
    }
    if config.classification_default != DataClassification::Restricted {
        return Err(GovernanceConfigError::BankRestrictedDefaultRequired);
    }
    Ok(())
}

fn validate_enforcing_config(config: &GovernanceConfig) -> Result<(), GovernanceConfigError> {
    if !config.mode.is_enforcing() {
        return Ok(());
    }
    if config.classification_unknown != GovernanceUnknownClassificationBehavior::Deny {
        return Err(GovernanceConfigError::EnforceUnknownClassificationMustDeny);
    }
    if config.policy_failure_mode != GovernancePolicyFailureMode::Closed {
        return Err(GovernanceConfigError::EnforcePolicyMustFailClosed);
    }
    if config.active_policy_revision.is_none() {
        return Err(GovernanceConfigError::EnforceActiveRevisionRequired);
    }
    if config.session.absolute_timeout_seconds.is_none()
        || config.session.idle_timeout_seconds.is_none()
        || config.session.max_concurrent.is_none()
    {
        return Err(GovernanceConfigError::EnforceSessionBoundsRequired);
    }
    Ok(())
}

fn validate_session_bounds(session: GovernanceSessionConfig) -> Result<(), GovernanceConfigError> {
    if session
        .absolute_timeout_seconds
        .is_some_and(|value| !(300..=86_400).contains(&value))
        || session
            .idle_timeout_seconds
            .is_some_and(|value| !(60..=3_600).contains(&value))
        || session
            .max_concurrent
            .is_some_and(|value| value == 0 || value > 10_000)
        || matches!(
            (session.idle_timeout_seconds, session.absolute_timeout_seconds),
            (Some(idle), Some(absolute)) if idle > absolute
        )
    {
        return Err(GovernanceConfigError::InvalidSessionBounds);
    }
    Ok(())
}
